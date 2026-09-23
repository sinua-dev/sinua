// @sinua/voice sources on fakes (test/fakes.mjs): the protocol and state
// rules that moved out of the Studio. No device, no network, no sound.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { install, restore, audio, mic, net, FakeWebSocket, FakeRTCPeerConnection, FakeAudioWorkletNode, until, tick } from "./fakes.mjs";

beforeEach(install);
afterEach(restore);

function watch(src) {
  const states = [];
  const metrics = [];
  let interrupts = 0;
  src.onStateChange((s) => states.push(s));
  src.onMetrics((m) => metrics.push(m));
  src.onInterrupt?.(() => interrupts++);
  return { states, metrics, interrupts: () => interrupts };
}
const pcmB64 = (n = 1600) => Buffer.from(new Int16Array(n).fill(8000).buffer).toString("base64");

test("tone: listening + metrics from its analyser, idle and closed after disconnect", async () => {
  const { TestToneVoiceSource } = await import("../dist/tone.js");
  const src = new TestToneVoiceSource();
  const w = watch(src);
  audio.byte = 200;
  await src.connect();
  try {
    await until(() => w.metrics.length > 2);
    assert.deepEqual(w.states.slice(0, 2), ["initializing", "listening"]);
    assert.ok(w.metrics.at(-1).level > 0);
    assert.equal(w.metrics.at(-1).bands.length, 16);
    assert.equal(w.states.at(-1), "speaking", "a loud tone reads as speaking");
  } finally {
    src.disconnect();
  }
  assert.equal(w.states.at(-1), "idle");
  assert.equal(audio.contexts[0].state, "closed");
});

test("mic: asks for the mic once, stops its tracks on disconnect", async () => {
  const { LocalMicVoiceSource } = await import("../dist/mic.js");
  const src = new LocalMicVoiceSource();
  const w = watch(src);
  audio.byte = 120;
  await src.connect();
  try {
    assert.equal(mic.requests, 1);
    await until(() => w.metrics.length > 0);
  } finally {
    src.disconnect();
  }
  assert.equal(mic.stopped, 1);
  assert.deepEqual(w.states.slice(0, 2), ["initializing", "listening"]);
  assert.equal(w.states.at(-1), "idle");
});

test("elevenlabs: init message, metadata starts audio, audio -> thinking -> speaking, interruption, mic chunks", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const src = new ElevenLabsVoiceSource({ credential: " agent_123 ", overrides: { agent: { first_message: "hi" } } });
  const w = watch(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  assert.equal(ws.url, "wss://api.elevenlabs.io/v1/convai/conversation?agent_id=agent_123");
  assert.deepEqual(ws.protocols, ["convai"]);
  ws.open();
  assert.deepEqual(ws.sent[0], { type: "conversation_initiation_client_data", conversation_config_override: { agent: { first_message: "hi" } } });
  ws.receive({ type: "conversation_initiation_metadata", conversation_initiation_metadata_event: { agent_output_audio_format: "pcm_16000", user_input_audio_format: "pcm_16000" } });
  await connecting;
  assert.equal(w.states.at(-1), "listening");
  assert.equal(audio.contexts.length, 2, "capture + playback contexts");

  // Mic capture goes out as base64 PCM16.
  FakeAudioWorkletNode.last.port.onmessage({ data: new Float32Array(1024).fill(0.25) });
  assert.ok(typeof ws.sent.at(-1).user_audio_chunk === "string");

  ws.receive({ type: "ping", ping_event: { event_id: 7 } });
  assert.deepEqual(ws.sent.at(-1), { type: "pong", event_id: 7 });

  audio.time = 1;
  ws.receive({ type: "audio", audio_event: { audio_base_64: pcmB64(), event_id: 1 } });
  assert.equal(w.states.at(-1), "thinking");
  audio.time = 1.15; // past the 0.1 s lead: audible
  await until(() => w.states.at(-1) === "speaking");

  ws.receive({ type: "interruption", interruption_event: { event_id: 2 } });
  assert.equal(w.states.at(-1), "listening");
  assert.equal(w.interrupts(), 1);
  // Audio from before the interruption is dropped.
  ws.receive({ type: "audio", audio_event: { audio_base_64: pcmB64(), event_id: 1 } });
  assert.equal(w.states.at(-1), "listening");

  src.disconnect();
  assert.equal(w.states.at(-1), "idle");
  assert.equal(ws.readyState, 3);
});

test("elevenlabs: a close during setup rejects and returns to idle", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const src = new ElevenLabsVoiceSource({ credential: "wss://relay.example/convai?token=x" });
  const w = watch(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  assert.equal(FakeWebSocket.instances[0].url, "wss://relay.example/convai?token=x", "a signed URL is used as is");
  FakeWebSocket.instances[0].close(1008, "bad agent");
  await assert.rejects(connecting, /closed during setup \(code 1008: bad agent\)/);
  assert.equal(w.states.at(-1), "idle");
});

test("elevenlabs: a non-PCM input format is refused before streaming", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const src = new ElevenLabsVoiceSource({ credential: "agent" });
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  ws.open();
  ws.receive({ type: "conversation_initiation_metadata", conversation_initiation_metadata_event: { agent_output_audio_format: "pcm_16000", user_input_audio_format: "ulaw_8000" } });
  await assert.rejects(connecting, /PCM only/);
});

test("gemini: token URL + setup, setupComplete -> listening, model audio -> speaking, interrupted", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const src = new GeminiLiveVoiceSource({ credential: "auth_tokens/abc", model: "gemini-live-test", instructions: "Be brief." });
  const w = watch(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  assert.match(ws.url, /BidiGenerateContentConstrained\?access_token=auth_tokens%2Fabc$/);
  ws.open();
  assert.equal(ws.sent[0].setup.model, "models/gemini-live-test");
  assert.deepEqual(ws.sent[0].setup.systemInstruction, { parts: [{ text: "Be brief." }] });
  // Nothing is streamed before setupComplete.
  FakeAudioWorkletNode.last.port.onmessage({ data: new Float32Array(1024) });
  assert.equal(ws.sent.length, 1);
  ws.receive({ setupComplete: {} });
  await connecting;
  assert.equal(w.states.at(-1), "listening");
  FakeAudioWorkletNode.last.port.onmessage({ data: new Float32Array(1024) });
  assert.equal(ws.sent.at(-1).realtimeInput.audio.mimeType, "audio/pcm;rate=16000");

  audio.time = 2;
  ws.receive({ serverContent: { modelTurn: { parts: [{ inlineData: { data: pcmB64(2400), mimeType: "audio/pcm;rate=24000" } }] } } });
  assert.equal(w.states.at(-1), "thinking");
  audio.time = 2.15;
  await until(() => w.states.at(-1) === "speaking");
  ws.receive({ serverContent: { interrupted: true } });
  assert.equal(w.states.at(-1), "listening");
  assert.equal(w.interrupts(), 1);
  src.disconnect();
  assert.equal(w.states.at(-1), "idle");
});

test("gemini: an API key uses the key endpoint (opt-in path)", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const src = new GeminiLiveVoiceSource({ credential: "AIza-test", allowInsecureApiKey: true });
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  assert.match(FakeWebSocket.instances[0].url, /BidiGenerateContent\?key=AIza-test$/);
  FakeWebSocket.instances[0].close(1006);
  await assert.rejects(connecting, /closed during setup/);
});

test("openai: ek_ key posts the SDP offer, data channel events drive the state", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ credential: "ek_test", reconnect: false });
  const w = watch(src);
  await src.connect();
  assert.equal(w.states.at(-1), "listening");
  assert.equal(net.requests.length, 1, "no client_secrets mint for an ek_ key");
  const call = net.requests[0];
  assert.match(call.url, /^https:\/\/api\.openai\.com\/v1\/realtime\/calls\?model=/);
  assert.equal(call.init.headers.Authorization, "Bearer ek_test");
  assert.equal(call.init.headers["Content-Type"], "application/sdp");
  assert.equal(call.init.body, "v=0 fake-offer");
  const pc = FakeRTCPeerConnection.last;
  assert.deepEqual(pc.remote, { type: "answer", sdp: "v=0 fake-answer" });
  assert.equal(pc.dc.label, "oai-events");
  const send = (type) => pc.dc.onmessage({ data: JSON.stringify({ type }) });
  send("response.created");
  assert.equal(w.states.at(-1), "thinking");
  send("output_audio_buffer.started");
  assert.equal(w.states.at(-1), "speaking");
  send("input_audio_buffer.speech_started");
  assert.equal(w.states.at(-1), "listening");
  assert.equal(w.interrupts(), 1, "speech over audible output is a barge-in");
  send("input_audio_buffer.speech_stopped");
  send("response.done");
  assert.equal(w.states.at(-1), "listening", "a response with no audio doesn't hang in thinking");
  src.disconnect();
  assert.equal(w.states.at(-1), "idle");
  assert.equal(mic.stopped >= 1, true);
});

test("openai: a 401 on the call is fatal and returns to idle", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  net.respond = async () => ({ ok: false, status: 401, text: async () => "invalid key", json: async () => ({}) });
  const src = new OpenAIRealtimeVoiceSource({ credential: "ek_bad" });
  const w = watch(src);
  await assert.rejects(src.connect(), /returned 401: invalid key/);
  assert.equal(w.states.at(-1), "idle");
});

test("livekit: parseUrlAndToken", async () => {
  const { parseUrlAndToken } = await import("../dist/livekitAgent.js");
  assert.deepEqual(parseUrlAndToken(" wss://x.livekit.cloud  eyJtoken "), { url: "wss://x.livekit.cloud", token: "eyJtoken" });
  assert.deepEqual(parseUrlAndToken("eyJtoken wss://x"), { url: "wss://x", token: "eyJtoken" });
  assert.deepEqual(parseUrlAndToken(""), { url: "", token: "" });
});

void tick;

// --- ElevenLabs turn end: agent_response_complete / is_final (Agents WebSocket reference) ---

async function elevenLabsConnected() {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const src = new ElevenLabsVoiceSource({ credential: "agent" });
  const w = watch(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  ws.open();
  ws.receive({ type: "conversation_initiation_metadata", conversation_initiation_metadata_event: { agent_output_audio_format: "pcm_16000", user_input_audio_format: "pcm_16000" } });
  await connecting;
  return { src, w, ws };
}
const chunk = (id, is_final) => ({ type: "audio", audio_event: { audio_base_64: pcmB64(), event_id: id, ...(is_final === undefined ? {} : { is_final }) } });
const complete = { type: "agent_response_complete", agent_response_complete_event: { event_id: 9 } };

test("elevenlabs: drained before agent_response_complete is a stall (thinking), not the end of the turn", async () => {
  const { src, w, ws } = await elevenLabsConnected();
  try {
    audio.time = 1;
    ws.receive(chunk(1));
    audio.time = 1.15; // audible
    await until(() => w.states.at(-1) === "speaking");
    audio.time = 5; // everything played, reply not complete
    await until(() => w.states.at(-1) === "thinking");
    ws.receive(chunk(1));
    audio.time = 5.15;
    await until(() => w.states.at(-1) === "speaking");
    ws.receive(complete);
    audio.time = 9;
    await until(() => w.states.at(-1) === "listening");
  } finally {
    src.disconnect();
  }
});

test("elevenlabs: an is_final chunk ends the reply like complete; trailing chunks don't reopen it", async () => {
  const { src, w, ws } = await elevenLabsConnected();
  try {
    audio.time = 1;
    ws.receive(chunk(1, false));
    ws.receive(chunk(1, true));
    ws.receive(chunk(1)); // a trailing chunk after the final one
    audio.time = 1.15;
    await until(() => w.states.at(-1) === "speaking");
    audio.time = 9;
    await until(() => w.states.at(-1) === "listening");
  } finally {
    src.disconnect();
  }
});

test("elevenlabs: a reply without audio ends thinking at agent_response_complete", async () => {
  const { src, w, ws } = await elevenLabsConnected();
  try {
    ws.receive({ type: "user_transcript", user_transcription_event: { user_transcript: "hi" } });
    ws.receive({ type: "agent_response", agent_response_event: { agent_response: "Hello.", event_id: 9 } });
    assert.equal(w.states.at(-1), "thinking");
    ws.receive(complete);
    assert.equal(w.states.at(-1), "listening", "no 4 s wait");
  } finally {
    src.disconnect();
  }
});

test("elevenlabs: an open reply with no audio for 10 s gives up to listening", async () => {
  const realNow = Date.now;
  let offset = 0;
  Date.now = () => realNow() + offset;
  const { src, w, ws } = await elevenLabsConnected();
  try {
    audio.time = 1;
    ws.receive(chunk(1));
    audio.time = 1.15;
    await until(() => w.states.at(-1) === "speaking");
    audio.time = 5;
    await until(() => w.states.at(-1) === "thinking");
    offset = 5_000;
    await tick(80);
    assert.equal(w.states.at(-1), "thinking", "the 4 s flicker guard doesn't apply mid-reply");
    offset = 10_500;
    await until(() => w.states.at(-1) === "listening");
  } finally {
    Date.now = realNow;
    src.disconnect();
  }
});

// ---- Raw API keys: refused unless the caller opts in ----------------------
// The rule lives in src/insecureCredential.ts. What matters here is not only
// that connect() rejects, but that it rejects *before* the microphone, the
// audio graph or the socket -- a refused credential must not open a device.

function captureWarnings() {
  const seen = [];
  const original = console.warn;
  console.warn = (...args) => seen.push(args.join(" "));
  return { seen, restore: () => { console.warn = original; } };
}

test("gemini: a raw API key is refused before the mic or the socket", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const src = new GeminiLiveVoiceSource({ credential: "AIza-test" });
  const w = watch(src);
  await assert.rejects(src.connect(), /refusing a raw, long-lived API key/);
  assert.match(
    (await src.connect().catch((e) => e.message)),
    /auth_tokens\/….*v1beta\/auth_tokens.*allowInsecureApiKey/s,
    "the message says what to pass instead and how to opt in",
  );
  assert.equal(mic.requests, 0, "no microphone was requested");
  assert.equal(audio.contexts.length, 0, "no AudioContext was created");
  assert.equal(FakeWebSocket.instances.length, 0, "no socket was opened");
  assert.deepEqual(w.states, [], "a refusal never enters initializing");
});

test("gemini: an auth_tokens/ credential is unaffected by the gate", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const cap = captureWarnings();
  try {
    const src = new GeminiLiveVoiceSource({ credential: "auth_tokens/abc" });
    const connecting = src.connect();
    await until(() => FakeWebSocket.instances.length === 1);
    assert.match(FakeWebSocket.instances[0].url, /BidiGenerateContentConstrained\?access_token=/);
    assert.deepEqual(cap.seen, [], "an ephemeral token warns about nothing");
    FakeWebSocket.instances[0].close(1006);
    await assert.rejects(connecting);
  } finally {
    cap.restore();
  }
});

test("gemini: the opt-in connects and warns exactly once", async () => {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const cap = captureWarnings();
  try {
    const src = new GeminiLiveVoiceSource({ credential: "AIza-test", allowInsecureApiKey: true });
    const connecting = src.connect();
    await until(() => FakeWebSocket.instances.length === 1);
    const warnings = cap.seen.filter((m) => m.includes("raw, long-lived API key"));
    assert.equal(warnings.length, 1, "one warning per connect");
    assert.match(warnings[0], /local demos, not for shipping/);
    FakeWebSocket.instances[0].close(1006);
    await assert.rejects(connecting);
  } finally {
    cap.restore();
  }
});

test("openai: a raw API key is refused before the mic, and is fatal", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ credential: "sk-test" });
  const w = watch(src);
  await assert.rejects(src.connect(), /refusing a raw, long-lived API key/);
  assert.equal(net.requests.length, 0, "no client_secrets mint was attempted");
  assert.equal(mic.requests, 0, "no microphone was requested");
  assert.equal(w.states.at(-1), "idle", "the refusal ends in idle, not a half-open session");
  assert.ok(!w.states.includes("listening"));
});

test("openai: getCredential returning a raw key is refused too", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ getCredential: async () => "sk-from-backend" });
  await assert.rejects(src.connect(), /refusing a raw, long-lived API key/);
  assert.equal(net.requests.length, 0);
  assert.equal(mic.requests, 0);
});

test("openai: the opt-in mints an ek_ from the raw key, as before", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const cap = captureWarnings();
  try {
    net.respond = async (url) =>
      String(url).includes("client_secrets")
        ? { ok: true, status: 200, text: async () => "", json: async () => ({ value: "ek_minted" }) }
        : { ok: true, status: 200, text: async () => "v=0 fake-answer", json: async () => ({}) };
    const src = new OpenAIRealtimeVoiceSource({ credential: "sk-test", allowInsecureApiKey: true, reconnect: false });
    await src.connect();
    try {
      assert.match(net.requests[0].url, /client_secrets$/, "the dev mint still runs");
      assert.equal(net.requests[1].init.headers.Authorization, "Bearer ek_minted");
      assert.equal(cap.seen.filter((m) => m.includes("raw, long-lived API key")).length, 1);
    } finally {
      src.disconnect();
    }
  } finally {
    cap.restore();
  }
});
