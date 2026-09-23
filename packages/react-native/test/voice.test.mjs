// The RN voice handle API (src/voice.ts) against a fake native module: handles,
// per-id events, the OpenAI credential round trip, release. No native, no audio.
import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { emit, setNativeModule } from "./react-native-stub.mjs";

const { createVoiceSource, isVoiceSourceHandle } = await import("../src/voice.ts");

let calls;
const fakeNative = {
  create: async (config) => {
    calls.push(["create", config]);
    return config.id;
  },
  connect: async (id) => void calls.push(["connect", id]),
  disconnect: (id) => calls.push(["disconnect", id]),
  release: (id) => calls.push(["release", id]),
  provideCredential: (requestId, credential, error) => calls.push(["provideCredential", requestId, credential, error]),
};

beforeEach(() => {
  calls = [];
  setNativeModule(fakeNative);
});

test("create passes the config natively once; connect/disconnect/release address the id", async () => {
  const voice = createVoiceSource({ vendor: "gemini", credential: "auth_tokens/x", model: "m" });
  assert.ok(isVoiceSourceHandle(voice));
  assert.equal(voice.vendor, "gemini");
  await voice.connect();
  voice.disconnect();
  voice.release();
  assert.deepEqual(calls.map((c) => c[0]), ["create", "connect", "disconnect", "release"]);
  const config = calls[0][1];
  assert.equal(config.credential, "auth_tokens/x");
  assert.equal(config.vendor, "gemini");
  assert.equal(config.hasCredentialProvider, false);
  assert.equal(config.id, voice.id);
  assert.equal(calls[1][1], voice.id);
});

test("events reach only their own handle, and stop after release / unsubscribe", async () => {
  const a = createVoiceSource({ vendor: "test" });
  const b = createVoiceSource({ vendor: "mic" });
  const seen = { a: [], b: [], errors: [], interrupts: 0 };
  a.onStateChange((s) => seen.a.push(s));
  const off = b.onStateChange((s) => seen.b.push(s));
  a.onError((m) => seen.errors.push(m));
  a.onInterrupt(() => (seen.interrupts += 1));

  emit({ id: a.id, event: "state", state: "listening" });
  emit({ id: b.id, event: "state", state: "speaking" });
  emit({ id: a.id, event: "error", message: "socket closed" });
  emit({ id: a.id, event: "interrupt" });
  assert.deepEqual(seen.a, ["listening"]);
  assert.deepEqual(seen.b, ["speaking"]);
  assert.deepEqual(seen.errors, ["socket closed"]);
  assert.equal(seen.interrupts, 1);

  off();
  emit({ id: b.id, event: "state", state: "idle" });
  assert.deepEqual(seen.b, ["speaking"], "unsubscribed");

  a.release();
  emit({ id: a.id, event: "state", state: "idle" });
  assert.deepEqual(seen.a, ["listening"], "a released handle gets nothing");
  await assert.rejects(a.connect(), /released/);
});

test("openai: the native side asks for a credential and gets the fresh one", async () => {
  const keys = ["ek_first", "ek_second"];
  const voice = createVoiceSource({ vendor: "openai", getCredential: async () => keys.shift() });
  assert.equal(calls[0][1].hasCredentialProvider, true);
  assert.equal(calls[0][1].getCredential, undefined, "functions never cross the bridge");
  emit({ id: voice.id, event: "credentialRequest", requestId: "r1" });
  await new Promise((r) => setImmediate(r));
  emit({ id: voice.id, event: "credentialRequest", requestId: "r2" });
  await new Promise((r) => setImmediate(r));
  assert.deepEqual(
    calls.filter((c) => c[0] === "provideCredential"),
    [["provideCredential", "r1", "ek_first", null], ["provideCredential", "r2", "ek_second", null]],
    "a fresh key per request (an ek_ is single-use)",
  );
});

test("openai: a failing getCredential is reported back, not swallowed", async () => {
  const voice = createVoiceSource({ vendor: "openai", getCredential: async () => { throw new Error("backend 503"); } });
  emit({ id: voice.id, event: "credentialRequest", requestId: "r1" });
  await new Promise((r) => setImmediate(r));
  assert.deepEqual(calls.at(-1), ["provideCredential", "r1", null, "backend 503"]);
});

test("without the native module, using a source explains how to fix it", async () => {
  setNativeModule(undefined);
  assert.throws(() => createVoiceSource({ vendor: "test" }), /isn't linked/);
});

test("voice prop mapping: a handle binds by id, the shorthands stay strings", async () => {
  const { voiceProps } = await import("../src/voice.ts");
  const handle = createVoiceSource({ vendor: "livekit", url: "wss://x", token: "t" });
  assert.deepEqual(voiceProps(handle), { voice: "none", voiceSourceId: handle.id });
  assert.deepEqual(voiceProps("test"), { voice: "test", voiceSourceId: undefined });
  assert.deepEqual(voiceProps(undefined), { voice: undefined, voiceSourceId: undefined });
});
