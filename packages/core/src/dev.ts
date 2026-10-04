// `@sinua/core/dev` (design note 33): what only the Studio, dev tools, tests and
// demos use -- render cost, liquid suitability, the parameter catalog, override
// checks, simulated conversations and the JSON bridges -- kept out of the default
// entry so apps don't ship them.
//
// The entry (`dist-dev/dev-entry.js`, written by scripts/dev-entry.mjs) re-exports
// the whole default API too, bound to the same dev wasm, so a tool that imports
// from here runs ONE engine: catalog packs and recipes it registers are the ones
// its views draw.
import {
  frame_json,
  frame_json_with_overrides,
  frame_from_fx_spec_json,
  estimate_cost_json,
  fx_spec_cost_json,
  liquid_suitability_json,
  parameter_catalog_json,
  check_overrides_json,
} from "../pkg-dev/sinua_core_inline.js";
import { specText } from "./engine.js";
import type {
  FxContext,
  FxCost,
  FxDiagnostic,
  FxSpec,
  LiquidSuitability,
  OrbFrame,
  OrbSize,
  OrbState,
  ParameterCatalog,
} from "./index.js";

export * from "./conversation.js";
export * from "./simulated.js";

/**
 * `frame` through the old JSON bridge. Only for the parity tests
 * (`test/packed.test.mjs`) and `bench/transport.mjs`; apps use `frame`.
 */
export function frameViaJson(state: OrbState, size: OrbSize, t: number): OrbFrame | null {
  return JSON.parse(frame_json(state, size, t)) as OrbFrame | null;
}

/** `frameWithOverrides` through the old JSON bridge (parity tests and benchmarks). */
export function frameWithOverridesViaJson(
  state: OrbState,
  size: OrbSize,
  t: number,
  overrides: Partial<Record<string, number>>
): OrbFrame | null {
  return JSON.parse(frame_json_with_overrides(state, size, t, JSON.stringify(overrides))) as OrbFrame | null;
}

/** `frameFromFxSpec` through the old JSON bridge (parity tests and benchmarks). */
export function frameFromFxSpecViaJson(spec: FxSpec | string, elapsed: number, ctx: FxContext = {}): OrbFrame | null {
  return JSON.parse(frame_from_fx_spec_json(specText(spec), elapsed, JSON.stringify(ctx))) as OrbFrame | null;
}

/** Cost of `(state, size)` with `overrides` (e.g. the Studio's live knobs). */
export function estimateCost(state: OrbState, size: OrbSize, overrides: Partial<Record<string, number>> = {}): FxCost | null {
  return JSON.parse(estimate_cost_json(state, size, JSON.stringify(overrides))) as FxCost | null;
}

/** Cost of an FX Spec as resolved for `ctx` (state, inputs, lowPower). `null` if it has errors. */
export function fxSpecCost(spec: FxSpec | string, ctx: FxContext = {}): FxCost | null {
  return JSON.parse(fx_spec_cost_json(specText(spec), JSON.stringify(ctx))) as FxCost | null;
}

/** Per-state liquid suitability for a Studio badge; `null` for an unknown state. */
export function liquidSuitability(state: OrbState): LiquidSuitability | null {
  return JSON.parse(liquid_suitability_json(state)) as LiquidSuitability | null;
}

let catalogCache: ParameterCatalog | undefined;

/**
 * The parameter catalog: every object, pattern and tunable, with labels,
 * valid/UI ranges, groups, paths and per-size defaults. Rust owns it; this is
 * `spec/parameters.json` without the descriptions (they stay out of the
 * runtime; they are in that file, the components' doc comments and on
 * sinua.dev). Parsed once.
 */
export function parameterCatalog(): ParameterCatalog {
  return (catalogCache ??= JSON.parse(parameter_catalog_json()) as ParameterCatalog);
}

/**
 * Validates engine overrides for a pattern against the catalog. Warnings
 * only (unknown key with a did-you-mean, out of range, fractional value for
 * a whole-number key, renamed key); the frame renders regardless.
 */
export function checkOverrides(pattern: OrbState, size: OrbSize, overrides: Partial<Record<string, number>>): FxDiagnostic[] {
  return JSON.parse(check_overrides_json(pattern, size, JSON.stringify(overrides))) as FxDiagnostic[];
}
