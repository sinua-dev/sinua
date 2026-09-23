#import <React/RCTBridgeModule.h>
#import <React/RCTEventEmitter.h>

@interface RCT_EXTERN_MODULE (SinuaVoice, RCTEventEmitter)

RCT_EXTERN_METHOD(create
                  : (NSDictionary *)config resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(connect
                  : (NSString *)id resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(disconnect : (NSString *)id)

RCT_EXTERN_METHOD(release : (NSString *)id)

RCT_EXTERN_METHOD(provideCredential
                  : (NSString *)requestId credential
                  : (NSString *)credential error
                  : (NSString *)error)

@end
