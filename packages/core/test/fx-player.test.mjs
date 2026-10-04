// FxSpecPlayer: the caller-side loop around a stateless FX Spec v1.1 --
// state transitions (the engine's transition system: params / morph /
// cross-fade, 1.9 `transitions` timing), input easing, extra runtime overrides.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  FxSpecPlayer,
  frameFromFxSpec,
  frameWithOverrides,
  resolveFxSpec,
  resolvedOpts,
  transitionMix,
  fxSpecTransition,
  LAG_95,
} from "../dist/index.js";

const read = (f) => readFileSync(fileURLToPath(new URL(`../../../spec/examples/${f}`, import.meta.url)), "utf8");
const voice = read("voice-assistant.fxspec.json");
const fit = read("fitness-rings.fxspec.json");
const glowing = read("voice-assistant-glowing.fxspec.json");

/** A state of a spec as a transition side (effective speed), the way the player builds it. */
const sideOf = (spec, state) => {
  const r = resolveFxSpec(spec, { state });
  return { side: { state: r.state, speed: resolvedOpts(r.state, r.size).speed * r.speed, overrides: r.overrides }, size: r.size };
};

test("different patterns: nothing until the state changes, then a cross-fade on the transition clock (0.25 s into speaking)", () => {
  const p = new FxSpecPlayer(voice);
  let f = p.frame(1, 1 / 60);
  assert.equal(f.previous, null);
  assert.equal(f.blend, 1);
  assert.deepEqual(f.frame, frameFromFxSpec(voice, 1));
  p.setState("speaking");
  f = p.frame(1.1, 0.1);
  const from = sideOf(voice, undefined).side;
  const { side: to, size } = sideOf(voice, "speaking");
  // Design note 31: the profile's time for the pair, on the three-lag clock (~95 % in d).
  const d = fxSpecTransition(voice, undefined, "speaking").duration;
  assert.equal(d, 0.25);
  const k = 1 - Math.exp(-(LAG_95 / d) * 0.1);
  const weight = k * k * k; // three lags from 0 after one step
  assert.ok(f.previous, "two frames while it fades");
  assert.ok(Math.abs(f.blend - weight) < 1e-12, `${f.blend} vs ${weight}`);
  // The phase: `elapsed × speed` until the speed changed, then the mixed speed from there.
  const t = 1 * from.speed + 0.1 * ((1 - weight) * from.speed + weight * to.speed);
  assert.deepEqual(f.previous, frameWithOverrides(from.state, size, t, from.overrides));
  assert.deepEqual(f.frame, frameWithOverrides(to.state, size, t, to.overrides));
  assert.equal(f.resolved.stateKey, "speaking");
  for (let i = 0; i < 6; i++) p.frame(1.2 + i / 10, 0.1);
  f = p.frame(1.8, 0.1);
  assert.equal(f.previous, null, "the old state's weight has gone");
  assert.equal(f.blend, 1);
});

test("same pattern: one frame whose parameters flow; counts swap only mid-way", () => {
  const p = new FxSpecPlayer(glowing);
  p.setState("idle");
  p.frame(1, 1 / 60);
  p.setState("speaking");
  const from = sideOf(glowing, "idle").side;
  const { side: to, size } = sideOf(glowing, "speaking");
  const seen = [];
  for (let i = 1; i <= 6; i++) {
    const u = i / 10 / 0.6;
    const f = p.frame(1 + i / 10, 0.1);
    const mix = transitionMix(from, to, size, Math.min(1, u), "easeInOut");
    assert.equal(mix.technique, "params");
    seen.push(f.previous ? "two" : "one");
    if (!f.previous && u < 1 && Object.keys(mix.structuralTo).length === 0) {
      assert.deepEqual(f.frame, frameWithOverrides(to.state, size, (1 + i / 10) * mix.speed, mix.overrides));
    }
  }
  assert.ok(seen.filter((s) => s === "two").length <= 2, `a dissolve only inside the swap window: ${seen}`);
  assert.equal(p.frame(2, 0.1).previous, null);
});

test("timing: the spec's 1.9 transitions block, crossFade override, and a cut", () => {
  const spec = JSON.parse(glowing);
  spec.fxSpec = "1.9";
  spec.transitions = { default: { duration: 1.2, curve: "linear" }, "idle->speaking": { duration: 0.2 } };
  const text = JSON.stringify(spec);
  assert.deepEqual(fxSpecTransition(text, "idle", "speaking"), { duration: 0.2, curve: "linear", authored: true });
  assert.deepEqual(fxSpecTransition(text, "speaking", "idle"), { duration: 1.2, curve: "linear", authored: true });
  // No rule of its own: the voice-state profile's time, the view's own clock.
  assert.deepEqual(fxSpecTransition(glowing, "idle", "speaking"), { duration: 0.25, curve: "easeInOut", authored: false });
  assert.deepEqual(fxSpecTransition(glowing, "ok", "error"), { duration: 0.6, curve: "easeInOut", authored: false });
  assert.ok(resolveFxSpec(text).ok);

  const quick = new FxSpecPlayer(text);
  quick.setState("idle");
  quick.frame(0, 0.016);
  quick.setState("speaking");
  const during = quick.speed();
  quick.frame(0.1, 0.1);
  const after = quick.frame(0.25, 0.15);
  const { side: toQ } = sideOf(text, "speaking");
  assert.notEqual(during, toQ.speed, "still mixing at 0.1 s");
  assert.equal(after.previous, null);
  assert.equal(quick.speed(), toQ.speed, "done within its 0.2 s");

  const cut = new FxSpecPlayer(glowing, { crossFade: 0 });
  cut.setState("idle");
  cut.frame(0, 0.016);
  cut.setState("speaking");
  const f = cut.frame(0.05, 0.05);
  assert.equal(f.previous, null);
  const { side: to, size } = sideOf(glowing, "speaking");
  assert.deepEqual(f.frame, frameWithOverrides(to.state, size, 0.05 * to.speed, to.overrides), "crossFade: 0 is a cut");
});

test("a change mid-transition starts from what is on screen", () => {
  const p = new FxSpecPlayer(glowing);
  p.setState("idle");
  p.frame(0, 0.016);
  p.setState("speaking");
  p.frame(0.2, 0.2);
  const mid = p.speed();
  p.setState("thinking");
  const f = p.frame(0.2, 0);
  assert.equal(f.previous, null, "no jump back to idle or on to speaking");
  assert.ok(Math.abs(p.speed() - mid) < 1e-9, "the speed continues from the mix");
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
