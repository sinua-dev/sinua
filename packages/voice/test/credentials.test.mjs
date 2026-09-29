// The credential contract (src/credential.ts) on every web source: one shape
// `{ credential, expiresAt?, url? }` from a provider or a `credentialUrl`, asked
// for again on every (re)connect. Fakes only: no device, no network, no sound.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { install, restore, mic, net, FakeWebSocket, until, tick } from "./fakes.mjs";

beforeEach(install);
afterEach(restore);

const json = (body, status = 200) => ({ ok: status < 400, status, text: async () => JSON.stringify(body), json: async () => body });
const answer = { ok: true, status: 200, text: async () => "v=0 fake-answer", json: async () => ({}) };

test("resolveCredential: string, provider (string or object), url; the shape is validated", async () => {
  const { resolveCredential, parseCredential } = await import("../dist/credential.js");
  assert.deepEqual(await resolveCredential("V", { credential: " ek_a " }), { credential: "ek_a" });
  assert.deepEqual(await resolveCredential("V", { credential: async () => "ek_b" }), { credential: "ek_b" });
  assert.deepEqual(await resolveCredential("V", { credential: async () => ({ credential: "ek_c", expiresAt: 5 }) }), {
    credential: "ek_c",
    expiresAt: 5,
  });
  net.respond = async () => json({ credential: "jwt", url: "wss://lk.example", expiresAt: 9 });
  assert.deepEqual(await resolveCredential("V", { credentialUrl: "/api/voice/livekit", credential: "ignored" }, { needsUrl: true }), {
    credential: "jwt",
    url: "wss://lk.example",
    expiresAt: 9,
  });
  const req = net.requests.at(-1);
  assert.equal(req.url, "/api/voice/livekit");
  assert.equal(req.init.method, "POST");
  assert.equal(req.init.cache, "no-store");

  for (const bad of [undefined, "", "  ", {}, { key: "ek_old" }, { credential: 1 }, { credential: "x", expiresAt: "soon" }]) {
    assert.throws(() => parseCredential("V", bad), (e) => e.fatal === true, JSON.stringify(bad));
  }
  await assert.rejects(resolveCredential("V", { credential: "jwt" }, { needsUrl: true }), /no `url`/);
});

test("credentialUrl: a 4xx or non-JSON is fatal, a 5xx or 429 is retryable, and the credential never leaks", async () => {
  const { resolveCredential } = await import("../dist/credential.js");
  const fatal = async (status) => {
    net.respond = async () => json({ error: "nope" }, status);
    return resolveCredential("V", { credentialUrl: "/c" }).catch((e) => e);
  };
  assert.equal((await fatal(401)).fatal, true);
  assert.equal((await fatal(404)).fatal, true);
  assert.equal((await fatal(500)).fatal, undefined, "5xx: retry");
  assert.equal((await fatal(429)).fatal, undefined, "429: retry");
  net.respond = async () => ({ ok: true, status: 200, text: async () => "<html>", json: async () => JSON.parse("<html>") });
  assert.match((await resolveCredential("V", { credentialUrl: "/c" }).catch((e) => e)).message, /did not return JSON/);
  net.respond = async () => json({ secret: "ek_SECRET" });
  const e = await resolveCredential("V", { credentialUrl: "/c" }).catch((err) => err);
  assert.doesNotMatch(e.message, /ek_SECRET/, "an error names keys, never values");
});

test("openai: credentialUrl is fetched before the mic, and the ek_ it returns opens the call", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  net.respond = async (url) => (String(url) === "/api/voice/openai" ? json({ credential: "ek_url", expiresAt: 1 }) : answer);
  const src = new OpenAIRealtimeVoiceSource({ credentialUrl: "/api/voice/openai", reconnect: false });
  await src.connect();
  try {
    assert.equal(net.requests[0].url, "/api/voice/openai");
    assert.equal(net.requests[1].init.headers.Authorization, "Bearer ek_url");
  } finally {
    src.disconnect();
  }
});

test("openai: voice/instructions are ignored now, with one warning", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const seen = [];
  const warn = console.warn;
  console.warn = (...a) => seen.push(a.join(" "));
  try {
    new OpenAIRealtimeVoiceSource({ credential: "ek_x", instructions: "Be brief.", voice: "marin" });
  } finally {
    console.warn = warn;
  }
  assert.equal(seen.length, 1);
  assert.match(seen[0], /mintOpenAIRealtimeCredential/);
});

test("gemini: every reconnect resumes with a NEW token from the provider", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  let n = 0;
  const src = new GeminiLiveVoiceSource({ credential: async () => ({ credential: `auth_tokens/t${++n}` }) });
  const quiet = console.warn;
  console.warn = () => {};
  try {
    const connecting = src.connect();
    await until(() => FakeWebSocket.instances.length === 1);
    const first = FakeWebSocket.instances[0];
    assert.match(first.url, /access_token=auth_tokens%2Ft1$/);
    first.open();
    first.receive({ setupComplete: {} });
    await connecting;
    first.receive({ sessionResumptionUpdate: { resumable: true, newHandle: "h1" } });

    first.close(1006); // a drop
    await until(() => FakeWebSocket.instances.length === 2);
    const second = FakeWebSocket.instances[1];
    assert.match(second.url, /access_token=auth_tokens%2Ft2$/, "a fresh token, not the spent one");
    second.open();
    assert.deepEqual(second.sent[0].setup.sessionResumption, { handle: "h1" }, "the session itself is resumed");
    second.receive({ setupComplete: {} });
    await tick(0); // the reconnect finishes on the next turn

    second.receive({ goAway: { timeLeft: "5s" } }); // and a goAway
    await until(() => FakeWebSocket.instances.length === 3);
    assert.match(FakeWebSocket.instances[2].url, /auth_tokens%2Ft3$/);
    assert.equal(n, 3);
    // Let the third setup finish, so no 15 s setup timer outlives the test.
    FakeWebSocket.instances[2].open();
    FakeWebSocket.instances[2].receive({ setupComplete: {} });
    await tick(0);
  } finally {
    src.disconnect();
    console.warn = quiet;
  }
});

test("gemini: a provider that starts returning a raw key stops the reconnect (fatal)", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  let n = 0;
  const src = new GeminiLiveVoiceSource({ credential: async () => (++n === 1 ? "auth_tokens/ok" : "AIza-raw") });
  const quiet = [console.warn, console.error];
  console.warn = console.error = () => {};
  try {
    const connecting = src.connect();
    await until(() => FakeWebSocket.instances.length === 1);
    FakeWebSocket.instances[0].open();
    FakeWebSocket.instances[0].receive({ setupComplete: {} });
    await connecting;
    FakeWebSocket.instances[0].close(1006);
    await tick(50);
    assert.equal(n, 2, "asked once more, then gave up at once");
    assert.equal(FakeWebSocket.instances.length, 1, "no socket for a refused credential");
  } finally {
    src.disconnect();
    [console.warn, console.error] = quiet;
  }
});

test("elevenlabs: each connect() signs a new URL; a public agent id still works", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  let n = 0;
  net.respond = async () => json({ credential: `wss://api.elevenlabs.io/v1/convai/conversation?agent_id=a&conversation_signature=s${++n}` });
  const src = new ElevenLabsVoiceSource({ credentialUrl: "/api/voice/elevenlabs" });
  for (const i of [1, 2]) {
    const connecting = src.connect();
    await until(() => FakeWebSocket.instances.length === i);
    assert.match(FakeWebSocket.instances[i - 1].url, new RegExp(`signature=s${i}$`));
    FakeWebSocket.instances[i - 1].close(1006);
    await assert.rejects(connecting);
  }
  assert.equal(n, 2);

  const pub = new ElevenLabsVoiceSource({ credential: "agent_public" });
  const connecting = pub.connect();
  await until(() => FakeWebSocket.instances.length === 3);
  assert.match(FakeWebSocket.instances[2].url, /\?agent_id=agent_public$/);
  FakeWebSocket.instances[2].close(1006);
  await assert.rejects(connecting);
});

test("a failing credentialUrl never touches the mic or opens a socket", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  net.respond = async () => json({ error: "unauthorized" }, 401);
  for (const src of [
    new ElevenLabsVoiceSource({ credentialUrl: "/c" }),
    new GeminiLiveVoiceSource({ credentialUrl: "/c" }),
    new OpenAIRealtimeVoiceSource({ credentialUrl: "/c" }),
  ]) {
    await assert.rejects(src.connect(), /returned 401/);
  }
  assert.equal(mic.requests, 0);
  assert.equal(FakeWebSocket.instances.length, 0);
});

test("livekit: a credential without `url` is refused before any audio or Room", async () => {
  const { LiveKitVoiceSource } = await import("../dist/livekit.js");
  const { audio } = await import("./fakes.mjs");
  net.respond = async () => json({ credential: "jwt-only" });
  const src = new LiveKitVoiceSource({ credentialUrl: "/api/voice/livekit" });
  await assert.rejects(src.connect(), /no `url`/);
  assert.equal(audio.contexts.length, 0);
});
