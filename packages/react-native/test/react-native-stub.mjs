// Minimal "react-native" stand-in for the node tests: just the pieces src/voice.ts
// touches. The test installs its own module + emitter through `setNative`.
export const NativeModules = { SinuaVoice: undefined };

let handler = null;
export class NativeEventEmitter {
  constructor(_module) {}
  addListener(_event, cb) {
    handler = cb;
    return { remove: () => (handler = null) };
  }
}

/** Test helper: deliver one native event. */
export function emit(payload) {
  handler?.(payload);
}
export function setNativeModule(mod) {
  NativeModules.SinuaVoice = mod;
}
