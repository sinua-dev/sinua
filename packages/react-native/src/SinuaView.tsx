import * as React from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";
import NativeSinuaView, { type FrameEvent } from "./specs/SinuaViewNativeComponent";
import { voiceProps, type VoiceSourceHandle } from "./voice";

/**
 * The drop-in sinua view for React Native (docs/fx-view.md): the native
 * `SinuaView` of each platform (SwiftUI on iOS, Compose on Android) in a Fabric
 * component, so the frame loop, painter, voice and power policy are exactly
 * the native ones -- no per-frame bridge traffic. Give it a size via `style`.
 */
export type SinuaViewProps = ViewProps & {
  /** An FX Spec: JSON text or the parsed object. Takes precedence over `pattern`. */
  spec?: string | object;
  /** A pattern (e.g. "speaking", "tracking") when there's no spec. */
  pattern?: string;
  /**
   * The agent's lifecycle state (e.g. "listening"); default: the voice's state. With a
   * `spec` it picks that file's `states` entry; with a `pattern` and no spec it picks the
   * built-in voice-state behaviour for that pattern.
   * Deprecated fallback: with neither `spec` nor `pattern`, `state` is read as the pattern.
   */
  state?: string;
  size?: 20 | 32 | 64;
  overrides?: Record<string, number>;
  speed?: number;
  /** @deprecated Renamed to `state`. */
  specState?: string;
  inputs?: Record<string, number>;
  voiceLevelInput?: string;
  crossFade?: number;
  /**
   * A voice source: a handle from `createVoiceSource` (any vendor, or the mic / test
   * tone), or the shorthands `"test"` / `"mic"`. The view binds it and reads its state
   * and audio; it never connects or disconnects it.
   */
  voice?: "none" | "test" | "mic" | VoiceSourceHandle;
  theme?: "auto" | "light" | "dark";
  paused?: boolean;
  reducedMotion?: "auto" | "always" | "never";
  maxFps?: number;
  lowPower?: "auto" | "on" | "off";
  accessibilityLabel?: string;
  /** Frame timings, at most 4 per second. */
  onFrame?: (stats: { dtMs: number; computeMs: number; paintMs: number }) => void;
};

let warnedPlainState = false;

/**
 * FX Spec 1.7 labels -> the native component's props (`state` = pattern, `specState` = lifecycle).
 * `state` is the lifecycle state with a spec; without one it's the deprecated pattern label.
 */
export function nativeLabels(p: Pick<SinuaViewProps, "spec" | "pattern" | "state" | "specState">): { state?: string; specState?: string } {
  // With a `pattern`, `state` is the agent's lifecycle state (the built-in voice-state
  // behaviour, docs/fx-view.md); it reaches the native view as `specState` either way.
  if (p.spec != null || p.pattern != null) return { state: p.pattern, specState: p.state ?? p.specState };
  if (p.pattern == null && p.state != null && !warnedPlainState) {
    warnedPlainState = true;
    console.warn('SinuaView: `state` without a `spec` is deprecated; use `pattern` (FX Spec 1.7 naming).');
  }
  return { state: p.pattern ?? p.state, specState: p.specState };
}

export function SinuaView({ spec, pattern, state, specState, overrides, inputs, onFrame, accessibilityLabel, maxFps, voice, ...rest }: SinuaViewProps) {
  const labels = nativeLabels({ spec, pattern, state, specState });
  const specText = React.useMemo(() => (spec == null ? undefined : typeof spec === "string" ? spec : JSON.stringify(spec)), [spec]);
  const overridesJson = React.useMemo(() => (overrides ? JSON.stringify(overrides) : undefined), [overrides]);
  const inputsJson = React.useMemo(() => (inputs ? JSON.stringify(inputs) : undefined), [inputs]);
  const handler = React.useCallback((e: NativeSyntheticEvent<FrameEvent>) => onFrame?.(e.nativeEvent), [onFrame]);
  // A handle is bound by id through the native registry; the shorthands stay as they were.
  const bound = voiceProps(voice);
  return (
    <NativeSinuaView
      {...rest}
      voice={bound.voice}
      voiceSourceId={bound.voiceSourceId}
      state={labels.state}
      specState={labels.specState}
      spec={specText}
      overridesJson={overridesJson}
      inputsJson={inputsJson}
      maxFps={maxFps ?? 0}
      label={accessibilityLabel}
      accessibilityLabel={accessibilityLabel}
      reportFrames={onFrame != null}
      onFrame={onFrame ? handler : undefined}
    />
  );
}
