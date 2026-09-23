// FxSpecPlayer: the caller-side loop around a stateless FX Spec v1.1 --
// state cross-fade (the Studio's 250 ms cubic ease-out), input easing,
// extra runtime overrides.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { FxSpecPlayer, frameFromFxSpec } from "../dist/index.js";

const read = (f) => readFileSync(fileURLToPath(new URL(`../../../spec/examples/${f}`, import.meta.url)), "utf8");
const voice = read("voice-assistant.fxspec.json");
const fit = read("fitness-rings.fxspec.json");

test("no fade until the state changes; then 250 ms cubic ease-out", () => {
  const p = new FxSpecPlayer(voice);
  let f = p.frame(1, 1 / 60);
  assert.equal(f.previous, null);
  assert.equal(f.blend, 1);
  assert.deepEqual(f.frame, frameFromFxSpec(voice, 1));
  p.setState("speaking");
  f = p.frame(1.1, 0.1);
  assert.ok(f.previous, "fading");
  assert.ok(Math.abs(f.blend - (1 - Math.pow(1 - 0.4, 3))) < 1e-12);
  assert.deepEqual(f.previous, frameFromFxSpec(voice, 1.1));
  assert.deepEqual(f.frame, frameFromFxSpec(voice, 1.1, { state: "speaking" }));
  assert.equal(f.resolved.stateKey, "speaking");
  p.frame(1.2, 0.1);
  f = p.frame(1.3, 0.1);
  assert.equal(f.previous, null);
  assert.equal(f.blend, 1);
});

test("inputs: first value as-is, later ones eased; unlisted inputs raw", () => {
  const p = new FxSpecPlayer(fit, { crossFade: 0, inputEaseRate: { heartRate: 5 } });
  p.setInput("heartRate", 60);
  p.setInput("steps", 5000);
  p.frame(0, 0.016);
  assert.equal(p.inputs.heartRate, 60);
  p.setInput("heartRate", 160);
  p.setInput("steps", 8000);
  const f = p.frame(0.1, 0.1);
  assert.equal(p.inputs.heartRate, 60 + 100 * 0.5);
  assert.equal(p.inputs.steps, 8000);
  assert.deepEqual(f.frame, frameFromFxSpec(fit, 0.1, { inputs: { heartRate: 110, steps: 8000 } }));
  assert.deepEqual(f.resolved.inactiveBindings.sort(), ["progress1", "progress2"]);
  p.clearInput("steps");
  assert.ok(p.frame(0.2, 0.016).resolved.inactiveBindings.includes("progress0"));
});

test("extra runtime overrides are spread last", () => {
  const p = new FxSpecPlayer(voice, { crossFade: 0 });
  p.setState("speaking");
  const extra = { pointerX: 30, pointerY: 28, pointerRadius: 20, pointerStrength: 6 };
  const f = p.frame(2, 0.016, extra);
  const plain = p.frame(2, 0.016);
  assert.notDeepEqual(f.frame, plain.frame, "pointer scatter applied");
  assert.deepEqual(plain.frame, frameFromFxSpec(voice, 2, { state: "speaking" }));
});
