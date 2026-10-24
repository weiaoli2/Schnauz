// Offline adapter fixtures. Never instantiate the runtime adapter or a domain.
#import "Local.h"
#import <FileProvider/FileProvider.h>
@interface NSObject (UnscheduledRuntimeFixture)
- (id)initWithProviderID:(id)p domainIdentifier:(id)d itemIdentifier:(id)i parentItemIdentifier:(id)parent filename:(id)n isDirectory:(BOOL)directory;
- (id)providerDomainID;
- (id)initWithActionIdentifier:(id)a providerDomainID:(id)d itemIdentifiers:(id)i;
- (void)setActionCompletionBlock:(void (^)(NSError *))completion;
- (void)finishWithResult:(id)result error:(NSError *)error;
@end
extern BOOL SchnauzNextcloudCheckMethod(id,NSString *,NSString *,NSArray *);
@interface BadABI : NSObject
- (NSInteger)isDownloaded;
- (id)fetchItemForURL:(id)url completionHandler:(id)handler;
@end
@implementation BadABI
- (NSInteger)isDownloaded {return 0;}
- (id)fetchItemForURL:(id)url completionHandler:(id)handler {return nil;}
@end
@interface FakeLocal : NSObject <SchnauzNextcloudLocalAdapter>
@property NSDictionary *item;
@property NSInteger reads;
@property NSInteger writes;
@property NSError *failure;
@end
@implementation FakeLocal
- (void)readItem:(NSString *)path completion:(void (^)(NSDictionary *,NSError *))completion {
    self.reads++; completion(self.item,self.failure);
}
- (void)keepItem:(NSDictionary *)item completion:(void (^)(NSError *))completion {
    self.writes++; completion(nil);
}
@end
static NSDictionary *call(NSDictionary *request,FakeLocal *adapter) {
    __block NSDictionary *reply=nil;
    SchnauzNextcloudLocalHandle(request,adapter,^(NSDictionary *value){reply=value;});
    NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:1];
    while(!reply && deadline.timeIntervalSinceNow>0) [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.001]];
    NSCAssert(reply,@"Fixture reply missing"); return reply;
}
int main(void) { @autoreleasepool {
    // Verify the callback contract on our own UNSCHEDULED, synthetic operation.
    // No manager, domain lookup, Nextcloud app or provider action is requested.
    (void)NSFileProviderManager.class;
    @try {
        id item=[[NSClassFromString(@"FPItem") alloc] initWithProviderID:@"local.schnauz.fixture"
            domainIdentifier:@"fixture" itemIdentifier:@"one" parentItemIdentifier:@"root" filename:@"fixture" isDirectory:NO];
        for(NSNumber *fail in @[@NO,@YES]) {
            id op=[[NSClassFromString(@"FPVendorDefinedActionOperation") alloc] initWithActionIdentifier:@"local.schnauz.unscheduled"
                providerDomainID:[item providerDomainID] itemIdentifiers:@[@"one"]];
            NSCAssert(op,@"Reference OS must expose the checked action operation");
            NSError *expected=fail.boolValue ? [NSError errorWithDomain:@"fixture" code:7 userInfo:nil] : nil;
            __block BOOL called=NO, correct=NO;
            [op setActionCompletionBlock:^(NSError *error){called=YES;correct=error==expected;}];
            [op finishWithResult:@[@"synthetic-result"] error:expected];
            NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:2];
            while(!called && deadline.timeIntervalSinceNow>0) [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.001]];
            NSCAssert(called && correct,@"Reference private completion must pass NSError first, including nil success");
        }
    } @catch(NSException *exception) { NSCAssert(NO,@"Unscheduled runtime ABI fixture failed"); }
    BadABI *bad=[BadABI new];
    NSCAssert(!SchnauzNextcloudCheckMethod(bad,@"isDownloaded",@"BOOL",@[]),@"Reject incompatible private BOOL ABI");
    NSCAssert(!SchnauzNextcloudCheckMethod(bad,@"fetchItemForURL:completionHandler:",@"v",@[@"@",@"@?"]),@"Reject incompatible block/return ABI");
    NSCAssert(SchnauzNextcloudCheckMethod(bad,@"description",@"@",@[]),@"Accept a known object return ABI");
    FakeLocal *adapter=[FakeLocal new];
    NSDictionary *base=@{@"path":@"/fixture/online.txt",@"provider":@"com.nextcloud.desktopclient.FileProviderExt",
        @"folder":@NO,@"downloaded":@NO,@"recursivelyDownloaded":@NO,
        @"downloading":@NO,@"keepDownloaded":@NO,@"supported":@YES,
        @"_domain":@"private-routing",@"_identifier":@"private-id"};
    adapter.item=base;
    NSDictionary *status=call(@{@"operation":@"local-status",@"paths":@[@"/fixture/online.txt"]},adapter);
    NSDictionary *item=[status[@"items"] firstObject];
    NSCAssert([item[@"canKeepDownloaded"] boolValue],@"Online Nextcloud item offers Keep");
    NSCAssert(!item[@"_domain"] && !item[@"_identifier"],@"Do not expose routing identifiers");
    NSCAssert(adapter.writes==0,@"Status is read-only");
    // Runtime availability combines flags with &&, producing boxed C ints.
    // Exercise the actual public JSON boundary, not just Objective-C boolValue.
    for(NSNumber *flag in @[@0,@1]) {
        NSMutableDictionary *numeric=[base mutableCopy];
        numeric[@"recursivelyDownloaded"]=flag; adapter.item=numeric;
        NSDictionary *reply=call(@{@"operation":@"local-status",@"paths":@[@"/fixture/online.txt"]},adapter);
        NSData *json=[NSJSONSerialization dataWithJSONObject:reply options:0 error:nil];
        NSDictionary *decoded=[NSJSONSerialization JSONObjectWithData:json options:0 error:nil];
        NSDictionary *wireItem=[decoded[@"items"] firstObject];
        for(NSString *key in @[@"supported",@"folder",@"downloaded",@"recursivelyDownloaded",@"downloading",@"keepDownloaded",@"canKeepDownloaded"]) {
            NSCAssert(CFGetTypeID((__bridge CFTypeRef)wireItem[key])==CFBooleanGetTypeID(),@"Every wire flag must be a JSON Boolean, never numeric 0/1");
        }
        NSCAssert([wireItem[@"recursivelyDownloaded"] boolValue]==flag.boolValue,@"Preserve native flag values");
        NSCAssert(adapter.writes==0,@"Reading numeric metadata must never pin an item");
    }
    adapter.item=base;
    for(NSString *key in @[@"downloaded",@"recursivelyDownloaded",@"supported",@"folder",@"downloading",@"keepDownloaded"]) {
        NSMutableDictionary *incomplete=[base mutableCopy]; [incomplete removeObjectForKey:key];adapter.item=incomplete;
        NSCAssert(![call(@{@"operation":@"keep-downloaded",@"path":@"/fixture/online.txt"},adapter)[@"success"] boolValue],@"Unknown fields must not become nonlocal state");
        NSCAssert(adapter.writes==0,@"Incomplete metadata must never write");
    }
    for(NSString *key in @[@"downloaded",@"keepDownloaded",@"downloading"]) {
        NSMutableDictionary *changed=[base mutableCopy]; changed[key]=@YES; adapter.item=changed;
        status=call(@{@"operation":@"local-status",@"paths":@[@"/fixture/online.txt"]},adapter);
        NSCAssert(![[status[@"items"] firstObject][@"canKeepDownloaded"] boolValue],@"Do not offer irrelevant Keep");
        NSDictionary *reply=call(@{@"operation":@"keep-downloaded",@"path":@"/fixture/online.txt"},adapter);
        NSCAssert(![reply[@"success"] boolValue] && adapter.writes==0,@"Revalidate stale menu before writing");
    }
    NSMutableDictionary *folder=[base mutableCopy]; folder[@"folder"]=@YES; folder[@"downloaded"]=@YES;
    adapter.item=folder;
    status=call(@{@"operation":@"local-status",@"paths":@[@"/fixture/online.txt"]},adapter);
    NSCAssert([[[status[@"items"] firstObject] objectForKey:@"canKeepDownloaded"] boolValue],@"Partly downloaded folder can be pinned");
    folder[@"recursivelyDownloaded"]=@YES;
    status=call(@{@"operation":@"local-status",@"paths":@[@"/fixture/online.txt"]},adapter);
    NSCAssert(![[[status[@"items"] firstObject] objectForKey:@"canKeepDownloaded"] boolValue],@"Fully local folder hides Keep");
    NSMutableDictionary *foreign=[base mutableCopy]; foreign[@"provider"]=@"other.provider"; adapter.item=foreign;
    NSCAssert(![call(@{@"operation":@"keep-downloaded",@"path":@"/fixture/online.txt"},adapter)[@"success"] boolValue],@"Reject foreign providers");
    NSCAssert(adapter.writes==0,@"Never forward foreign action");
    adapter.item=base;
    NSCAssert([call(@{@"operation":@"keep-downloaded",@"path":@"/fixture/online.txt"},adapter)[@"success"] boolValue],@"Explicit Keep succeeds");
    NSCAssert(adapter.writes==1,@"Exactly one explicit provider action");
    adapter.failure=[NSError errorWithDomain:@"fixture" code:1 userInfo:nil];
    NSCAssert(![call(@{@"operation":@"keep-downloaded",@"path":@"/fixture/online.txt"},adapter)[@"success"] boolValue],@"Unknown status must not write");
    NSCAssert(adapter.writes==1,@"No writes on read failure");
    NSInteger reads=adapter.reads;
    NSCAssert(![call(@{@"operation":@"keep-downloaded",@"path":@"relative"},adapter)[@"success"] boolValue],@"Reject invalid paths");
    NSCAssert(adapter.reads==reads,@"Reject invalid input before backend access");
    puts("PASS Nextcloud local status/Keep policy, private ABI checks, file/folder visibility, stale-menu/foreign/unknown rejection and read-only status; injected adapter only");
} return 0; }
