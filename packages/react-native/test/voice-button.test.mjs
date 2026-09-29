// The RN voice button logic (src/voiceButton.ts) against spec/voice-button-cases.json --
// the table Web, iOS and Android read too -- and the controller on a fake handle.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const { voiceButtonStep, VOICE_BUTTON_INITIAL, VoiceButtonController } = await import("../src/voiceButton.ts");
const table = JSON.parse(readFileSync(new URL("../../../spec/voice-button-cases.json", import.meta.url), "utf8"));

function parseEvent(s) {
  const [type, arg] = s.split(/:(.*)/s);
  if (type === "connectFail") return { type, reason: arg };
  if (type === "muted") return { type: "muteChanged", muted: arg === "true" };
  return { type };
}

test("the RN state machine follows every case in the shared table", () => {
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

/** A handle whose native events the test fires. */
function fakeHandle({ reject = null } = {}) {
  const subs = { connection: new Set(), error: new Set(), mute: new Set() };
  const on = (k) => (cb) => {
    subs[k].add(cb);
    return () => subs[k].delete(cb);
  };
  return {
    id: "h",
    calls: [],
    muted: false,
    fire: (k, v) => [...subs[k]].forEach((cb) => cb(v)),
    connect() {
      this.calls.push("connect");
      return reject ? Promise.reject(new Error(reject)) : Promise.resolve();
    },
    disconnect() {
      this.calls.push("disconnect");
    },
    setMuted(m) {
      this.calls.push(m ? "mute" : "unmute");
    },
    onConnectionChange: on("connection"),
    onError: on("error"),
    onMuteChange: on("mute"),
  };
}

test("the controller: up on the connection event, not on connect() resolving (Android opens async)", async () => {
  const h = fakeHandle();
  const c = new VoiceButtonController(h);
  c.press();
  await Promise.resolve();
  assert.equal(c.state, "connecting");
  h.fire("connection", true);
  assert.equal(c.state, "listening");
  c.press();
  assert.equal(c.state, "muted");
  assert.deepEqual(h.calls, ["unmute", "connect", "mute"]);
  h.fire("connection", false);
  assert.equal(c.state, "ready");
});

test("the controller: an error event while connecting, or a rejected connect(), shows the reason", async () => {
  const h = fakeHandle();
  const c = new VoiceButtonController(h);
  c.press();
  h.fire("error", "Gemini Live setup did not complete within 15s");
  assert.equal(c.state, "error");
  assert.equal(c.reason, "Gemini Live setup did not complete within 15s");
  const r = new VoiceButtonController(fakeHandle({ reject: "no credential" }));
  r.press();
  await new Promise((res) => setImmediate(res));
  assert.equal(r.state, "error");
  assert.equal(r.reason, "no credential");
});

test("the view's accessibility props cross to the native component", async () => {
  const { a11yNativeProps } = await import("../src/voice.ts");
  assert.deepEqual(a11yNativeProps(undefined, undefined), { labelsJson: undefined, announce: "auto" });
  assert.deepEqual(a11yNativeProps({ listening: "Koç dinliyor" }, false), { labelsJson: '{"listening":"Koç dinliyor"}', announce: "off" });
  assert.equal(a11yNativeProps(undefined, true).announce, "on");
});
