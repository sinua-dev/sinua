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
  // Latency in ms -> a veil's level: 0 ms is full, 500 ms is empty.
  const m = reactiveMapper({ target: "level", input: [500, 0] });
  close(m(0).level, 1);
  close(m(500).level, 0);
  close(m(125).level, 0.75);
  assert.equal(m(9999).level, 0);
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
  assert.equal(bindReactiveInput({ value: NaN, target: "level" }).level, 0);
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

test("level is a plain 0..1 target: default output is its range, no companions", () => {
  close(bindReactiveInput({ value: 0.5, target: "level" }).level, 0.5);
  assert.equal(bindReactiveInput({ value: 9, target: "level" }).level, 1);
  assert.equal(bindReactiveInput({ value: 10_000, target: "level", input: [0, 10_000] }).level, 1);
  assert.deepEqual(Object.keys(bindReactiveInput({ value: 0.2, target: "level" })), ["level"]);
  assert.deepEqual(REACTIVE_TARGETS.level.range, [0, 1]);
  // The removed targets are plain unknown names now.
  for (const gone of ["progress0", "progress3"]) {
    assert.throws(() => bindReactiveInput({ value: 0.5, target: gone }), /unknown target/);
  }
  const bound = frameWithOverrides("crystal", 64, 0, bindReactiveInput({ value: 0.4, target: "level" }));
  assert.deepEqual(bound, frameWithOverrides("crystal", 64, 0, { level: 0.4 }));
});

test("accuracy is a plain 0..1 target: default output is its range, no companions", () => {
  close(bindReactiveInput({ value: 0.5, target: "accuracy" }).accuracy, 0.5);
  assert.equal(bindReactiveInput({ value: 9, target: "accuracy" }).accuracy, 1);
  assert.equal(bindReactiveInput({ value: -3, target: "accuracy" }).accuracy, 0);
  assert.deepEqual(Object.keys(bindReactiveInput({ value: 0.2, target: "accuracy" })), ["accuracy"]);
  assert.deepEqual(REACTIVE_TARGETS.accuracy.range, [0, 1]);
  const bound = frameWithOverrides("halo", 64, 0, bindReactiveInput({ value: 0.4, target: "accuracy" }));
  assert.deepEqual(bound, frameWithOverrides("halo", 64, 0, { accuracy: 0.4 }));
});

test("quality is a plain 0..1 target: default output is its range, no companions", () => {
  close(bindReactiveInput({ value: 0.5, target: "quality" }).quality, 0.5);
  assert.equal(bindReactiveInput({ value: 9, target: "quality" }).quality, 1);
  assert.equal(bindReactiveInput({ value: -3, target: "quality" }).quality, 0);
  assert.deepEqual(Object.keys(bindReactiveInput({ value: 0.2, target: "quality" })), ["quality"]);
  assert.deepEqual(REACTIVE_TARGETS.quality.range, [0, 1]);
  const bound = frameWithOverrides("cohesion", 64, 0, bindReactiveInput({ value: 0.4, target: "quality" }));
  assert.deepEqual(bound, frameWithOverrides("cohesion", 64, 0, { quality: 0.4 }));
});

test("end to end through the wasm engine", () => {
  const bound = frameWithOverrides("dune", 64, 0, bindReactiveInput({ value: 5000, target: "progress", input: [0, 10000] }));
  const direct = frameWithOverrides("dune", 64, 0, { progress: 0.5 });
  assert.deepEqual(bound, direct);

  const still = frameWithOverrides("working", 64, 1.2, {});
  const breathing = frameWithOverrides("working", 64, 1.2, bindReactiveInput({ value: 170, target: "audioLevel", input: [60, 180] }));
  assert.notDeepEqual(breathing, still, "a bound heart rate breathes the orb");
});
