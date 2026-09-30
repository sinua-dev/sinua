// OpenAI GPT-Live: the shared state table (spec/openai-live-cases.json, also run by
// iOS and Android), the source on fakes (test/fakes.mjs) and the server helper.
// No device, no network, no sound.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { install, restore, mic, net, media, FakeRTCPeerConnection, until, tick } from "./fakes.mjs";

beforeEach(install);
afterEach(restore);

const cases = JSON.parse(readFileSync(new URL("../../../spec/openai-live-cases.json", import.meta.url), "utf8"));
const OWN = "https://app.example/api/voice/live"; // reserved name: never a real host
const liveAnswer = (id = "live_123") => async () => ({
  ok: true,
  status: 201,
  text: async () => JSON.stringify({ session: { id }, transport: { type: "webrtc", sdp: "v=0 fake-answer" } }),
});

test("OpenAILiveSession: spec/openai-live-cases.json", async () => {
  const { OpenAILiveSession, LIVE_SPEAKING_LEVEL, LIVE_SPEAKING_TAIL_FRAMES, LIVE_BARGE_IN_WINDOW_MS, LIVE_DELEGATION_TIMEOUT_MS } = await import(
    "../dist/openaiLive.js"
  );
  assert.deepEqual(cases.constants, {
    speakingLevel: LIVE_SPEAKING_LEVEL,
    speakingTailFrames: LIVE_SPEAKING_TAIL_FRAMES,
    bargeInWindowMs: LIVE_BARGE_IN_WINDOW_MS,
    delegationTimeoutMs: LIVE_DELEGATION_TIMEOUT_MS,
  });
  for (const c of cases.cases) {
    const s = new OpenAILiveSession();
    let interrupts = 0;
    s.onInterrupt = () => interrupts++;
    s.connecting();
    c.steps.forEach((step, i) => {
      const at = `${c.name}, step ${i + 1}`;
      if (step.event) s.handle(JSON.stringify(step.event), step.t);
      else for (let k = 0; k < (step.repeat ?? 1); k++) s.tick(step.level, step.t + k * 33);
      assert.equal(s.state, step.state, at);
      if (step.interrupts !== undefined) assert.equal(interrupts, step.interrupts, `${at}: interrupts`);
      if (step.closed !== undefined) assert.equal(s.closedReason, step.closed, `${at}: closed`);
      if (step.fatal !== undefined) assert.equal(s.fatalCode, step.fatal, `${at}: fatal`);
    });
  }
});

test("liveAnswerSdp: OpenAI's 201 JSON or bare SDP; anything else is null", async () => {
  const { liveAnswerSdp } = await import("../dist/openaiLive.js");
  assert.deepEqual(liveAnswerSdp(JSON.stringify({ session: { id: "live_1" }, transport: { type: "webrtc", sdp: "v=0 a" } })), {
    sdp: "v=0 a",
    sessionId: "live_1",
  });
  assert.deepEqual(liveAnswerSdp("v=0 b"), { sdp: "v=0 b", sessionId: null });
  assert.equal(liveAnswerSdp(JSON.stringify({ error: "nope" })), null);
  assert.equal(liveAnswerSdp("<html>"), null);
});

/** Connects a source against the fakes: answers the offer, then sends session.started. */
async function connectLive(src, id = "live_123") {
  net.respond = liveAnswer(id);
  const connecting = src.connect();
  await until(() => FakeRTCPeerConnection.last?.remote);
  const pc = FakeRTCPeerConnection.last;
  pc.dc.onmessage({ data: JSON.stringify({ type: "session.started", session: { id } }) });
  await connecting;
  return pc;
}
const send = (pc, ev) => pc.dc.onmessage({ data: JSON.stringify(ev) });
/** disconnect(), answered by the server's session.closed (no 5 s drain timer left behind). */
function end(src, pc = FakeRTCPeerConnection.last) {
  src.disconnect();
  pc.dc.onmessage?.({ data: JSON.stringify({ type: "session.closed", reason: "close_requested" }) });
}

function watch(src) {
  const states = [];
  let interrupts = 0;
  src.onStateChange((s) => states.push(s));
  src.onMetrics(() => {});
  src.onInterrupt(() => interrupts++);
  return { states, interrupts: () => interrupts };
}

test("live: posts { sdp } as JSON with your token, after ICE gathering; live at session.started", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  FakeRTCPeerConnection.slowGathering = true;
  net.respond = liveAnswer();
  const src = new OpenAILiveVoiceSource({ sessionUrl: OWN, credential: "dvf_token", reconnect: false });
  const w = watch(src);
  const connecting = src.connect();
  await until(() => FakeRTCPeerConnection.last?.local);
  const pc = FakeRTCPeerConnection.last;
  await tick(10);
  assert.equal(net.requests.length, 0, "no offer before ICE gathering completes");
  pc.finishGathering();
  await until(() => pc.remote);
  const call = net.requests[0];
  assert.equal(call.url, OWN);
  assert.equal(call.init.method, "POST");
  assert.equal(call.init.headers["Content-Type"], "application/json");
  assert.equal(call.init.headers.Authorization, "Bearer dvf_token");
  assert.deepEqual(JSON.parse(call.init.body), { sdp: "v=0 fake-offer" });
  assert.deepEqual(pc.remote, { type: "answer", sdp: "v=0 fake-answer" });
  assert.equal(pc.dc.label, "oai-events");
  assert.equal(pc.dc.options, undefined, "no negotiated channel: OpenAI documents no dcid for Live");
  await tick(5);
  assert.equal(w.states.at(-1), "initializing", "not live until session.started");
  send(pc, { type: "session.started", session: { id: "live_123" } });
  await connecting;
  assert.equal(w.states.at(-1), "listening");
  assert.equal(src.sessionId, "live_123");
  assert.equal(pc.dc.sent.length, 0, "the client never sends session.start");
  send(pc, { type: "session.delegation.created", delegation: { id: "d1" } });
  assert.equal(w.states.at(-1), "thinking");
  send(pc, { type: "response.event", delegation_id: "d1", event: { type: "response.completed" } });
  assert.equal(w.states.at(-1), "listening");
  end(src, pc);
});

test("live: no credential means no Authorization header; a bare SDP answer works", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAILiveVoiceSource({ sessionUrl: OWN, reconnect: false });
  const connecting = src.connect(); // default fake answer: 200 + "v=0 fake-answer"
  await until(() => FakeRTCPeerConnection.last?.remote);
  send(FakeRTCPeerConnection.last, { type: "session.started" });
  await connecting;
  assert.equal(net.requests[0].init.headers.Authorization, undefined);
  end(src);
});

test("live: api.openai.com as sessionUrl and a raw sk- token are refused before the mic", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  await assert.rejects(
    new OpenAILiveVoiceSource({ sessionUrl: "https://api.openai.com/v1/live/sessions" }).connect(),
    /sessionUrl must be your own endpoint/,
  );
  await assert.rejects(
    new OpenAILiveVoiceSource({ sessionUrl: OWN, credential: "sk-proj-abc" }).connect(),
    /never a raw OpenAI key \(sk-…\)/,
  );
  assert.equal(net.requests.length, 0);
  assert.equal(mic.requests, 0);
});

test("live: a 4xx is fatal, a 2xx without an SDP answer too", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  net.respond = async () => ({ ok: false, status: 403, text: async () => "forbidden" });
  await assert.rejects(new OpenAILiveVoiceSource({ sessionUrl: OWN }).connect(), /returned 403: forbidden/);
  net.respond = async () => ({ ok: true, status: 200, text: async () => "{}" });
  await assert.rejects(new OpenAILiveVoiceSource({ sessionUrl: OWN }).connect(), /without an SDP answer/);
});

test("live: disconnect is idle at once, then closes gracefully at session.closed", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAILiveVoiceSource({ sessionUrl: OWN, reconnect: false });
  const w = watch(src);
  const pc = await connectLive(src);
  pc.dc.readyState = "open";
  const connections = [];
  src.onConnectionChange((c) => connections.push(c));
  src.disconnect();
  assert.equal(w.states.at(-1), "idle");
  assert.deepEqual(connections, [false]);
  assert.deepEqual(pc.dc.sent.at(-1), { type: "session.close" });
  assert.equal(mic.stopped, 0, "the call drains until session.closed");
  assert.notEqual(pc.connectionState, "closed");
  send(pc, { type: "session.closed", reason: "close_requested", usage: { seconds: 12 } });
  assert.equal(mic.stopped, 1);
  assert.equal(pc.connectionState, "closed");
  assert.equal(media.elements.every((e) => e.srcObject === null), true);
});

test("live: an expired session is replaced by a new one", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  let n = 0;
  const src = new OpenAILiveVoiceSource({ sessionUrl: OWN, credential: async () => `dvf_${++n}` });
  const w = watch(src);
  const first = await connectLive(src, "live_1");
  send(first, { type: "session.closed", reason: "expired" });
  assert.equal(w.states.at(-1), "initializing");
  await until(() => FakeRTCPeerConnection.last !== first && FakeRTCPeerConnection.last.remote);
  const second = FakeRTCPeerConnection.last;
  assert.equal(net.requests.at(-1).init.headers.Authorization, "Bearer dvf_2", "a fresh token per session");
  send(second, { type: "session.started", session: { id: "live_2" } });
  assert.equal(w.states.at(-1), "listening");
  assert.equal(src.sessionId, "live_2");
  assert.equal(mic.requests, 1, "the mic is kept across sessions");
  end(src, second);
});

test("live: a server-side close (content, remote_hangup) ends in idle without reconnecting", async () => {
  const { OpenAILiveVoiceSource } = await import("../dist/openai.js");
  for (const reason of ["content", "remote_hangup", "close_requested"]) {
    FakeRTCPeerConnection.last = null;
    const src = new OpenAILiveVoiceSource({ sessionUrl: OWN });
    const w = watch(src);
    const pc = await connectLive(src);
    const requests = net.requests.length;
    send(pc, { type: "session.closed", reason });
    assert.equal(w.states.at(-1), "idle", reason);
    await tick(150);
    assert.equal(net.requests.length, requests, `${reason}: no new session`);
  }
});

test("realtime warp: a pre-negotiated event channel and its dcid", async () => {
  const { OpenAIRealtimeVoiceSource } = await import("../dist/openai.js");
  const src = new OpenAIRealtimeVoiceSource({ credential: "ek_test", warp: true, reconnect: false });
  await src.connect();
  const pc = FakeRTCPeerConnection.last;
  assert.deepEqual(pc.dc.options, { negotiated: true, id: 1 });
  const url = new URL(net.requests[0].url);
  assert.equal(url.searchParams.get("dcid"), "1");
  assert.equal(url.searchParams.get("model"), "gpt-realtime");
  src.disconnect();
  const plain = new OpenAIRealtimeVoiceSource({ credential: "ek_test", reconnect: false });
  await plain.connect();
  assert.equal(FakeRTCPeerConnection.last.dc.options, undefined);
  assert.equal(new URL(net.requests[1].url).searchParams.has("dcid"), false);
  plain.disconnect();
});

test("server: createOpenAILiveSession posts { session, transport } and openAILiveResponse answers 201", async () => {
  const { createOpenAILiveSession, openAILiveResponse } = await import("../dist/server.js");
  const saved = globalThis.document;
  delete globalThis.document; // the helper refuses to run where there's a DOM
  try {
    const calls = [];
    const fetch = async (url, init) => {
      calls.push({ url, init });
      return new Response(JSON.stringify({ session: { id: "live_9" }, transport: { type: "webrtc", sdp: "v=0 ans" } }), { status: 201 });
    };
    const s = await createOpenAILiveSession({
      apiKey: "sk-test",
      sdp: "v=0 offer",
      session: { instructions: "Be brief." },
      safetyIdentifier: "hash",
      fetch,
    });
    assert.deepEqual(s, { sessionId: "live_9", sdp: "v=0 ans" });
    assert.equal(calls[0].url, "https://api.openai.com/v1/live/sessions");
    assert.equal(calls[0].init.headers.Authorization, "Bearer sk-test");
    assert.equal(calls[0].init.headers["OpenAI-Safety-Identifier"], "hash");
    assert.deepEqual(JSON.parse(calls[0].init.body), {
      session: { model: "gpt-live-1", instructions: "Be brief." },
      transport: { type: "webrtc", sdp: "v=0 offer" },
    });
    const res = openAILiveResponse(s);
    assert.equal(res.status, 201);
    assert.equal(res.headers.get("Cache-Control"), "no-store");
    assert.deepEqual(await res.json(), { session: { id: "live_9" }, transport: { type: "webrtc", sdp: "v=0 ans" } });
    const failing = async () => new Response("bad key sk-test", { status: 401 });
    await assert.rejects(createOpenAILiveSession({ apiKey: "sk-test", sdp: "v=0", fetch: failing }), (e) => {
      assert.equal(e.status, 401);
      assert.doesNotMatch(e.message, /sk-test/, "the key never ends up in a message");
      return true;
    });
  } finally {
    if (saved !== undefined) globalThis.document = saved;
  }
});
