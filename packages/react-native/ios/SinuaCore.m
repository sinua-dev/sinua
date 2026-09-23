#import <React/RCTBridgeModule.h>

@interface RCT_EXTERN_MODULE (SinuaCore, NSObject)

RCT_EXTERN_METHOD(resolveFrame
                  : (NSString *)state size
                  : (nonnull NSNumber *)size t
                  : (nonnull NSNumber *)t resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(resolveFrameWithOverrides
                  : (NSString *)state size
                  : (nonnull NSNumber *)size t
                  : (nonnull NSNumber *)t overrides
                  : (NSDictionary *)overrides resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(resolveFxSpec
                  : (NSString *)json state
                  : (NSString *)state inputs
                  : (NSDictionary *)inputs lowPower
                  : (BOOL)lowPower resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(frameFromFxSpec
                  : (NSString *)json elapsed
                  : (nonnull NSNumber *)elapsed state
                  : (NSString *)state inputs
                  : (NSDictionary *)inputs lowPower
                  : (BOOL)lowPower resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(fxColorToHsl
                  : (NSString *)color resolver
                  : (RCTPromiseResolveBlock)resolve rejecter
                  : (RCTPromiseRejectBlock)reject)

@end
