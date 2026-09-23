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
const frames = [...golden.cases, ...SYNTHETIC]
  .map((c) => ({ key: c.key, frame: frameWithOverrides(c.state, c.size, c.t, c.overrides) }))
  // Materials by key, plus any case with per-vertex stroke colour (e.g. `…-gradient3`).
  .filter(({ key, frame }) => MATERIALS.test(key) || key.startsWith("x-") || hasHues(frame));
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(frames));
console.log(`${frames.length} materials cases -> ${out}`);
