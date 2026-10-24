// Independent bridge to Nextcloud's installed File Provider actions. Selector
// contracts are runtime-checked; no Nextcloud implementation code is bundled.
#import "Local.h"
#import <AppKit/AppKit.h>
#import <FileProvider/FileProvider.h>
#import <objc/runtime.h>
#import <sys/stat.h>

static NSString *const NCProvider = @"com.nextcloud.desktopclient.FileProviderExt";
static NSString *const NCKeepAction = @"com.nextcloud.desktopclient.FileProviderExt.KeepDownloadedAction";
static NSError *NCError(NSString *message) {
    return [NSError errorWithDomain:@"local.schnauz.nextcloud.local" code:1
        userInfo:@{NSLocalizedDescriptionKey:message}];
}
static BOOL NCPath(NSString *path) {
    return [path isKindOfClass:NSString.class] && path.isAbsolutePath && path.length <= 32768
        && [path rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location == NSNotFound;
}
static BOOL NCBool(NSDictionary *item, NSString *key) {
    id value = item[key];
    return [value isKindOfClass:NSNumber.class] && [value boolValue];
}
static BOOL NCSig(NSMethodSignature *signature,const char *result,NSArray<NSString *> *arguments) {
    if(!signature || signature.numberOfArguments != arguments.count+2) return NO;
    BOOL (^typeMatches)(const char *,const char *)=^BOOL(const char *actual,const char *expected) {
        while(*actual && strchr("rnNoORV",*actual)) actual++;
        if(strcmp(expected,"BOOL")==0) return strcmp(actual,"B")==0 || strcmp(actual,"c")==0;
        if(strcmp(expected,"@")==0) return actual[0]=='@' && actual[1]!='?';
        return strcmp(actual,expected)==0;
    };
    if(!typeMatches(signature.methodReturnType,result)) return NO;
    for(NSUInteger i=0;i<arguments.count;i++) {
        if(!typeMatches([signature getArgumentTypeAtIndex:i+2],arguments[i].UTF8String)) return NO;
    }
    return YES;
}
static BOOL NCMethod(id receiver,SEL selector,const char *result,NSArray<NSString *> *arguments) {
    return [receiver respondsToSelector:selector] && NCSig([receiver methodSignatureForSelector:selector],result,arguments);
}
#ifdef SCHNAUZ_NEXTCLOUD_TESTS
BOOL SchnauzNextcloudCheckMethod(id receiver,NSString *selector,NSString *result,NSArray *arguments) {
    return NCMethod(receiver,NSSelectorFromString(selector),result.UTF8String,arguments);
}
#endif
static BOOL NCCanKeep(NSDictionary *item) {
    for(NSString *key in @[@"supported",@"keepDownloaded",@"downloading",@"folder",@"downloaded",@"recursivelyDownloaded"]) {
        if(![item[key] isKindOfClass:NSNumber.class]) return NO;
    }
    if (![item[@"provider"] isEqual:NCProvider] || !NCBool(item,@"supported")
        || NCBool(item,@"keepDownloaded") || NCBool(item,@"downloading")) return NO;
    return !NCBool(item,NCBool(item,@"folder") ? @"recursivelyDownloaded" : @"downloaded");
}

// Kept in Objective-C so unsupported OS selectors and exceptions fail as an
// ordinary operation error rather than crossing Swift or Rust ABI boundaries.
@interface NSObject (SchnauzFileProviderContract)
+ (id)defaultManager;
- (void)fetchItemForURL:(NSURL *)url completionHandler:(void (^)(id,NSError *))completion;
- (id)providerID;
- (id)providerDomainID;
- (id)itemIdentifier;
- (id)userInfo;
- (BOOL)isFolder;
- (BOOL)isDownloaded;
- (BOOL)isRecursivelyDownloaded;
- (BOOL)isDownloading;
- (BOOL)isKeepDownloaded;
- (id)initWithActionIdentifier:(NSString *)action providerDomainID:(id)domain itemIdentifiers:(NSArray *)identifiers;
- (void)setActionCompletionBlock:(void (^)(NSError *))completion;
- (void)scheduleAction:(id)operation;
@end

@interface SchnauzNextcloudRuntime : NSObject <SchnauzNextcloudLocalAdapter>
@property id manager;
@property BOOL installed;
- (void)confirmPolicy:(NSString *)path attempt:(NSUInteger)attempt completion:(void (^)(NSError *))completion;
@end
@implementation SchnauzNextcloudRuntime
- (instancetype)init {
    self=[super init]; if(!self) return nil;
    // Resolve each user's installed app through LaunchServices, never a personal
    // path, account configuration, app-group database or shared Keychain item.
    NSURL *url=[NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:@"com.nextcloud.desktopclient"];
    NSBundle *bundle=url ? [NSBundle bundleWithURL:url] : nil;
    NSString *version=bundle.infoDictionary[@"CFBundleShortVersionString"];
    if (![version isKindOfClass:NSString.class]
        || [version compare:@"34.0.1" options:NSNumericSearch] == NSOrderedAscending) return self;
    NSURL *plugin=[bundle.builtInPlugInsURL URLByAppendingPathComponent:@"FileProviderExt.appex"];
    NSBundle *extension=plugin ? [NSBundle bundleWithURL:plugin] : nil;
    if (![extension.bundleIdentifier isEqual:NCProvider]) return self;
    NSArray *actions=extension.infoDictionary[@"NSExtension"][@"NSExtensionFileProviderActions"];
    for(NSDictionary *action in actions) {
        if ([action[@"NSExtensionFileProviderActionIdentifier"] isEqual:NCKeepAction]) self.installed=YES;
    }
    // FileProvider exports these runtime classes on the reference OS. These
    // are OS-internal APIs: availability checks are mandatory on other Macs.
    (void)NSFileProviderManager.class;
    Class cls=NSClassFromString(@"FPItemManager");
    if (NCMethod(cls,@selector(defaultManager),"@",@[])) {
        @try { self.manager=[cls defaultManager]; } @catch(NSException *exception) { self.manager=nil; }
    }
    if (!NCMethod(self.manager,@selector(fetchItemForURL:completionHandler:),"v",@[@"@",@"@?"])
        || !NCMethod(self.manager,@selector(scheduleAction:),"v",@[@"@"])) self.manager=nil;
    return self;
}
- (void)readItem:(NSString *)path completion:(void (^)(NSDictionary *,NSError *))completion {
    if(!self.installed || !self.manager) {
        completion(nil,NCError(@"Nextcloud Desktop 34.0.1 or newer and its compatible macOS File Provider integration are required.")); return;
    }
    @try {
        NSURL *itemURL=[[NSURL fileURLWithPath:path] URLByResolvingSymlinksInPath];
        [self.manager fetchItemForURL:itemURL completionHandler:^(id item,NSError *error) {
            @try {
                if(error || !item) { completion(nil,NCError(@"Nextcloud local status is unavailable for this item.")); return; }
                for(NSString *name in @[@"providerID",@"providerDomainID",@"itemIdentifier",@"isFolder",@"isDownloaded",@"isRecursivelyDownloaded",@"isDownloading",@"isKeepDownloaded",@"userInfo"]) {
                    BOOL object=[@[@"providerID",@"providerDomainID",@"itemIdentifier",@"userInfo"] containsObject:name];
                    if(!NCMethod(item,NSSelectorFromString(name),object ? "@" : "BOOL",@[])) {
                        completion(nil,NCError(@"This macOS File Provider interface is not compatible.")); return;
                    }
                }
                if(![[item providerID] isEqual:NCProvider]) {
                    completion(nil,NCError(@"This item is not managed by the Nextcloud File Provider.")); return;
                }
                id domain=[item providerDomainID], identifier=[item itemIdentifier];
                if(!domain || ![identifier isKindOfClass:NSString.class]) {
                    completion(nil,NCError(@"Nextcloud could not identify this item.")); return;
                }
                NSDictionary *userInfo=[item userInfo];
                BOOL offered= [userInfo isKindOfClass:NSDictionary.class] && [userInfo[@"displayKeepDownloaded"] boolValue];
                BOOL kept=[item isKeepDownloaded] || ([userInfo isKindOfClass:NSDictionary.class]
                    && [userInfo[@"displayAllowAutoEvicting"] boolValue]);
                // Kernel materialization is authoritative for local file data;
                // a provider's cached remote metadata can lag behind it. This
                // reads metadata only, never a placeholder's contents.
                struct stat metadata;
                if(stat(itemURL.fileSystemRepresentation,&metadata)!=0) {
                    completion(nil,NCError(@"The item's local availability could not be checked."));return;
                }
                BOOL local=(metadata.st_flags & SF_DATALESS)==0;
                completion(@{@"path":path,@"provider":NCProvider,@"supported":@(offered),
                    @"folder":@([item isFolder]),@"downloaded":@(local),
                    @"recursivelyDownloaded":@(local && [item isRecursivelyDownloaded]),
                    @"downloading":@([item isDownloading]),@"keepDownloaded":@(kept),
                    @"_domain":domain,@"_identifier":identifier},nil);
            } @catch(NSException *exception) { completion(nil,NCError(@"Nextcloud local metadata could not be read safely.")); }
        }];
    } @catch(NSException *exception) { completion(nil,NCError(@"Nextcloud local status could not be requested.")); }
}
- (void)keepItem:(NSDictionary *)item completion:(void (^)(NSError *))completion {
    // Called only with freshly fetched metadata after an explicit user action.
    Class cls=NSClassFromString(@"FPVendorDefinedActionOperation");
    if(!cls || !NCSig([cls instanceMethodSignatureForSelector:@selector(initWithActionIdentifier:providerDomainID:itemIdentifiers:)],"@",@[@"@",@"@",@"@"])
        || !NCSig([cls instanceMethodSignatureForSelector:@selector(setActionCompletionBlock:)],"v",@[@"@?"])) {
        completion(NCError(@"This macOS version cannot invoke the Nextcloud offline action.")); return;
    }
    @try {
        id operation=[[cls alloc] initWithActionIdentifier:NCKeepAction
            providerDomainID:item[@"_domain"] itemIdentifiers:@[item[@"_identifier"]]];
        if(!operation || ![operation isKindOfClass:NSOperation.class]
            || !NCMethod(operation,@selector(setActionCompletionBlock:),"v",@[@"@?"])) {
            completion(NCError(@"Nextcloud offline operation is unavailable.")); return;
        }
        [operation setActionCompletionBlock:^(NSError *error) {
            if(error) { completion(NCError(@"Nextcloud could not confirm the offline action. Refresh its status before trying again.")); return; }
            [self confirmPolicy:item[@"path"] attempt:0 completion:completion];
        }];
        [self.manager scheduleAction:operation];
    } @catch(NSException *exception) { completion(NCError(@"Nextcloud offline action could not be started safely.")); }
}
- (void)confirmPolicy:(NSString *)path attempt:(NSUInteger)attempt completion:(void (^)(NSError *))completion {
    [self readItem:path completion:^(NSDictionary *item,NSError *error) {
        if(!error && [item[@"provider"] isEqual:NCProvider] && NCBool(item,@"keepDownloaded")) {
            completion(nil); return;
        }
        if(attempt>=9) {
            completion(NCError(@"Nextcloud accepted the request, but its offline policy could not yet be confirmed. Refresh the status; the action was not repeated."));return;
        }
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW,200*NSEC_PER_MSEC),dispatch_get_main_queue(),^{
            [self confirmPolicy:path attempt:attempt+1 completion:completion];
        });
    }];
}
@end

static NSDictionary *NCPublic(NSDictionary *item) {
    NSMutableDictionary *result=[item mutableCopy];
    [result removeObjectForKey:@"_domain"]; [result removeObjectForKey:@"_identifier"];
    // C logical expressions promote BOOL operands to int. NSNumber then emits
    // JSON 0/1, which the Rust protocol correctly rejects as non-Boolean.
    // Normalize known numeric flags at the bridge boundary, preserving missing
    // or malformed fields so unknown metadata cannot enable an action.
    for(NSString *key in @[@"supported",@"folder",@"downloaded",@"recursivelyDownloaded",@"downloading",@"keepDownloaded"]) {
        if([result[key] isKindOfClass:NSNumber.class]) {
            result[key]=[NSNumber numberWithBool:[result[key] boolValue]];
        }
    }
    result[@"canKeepDownloaded"]=@(NCCanKeep(item)); return result;
}
void SchnauzNextcloudLocalHandle(NSDictionary *request,id<SchnauzNextcloudLocalAdapter> adapter,void (^completion)(NSDictionary *)) {
    NSString *operation=request[@"operation"];
    if([operation isEqual:@"keep-downloaded"]) {
        NSString *path=request[@"path"];
        if(!NCPath(path)) { completion(@{@"success":@NO,@"message":@"Choose a valid local Nextcloud item."}); return; }
        [adapter readItem:path completion:^(NSDictionary *item,NSError *error) {
            if(error || !item) { completion(@{@"success":@NO,@"message":error.localizedDescription ?: @"Nextcloud local status is unavailable."}); return; }
            if(!NCCanKeep(item)) {
                completion(@{@"success":@NO,@"message":@"Offline availability is not applicable to this item's current state. Refresh the view."}); return;
            }
            [adapter keepItem:item completion:^(NSError *error) {
                completion(@{@"success":@(!error),@"message":error.localizedDescription ?: @"Nextcloud confirmed Always keep downloaded. Content may still be downloading."});
            }];
        }]; return;
    }
    NSArray *paths=request[@"paths"];
    if(![operation isEqual:@"local-status"] || ![paths isKindOfClass:NSArray.class] || paths.count>32) {
        completion(@{@"success":@NO,@"message":@"Invalid Nextcloud local request."}); return;
    }
    if(!paths.count) { completion(@{@"success":@YES,@"items":@[]}); return; }
    NSMutableArray *results=[NSMutableArray array]; dispatch_group_t group=dispatch_group_create();
    for(NSString *path in paths) {
        if(!NCPath(path)) continue;
        dispatch_group_enter(group);
        [adapter readItem:path completion:^(NSDictionary *item,NSError *error) {
            NSDictionary *result=item && !error ? NCPublic(item) : @{@"path":path,@"error":error.localizedDescription ?: @"Status unavailable"};
            @synchronized(results) { [results addObject:result]; }
            dispatch_group_leave(group);
        }];
    }
    dispatch_group_notify(group,dispatch_get_main_queue(),^{ completion(@{@"success":@YES,@"items":results}); });
}
void SchnauzNextcloudLocalRequest(NSDictionary *request,void (^completion)(NSDictionary *)) {
    @try { SchnauzNextcloudLocalHandle(request,[SchnauzNextcloudRuntime new],completion); }
    @catch(NSException *exception) {
        completion(@{@"success":@NO,@"message":@"Nextcloud local integration is unavailable on this installation."});
    }
}
