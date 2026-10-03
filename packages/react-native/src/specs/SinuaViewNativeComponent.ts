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
  /** Every state change's duration, seconds; < 0 = the spec's `transitions` (default 0.6 s). */
  crossFade?: CodegenTypes.WithDefault<CodegenTypes.Double, -1>;
  /** The bound voice's pulse (VoiceOverrides `audioStrength`); < 0 = the view's default. */
  audioStrength?: CodegenTypes.WithDefault<CodegenTypes.Double, -1>;
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
  /** JSON object state -> words for the accessible name and announcements. */
  labelsJson?: string;
  /** Speak state changes: "auto" = the spec's, else on (Codegen has no optional boolean). */
  announce?: CodegenTypes.WithDefault<"auto" | "on" | "off", "auto">;
  haptics?: CodegenTypes.WithDefault<boolean, false>;
  rules?: CodegenTypes.WithDefault<boolean, true>;
  /** Tap to hop (design note 15): a tap plays `hop`, glancing toward it. Characters only. */
  tap?: CodegenTypes.WithDefault<boolean, false>;
  /** A character's expression (design note 16); "" = the spec's. */
  expression?: string;
  /** A character's palette, in part (design note 19), as JSON (slot -> hex); "" = the character's own. */
  paletteJson?: string;
  /** An end user's loadout (FX Spec 1.13, design note 25), as JSON; "" = none (the file as it is). */
  loadoutJson?: string;
  /** Catalog packs (FX Spec 1.13, design note 26) as a JSON array; "" = none. Each loads once per change. */
  catalogsJson?: string;
  /** A one-shot effect (`success` / `error` / `celebrate` / `hop`); it plays when `effectKey` changes. */
  effectName?: string;
  effectKey?: CodegenTypes.WithDefault<CodegenTypes.Int32, 0>;
  /** Sent at most 4 times a second (native-side throttle); only while a handler is set. */
  reportFrames?: CodegenTypes.WithDefault<boolean, false>;
  onFrame?: CodegenTypes.DirectEventHandler<FrameEvent>;
}

export default codegenNativeComponent<NativeProps>("SinuaView") as HostComponent<NativeProps>;
