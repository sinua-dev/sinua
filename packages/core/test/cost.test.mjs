// estimateCost / fxSpecCost through wasm: the classes the Rust unit tests
// pin, the no-materials baseline, and FX Spec 1.2 low-power shedding.
import { test } from "node:test";
import assert from "node:assert/strict";
import { estimateCost, fxSpecCost, resolveFxSpec } from "../dist-dev/dev-entry.js";

// An always-on orb with glow and noise, power-aware: 30 fps, 15 fps and no glow / noise in low power.
const power = JSON.stringify({
  fxSpec: "1.8",
  name: "Status orb (power-aware)",
  object: "orb",
  pattern: "working",
  size: 64,
  color: { value: "#30A46C", mix: 0.8 },
  materials: { glow: { strength: 0.6, radius: 3 }, noise: { strength: 0.4, amplitude: 0.02 } },
  performance: { maxFps: 30, lowPower: { maxFps: 15, disable: ["glow", "noise"] } },
});

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

