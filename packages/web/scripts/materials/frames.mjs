// Materials cross-platform check, step 1: the Web side's frames for every
// materials golden case in spec/sinua-golden.json (fills, effects, liquid,
// particles, holo, per-vertex hues), built with the same state/t/overrides the native render tests use.
//   node --experimental-wasm-modules frames.mjs out/frames.json
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { frameWithOverrides } from "@sinua/core";

const golden = JSON.parse(readFileSync(new URL("../../../../spec/sinua-golden.json", import.meta.url), "utf8"));
const MATERIALS = /(fill|glow-blur|liquid|particles|holo)/;
// Synthetic, information-only cases (not in the golden file): per-vertex strokes
// under a blur / additive effect run at the composite -- no golden case has one.
// compare.cjs reports keys starting "x-" without failing on them.
export const SYNTHETIC = [
  { key: "x-completing-64-0.6-holo-glowblur", state: "completing", size: 64, t: 0.6, overrides: { holoStrength: 1, glowStrength: 0.8, glowMode: 1 } },
  { key: "x-completing-64-0.6-holo-glowblur-additive", state: "completing", size: 64, t: 0.6, overrides: { holoStrength: 1, glowStrength: 0.8, glowMode: 1, glowBlend: 1 } },
];
const out = process.argv[2] ?? "out/frames.json";
const hasHues = (f) => !!f && f.polylines.some((p) => p.hues && p.hues.length);
// Per-vertex colour is one paint concept, checked at the size and time every native
// literal uses (64 / 0.6), on input cases (a key suffix): a pattern that is colourful
// by default (edge `framing`) would otherwise bring every plain case along. Box-layout
// cases with an aspect other than 1 are left out -- the render tests paint a square.
const hueCase = (key, c) => /-64-0\.6-/.test(key) && (c.overrides?.aspect ?? 1) === 1;
const frames = [...golden.cases, ...SYNTHETIC]
  .map((c) => ({ c, key: c.key, frame: frameWithOverrides(c.state, c.size, c.t, c.overrides) }))
  // Materials by key, plus any case with per-vertex stroke colour (e.g. `…-gradient3`).
  .filter(({ c, key, frame }) => MATERIALS.test(key) || key.startsWith("x-") || (hasHues(frame) && hueCase(key, c)))
  .map(({ key, frame }) => ({ key, frame }));
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(frames));
console.log(`${frames.length} materials cases -> ${out}`);
