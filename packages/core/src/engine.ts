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
  apply_loadout_json,
  cosmetics_for_json,
  frame_still_json,
  load_catalog_json,
  unload_catalog_json,
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
  FxDiagnostic,
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

/**
 * An end user's choice for a character (FX Spec 1.13, design note 25): small, so the
 * app stores it in its own account and hands it back next launch. `wear`: ids from the
 * spec's `wardrobe` (or its `cosmetics`), one per slot; `palette`: a `wardrobe.palettes`
 * name or a built-in palette (`sunset`, `ocean`, ...); `iris`: an eye colour by name, a
 * `wardrobe.irises` name or a catalog one (`catalog:eyes-hazel`, design note 27), joining the
 * palette; `eyeStyle`: an eye style.
 */
export interface Loadout {
  /** The loadout format, 1. */
  loadout?: number;
  /**
   * Ids, or `{ id, offset, scale, rotate }` for a bounded nudge relative to the slot
   * (C2: `offset` ±10 units, `scale` 0.8–1.2, `rotate` ±15°; out of range warns and is
   * clamped). It still follows the rig.
   */
  wear?: (string | { id: string; offset?: [number, number]; scale?: number; rotate?: number })[];
  palette?: string;
  /** An eye colour by name (never a colour): it colours the glossy eye. */
  iris?: string;
  eyeStyle?: "auto" | "shape" | "glossy" | "pixel" | "dot";
}

/** A loadout applied: the spec with the choices in it, and warnings. */
export interface LoadoutApplied {
  spec: string;
  /** Warnings only: what the spec no longer offers is skipped, the rest applies. */
  diagnostics: FxDiagnostic[];
}

/**
 * `loadout` applied to `spec` (design note 25). Nothing in a loadout is an error: an
 * item or palette the spec no longer offers, a newer format or an unknown key warns and
 * is skipped, and the spec still draws. The views do this for their `loadout` option.
 */
export function applyLoadout(spec: FxSpec | string, loadout: Loadout | unknown): LoadoutApplied {
  return JSON.parse(apply_loadout_json(specText(spec), JSON.stringify(loadout ?? null))) as LoadoutApplied;
}

/** One wardrobe item for a picker: whether it fits a character, and why not. */
export interface CosmeticFit {
  id: string;
  fits: boolean;
  /**
   * A key to translate: `fits`, `no-slot` (the character has no such slot),
   * `not-made-for` (its `fits` leaves it out) or `missing-tag` (it `requires` a tag the
   * character lacks).
   */
  reason: "fits" | "no-slot" | "not-made-for" | "missing-tag";
  /** The reason in English ("" when it fits). */
  why: string;
}

/**
 * What `spec`'s wardrobe (and its `cosmetics`) offers `character` (a built-in id, or the
 * spec's own recipe's id), for a picker screen. Labels and categories are in the spec.
 */
export function cosmeticsFor(spec: FxSpec | string, character: string): CosmeticFit[] {
  // The engine carries each row in the diagnostic record: path = id, severity = reason.
  const rows = JSON.parse(cosmetics_for_json(specText(spec), character)) as { path: string; severity: CosmeticFit["reason"]; message: string }[];
  return rows.map((d) => ({ id: d.path, fits: d.severity === "fits", reason: d.severity, why: d.message }));
}

/**
 * A thumbnail's frame (design note 25): `spec` with `loadout` in a still pose (no blink,
 * no glance) at `size`, turned `turnYaw` radians (0 = facing; about ±0.5 shows another
 * angle). It never takes the live characters' place in the engine. `null` if the spec
 * doesn't resolve. Paint it with your renderer, or use `@sinua/web`'s `characterThumbnail`.
 */
export function frameStill(
  spec: FxSpec | string,
  size: OrbSize,
  opts: { loadout?: Loadout; turnYaw?: number } = {}
): OrbFrame | null {
  const lo = opts.loadout ? JSON.stringify(opts.loadout) : "";
  return JSON.parse(frame_still_json(specText(spec), lo, size, opts.turnYaw ?? 0)) as OrbFrame | null;
}

/**
 * Loads a catalog pack (FX Spec 1.13, design note 26): ready cosmetics and palettes as
 * data, `{ "catalog": 1, "namespace": "...", "cosmetics": [...], "palettes": {...} }`.
 * Specs then name its items as `"<namespace>:<id>"` in `cosmetics`, `wardrobe.cosmetics`
 * and `palette` (a loadout's too). Loading a namespace again replaces it; an error loads
 * nothing. Sinua's own pack is `@sinua/web/catalog`; a brand loads its own the same way,
 * from a bundled file or a URL.
 */
export function loadCatalog(pack: string | object): FxDiagnostic[] {
  return JSON.parse(load_catalog_json(typeof pack === "string" ? pack : JSON.stringify(pack))) as FxDiagnostic[];
}

/** Forgets a catalog pack's items; whether it had any. */
export function unloadCatalog(namespace: string): boolean {
  return unload_catalog_json(namespace);
}
