// estimateCost / fxSpecCost through wasm: the classes the Rust unit tests
// pin, the no-materials baseline, and FX Spec 1.2 low-power shedding.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { estimateCost, fxSpecCost, resolveFxSpec, liquidSuitability, particleDefaults } from "../dist/index.js";

const power = readFileSync(fileURLToPath(new URL("../../../spec/examples/status-beacon-power.fxspec.json", import.meta.url)), "utf8");

test("classes and the baseline", () => {
  const plain = estimateCost("working", 64);
  const glow = estimateCost("working", 64, { glowStrength: 1 });
  assert.equal(plain.class, "light");
  assert.equal(glow.class, "heavy");
  assert.equal(glow.baseElements, plain.elements);
  assert.ok(glow.elements > 3 * glow.baseElements);
  assert.equal(estimateCost("not-a-state", 64), null);
});

test("FX Spec 1.2: low power caps fps, sheds glow + noise, and the cost shows it", () => {
  const hi = resolveFxSpec(power);
  const lo = resolveFxSpec(power, { lowPower: true });
  assert.equal(hi.maxFps, 30);
  assert.equal(lo.maxFps, 15);
  assert.deepEqual(lo.disabledMaterials, ["glow", "noise"]);
  assert.equal(lo.overrides.glowStrength, 0);
  const ch = fxSpecCost(power);
  const cl = fxSpecCost(power, { lowPower: true });
  assert.ok(cl.elements < ch.elements);
  assert.equal(cl.elements, ch.baseElements);
  assert.equal(fxSpecCost("{"), null);
});

test("liquid suitability: every level, reasons, tuned defaults", () => {
  assert.equal(liquidSuitability("searching").level, "recommended");
  assert.equal(liquidSuitability("composing").level, "ok");
  assert.equal(liquidSuitability("signaling").level, "notRecommended");
  assert.ok(liquidSuitability("signaling").reason.length > 10);
  assert.deepEqual(liquidSuitability("working").defaults, { liquidReach: 4, liquidThreshold: 0.4 });
  assert.deepEqual(liquidSuitability("glowing").defaults, {});
  assert.equal(liquidSuitability("nope"), null);
});

test("particle defaults: per state over the base table", () => {
  const base = { particleCount: 28, particleSize: 0.8, particleSpread: 0.18, particleLife: 4.5, particleStyle: 0, particleSync: 0, particleAudio: 0 };
  assert.deepEqual(particleDefaults("working"), base);
  assert.equal(particleDefaults("tracking").particleStyle, 3);
  assert.equal(particleDefaults("scanning").particleStyle, 1);
  assert.equal(particleDefaults("notifying").particleSync, 1);
  assert.equal(particleDefaults("speaking").particleAudio, 1);
  assert.equal(particleDefaults("breathing").particleStyle, 2);
  assert.equal(particleDefaults("nope"), null);
});
