// Materials cross-platform check, step 1: the Web side's frames for every
// materials golden case in spec/sinua-golden.json (effect runs, glow blur), built with
// the same state/t/overrides the native render tests use.
//   node --experimental-wasm-modules frames.mjs out/frames.json
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { frameWithOverrides } from "@sinua/core";

const golden = JSON.parse(readFileSync(new URL("../../../../spec/sinua-golden.json", import.meta.url), "utf8"));
const MATERIALS = /glow-blur/;
// Synthetic, information-only cases (not in the golden file): keys starting "x-"
// are reported by compare.cjs without failing. None at the moment.
export const SYNTHETIC = [];
const out = process.argv[2] ?? "out/frames.json";
const frames = [...golden.cases, ...SYNTHETIC]
  .map((c) => ({ key: c.key, frame: frameWithOverrides(c.state, c.size, c.t, c.overrides) }))
  .filter(({ key }) => MATERIALS.test(key) || key.startsWith("x-"));
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(frames));
console.log(`${frames.length} materials cases -> ${out}`);
