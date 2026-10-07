// The transition clock's shared vectors (design note 31): spec/transition-timeline.json.
// Every view runs the same steps; Swift (SinuaTests) and Kotlin (sinua-view) replay
// these cases and must match to 1e-9. `TIMELINE_WRITE=1 npm test` writes the file from
// this implementation (only when the clock deliberately changes).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { StateTransition, voiceStateProfile, resolvedOpts } from "../dist/index.js";

const FILE = fileURLToPath(new URL("../../../spec/transition-timeline.json", import.meta.url));
const plain = (state, speed) => ({ state, speed, overrides: {} });
const voice = (pattern, state) => {
  const p = voiceStateProfile(pattern, state);
  return { state: pattern, speed: resolvedOpts(pattern, 64).speed * p.speed, overrides: p.overrides };
};
const ms = (x) => x / 1000;
const steady = (n, dt = 1 / 60) => Array.from({ length: n }, () => dt);

// sides: name -> side; events: [frame index, side name, duration, curve, authored]
// (the first event is the starting state); dts: one per frame.
const CASES = [
  { name: "clean change", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [5, "b", 0.25, "easeInOut"]], dts: steady(50) },
  { name: "interrupted at 30 %", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15), c: plain("glowing", 0.9) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"], [16, "c", 0.45, "easeInOut"]], dts: steady(80) },
  { name: "interrupted at 50 %", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"], [23, "a", 0.9, "easeInOut"]], dts: steady(90) },
  { name: "three-way: thinking cut short by speaking, then a barge-in", t0: 1, sides: { l: plain("glowing", 0.9), t: plain("glowing", 1.05), s: plain("glowing", 1.15) }, events: [[0, "l"], [5, "t", 0.4, "easeInOut"], [20, "s", 0.3, "easeInOut"], [32, "l", 0.45, "easeInOut"]], dts: steady(90) },
  { name: "a cut", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [5, "b", 0, "easeInOut"]], dts: steady(12) },
  { name: "two patterns cross-fade by group weight", t0: 1, sides: { a: plain("working", 1), b: plain("speaking", 1.2) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"]], dts: steady(60) },
  { name: "the lattice trio morphs", t0: 1, sides: { a: plain("glowing", 1), b: plain("calibrating", 1) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"]], dts: steady(60) },
  { name: "an authored curve keeps it, velocity carried", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15), c: plain("glowing", 0.9) }, events: [[0, "a"], [5, "b", 0.5, "easeOut", true], [14, "c", 0.5, "easeOut", true]], dts: steady(60) },
  { name: "irregular frames (16 / 33 / 8 ms)", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [4, "b", 0.5, "easeInOut"]], dts: Array.from({ length: 60 }, (_, i) => ms([16, 33, 8][i % 3])) },
  { name: "a 250 ms hitch continues the change", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"]], dts: [...steady(10), ms(250), ...steady(30)] },
  { name: "a 3 s gap (back from the background) lands on the target", t0: 1, sides: { a: plain("glowing", 0.6), b: plain("glowing", 1.15) }, events: [[0, "a"], [5, "b", 0.6, "easeInOut"]], dts: [...steady(10), 3, ...steady(5)] },
  { name: "rates accumulate from the first change of a resting view (confirming at 300 s)", t0: 300, sides: { i: voice("confirming", "idle"), s: voice("confirming", "speaking") }, events: [[0, "i"], [10, "s", 0.25, "easeInOut"]], dts: steady(60) },
];

function replay(c) {
  const tr = new StateTransition();
  let cur = c.events[0][1];
  let phase = c.t0 * c.sides[cur].speed;
  const frames = [];
  let e = 1;
  for (let i = 0; i < c.dts.length; i++) {
    while (e < c.events.length && c.events[e][0] === i) {
      const [, name, duration, curve, authored] = c.events[e++];
      tr.start(duration, curve, !!authored);
      cur = name;
    }
    const dt = c.dts[i];
    tr.advance(dt);
    const to = c.sides[cur];
    phase += Math.min(dt, 0.1) * tr.speed(to, 64);
    const out = tr.frames(to, 64, phase);
    frames.push({ weights: tr.weights, speed: tr.speed(to, 64), phase, two: out.previous !== null, blend: out.blend, sums: tr.rateSums(to.state) });
  }
  return frames;
}

test("the transition clock replays its shared vectors", () => {
  const out = { about: "The transition clock's steps (packages/core/test/transition-timeline.test.mjs writes it; Swift and Kotlin replay it to 1e-9)", cases: CASES.map((c) => ({ ...c, frames: replay(c) })) };
  if (process.env.TIMELINE_WRITE === "1") writeFileSync(FILE, JSON.stringify(out, null, 1) + "\n");
  const want = JSON.parse(readFileSync(FILE, "utf8"));
  assert.equal(want.cases.length, out.cases.length);
  for (const [k, c] of out.cases.entries()) {
    const w = want.cases[k];
    assert.equal(w.name, c.name);
    for (const [i, f] of c.frames.entries()) {
      const g = w.frames[i];
      const close = (a, b) => Math.abs(a - b) < 1e-9;
      assert.ok(f.weights.length === g.weights.length && f.weights.every((x, j) => close(x, g.weights[j])), `${c.name} frame ${i}: weights`);
      assert.ok(close(f.speed, g.speed) && close(f.phase, g.phase) && f.two === g.two && close(f.blend, g.blend), `${c.name} frame ${i}`);
      assert.deepEqual(Object.keys(f.sums).sort(), Object.keys(g.sums).sort(), `${c.name} frame ${i}: sums`);
      for (const key of Object.keys(f.sums)) assert.ok(close(f.sums[key], g.sums[key]), `${c.name} frame ${i}: ${key}`);
    }
  }
});

test("the clock's rules: no overshoot at any dt, a long gap settles, a cut is immediate", () => {
  for (const c of CASES) for (const f of replay(c)) for (const x of f.weights) assert.ok(x >= -1e-12 && x <= 1 + 1e-12, c.name);
  const gap = replay(CASES.find((c) => c.name.startsWith("a 3 s gap")));
  assert.deepEqual(gap[10].weights, [1]);
  const cut = replay(CASES.find((c) => c.name === "a cut"));
  assert.deepEqual(cut[5].weights, [1]);
  const rates = replay(CASES.at(-1));
  assert.deepEqual(rates[9].sums, {}, "a resting view keeps the engine's own t × rate");
  assert.ok(Object.keys(rates[11].sums).length > 0, "from the first change the view keeps the sums");
});
