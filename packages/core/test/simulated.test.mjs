// SimulatedVoiceSource: a scripted conversation as a VoiceSource, driven by the
// engine (conversationAt). No timers (autoTick: false), no audio, no mic.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { SimulatedVoiceSource, conversationAt, conversationSample, conversationSampleNames } from "../dist/index.js";

const vectors = JSON.parse(readFileSync(fileURLToPath(new URL("../../../spec/conversation-vectors.json", import.meta.url)), "utf8"));

test("the engine's conversations match spec/conversation-vectors.json (the four platforms' shared file)", () => {
  assert.deepEqual(conversationSampleNames(), ["calendar", "quick-answer", "long-answer", "barge-in"]);
  for (const c of vectors.cases) {
    const f = conversationAt(conversationSample(c.sample), c.t, vectors.bands);
    assert.equal(f.state, c.state, `${c.sample} @${c.t}`);
    assert.equal(f.turn, c.turn);
    assert.equal(f.shown, c.shown);
    assert.ok(Math.abs(f.level - c.level) < vectors.tolerance);
    f.bands.forEach((b, i) => assert.ok(Math.abs(b - c.bands[i]) < vectors.tolerance));
  }
});

function watch(src) {
  const w = { states: [], metrics: [], interrupts: 0, frames: [] };
  src.onStateChange((s) => w.states.push(s));
  src.onMetrics((m) => w.metrics.push(m));
  src.onInterrupt(() => w.interrupts++);
  src.onFrame((f) => w.frames.push(f));
  return w;
}

test("plays the script: states in order, metrics are the engine's, one barge-in flash", async () => {
  const src = new SimulatedVoiceSource("barge-in", { autoTick: false, bands: 8 });
  const w = watch(src);
  await src.connect();
  const script = conversationSample("barge-in");
  for (let i = 0; i < Math.round(src.duration * 30); i++) src.advance(1 / 30);
  assert.deepEqual(
    [...new Set(w.states)].sort(),
    ["idle", "listening", "speaking", "thinking"],
  );
  assert.deepEqual(w.states.slice(0, 5), ["idle", "listening", "thinking", "speaking", "listening"]);
  assert.equal(w.interrupts, 1, "the barge-in turn flashes once");
  const f = conversationAt(script, src.time, 8);
  assert.deepEqual(w.metrics.at(-1), { level: f.level, bands: f.bands });
  assert.equal(src.duration, conversationAt(script, 0, 1).total);
  src.disconnect();
  assert.equal(w.states.at(-1), "idle");
  assert.equal(w.metrics.at(-1).level, 0);
});

test("loop wraps, loop: false holds the end, pause stops time, seek jumps without a flash", async () => {
  const src = new SimulatedVoiceSource("quick-answer", { autoTick: false });
  await src.connect();
  src.advance(src.duration + 0.5);
  assert.ok(Math.abs(src.time - 0.5) < 1e-9, "the sample loops");
  const once = new SimulatedVoiceSource("quick-answer", { autoTick: false, loop: false });
  await once.connect();
  once.advance(99);
  assert.equal(once.time, once.duration);
  src.pause();
  src.advance(1);
  assert.ok(Math.abs(src.time - 0.5) < 1e-9, "paused");
  src.play();
  const b = new SimulatedVoiceSource("barge-in", { autoTick: false });
  const wb = watch(b);
  await b.connect();
  const bargeTurn = b.turns.find((t) => t.bargeIn);
  b.seek(bargeTurn.start + 0.1);
  assert.equal(wb.interrupts, 0, "seeking into a barge-in doesn't flash");
  assert.equal(wb.states.at(-1), "listening");
});

test("scripts: an object or JSON works, a bad one throws with its path, and no audio is touched", async () => {
  for (const k of ["AudioContext", "navigator"]) assert.equal(globalThis[k]?.mediaDevices, undefined);
  const src = new SimulatedVoiceSource({ turns: [{ state: "idle", seconds: 1 }, { state: "speaking", seconds: 2, voice: "agent" }] }, { autoTick: false });
  assert.equal(src.duration, 3);
  assert.equal(src.turns[1].start, 1);
  assert.throws(() => new SimulatedVoiceSource({ turns: [{ state: "idle", seconds: 0 }] }), /\/turns\/0\/seconds/);
  assert.throws(() => new SimulatedVoiceSource("not-a-sample"), /SimulatedVoiceSource/);
});
