// The RN voice handle API (src/voice.ts) against a fake native module: handles,
// per-id events, the credential round trip (providers and credentialUrl, the shared
// `{ credential, expiresAt?, url? }` contract), release. No native, no audio, no network.
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
  setMuted: (id, muted) => calls.push(["setMuted", id, muted]),
  release: (id) => calls.push(["release", id]),
  provideCredential: (requestId, credential, url, error, fatal) =>
    calls.push(["provideCredential", requestId, credential, url, error, fatal]),
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
    [["provideCredential", "r1", "ek_first", null, null, false], ["provideCredential", "r2", "ek_second", null, null, false]],
    "a fresh key per request (an ek_ is single-use)",
  );
});

test("openai: a failing getCredential is reported back, not swallowed", async () => {
  const voice = createVoiceSource({ vendor: "openai", getCredential: async () => { throw new Error("backend 503"); } });
  emit({ id: voice.id, event: "credentialRequest", requestId: "r1" });
  await new Promise((r) => setImmediate(r));
  assert.deepEqual(calls.at(-1), ["provideCredential", "r1", null, null, "backend 503", false]);
});

const settle = () => new Promise((r) => setImmediate(r));

test("credentialUrl: POSTed per request in JS, parsed to the shared shape, url passed on (LiveKit)", async () => {
  const requests = [];
  let n = 0;
  globalThis.fetch = async (url, init) => {
    requests.push({ url, init });
    n += 1;
    return { ok: true, status: 200, json: async () => ({ credential: `jwt${n}`, url: "wss://lk.example", expiresAt: 9 }) };
  };
  const voice = createVoiceSource({ vendor: "livekit", credentialUrl: "https://app.example/api/voice/livekit" });
  const config = calls[0][1];
  assert.equal(config.hasCredentialProvider, true);
  assert.equal(config.credentialUrl, undefined, "the URL stays in JS");
  for (const r of ["r1", "r2"]) {
    emit({ id: voice.id, event: "credentialRequest", requestId: r });
    await settle();
  }
  assert.deepEqual(
    calls.filter((c) => c[0] === "provideCredential"),
    [["provideCredential", "r1", "jwt1", "wss://lk.example", null, false], ["provideCredential", "r2", "jwt2", "wss://lk.example", null, false]],
    "asked again on every request",
  );
  assert.equal(requests[0].init.method, "POST");
  assert.equal(requests[0].init.headers["Cache-Control"], "no-store");
});

test("credentialUrl: a 4xx or a bad shape is fatal, a 5xx is retryable, and no value is echoed", async () => {
  const run = async (res) => {
    globalThis.fetch = async () => res;
    const voice = createVoiceSource({ vendor: "gemini", credentialUrl: "/c" });
    emit({ id: voice.id, event: "credentialRequest", requestId: "r" });
    await settle();
    return calls.at(-1);
  };
  assert.equal((await run({ ok: false, status: 401 })).at(-1), true);
  assert.equal((await run({ ok: false, status: 503 })).at(-1), false);
  const bad = await run({ ok: true, status: 200, json: async () => ({ key: "ek_SECRET" }) });
  assert.equal(bad.at(-1), true);
  assert.doesNotMatch(bad[4], /SECRET/);
});

test("a credential provider (any vendor) resolves per request; a string stays a fixed value", async () => {
  let n = 0;
  const voice = createVoiceSource({ vendor: "elevenlabs", credential: async () => ({ credential: `wss://signed/${++n}` }) });
  assert.equal(calls[0][1].credential, undefined, "functions never cross the bridge");
  emit({ id: voice.id, event: "credentialRequest", requestId: "r1" });
  await settle();
  assert.deepEqual(calls.at(-1), ["provideCredential", "r1", "wss://signed/1", null, null, false]);

  createVoiceSource({ vendor: "elevenlabs", credential: "agent_public" });
  assert.equal(calls.at(-1)[1].credential, "agent_public");
  assert.equal(calls.at(-1)[1].hasCredentialProvider, false);
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

test("simulated: a sample or a script goes to the native side as data (no functions, the script as JSON)", () => {
  createVoiceSource({ vendor: "simulated", sample: "barge-in", loop: false });
  let c = calls.at(-1)[1];
  assert.equal(c.vendor, "simulated");
  assert.equal(c.sample, "barge-in");
  assert.equal(c.loop, false);
  assert.equal(c.hasCredentialProvider, false);
  createVoiceSource({ vendor: "simulated", script: { turns: [{ state: "idle", seconds: 1 }] } });
  c = calls.at(-1)[1];
  assert.equal(typeof c.script, "string");
  assert.deepEqual(JSON.parse(c.script), { turns: [{ state: "idle", seconds: 1 }] });
});

test("setMuted reaches the native side by id; mute and connection events reach the handle", async () => {
  const voice = createVoiceSource({ vendor: "simulated", sample: "calendar" });
  const conn = [];
  const mutes = [];
  voice.onConnectionChange((c) => conn.push(c));
  voice.onMuteChange((m) => mutes.push(m));
  voice.setMuted(true);
  assert.deepEqual(calls.at(-1), ["setMuted", voice.id, true]);
  emit({ id: voice.id, event: "mute", muted: true });
  emit({ id: voice.id, event: "connection", connected: true });
  emit({ id: voice.id, event: "connection", connected: false });
  emit({ id: "someone-else", event: "mute", muted: false });
  assert.equal(voice.muted, true);
  assert.deepEqual(mutes, [true]);
  assert.deepEqual(conn, [true, false]);
  voice.release();
  voice.setMuted(false);
  assert.notDeepEqual(calls.at(-1), ["setMuted", voice.id, false], "released: no native call");
});
