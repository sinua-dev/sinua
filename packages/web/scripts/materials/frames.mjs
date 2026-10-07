// Materials cross-platform check, step 1: the Web side's frames for every
// materials golden case in spec/sinua-golden.json (fills, effects, glow blur,
// per-vertex hues), built with the same state/t/overrides the native render tests use.
//   node --experimental-wasm-modules frames.mjs out/frames.json
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { frameWithOverrides, resolveFxSpec } from "@sinua/core";

const golden = JSON.parse(readFileSync(new URL("../../../../spec/sinua-golden.json", import.meta.url), "utf8"));
const MATERIALS = /(fill|glow-blur)/;
// A character is fills only (docs/character.md): per character, the celebrate effect
// (blurred glows, gradients, clipped shading, sparkles) and one more everyday frame.
const CHARACTER = /^(buzzy-64-0\.6-(celebrate|muted|turned)|(hum|wisp|chirp)-64-0\.6-(celebrate|barge-in)|chirp-64-0\.6-turned|cuppa-64-0\.6-celebrate|bean-64-0\.6-turned|beep-64-0\.6-celebrate)$/;
// Synthetic, information-only cases (not in the golden file): per-vertex strokes
// under a blur / additive effect run at the composite -- no golden case has one.
// compare.cjs reports keys starting "x-" without failing on them.
export const SYNTHETIC = [
  { key: "x-completing-64-0.6-gradient-glowblur", state: "completing", size: 64, t: 0.6, overrides: { gradientStrength: 1, gradientHue: 200, gradientHue2: 300, gradientHue3: 40, glowStrength: 0.8, glowMode: 1 } },
  { key: "x-completing-64-0.6-gradient-glowblur-additive", state: "completing", size: 64, t: 0.6, overrides: { gradientStrength: 1, gradientHue: 200, gradientHue2: 300, gradientHue3: 40, glowStrength: 0.8, glowMode: 1, glowBlend: 1 } },
];
// FX Spec rows (1.13, design note 22): the showcase examples (elliptical gradients,
// soft layers, rims, grain), resolved from spec/examples at their base design.
// Checked like golden rows (they fail on a mismatch).
export const SPEC_ROWS = [
  { key: "rich-bean-64-0.6-spec", file: "rich-bean.fxspec.json" },
  { key: "rich-buzzy-64-0.6-spec", file: "rich-buzzy.fxspec.json" },
];
const specFrame = (file) => {
  const json = readFileSync(new URL(`../../../../spec/examples/${file}`, import.meta.url), "utf8");
  const r = resolveFxSpec(json, { state: "idle" });
  if (!r.ok) throw new Error(`${file}: ${JSON.stringify(r.diagnostics)}`);
  return frameWithOverrides(r.state, 64, 0.6, r.overrides);
};
const out = process.argv[2] ?? "out/frames.json";
const hasHues = (f) => !!f && f.polylines.some((p) => p.hues && p.hues.length);
// Per-vertex colour is one paint concept, checked at the size and time every native
// literal uses (64 / 0.6), on input cases (a key suffix): a pattern that is colourful
// by default would otherwise bring every plain case along. Box-layout
// cases with an aspect other than 1 are left out -- the render tests paint a square.
const hueCase = (key, c) => /-64-0\.6-/.test(key) && (c.overrides?.aspect ?? 1) === 1;
const frames = [...golden.cases, ...SYNTHETIC]
  .map((c) => ({ c, key: c.key, frame: frameWithOverrides(c.state, c.size, c.t, c.overrides) }))
  // Materials by key, plus any case with per-vertex stroke colour (e.g. `…-gradient3`).
  .filter(({ c, key, frame }) => MATERIALS.test(key) || CHARACTER.test(key) || key.startsWith("x-") || (hasHues(frame) && hueCase(key, c)))
  .map(({ key, frame }) => ({ key, frame }))
  .concat(SPEC_ROWS.map(({ key, file }) => ({ key, frame: specFrame(file) })));
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(frames));
console.log(`${frames.length} materials cases -> ${out}`);
