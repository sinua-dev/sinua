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
import { resolved_opts_json, resolve_fx_spec_json } from "../pkg/sinua_core_inline.js";
import { frameWithOverridesPacked, unpackFrame } from "./packed.js";
import type { FxContext, FxSpec, FxSpecResolved, OrbFrame, OrbSize, OrbState, ResolvedPreset } from "./index.js";

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
