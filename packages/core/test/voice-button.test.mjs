// The voice button's state machine against spec/voice-button-cases.json (the
// table iOS and Android read too), the controller on a fake source, and the
// SharedVoiceSource fan-out it relies on. No audio anywhere.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  SharedVoiceSource,
  SimulatedVoiceSource,
  VOICE_BUTTON_INITIAL,
  VoiceButtonController,
  voiceButtonStep,
} from "../dist-dev/dev-entry.js";

const table = JSON.parse(readFileSync(new URL("../../../spec/voice-button-cases.json", import.meta.url), "utf8"));

function parseEvent(s) {
  const [type, arg] = s.split(/:(.*)/s);
  if (type === "connectFail") return { type, reason: arg };
  if (type === "muted") return { type: "muteChanged", muted: arg === "true" };
  return { type };
}

test("the state machine follows every case in the shared table", () => {
  assert.ok(table.cases.length >= 10);
  for (const c of table.cases) {
    let m = VOICE_BUTTON_INITIAL;
    for (const [ev, state, effects] of c.steps) {
      const out = voiceButtonStep(m, parseEvent(ev), { mode: c.mode, canMute: c.canMute });
      assert.equal(out.model.state, state, `${c.name}: after ${ev}`);
      assert.deepEqual(out.effects, effects, `${c.name}: effects of ${ev}`);
      m = out.model;
    }
  }
});

test("an error keeps its reason until the next attempt", () => {
  let m = voiceButtonStep(VOICE_BUTTON_INITIAL, { type: "press" }, { mode: "toggle", canMute: true }).model;
  m = voiceButtonStep(m, { type: "connectFail", reason: "no credential" }, { mode: "toggle", canMute: true }).model;
  assert.equal(m.reason, "no credential");
  m = voiceButtonStep(m, { type: "press" }, { mode: "toggle", canMute: true }).model;
  assert.equal(m.reason, null);
});

/** A source with one callback of each kind, like every real one. */
function fakeSource({ mute = true, connection = true, fail = null } = {}) {
  const s = {
    cbs: {},
    muted: [],
    connects: 0,
    disconnects: 0,
    onMetrics(cb) { this.cbs.metrics = cb; },
    onStateChange(cb) { this.cbs.state = cb; },
    onInterrupt(cb) { this.cbs.interrupt = cb; },
    async connect() {
      this.connects++;
      if (fail) throw new Error(fail);
      this.cbs.connection?.(true);
    },
    disconnect() {
      this.disconnects++;
      this.cbs.connection?.(false);
      this.cbs.state?.("idle");
    },
  };
  if (mute) s.setMuted = function (m) { this.muted.push(m); };
  if (connection) s.onConnectionChange = function (cb) { this.cbs.connection = cb; };
  return s;
}

const tick = () => new Promise((r) => setImmediate(r));

test("SharedVoiceSource fans one subscription out and hands back the same instance", () => {
  const src = fakeSource();
  const a = SharedVoiceSource.of(src);
  assert.equal(SharedVoiceSource.of(src), a);
  assert.equal(SharedVoiceSource.of(a), a);
  const seen = [];
  const off1 = a.onStateChange((s) => seen.push(["one", s]));
  a.onStateChange((s) => seen.push(["two", s]));
  src.cbs.state("listening");
  off1();
  src.cbs.state("speaking");
  assert.deepEqual(seen, [["one", "listening"], ["two", "listening"], ["two", "speaking"]]);
  assert.equal(a.state, "speaking");
});

test("tracked views each get their own easing and follow the mute", () => {
  const src = fakeSource();
  const shared = SharedVoiceSource.of(src);
  const orb = shared.track({ bandEaseRate: Infinity });
  const ring = shared.track();
  src.cbs.metrics({ level: 0.5, bands: [0.4, 0.4] });
  assert.equal(orb.overrides.metrics.level, 0.5);
  assert.equal(ring.overrides.metrics.level, 0.5);
  shared.setMuted(true);
  assert.deepEqual(src.muted, [true]);
  assert.equal(orb.overrides.overrides(0.016).muted, 1);
  ring.release();
  src.cbs.metrics({ level: 0.9, bands: [0.4, 0.4] });
  assert.equal(ring.overrides.metrics.level, 0.5, "released: no longer fed");
  assert.equal(orb.overrides.metrics.level, 0.9);
});

test("a source that can't mute stays unmuted", () => {
  const shared = SharedVoiceSource.of(fakeSource({ mute: false }));
  assert.equal(shared.canMute, false);
  shared.setMuted(true);
  assert.equal(shared.muted, false);
});

test("the controller connects, mutes through the source, and follows a remote drop", async () => {
  const src = fakeSource();
  const b = new VoiceButtonController(src);
  const states = [];
  b.onChange((m) => states.push(m.state));
  b.press();
  assert.equal(b.state, "connecting");
  await tick();
  assert.equal(b.state, "listening");
  b.press();
  assert.equal(b.state, "muted");
  // connect() unmutes first, then the press mutes.
  assert.deepEqual(src.muted, [false, true]);
  src.cbs.connection(false); // the agent hung up
  assert.equal(b.state, "ready");
  assert.deepEqual(states, ["connecting", "listening", "muted", "ready"]);
  b.destroy();
});

test("the controller: an agent that is idle while connected is not a drop", async () => {
  const src = fakeSource();
  const b = new VoiceButtonController(src);
  b.press();
  await tick();
  src.cbs.state("idle");
  assert.equal(b.state, "listening");
  b.end();
  assert.equal(b.state, "ready");
  assert.equal(src.disconnects, 1);
});

test("the controller without a connection signal treats idle as the end", async () => {
  const src = fakeSource({ connection: false });
  const b = new VoiceButtonController(src);
  b.press();
  await tick();
  src.cbs.state("idle");
  assert.equal(b.state, "ready");
});

test("the controller shows a failed connect's message", async () => {
  const b = new VoiceButtonController(fakeSource({ fail: "no credential" }));
  b.press();
  await tick();
  assert.equal(b.state, "error");
  assert.equal(b.reason, "no credential");
});

test("the controller ignores a connect that finishes after end()", async () => {
  let resolve;
  const src = fakeSource();
  src.connect = () => new Promise((r) => (resolve = r));
  const b = new VoiceButtonController(src);
  b.press();
  b.end();
  resolve();
  await tick();
  assert.equal(b.state, "ready");
});

test("a muted simulated conversation silences the user's turns, not the agent's", async () => {
  const v = new SimulatedVoiceSource("calendar", { autoTick: false, loop: false });
  let last = null;
  v.onMetrics((m) => (last = m));
  const conn = [];
  v.onConnectionChange((c) => conn.push(c));
  await v.connect();
  const user = v.turns.find((t) => t.voice === "user");
  const agent = v.turns.find((t) => t.voice === "agent");
  v.seek(user.start + user.seconds / 2);
  assert.ok(last.level > 0);
  v.setMuted(true);
  assert.equal(last.level, 0);
  assert.ok(last.bands.every((b) => b === 0));
  v.seek(agent.start + agent.seconds / 2);
  assert.ok(last.level > 0, "the agent still talks");
  v.setMuted(false);
  v.seek(user.start + user.seconds / 2);
  assert.ok(last.level > 0);
  v.disconnect();
  assert.deepEqual(conn, [true, false]);
});
