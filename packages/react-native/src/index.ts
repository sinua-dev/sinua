// React Native bridge over the Sinua Rust geometry engine.
//
// Classic (Promise-based) native module, not wasm and not a JS
// reimplementation -- `ios/SinuaCore.swift` and
// `android/.../SinuaCoreModule.kt` both call the *same* native
// core_engine binary that packages/ios and packages/android already
// vendor and test against `spec/orbs-golden.json`. JSON crosses the
// bridge here (matching the tradeoff already made for the Web/wasm path);
// revisit for a JSI/TurboModule if a real perf ceiling shows up.

import { NativeModules } from "react-native";

/**
 * Mirrors `packages/core`'s `OrbState` exactly: `orbs::presets::STATES`
 * (the 9 ported states plus the 8 additive ones) and `signal::presets::
 * STATES` -- the same native binary serves every family through one
 * `frame` entry point, so the union is the same on every platform.
 */
export type OrbState =
  | "working"
  | "searching"
  | "solving"
  | "listening"
  | "connecting"
  | "weaving"
  | "composing"
  | "breathing"
  | "shaping"
  | "glowing"
  | "drifting"
  | "speaking"
  | "confirming"
  | "initializing"
  | "calibrating"
  | "progressing"
  | "concluding"
  | "muted"
  | "signaling"
  | "waveform"
  | "scrolling"
  | "metering"
  | "completing"
  | "loading"
  | "tracking"
  | "stepping"
  | "measuring"
  | "notifying"
  | "reconnecting"
  | "locating"
  | "scanning"
  | "broadcasting"
  | "generating"
  | "typing";

/** Mirrors the sizes shipped in `orbs::presets::presets()`. */
export type OrbSize = 20 | 32 | 64;

/** Mirrors `primitives::Dot` -- same fields as `packages/core`'s `Dot`. */
export interface Dot {
  x: number;
  y: number;
  z: number;
  r: number;
  white: number;
  a: number;
  /** 0 = grayscale ink; above 0, HSL toward `hue`. See `packages/core`. */
  saturation: number;
  hue: number;
}

/** Mirrors `primitives::Line` (the `connecting` state's constellation edges). */
export interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  white: number;
  a: number;
  w: number;
  /** Same semantics as `Dot.saturation` -- 0 = grey (every mode's default). Added 2026-09-18. */
  saturation: number;
  /** Same semantics as `Dot.hue`. */
  hue: number;
}

/** Mirrors `primitives::Point` -- one vertex of a `Polyline`. */
export interface Point {
  x: number;
  y: number;
}

/**
 * Mirrors `primitives::Polyline`: one continuous stroked path (round caps,
 * round joins, no fill) -- see `packages/core`'s `Polyline` for the paint
 * contract. Only the `signal` family emits these.
 */
export interface Polyline {
  points: Point[];
  white: number;
  a: number;
  w: number;
  saturation: number;
  hue: number;
}

/** Mirrors `primitives::OrbFrame` -- the same shape `packages/core` returns. */
/**
 * Mirrors `primitives::ColorMode`: how a renderer resolves lightness per
 * theme. `"ink"` (default) mirrors it on dark themes (`L = 1 - white`);
 * `"fixed"` uses `white` as-is in both themes (Material 3's "fixed"
 * roles). Set by the `colorMode` opt (`apply_color`).
 */
export type ColorMode = "ink" | "fixed";

export interface OrbFrame {
  dots: Dot[];
  lines: Line[];
  polylines: Polyline[];
  /** Paint mode for the whole frame -- see `ColorMode`. */
  colorMode: ColorMode;
  /** Filled shapes (materials phase 1), drawn first; absent when none. Same shape as `packages/core`. */
  fills?: Fill[];
  /** Paint effects on element ranges (real-blur glow); absent when none. */
  effects?: EffectRun[];
}

/** Mirrors `primitives::Fill` -- see docs/engine.md, *Paint contract*. */
export interface Fill {
  points: { x: number; y: number }[];
  /** Inner rings (liquid): paint outer + holes as one even-odd path. Absent when none. */
  holes?: { x: number; y: number }[][];
  white: number;
  a: number;
  saturation: number;
  hue: number;
  gradient: {
    kind: 0 | 1;
    x0: number;
    y0: number;
    x1: number;
    y1: number;
    r: number;
    stops: { offset: number; white: number; a: number; saturation: number; hue: number }[];
  } | null;
  blur: number;
  blend: 0 | 1;
}

/** Elements [start, start+count) of list `target` (0 dots, 1 lines, 2 polylines) get blur (sigma) / blend. */
export interface EffectRun {
  target: 0 | 1 | 2;
  start: number;
  count: number;
  blur: number;
  blend: 0 | 1;
}

interface SinuaCoreNativeModule {
  // Named `resolveFrame` on the native side (both `ios/SinuaCore.swift`
  // and `android/.../SinuaCoreModule.kt`) to avoid colliding with the
  // vendored `frame(state:size:t:)` free function each platform's own file
  // already exports at module scope.
  resolveFrame(state: string, size: number, t: number): Promise<OrbFrame | null>;
  resolveFrameWithOverrides(
    state: string,
    size: number,
    t: number,
    overrides: Record<string, number>
  ): Promise<OrbFrame | null>;
  resolveFxSpec(
    json: string,
    state: string | null,
    inputs: Record<string, number>,
    lowPower: boolean
  ): Promise<FxSpecResolved>;
  frameFromFxSpec(
    json: string,
    elapsed: number,
    state: string | null,
    inputs: Record<string, number>,
    lowPower: boolean
  ): Promise<OrbFrame | null>;
  fxColorToHsl(color: string): Promise<FxHsl | null>;
}

const native = NativeModules.SinuaCore as SinuaCoreNativeModule | undefined;

/**
 * Resolve `(state, size)` and render one frame at time `t` (seconds).
 * Mirrors `core_engine::frame` in Rust -- see `crates/core_engine/src/lib.rs`.
 * Resolves to `null` for a state/size pair with no preset.
 */
export function frame(state: OrbState, size: OrbSize, t: number): Promise<OrbFrame | null> {
  if (!native) {
    throw new Error(
      "@sinua/react-native: native module 'SinuaCore' is not linked. " +
        "Did you run pod install (iOS) / rebuild the app (Android) after adding this package?"
    );
  }
  return native.resolveFrame(state, size, t);
}

/**
 * Same as `frame`, but overlays `overrides` onto the resolved preset's opts
 * before rendering. Mirrors `core_engine::frame_with_overrides` in Rust --
 * see `packages/core`'s `frameWithOverrides` for the Web/wasm equivalent
 * this was extended to match.
 */
export function frameWithOverrides(
  state: OrbState,
  size: OrbSize,
  t: number,
  overrides: Partial<Record<string, number>>
): Promise<OrbFrame | null> {
  if (!native) {
    throw new Error(
      "@sinua/react-native: native module 'SinuaCore' is not linked. " +
        "Did you run pod install (iOS) / rebuild the app (Android) after adding this package?"
    );
  }
  return native.resolveFrameWithOverrides(state, size, t, overrides as Record<string, number>);
}

// ---------------------------------------------------------------- FX Spec --
// Same contract as `packages/core`'s FX Spec wrappers (docs/fx-spec.md);
// parsing, validation and color conversion run in Rust on the native side.

/** An FX Spec document (v1) -- see `spec/fx-spec-1.schema.json`. */
export interface FxSpec {
  fxSpec: string;
  object: "orb" | "signal" | "ring" | "beacon" | "core";
  state: string;
  size?: OrbSize;
  speed?: number;
  [section: string]: unknown;
}

/** `"#RRGGBB"` / `"#RGB"`, or a W3C DTCG (2025.10) color value. */
export type FxColor = string | { colorSpace: string; components: (number | "none")[]; alpha?: number; hex?: string };

/** One problem in a spec; `path` is a JSON Pointer (e.g. `/params/colorHeu`). */
export interface FxDiagnostic {
  severity: "error" | "warning";
  path: string;
  message: string;
}

/** Mirrors `core_engine::FxSpecResolved`. `ok` is false if any diagnostic is an error. */
export interface FxSpecResolved {
  ok: boolean;
  state: string;
  size: number;
  speed: number;
  overrides: Record<string, number>;
  diagnostics: FxDiagnostic[];
  /** v1.1: the `states` key that rendered (`""` = the top-level design). */
  stateKey: string;
  /** v1.1: every key of `states`. */
  stateKeys: string[];
  /** v1.1: bound targets whose input wasn't in `ctx.inputs`. */
  inactiveBindings: string[];
  /** v1.2: the frame cap to honour (low-power cap when `ctx.lowPower`); `null` = host default. */
  maxFps: number | null;
  /** v1.2: materials shed because `ctx.lowPower` was set. */
  disabledMaterials: string[];
}

/** v1.1 call context: the current lifecycle key and the app's input values. */
export interface FxContext {
  state?: string;
  inputs?: Record<string, number>;
  /** 1.2: the host is in low power -- apply the spec's `performance.lowPower`. */
  lowPower?: boolean;
}

/** CSS Color 4 HSL (`h` degrees, `0` when `achromatic`; `s`/`l` 0..1) plus the normalized hex. */
export interface FxHsl {
  h: number;
  s: number;
  l: number;
  achromatic: boolean;
  hex: string;
}

function linked(): SinuaCoreNativeModule {
  if (!native) {
    throw new Error(
      "@sinua/react-native: native module 'SinuaCore' is not linked. " +
        "Did you run pod install (iOS) / rebuild the app (Android) after adding this package?"
    );
  }
  return native;
}

const specText = (spec: FxSpec | string) => (typeof spec === "string" ? spec : JSON.stringify(spec));

/**
 * Validate and resolve an FX Spec. Mirrors `core_engine::resolve_fx_spec_with`:
 * `ctx.state` picks a `states` entry (absent/unknown = the top-level design),
 * `ctx.inputs` drives `bindings`.
 */
export function resolveFxSpec(spec: FxSpec | string, ctx: FxContext = {}): Promise<FxSpecResolved> {
  return linked().resolveFxSpec(specText(spec), ctx.state ?? null, ctx.inputs ?? {}, ctx.lowPower ?? false);
}

/** Render an FX Spec at `elapsed` wall-clock seconds (speed scaling included); `null` on errors. */
export function frameFromFxSpec(spec: FxSpec | string, elapsed: number, ctx: FxContext = {}): Promise<OrbFrame | null> {
  return linked().frameFromFxSpec(specText(spec), elapsed, ctx.state ?? null, ctx.inputs ?? {}, ctx.lowPower ?? false);
}

/** A spec color as HSL, via the same conversion the resolver uses; `null` if it doesn't parse. */
export function fxColorToHsl(color: FxColor): Promise<FxHsl | null> {
  return linked().fxColorToHsl(typeof color === "string" ? color : JSON.stringify(color));
}

// The drop-in view (Fabric component over the native SinuaViews) -- docs/fx-view.md, *React Native*.
export { SinuaView } from "./SinuaView";
export type { SinuaViewProps } from "./SinuaView";

// Native voice sources (docs/audio-pipeline.md): created here, bound with `voice={handle}`.
export { createVoiceSource, isVoiceSourceHandle } from "./voice";
export type { AgentState, VoiceSourceConfig, VoiceSourceHandle } from "./voice";

// Typed components (SinuaOrb, SinuaRing, …) generated from spec/parameters.json -- docs/fx-view.md, *Typed components*.
export * from "./generated";
