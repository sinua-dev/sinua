// node --test: the Reactive-Input binding helper (src/reactive.ts), run
// against the built dist/ (so `npm run build` first), plus an end-to-end
// check through the real wasm engine.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  bindReactiveInput,
  reactiveMapper,
  ReactiveBinding,
  REACTIVE_TARGETS,
  frameWithOverrides,
} from "../dist/index.js";

const close = (a, b, eps = 1e-6) => assert.ok(Math.abs(a - b) < eps, `${a} ≉ ${b}`);

test("linear two-stop mapping, clamped at both ends", () => {
  const m = reactiveMapper({ target: "progress", input: [0, 10000] });
  close(m(5000).progress, 0.5);
  assert.equal(m(-50).progress, 0);
  assert.equal(m(25000).progress, 1);
  assert.deepEqual(Object.keys(m(1)), ["progress"], "no companions for progress");
});

test("descending input maps in input order", () => {
  // Latency in ms -> connection quality: 0 ms is perfect, 500 ms is lost.
  const m = reactiveMapper({ target: "quality", input: [500, 0] });
  close(m(0).quality, 1);
  close(m(500).quality, 0);
  close(m(125).quality, 0.75);
  assert.equal(m(9999).quality, 0);
});

test("multi-stop with one curve per segment", () => {
  const m = reactiveMapper({
    target: "glowStrength",
    input: [0, 50, 100],
    output: [0, 0.2, 1],
    curve: ["linear", "easeIn"],
  });
  close(m(25).glowStrength, 0.1);
  close(m(50).glowStrength, 0.2);
  assert.ok(m(75).glowStrength < 0.6, "easeIn sits below linear mid-segment");
  close(m(100).glowStrength, 1);
});

test("named curves: endpoints exact, easeIn below / easeOut above linear", () => {
  for (const curve of ["ease", "easeIn", "easeOut", "easeInOut"]) {
    const m = reactiveMapper({ target: "pulseStrength", curve });
    close(m(0).pulseStrength, 0);
    close(m(1).pulseStrength, 1);
  }
  assert.ok(bindReactiveInput({ value: 0.5, target: "noiseStrength", curve: "easeIn" }).noiseStrength < 0.5);
  assert.ok(bindReactiveInput({ value: 0.5, target: "noiseStrength", curve: "easeOut" }).noiseStrength > 0.5);
  close(bindReactiveInput({ value: 0.5, target: "noiseStrength", curve: "easeInOut" }).noiseStrength, 0.5, 1e-4);
  close(bindReactiveInput({ value: 0.3, target: "noiseStrength", curve: (u) => u * u }).noiseStrength, 0.09);
});

test("output is clamped to the target's legal range; NaN maps to its min", () => {
  close(bindReactiveInput({ value: 1, target: "progress", output: [0, 3] }).progress, 1);
  close(bindReactiveInput({ value: 0.1, target: "progress", output: [-2, 3] }).progress, 0);
  assert.equal(bindReactiveInput({ value: NaN, target: "accuracy" }).accuracy, 0);
});

test("audioLevel carries its audioStrength companion, and a later spread wins", () => {
  const o = bindReactiveInput({ value: 120, target: "audioLevel", input: [60, 180] });
  close(o.audioLevel, 0.5);
  assert.equal(o.audioStrength, REACTIVE_TARGETS.audioLevel.companions.audioStrength);
  assert.equal({ ...o, audioStrength: 0.3 }.audioStrength, 0.3);
});

test("invalid specs throw at construction", () => {
  assert.throws(() => reactiveMapper({ target: "nope" }), /unknown target/);
  assert.throws(() => reactiveMapper({ target: "progress", input: [0] }), /two stops/);
  assert.throws(() => reactiveMapper({ target: "progress", input: [0, 1], output: [0, 0.5, 1] }), /same length/);
  assert.throws(() => reactiveMapper({ target: "progress", input: [0, 5, 3], output: [0, 0.5, 1] }), /ascending/);
  assert.throws(() => reactiveMapper({ target: "progress", input: [2, 2] }), /ascending/);
  assert.throws(() => reactiveMapper({ target: "progress", curve: ["linear", "linear"] }), /curve/);
  assert.throws(() => reactiveMapper({ target: "progress", curve: "bouncy" }), /unknown curve/);
});

test("ReactiveBinding: no easing by default, first push taken as-is, then eased", () => {
  const raw = new ReactiveBinding({ target: "progress", input: [0, 10] });
  assert.equal(raw.overrides(0.016).progress, 0, "range min before any push");
  raw.push(5);
  close(raw.overrides(0.016).progress, 0.5);
  raw.push(10);
  close(raw.overrides(0.016).progress, 1, 1e-12);

  const eased = new ReactiveBinding({ target: "progress", input: [0, 10] }, { easeRate: 4 });
  eased.push(2);
  close(eased.overrides(0.016).progress, 0.2, 1e-12);
  eased.push(10);
  let prev = 0.2;
  for (let i = 0; i < 60; i++) {
    const v = eased.overrides(1 / 60).progress;
    assert.ok(v > prev && v <= 1, "rises monotonically toward the goal");
    prev = v;
  }
  assert.ok(prev > 0.95, `settles within a second at rate 4 (got ${prev})`);

  const stall = new ReactiveBinding({ target: "progress" }, { easeRate: 4 });
  stall.push(0);
  stall.push(1);
  close(stall.overrides(10).progress, 0.4, 1e-12); // dt clamped to 0.1 -> k = 0.4
});

test("tracking's per-ring targets: default output = one lap (the goal), laps opt-in up to 3", () => {
  // Default output is [0, 1]: the goal fills one lap, overshoot clamps at the goal.
  close(bindReactiveInput({ value: 0.5, target: "progress2" }).progress2, 0.5);
  assert.equal(bindReactiveInput({ value: 9, target: "progress2" }).progress2, 1);
  assert.equal(bindReactiveInput({ value: 10_000, target: "progress0", input: [0, 10_000] }).progress0, 1);
  // Laps are explicit: a multi-stop output past 1, clamped to the 0..3 range.
  const laps = { target: "progress0", input: [0, 10_000, 30_000], output: [0, 1, 3] };
  assert.equal(bindReactiveInput({ ...laps, value: 20_000 }).progress0, 2);
  assert.equal(bindReactiveInput({ value: 1, target: "progress2", output: [0, 9] }).progress2, 3);
  close(bindReactiveInput({ value: 0.5, target: "progress1", output: [0, 1] }).progress1, 0.5);
  assert.deepEqual(REACTIVE_TARGETS.progress0.range, [0, 3]);
  // A bound 1.25 laps draws the same frame as the raw key: full lap + halo + top arc.
  const bound = frameWithOverrides("tracking", 64, 0, bindReactiveInput({ value: 1.25, target: "progress0", input: [0, 3], output: [0, 3] }));
  assert.deepEqual(bound, frameWithOverrides("tracking", 64, 0, { progress0: 1.25 }));
  assert.ok(bound.polylines.some((p) => p.white > 0.99), "the lap's separation halo is there");
});

test("end to end through the wasm engine", () => {
  const bound = frameWithOverrides("completing", 64, 0, bindReactiveInput({ value: 5000, target: "progress", input: [0, 10000] }));
  const direct = frameWithOverrides("completing", 64, 0, { progress: 0.5 });
  assert.deepEqual(bound, direct);

  const still = frameWithOverrides("working", 64, 1.2, {});
  const breathing = frameWithOverrides("working", 64, 1.2, bindReactiveInput({ value: 170, target: "audioLevel", input: [60, 180] }));
  assert.notDeepEqual(breathing, still, "a bound heart rate breathes the orb");
});
