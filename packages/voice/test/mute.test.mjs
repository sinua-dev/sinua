// setMuted and onConnectionChange on every @sinua/voice source (docs/audio-pipeline.md,
// *Mute*): silence goes out, the session stays up, and the connection signal says
// when a session starts and ends. Fakes only (test/fakes.mjs): no device, no network.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { install, restore, audio, FakeWebSocket, FakeRTCPeerConnection, FakeAudioWorkletNode, until } from "./fakes.mjs";

beforeEach(install);
afterEach(restore);

function connection(src) {
  const seen = [];
  src.onConnectionChange((c) => seen.push(c));
  return seen;
}
const pcm = (b64) => {
  const b = Buffer.from(b64, "base64");
  return new Int16Array(b.buffer, b.byteOffset, b.length / 2);
};

test("mic: muted, the track is disabled and the level reads 0; no watchdog re-attach", async () => {
  const { LocalMicVoiceSource } = await import("../dist/mic.js");
  const src = new LocalMicVoiceSource();
  const metrics = [];
  src.onMetrics((m) => metrics.push(m));
  const conn = connection(src);
  audio.byte = 120;
  await src.connect();
  try {
    await until(() => metrics.length > 0 && metrics.at(-1).level > 0);
    const contexts = audio.contexts.length;
    src.setMuted(true);
    const n = metrics.length;
    await until(() => metrics.length > n + 40, 3000); // past the ~1 s watchdog window
    assert.equal(metrics.at(-1).level, 0);
    assert.ok(metrics.at(-1).bands.every((b) => b === 0));
    assert.equal(audio.contexts.length, contexts);
    src.setMuted(false);
    await until(() => metrics.at(-1).level > 0);
  } finally {
    src.disconnect();
  }
  assert.deepEqual(conn, [true, false]);
});

test("tone: muted reads 0 like a muted mic", async () => {
  const { TestToneVoiceSource } = await import("../dist/tone.js");
  const src = new TestToneVoiceSource();
  const metrics = [];
  src.onMetrics((m) => metrics.push(m));
  const conn = connection(src);
  audio.byte = 200;
  await src.connect();
  try {
    await until(() => metrics.length > 0 && metrics.at(-1).level > 0);
    src.setMuted(true);
    const n = metrics.length;
    await until(() => metrics.length > n + 1);
    assert.equal(metrics.at(-1).level, 0);
  } finally {
    src.disconnect();
  }
  assert.deepEqual(conn, [true, false]);
});

test("openai: muted disables the mic track the call sends; the call stays up", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ credential: "ek_test", reconnect: false });
  const conn = connection(src);
  src.setMuted(true); // before connect: applied to the mic once it's acquired
  await src.connect();
  const track = FakeRTCPeerConnection.last.tracks[0];
  assert.equal(track.enabled, false);
  src.setMuted(false);
  assert.equal(track.enabled, true);
  src.setMuted(true);
  assert.equal(track.enabled, false);
  assert.equal(FakeRTCPeerConnection.last.connectionState, "new", "not closed");
  src.disconnect();
  assert.deepEqual(conn, [true, false]);
});

async function geminiUp() {
  const { GeminiLiveVoiceSource } = await import("../dist/gemini.js");
  const src = new GeminiLiveVoiceSource({ credential: "auth_tokens/abc" });
  const conn = connection(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  ws.open();
  ws.receive({ setupComplete: {} });
  await connecting;
  return { src, ws, conn };
}

test("gemini: muted sends zeroed PCM (same length, same rate) and disables the track", async () => {
  const { src, ws, conn } = await geminiUp();
  const chunk = () => FakeAudioWorkletNode.last.port.onmessage({ data: new Float32Array(1024).fill(0.25) });
  chunk();
  assert.ok(pcm(ws.sent.at(-1).realtimeInput.audio.data).some((v) => v !== 0));
  src.setMuted(true);
  chunk();
  const muted = pcm(ws.sent.at(-1).realtimeInput.audio.data);
  assert.equal(muted.length, 1024);
  assert.ok(muted.every((v) => v === 0), "silence goes out");
  src.setMuted(false);
  chunk();
  assert.ok(pcm(ws.sent.at(-1).realtimeInput.audio.data).some((v) => v !== 0));
  src.disconnect();
  assert.deepEqual(conn, [true, false]);
});

test("elevenlabs: muted sends zeroed PCM chunks", async () => {
  const { ElevenLabsVoiceSource } = await import("../dist/elevenlabs.js");
  const src = new ElevenLabsVoiceSource({ credential: "agent_123" });
  const conn = connection(src);
  const connecting = src.connect();
  await until(() => FakeWebSocket.instances.length === 1);
  const ws = FakeWebSocket.instances[0];
  ws.open();
  ws.receive({ type: "conversation_initiation_metadata", conversation_initiation_metadata_event: { agent_output_audio_format: "pcm_16000", user_input_audio_format: "pcm_16000" } });
  await connecting;
  src.setMuted(true);
  FakeAudioWorkletNode.last.port.onmessage({ data: new Float32Array(512).fill(0.5) });
  const muted = pcm(ws.sent.at(-1).user_audio_chunk);
  assert.equal(muted.length, 512);
  assert.ok(muted.every((v) => v === 0));
  ws.close(1006, "gone"); // a drop: the conversation isn't resumable, so the session ends
  await until(() => conn.length === 2);
  assert.deepEqual(conn, [true, false]);
});
