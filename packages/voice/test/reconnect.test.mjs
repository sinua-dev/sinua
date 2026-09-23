// The OpenAI Realtime reconnect path, end to end on the shared fakes.
//
// `realtimeReconnect.ts`'s helpers were already unit-tested; the orchestration
// that uses them -- `onDrop` -> `reconnect()` -> a *new* session -> transcript
// replay -- was not (36.4 % function coverage). This is the path that only
// runs when a live call breaks, which is exactly when nobody is watching, so
// it is the one worth pinning.
//
// Everything here is millisecond-scale by design: the adapter takes a
// `reconnect` policy, so the backoff is driven by real (tiny) timers rather
// than by mocking the clock.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { install, restore, audio, media, mic, net, FakeRTCPeerConnection, until, tick } from "./fakes.mjs";

const live = [];
let quiet;

beforeEach(() => {
  install();
  quiet = silence();
});
afterEach(() => {
  // A failing assertion must not leak the ~30fps metering interval; without
  // this the runner hangs instead of reporting the failure.
  for (const s of live.splice(0)) s.disconnect();
  quiet.restore();
  restore();
});

/** The reconnect path logs by design; keep the output and keep it off stdout. */
function silence() {
  const warn = console.warn;
  const error = console.error;
  const seen = [];
  console.warn = (...a) => seen.push(a.join(" "));
  console.error = (...a) => seen.push(a.join(" "));
  return {
    seen,
    restore: () => {
      console.warn = warn;
      console.error = error;
    },
  };
}

function watch(src) {
  const states = [];
  const metrics = [];
  src.onStateChange((s) => states.push(s));
  src.onMetrics((m) => metrics.push(m));
  return { states, metrics };
}

/** Fast enough that three attempts finish inside a test, real timers throughout. */
const FAST = { maxAttempts: 3, baseDelayMs: 1, maxDelayMs: 2 };

async function connected(opts = {}) {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ reconnect: FAST, ...opts });
  live.push(src);
  const w = watch(src);
  await src.connect();
  return { src, w, pc: FakeRTCPeerConnection.last };
}

const calls = () => net.requests.filter((r) => r.url.includes("/realtime/calls")).length;
/** The drop the adapter is most likely to see in the field. */
const dropDataChannel = (pc) => pc.dc.onclose();
const bearer = (i) => net.requests.filter((r) => r.url.includes("/realtime/calls"))[i].init.headers.Authorization;

test("reconnect: a dropped data channel opens a new session and ends back in listening", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_first" });
  assert.equal(calls(), 1);

  dropDataChannel(pc);
  assert.equal(w.states.at(-1), "initializing", "the drop is visible immediately, not after the first attempt");

  await until(() => w.states.at(-1) === "listening");
  assert.deepEqual(w.states, ["initializing", "listening", "initializing", "listening"]);
  assert.equal(calls(), 2, "a new call, because Realtime has no session resumption");
  assert.notEqual(FakeRTCPeerConnection.last, pc, "a new peer connection, not a reused one");
  assert.equal(mic.stopped, 0, "the mic is kept across the reconnect");
  src.disconnect();
});

test("reconnect: one zeroed metrics reading is emitted so visuals settle instead of freezing", async () => {
  // Loud, so a real reading is distinguishable from the zeroed one. With the
  // fakes silent this test passed even with the zeroing deleted -- the
  // readings were already zero and it was measuring nothing.
  audio.byte = 200;
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  // The model's voice arrives as an ordinary remote track. A real reading
  // first, so the zeroed one can be shown to match its shape.
  const stream = { getAudioTracks: () => [{ readyState: "live" }] };
  pc.ontrack({ streams: [stream] });
  assert.equal(media.elements.length, 1, "the remote track gets an <audio> sink, or Chrome won't pump it");
  assert.equal(media.elements[0].srcObject, stream);
  await until(() => w.metrics.length > 0 && w.metrics.at(-1).level > 0);
  const bands = w.metrics.at(-1).bands.length;
  assert.ok(bands > 0);

  dropDataChannel(pc);
  const zeroed = w.metrics.at(-1);
  assert.equal(zeroed.level, 0);
  assert.equal(zeroed.bands.length, bands, "same band count, so the visual doesn't resize on a drop");
  assert.ok(zeroed.bands.every((b) => b === 0));

  src.disconnect();
  assert.equal(media.elements[0].srcObject, null, "and teardown releases the sink");
});

test("reconnect: every attempt mints a fresh credential", async () => {
  let n = 0;
  const { src, w, pc } = await connected({ getCredential: async () => `ek_${++n}` });
  assert.equal(bearer(0), "Bearer ek_1");

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "listening");
  assert.equal(bearer(1), "Bearer ek_2", "the expired key is never reused");
  src.disconnect();
});

test("reconnect: a late event from the dropped peer connection cannot start a second reconnect", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  // Captured before the drop, while the handler is still attached.
  const staleHandler = pc.onconnectionstatechange;

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "listening");
  const after = calls();

  pc.connectionState = "failed";
  staleHandler();
  await tick(30);
  assert.equal(calls(), after, "the old session's failure is not this session's problem");
  assert.equal(w.states.at(-1), "listening");
  src.disconnect();
});

test("reconnect: the finalized transcript is replayed into the new session, in order", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  const send = (o) => pc.dc.onmessage({ data: JSON.stringify(o) });
  send({ type: "response.output_audio_transcript.done", transcript: "Hello there." });
  send({ type: "conversation.item.input_audio_transcription.completed", transcript: "What's the weather?" });

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "listening");

  const replayed = FakeRTCPeerConnection.last.dc.sent;
  assert.deepEqual(
    replayed.map((e) => [e.type, e.item.role, e.item.content[0].type, e.item.content[0].text]),
    [
      ["conversation.item.create", "assistant", "output_text", "Hello there."],
      ["conversation.item.create", "user", "input_text", "What's the weather?"],
    ],
    "assistant as output_text, user as input_text, oldest first"
  );
  src.disconnect();
});

test("reconnect: replayTranscript false sends nothing into the new session", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x", replayTranscript: false });
  pc.dc.onmessage({ data: JSON.stringify({ type: "response.output_audio_transcript.done", transcript: "Hello." }) });

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "listening");
  assert.deepEqual(FakeRTCPeerConnection.last.dc.sent, []);
  src.disconnect();
});

test("reconnect: it gives up after the policy's attempts and ends in idle", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  net.respond = async () => ({ ok: false, status: 503, text: async () => "upstream down", json: async () => ({}) });

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "idle", 5000);
  assert.equal(calls(), 1 + FAST.maxAttempts, "three attempts, then it stops");
  assert.ok(
    quiet.seen.some((m) => m.includes("gave up reconnecting")),
    "giving up is reported, not silent"
  );
  assert.ok(mic.stopped >= 1, "giving up releases the microphone");
  await tick(50);
  assert.equal(calls(), 1 + FAST.maxAttempts, "and no attempt arrives after the give-up");
  src.disconnect();
});

test("reconnect: a fatal error code seen before the drop means no attempt at all", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  pc.dc.onmessage({ data: JSON.stringify({ type: "error", error: { code: "invalid_api_key" } }) });

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "idle");
  await tick(200); // longer than the first backoff, had one been scheduled
  assert.equal(calls(), 1, "a key that cannot work is not retried");
  assert.ok(quiet.seen.some((m) => m.includes("fatal error invalid_api_key")));
  src.disconnect();
});

test("reconnect: a 401 on an attempt is fatal and stops the loop early", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  net.respond = async () => ({ ok: false, status: 401, text: async () => "expired", json: async () => ({}) });

  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "idle", 5000);
  assert.equal(calls(), 2, "one attempt, then the fatal status breaks out of the remaining two");
  src.disconnect();
});

test("reconnect: a pasted ek_ with no getCredential is not retried", async () => {
  const { src, w, pc } = await connected({ credential: "ek_pasted" });
  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "idle");
  await tick(200);
  assert.equal(calls(), 1, "a single-use, expiring key cannot open a second session");
  assert.ok(quiet.seen.some((m) => m.includes("pass getCredential")), "and the message says what to do about it");
  src.disconnect();
});

test("reconnect: false restores the old drop-to-idle behaviour", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x", reconnect: false });
  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "idle");
  await tick(200);
  assert.equal(calls(), 1);
  assert.ok(quiet.seen.some((m) => m.includes("reconnect disabled")));
  src.disconnect();
});

test("reconnect: disconnect() during the backoff wait stops the loop and leaves no stray timer", async () => {
  let minted = 0;
  const { src, w, pc } = await connected({
    getCredential: async () => {
      minted++;
      return "ek_x";
    },
  });
  dropDataChannel(pc);
  // Inside the first (~100ms) wait: the attempt has been scheduled, not made.
  assert.equal(calls(), 1);
  src.disconnect();
  assert.equal(w.states.at(-1), "idle");

  await tick(400);
  assert.equal(calls(), 1, "the scheduled attempt sees wantConnected false and returns");
  // The credential count, not just the call count: a disconnected source must
  // not wake up and ask the backend to mint another key. Without this the
  // check passes even when the wantConnected re-check after the wait is
  // deleted, because the attempt then fails later on the released mic.
  assert.equal(minted, 1, "and it does not ask the backend for another credential");
  assert.equal(w.states.at(-1), "idle", "and nothing flips the state back afterwards");
});

test("reconnect: the dropped session's handlers are detached, so it cannot speak again", async () => {
  const { src, w, pc } = await connected({ getCredential: async () => "ek_x" });
  const dc = pc.dc;
  dropDataChannel(pc);
  await until(() => w.states.at(-1) === "listening");

  // This is what makes a late `close` or `connectionstatechange` from the old
  // session a non-event: not only the `this.pc === pc` checks, but that the
  // handlers are gone at all. A real failure often produces both symptoms.
  assert.equal(dc.onclose, null);
  assert.equal(dc.onmessage, null);
  assert.equal(pc.onconnectionstatechange, null);
  assert.equal(pc.ontrack, null);
  assert.equal(pc.connectionState, "closed");
  src.disconnect();
});
