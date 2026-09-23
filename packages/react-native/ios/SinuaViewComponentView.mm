// The Fabric component behind `<SinuaView>` (src/specs/SinuaViewNativeComponent.ts),
// hosting the SwiftUI SinuaView through FxHostView.swift. No public header on
// purpose: RCTViewComponentView pulls in C++ headers, and anything in this
// pod's umbrella header is also parsed by the Swift side of the same module
// (Objective-C, not C++). Codegen's `componentProvider` finds the class by name.
#import <React/RCTViewComponentView.h>
#import <UIKit/UIKit.h>

#import <react/renderer/components/SinuaViewSpec/ComponentDescriptors.h>
#import <react/renderer/components/SinuaViewSpec/EventEmitters.h>
#import <react/renderer/components/SinuaViewSpec/Props.h>
#import <react/renderer/components/SinuaViewSpec/RCTComponentViewHelpers.h>

#if __has_include(<SinuaCore/SinuaCore-Swift.h>)
#import <SinuaCore/SinuaCore-Swift.h>
#else
#import "SinuaCore-Swift.h"
#endif

using namespace facebook::react;

static NSString *_Nullable str(const std::string &s) {
  return s.empty() ? nil : [NSString stringWithUTF8String:s.c_str()];
}

@interface SinuaViewComponentView : RCTViewComponentView <RCTSinuaViewViewProtocol>
@end

@implementation SinuaViewComponentView {
  SinuaHostView *_host;
}

+ (ComponentDescriptorProvider)componentDescriptorProvider
{
  return concreteComponentDescriptorProvider<SinuaViewComponentDescriptor>();
}

- (instancetype)initWithFrame:(CGRect)frame
{
  if (self = [super initWithFrame:frame]) {
    static const auto defaultProps = std::make_shared<const SinuaViewProps>();
    _props = defaultProps;
    _host = [[SinuaHostView alloc] initWithFrame:self.bounds];
    __weak SinuaViewComponentView *weakSelf = self;
    _host.onFrameStats = ^(double dtMs, double computeMs, double paintMs) {
      SinuaViewComponentView *strongSelf = weakSelf;
      if (!strongSelf || !strongSelf->_eventEmitter) return;
      std::static_pointer_cast<const SinuaViewEventEmitter>(strongSelf->_eventEmitter)
          ->onFrame(SinuaViewEventEmitter::OnFrame{dtMs, computeMs, paintMs});
    };
    self.contentView = _host;
  }
  return self;
}

- (void)updateProps:(Props::Shared const &)props oldProps:(Props::Shared const &)oldProps
{
  const auto &p = *std::static_pointer_cast<const SinuaViewProps>(props);
  [_host applyWithSpec:str(p.spec)
                 state:str(p.state)
                  size:p.size
         overridesJson:str(p.overridesJson)
                 speed:p.speed
             specState:str(p.specState)
            inputsJson:str(p.inputsJson)
       voiceLevelInput:str(p.voiceLevelInput)
             crossFade:p.crossFade
                 voice:[NSString stringWithUTF8String:toString(p.voice).c_str()]
         voiceSourceId:str(p.voiceSourceId)
                 theme:[NSString stringWithUTF8String:toString(p.theme).c_str()]
                paused:p.paused
         reducedMotion:[NSString stringWithUTF8String:toString(p.reducedMotion).c_str()]
                maxFps:p.maxFps
              lowPower:[NSString stringWithUTF8String:toString(p.lowPower).c_str()]
                 label:str(p.label)
          reportFrames:p.reportFrames];
  [super updateProps:props oldProps:oldProps];
}

@end
