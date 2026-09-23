/**
 * The 4 voice states as families tuned them. The engine owns the profile now,
 * so `voiceStateProfile(pattern, state)` returns the exact floats it uses --
 * no second mapping here, and no drift against the JSON. This module adds only
 * what a page has to supply: the simulated level standing in for a mic or an
 * agent's voice, as `audioLevel` plus 16 bands.
 *
 * `spec/voice-state-profile.json` is still read for what the engine doesn't
 * return: families' per-pattern `exceptions` and their `simulated` note (down
 * to the view-side clock, since `ink` and the inhale are engine keys now).
 */
import { voiceStateProfile } from "@sinua/core";
import profile from "../../../spec/voice-state-profile.json";

export type VoiceState = "idle" | "listening" | "thinking" | "speaking";
export const VOICE_STATES: VoiceState[] = ["idle", "listening", "thinking", "speaking"];

type Overrides = Record<string, number>;
interface StateEntry {
  speed?: number;
  ink?: number;
  audio?: "mic" | "agent" | null;
  audioStrength?: number;
  overrides?: Overrides;
}
const doc = profile as unknown as {
  simulated: Record<string, string>;
  voiceStateCode: Record<string, number>;
  base: Overrides;
  states: Record<string, StateEntry>;
  patterns: Record<string, { note?: string; states: Record<string, StateEntry> }>;
  exceptions: Record<string, { status: string; issue: string; suggested?: string; patterns?: string[] }>;
};

export interface VoiceStateVisual {
  overrides: Overrides;
  speed: number;
  /** True while the state is driven by a level (mic when listening, the agent when speaking). */
  audio: "mic" | "agent" | null;
}

/** Bands for a simulated level: a simple falling spectrum, so bars read as sound without any audio device. */
function bands(level: number, seed: number): Overrides {
  const out: Overrides = { audioBandCount: 16 };
  for (let i = 0; i < 16; i++) {
    const tilt = 1 - i / 20;
    const wobble = 0.75 + 0.25 * Math.sin(seed + i * 1.7);
    out[`audioBand${i}`] = Math.max(0, Math.min(1, level * tilt * wobble));
  }
  return out;
}

/**
 * The overrides for one pattern in one state. `level` (0..1) stands in for the
 * mic or the agent's voice; `phase` only shifts the simulated bands.
 */
export function voiceStateVisual(pattern: string, state: VoiceState, level = 0, phase = 0): VoiceStateVisual {
  // A pattern the engine has no profile for keeps its own defaults.
  const engine = voiceStateProfile(pattern, state) ?? { speed: 1, overrides: {}, audioInput: null };
  const overrides: Overrides = { ...engine.overrides, voiceStateCode: doc.voiceStateCode[state] ?? 0 };
  // `audioInput` says which level the view should feed; the page simulates it.
  const audio = engine.audioInput === "micLevel" ? "mic" : engine.audioInput ? "agent" : null;
  if (audio) {
    overrides.audioLevel = level;
    Object.assign(overrides, bands(level, phase));
  }
  return { overrides, speed: engine.speed, audio };
}

/**
 * families' open caveat for a pattern, if they listed one (shown as a caveat,
 * never tuned around, so the engine fix lands once). `fixed` entries are
 * already handled by their tuned overrides, so they aren't shown.
 */
export function stateCaveat(pattern: string): { issue: string; status: string } | null {
  const direct = doc.exceptions[pattern];
  if (direct && !direct.patterns) return direct.status === "fixed" ? null : { issue: direct.issue, status: direct.status };
  for (const e of Object.values(doc.exceptions)) {
    if (e.patterns?.includes(pattern) && e.status !== "fixed") return { issue: e.issue, status: e.status };
  }
  return null;
}

/** The `simulated` block's own wording, for the UI note. */
export const SIMULATED_NOTE = doc.simulated;
export const PROFILE_SOURCE = "spec/voice-state-profile.json";
