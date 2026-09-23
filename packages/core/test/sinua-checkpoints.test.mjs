// The RN example app's on-device sinua check
// (packages/react-native/example/sinuaCheck.js + sinuaCheckpoints.json)
// proven on the host before it ships: (1) the checkpoint file is exactly
// what the generator would extract from spec/sinua-golden.json today --
// so a re-baseline without regenerating it fails here, not silently on a
// phone; (2) the very function the app runs passes against the wasm build,
// and fails on a perturbed checkpoint.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { frameFromFxSpec, frameWithOverrides, fxColorToHsl, resolveFxSpec } from "../dist/index.js";
import { buildCheckpoints } from "../../react-native/example/scripts/gen-sinua-checkpoints.mjs";
import { FX_SPEC_EXAMPLE, runSinuaCheckpoints, runFxSpecCheck } from "../../react-native/example/sinuaCheck.js";

const read = (p) => JSON.parse(readFileSync(fileURLToPath(new URL(p, import.meta.url)), "utf8"));
const golden = read("../../../spec/sinua-golden.json");
const checkpoints = read("../../react-native/example/sinuaCheckpoints.json");
const wasm = async (...a) => frameWithOverrides(...a);

test("RN checkpoints are in sync with spec/sinua-golden.json", () => {
  assert.deepEqual(checkpoints, buildCheckpoints(golden), "re-run gen-sinua-checkpoints.mjs after re-baselining");
});

test("the RN on-device check passes against wasm, and catches a perturbation", async () => {
  assert.equal(await runSinuaCheckpoints(checkpoints, wasm), null);
  const bad = structuredClone(checkpoints);
  bad.cases[3].lastPolyline.last[0] += 0.01;
  assert.match(await runSinuaCheckpoints(bad, wasm), /tracking-64-0\.6-laps: lastPolyline\.last\[0\]/);
});

test("the RN FX Spec check mirrors spec/examples and passes against wasm", async () => {
  assert.deepEqual(FX_SPEC_EXAMPLE, read("../../../spec/examples/beacon-radar-glow.fxspec.json"));
  const api = {
    resolveFxSpec: async (s, ctx) => resolveFxSpec(s, ctx),
    frameFromFxSpec: async (s, e, ctx) => frameFromFxSpec(s, e, ctx),
    fxColorToHsl: async (c) => fxColorToHsl(c),
    frameWithOverrides: wasm,
  };
  assert.equal(await runFxSpecCheck(api), null);
  const broken = { ...api, frameFromFxSpec: async (s, e, ctx) => frameFromFxSpec(s, e + 0.5, ctx) };
  assert.match(await runFxSpecCheck(broken), /frameFromFxSpec != frameWithOverrides/);
});
