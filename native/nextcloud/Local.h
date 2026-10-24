#import <Foundation/Foundation.h>
NS_ASSUME_NONNULL_BEGIN

// Nonsecret item metadata and explicit provider actions only. No credential API.
@protocol SchnauzNextcloudLocalAdapter <NSObject>
- (void)readItem:(NSString *)path completion:(void (^)(NSDictionary * _Nullable, NSError * _Nullable))completion;
// A nil error means the policy was re-read and confirmed, not merely queued.
- (void)keepItem:(NSDictionary *)item completion:(void (^)(NSError * _Nullable))completion;
@end

void SchnauzNextcloudLocalHandle(NSDictionary *request,
    id<SchnauzNextcloudLocalAdapter> adapter,
    void (^completion)(NSDictionary *reply));
void SchnauzNextcloudLocalRequest(NSDictionary *request,
    void (^completion)(NSDictionary *reply));
NS_ASSUME_NONNULL_END
