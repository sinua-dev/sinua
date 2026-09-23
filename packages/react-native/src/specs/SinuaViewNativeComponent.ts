// Fabric Codegen spec for the native SinuaView host (docs/fx-view.md, *React
// Native*). The native side hosts packages/ios's SwiftUI `SinuaView` and
// packages/android's Compose `SinuaView` unchanged -- this file only declares
// the props that cross. Free-form maps (overrides, inputs) cross as JSON
// text: Codegen has no string-keyed map type. Use `SinuaView` (../SinuaView.tsx),
// which takes plain objects.
import type { CodegenTypes, HostComponent, ViewProps } from "react-native";
import { codegenNativeComponent } from "react-native";

export type FrameEvent = Readonly<{
  dtMs: CodegenTypes.Double;
  computeMs: CodegenTypes.Double;
  paintMs: CodegenTypes.Double;
}>;

export interface NativeProps extends ViewProps {
  /** FX Spec JSON text; when set, the plain-state props are ignored. */
  spec?: string;
  state?: string;
  size?: CodegenTypes.WithDefault<CodegenTypes.Int32, 64>;
  /** JSON object of engine overrides (plain state). */
  overridesJson?: string;
  speed?: CodegenTypes.WithDefault<CodegenTypes.Double, 1>;
  specState?: string;
  /** JSON object of binding inputs (spec). */
  inputsJson?: string;
  voiceLevelInput?: string;
  crossFade?: CodegenTypes.WithDefault<CodegenTypes.Double, 0.25>;
  voice?: CodegenTypes.WithDefault<"none" | "test" | "mic", "none">;
  /** A source created through the SinuaVoice module (src/voice.ts); takes precedence over `voice`. */
  voiceSourceId?: string;
  theme?: CodegenTypes.WithDefault<"auto" | "light" | "dark", "auto">;
  paused?: CodegenTypes.WithDefault<boolean, false>;
  reducedMotion?: CodegenTypes.WithDefault<"auto" | "always" | "never", "auto">;
  /** 0 = no cap beyond the display / spec. */
  maxFps?: CodegenTypes.WithDefault<CodegenTypes.Double, 0>;
  lowPower?: CodegenTypes.WithDefault<"auto" | "on" | "off", "auto">;
  label?: string;
  /** Sent at most 4 times a second (native-side throttle); only while a handler is set. */
  reportFrames?: CodegenTypes.WithDefault<boolean, false>;
  onFrame?: CodegenTypes.DirectEventHandler<FrameEvent>;
}

export default codegenNativeComponent<NativeProps>("SinuaView") as HostComponent<NativeProps>;
