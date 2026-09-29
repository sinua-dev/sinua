// The thin wasm wrappers that more than one module in this package needs.
//
// They used to live in `index.ts`, which made `fxPlayer.ts` import values from
// the barrel while the barrel re-exported the player -- a runtime cycle. It was
// benign (the player only calls them inside a method, so both modules are
// evaluated by then), but a loop through the barrel is the shape that makes
// evaluation order bundler-dependent, and this is the package whose history is
// two silent bundler-specific breaks. `test/no-import-cycles.test.mjs` keeps it
// from coming back.
//
// `index.ts` re-exports everything here, so the public API is unchanged: an app
// still writes `import { resolveFxSpec } from "@sinua/core"`.
//
// Types come from `./index.ts` with `import type`, which tsc erases -- so this
// module points back at the barrel on paper and not at runtime.
import {
  resolved_opts_json,
  resolve_fx_spec_json,
  transition_mix_json,
  frame_transition_with_overrides_json,
  fx_spec_transition_json,
  conversation_at_json,
  conversation_samples_json,
  conversation_sample_json,
  fx_spec_derive_state_json,
  fx_spec_accessibility_json,
  a11y_accessible_name_json,
  a11y_state_words_json,
  a11y_announce_step_json,
  effect_info_json,
} from "../pkg/sinua_core_inline.js";
import { frameWithOverridesPacked, unpackFrame } from "./packed.js";
import type {
  FxContext,
  FxSpec,
  FxSpecResolved,
  FxTransition,
  OrbFrame,
  OrbSize,
  OrbState,
  ResolvedPreset,
  TransitionMix,
  TransitionSide,
  ConversationFrame,
  FxAccessibility,
  AnnouncerState,
  AnnounceStep,
} from "./index.js";

/** A spec as the wasm bridge wants it: JSON text either way. */
export const specText = (spec: FxSpec | string): string =>
  typeof spec === "string" ? spec : JSON.stringify(spec);

/**
 * Same as `frame`, but overlays `overrides` onto the resolved preset's opts
 * before rendering -- e.g. `{ scanMul: 8 }` on `"searching"` speeds up the
 * scan sweep without touching any other tuned value. Mirrors
 * `core_engine::frame_with_overrides` in Rust. Built for the Studio's
 * live parameter sliders; `frame` has no way to accept a custom opts value.
 */
export function frameWithOverrides(
  state: OrbState,
  size: OrbSize,
  t: number,
  overrides: Partial<Record<string, number>>
): OrbFrame | null {
  return unpackFrame(frameWithOverridesPacked(state, size, t, overrides));
}

/**
 * The tuned default opts for `(state, size)`, before any override. `frame`
 * and `frameWithOverrides` only return the rendered frame, not the values
 * that produced it -- this is how a slider knows where "no override" sits.
 * Mirrors `core_engine::resolve_preset` in Rust. Returns `null` for a
 * state/size pair with no preset.
 */
export function resolvedOpts(state: OrbState, size: OrbSize): ResolvedPreset | null {
  return JSON.parse(resolved_opts_json(state, size)) as ResolvedPreset | null;
}

/**
 * Validate and resolve an FX Spec to `(state, size, speed, overrides)` plus
 * diagnostics. v1.1: `ctx.state` picks a `states` entry (absent/unknown =
 * the top-level design), `ctx.inputs` drives `bindings`. Unknown keys are always reported (errors in a 1.0 file,
 * warnings in a newer 1.x one). Mirrors `core_engine::resolve_fx_spec`.
 */
export function resolveFxSpec(spec: FxSpec | string, ctx: FxContext = {}): FxSpecResolved {
  return JSON.parse(resolve_fx_spec_json(specText(spec), JSON.stringify(ctx))) as FxSpecResolved;
}

/**
 * What to draw at `progress` (`0..1` of the duration, linear) of a state change
 * -- the one transition system every view uses (docs/fx-spec.md, *Transitions*).
 * Mirrors `core_engine::transition_mix`. `null` if a state doesn't resolve.
 */
export function transitionMix(
  from: TransitionSide,
  to: TransitionSide,
  size: OrbSize,
  progress: number,
  curve: string
): TransitionMix | null {
  return JSON.parse(transition_mix_json(JSON.stringify(from), JSON.stringify(to), size, progress, curve)) as TransitionMix | null;
}

/**
 * The lattice morph (glowing / calibrating / progressing) with each side's own
 * overrides, finished like any frame. Mirrors
 * `core_engine::frame_transition_with_overrides`.
 */
export function frameTransitionWithOverrides(
  from: TransitionSide,
  to: TransitionSide,
  size: OrbSize,
  t: number,
  blend: number
): OrbFrame | null {
  return JSON.parse(
    frame_transition_with_overrides_json(JSON.stringify(from), JSON.stringify(to), size, t, blend)
  ) as OrbFrame | null;
}

/**
 * An FX Spec's duration and curve for the change `from` -> `to` (1.9
 * `transitions`; `undefined` = the base design). Without the block: 0.6 s
 * `easeInOut`. Mirrors `core_engine::fx_spec_transition`.
 */
export function fxSpecTransition(spec: FxSpec | string, from: string | undefined, to: string | undefined): FxTransition {
  return JSON.parse(fx_spec_transition_json(specText(spec), from ?? "", to ?? "")) as FxTransition;
}

/**
 * A simulated conversation at `t` seconds: the agent state and a speech-like
 * level and `bands` bands, from a script of turns (docs/audio-pipeline.md,
 * *Simulated conversations*). Mirrors `core_engine::conversation_at`.
 */
export function conversationAt(script: string, t: number, bands = 16): ConversationFrame {
  return JSON.parse(conversation_at_json(script, t, bands)) as ConversationFrame;
}

/**
 * The `states` key an FX Spec's 1.9 `rules` pick for `inputs`, given the state the
 * rules picked last (`previous`, for hysteresis), or `null` when no rule holds (keep
 * your own state). Pure: keep `previous` yourself. Mirrors `core_engine::fx_spec_derive_state`.
 */
export function fxSpecDeriveState(json: string, inputs: Record<string, number>, previous: string | null): string | null {
  const out = fx_spec_derive_state_json(json, JSON.stringify(inputs), previous ?? "");
  return out === "" ? null : out;
}

/** An FX Spec's 1.9 `accessibility` block (docs/fx-view.md, *Accessibility*); empty when absent. */
export function fxSpecAccessibility(json: string): FxAccessibility {
  const a = JSON.parse(fx_spec_accessibility_json(json)) as { name?: string | null; states?: Record<string, string>; announce?: boolean | null };
  return { name: a.name ?? null, states: a.states ?? {}, announce: a.announce ?? null };
}

/**
 * A view's accessible name in `state`: the app's words for it, else the spec's
 * `accessibility.states`, else the built-in words for a voice state ("Coach, listening"),
 * else `name`. Mirrors `core_engine::a11y_accessible_name`.
 */
export function accessibleName(
  name: string,
  state: string | null,
  specWords: Record<string, string> = {},
  appWords: Record<string, string> = {},
): string {
  return a11y_accessible_name_json(name, state ?? "", JSON.stringify(specWords), JSON.stringify(appWords));
}

/** The words for `state` (as `accessibleName`), or `null` when it has none -- then nothing is spoken. */
export function a11yStateWords(
  name: string,
  state: string | null,
  specWords: Record<string, string> = {},
  appWords: Record<string, string> = {},
): string | null {
  const w = a11y_state_words_json(name, state ?? "", JSON.stringify(specWords), JSON.stringify(appWords));
  return w === "" ? null : w;
}

/**
 * One step of the announcer: call on every change of the words showing and again at
 * `recheckAt` (seconds, the same clock as `now`). Speaks a state once it has held 1 s, at
 * most once per 3 s, never the first one. Keep the returned `state` for the next call.
 */
export function announceStep(prev: AnnouncerState | null, words: string | null, now: number): AnnounceStep {
  return JSON.parse(a11y_announce_step_json(JSON.stringify(prev ?? {}), words ?? "", now)) as AnnounceStep;
}

/** A one-shot feedback effect (docs/fx-view.md, *One-shot effects*). */
export interface EffectInfo {
  /** The `effectCode` runtime key. */
  code: number;
  /** Seconds to keep feeding `effectAge`. */
  duration: number;
  /** What to speak (the app's `labels["effect:<name>"]` win). */
  words: string;
}

/** `success`, `error` or `celebrate`; `null` for an unknown name. Mirrors `core_engine::effect_info`. */
export function effectInfo(name: string): EffectInfo | null {
  return JSON.parse(effect_info_json(name)) as EffectInfo | null;
}

/** The built-in sample conversations: `calendar`, `quick-answer`, `long-answer`, `barge-in`. */
export function conversationSampleNames(): string[] {
  return JSON.parse(conversation_samples_json()) as string[];
}

/** A built-in sample conversation's script (JSON text), or `null`. */
export function conversationSample(name: string): string | null {
  const s = conversation_sample_json(name);
  return s === "null" ? null : s;
}
