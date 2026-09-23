// FX Spec through the wasm boundary: every spec/examples/*.fxspec.json
// resolves with no errors and `frameFromFxSpec` renders exactly what
// `frameWithOverrides(resolved)` at `elapsed * presetSpeed * spec.speed`
// does -- the same property crates/core_engine/src/fx_spec.rs checks
// natively, and SinuaGoldenTests checks on iOS/Android.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { frameFromFxSpec, frameWithOverrides, resolveFxSpec, resolvedOpts } from "../dist/index.js";

const dir = fileURLToPath(new URL("../../../spec/examples/", import.meta.url));
const examples = readdirSync(dir).filter((f) => f.endsWith(".fxspec.json"));

// The same fixed inputs every platform's parity test passes.
export const TEST_INPUTS = {
  micMuted: 0, micLevel: 0.6, agentVolume: 0.5, steps: 6200, waterMl: 1800, activeMinutes: 12, heartRate: 128,
};

test(`wasm renders all ${examples.length} FX Spec examples, every state`, () => {
  assert.ok(examples.length >= 7);
  let n = 0;
  for (const file of examples) {
    const text = readFileSync(dir + file, "utf8");
    for (const state of [undefined, ...resolveFxSpec(text).stateKeys]) {
      const ctx = { state, inputs: TEST_INPUTS };
      const r = resolveFxSpec(text, ctx);
      assert.ok(r.ok, `${file} ${state}: ${JSON.stringify(r.diagnostics)}`);
      assert.deepEqual(r.inactiveBindings, [], `${file} ${state}`);
      const t = 1.3 * resolvedOpts(r.state, r.size).speed * r.speed;
      assert.deepEqual(frameFromFxSpec(text, 1.3, ctx), frameWithOverrides(r.state, r.size, t, r.overrides), `${file} ${state}`);
      n++;
    }
  }
  assert.ok(n >= 7 + 5 + 1);
});

test("diagnostics carry JSON-Pointer paths and hints", () => {
  const r = resolveFxSpec({ fxSpec: "1.8", object: "orb", pattern: "working", params: { orbitn: 3, audioLevel: 1 } });
  assert.equal(r.ok, false);
  assert.ok(r.diagnostics.some((d) => d.path === "/params/orbitn" && d.message.includes("`orbitN`")));
  assert.ok(r.diagnostics.some((d) => d.path === "/params/audioLevel" && d.severity === "error"));
  assert.equal(frameFromFxSpec({ fxSpec: "2.0", object: "orb", pattern: "working" }, 0), null);
  // The floor: 1.0-1.7 were never published and aren't read (release decision 0.1).
  const old = resolveFxSpec({ fxSpec: "1.7", object: "orb", pattern: "working" });
  assert.equal(old.ok, false);
  assert.deepEqual(old.diagnostics.map((d) => d.path), ["/fxSpec"]);
  assert.match(old.diagnostics[0].message, /reads 1\.8 and later/);
  assert.equal(resolveFxSpec("{not json").ok, false);
});
