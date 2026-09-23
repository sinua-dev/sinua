// LiveKitVoiceSource against a fake Room. Until now this file was at **0%**
// coverage -- 302 lines of shipped logic whose only test was `parseUrlAndToken`,
// a pure string helper.
//
// Nothing here connects. The `{ room }` mode exists so an app can hand us the
// Room it already has, which is also what makes the class testable: it only
// ever calls `room.on(...)` and reads `state` / `remoteParticipants` /
// `localParticipant`, so a duck-typed object drives every handler.
//
// The enums come from the **real `livekit-client`** rather than hand-written
// constants, so a test that passes is a statement about the SDK's own values.
import { test, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { ParticipantKind, RoomEvent, Track } from "livekit-client";
import { install, restore, audio, tick, until } from "./fakes.mjs";

const live = [];

beforeEach(install);
afterEach(() => {
  // Every source gets torn down even when an assertion threw. Without this a
  // failing test leaks its ~30fps metering interval and node never exits: the
  // first mutation run hung here rather than reporting the failure.
  for (const s of live.splice(0)) s.disconnect();
  restore();
});

const AGENT_STATE = "lk.agent.state";

/** A Room the class can bind to: an emitter plus the three fields it reads. */
function fakeRoom({ state = "connected", participants = [], localSpeaking = false } = {}) {
  const handlers = new Map();
  const room = {
    state,
    remoteParticipants: new Map(participants.map((p) => [p.identity, p])),
    localParticipant: { isSpeaking: localSpeaking, setMicrophoneEnabled: async () => {} },
    on(event, cb) {
      (handlers.get(event) ?? handlers.set(event, []).get(event)).push(cb);
      return room;
    },
    off(event, cb) {
      const list = handlers.get(event) ?? [];
      const i = list.indexOf(cb);
      if (i >= 0) list.splice(i, 1);
      return room;
    },
    disconnect: async () => (room.disconnected = true),
    startAudio: async () => {},
    disconnected: false,
  };
  room.emit = (event, ...args) => {
    for (const cb of handlers.get(event) ?? []) cb(...args);
  };
  return room;
}

const participant = ({ identity = "p", kind = ParticipantKind.STANDARD, attributes = {} } = {}) => ({
  identity,
  kind,
  attributes,
  trackPublications: new Map(),
});

const agent = (attributes = {}) => participant({ identity: "agent", kind: ParticipantKind.AGENT, attributes });

const audioTrack = () => ({ kind: Track.Kind.Audio, mediaStreamTrack: { kind: "audio" } });
const videoTrack = () => ({ kind: Track.Kind.Video, mediaStreamTrack: { kind: "video" } });

function watch(src) {
  const states = [];
  let interrupts = 0;
  src.onStateChange((s) => states.push(s));
  src.onInterrupt(() => interrupts++);
  return { states, interrupts: () => interrupts };
}

const load = () => import("../dist/livekit.js");

/** Constructs a source and registers it for teardown (see afterEach). */
async function source(room) {
  const { LiveKitVoiceSource } = await load();
  const src = new LiveKitVoiceSource({ room });
  live.push(src);
  return src;
}

test("livekit: an agent already in the room is adopted, and its published state is used", async () => {
  const room = fakeRoom({ participants: [agent({ [AGENT_STATE]: "thinking" })] });
  const src = await source(room);
  const w = watch(src);
  await src.connect();
  assert.equal(w.states.at(-1), "thinking", "the agent's own lk.agent.state wins over the energy heuristic");

  src.disconnect();
  assert.equal(room.disconnected, false, "a Room we don't own is never disconnected");
});

test("livekit: a non-agent participant is not adopted", async () => {
  const room = fakeRoom({ participants: [participant({ attributes: { [AGENT_STATE]: "speaking" } })] });
  const src = await source(room);
  const w = watch(src);
  await src.connect();
  assert.ok(!w.states.includes("speaking"), "a STANDARD participant's attributes must not drive the visual");
});

test("livekit: an agent that joins later is adopted", async () => {
  const room = fakeRoom();
  const src = await source(room);
  const w = watch(src);
  await src.connect();
  room.emit(RoomEvent.ParticipantConnected, agent({ [AGENT_STATE]: "listening" }));
  assert.equal(w.states.at(-1), "listening");
});

test("livekit: attribute changes drive the lifecycle; an unknown value is ignored", async () => {
  const a = agent({ [AGENT_STATE]: "listening" });
  const room = fakeRoom({ participants: [a] });
  const src = await source(room);
  const w = watch(src);
  await src.connect();

  a.attributes = { [AGENT_STATE]: "speaking" };
  room.emit(RoomEvent.ParticipantAttributesChanged, { [AGENT_STATE]: "speaking" }, a);
  assert.equal(w.states.at(-1), "speaking");

  const before = w.states.at(-1);
  a.attributes = { [AGENT_STATE]: "pondering" };
  room.emit(RoomEvent.ParticipantAttributesChanged, { [AGENT_STATE]: "pondering" }, a);
  assert.equal(w.states.at(-1), before, "a value outside the five AgentStates is ignored, not passed through");
});

test("livekit: an audio track from the agent is analysed; a video track is not", async () => {
  const a = agent({ [AGENT_STATE]: "listening" });
  const room = fakeRoom({ participants: [a] });
  const src = await source(room);
  const metrics = [];
  src.onMetrics((m) => metrics.push(m));
  audio.byte = 200;
  await src.connect();

  room.emit(RoomEvent.TrackSubscribed, videoTrack(), {}, a);
  // Several metering ticks (~33ms each) pass; an analyser would have spoken.
  await tick(120);
  assert.equal(metrics.length, 0, "a video track must not open an analyser");

  room.emit(RoomEvent.TrackSubscribed, audioTrack(), {}, a);
  await until(() => metrics.length > 0);
  assert.equal(metrics.at(-1).bands.length, 16);
  await until(() => metrics.at(-1).level > 0);
});

test("livekit: a worker publishing on the agent's behalf is accepted", async () => {
  const a = agent({ [AGENT_STATE]: "listening" });
  const worker = participant({
    identity: "worker",
    kind: ParticipantKind.AGENT,
    attributes: { "lk.publish_on_behalf": "agent" },
  });
  const room = fakeRoom({ participants: [a] });
  const src = await source(room);
  const metrics = [];
  src.onMetrics((m) => metrics.push(m));
  audio.byte = 180;
  await src.connect();

  room.emit(RoomEvent.TrackSubscribed, audioTrack(), {}, worker);
  await until(() => metrics.length > 0 && metrics.at(-1).level > 0);
});

test("livekit: barge-in is inferred only when the local participant is speaking", async () => {
  // Two runs of the same transition, differing only in whether the user is
  // talking over it. The adapter gets no interruption event from LiveKit, so
  // this inference is the whole mechanism -- worth pinning from both sides.
  const bargeIn = async (localSpeaking) => {
    const a = agent({ [AGENT_STATE]: "speaking" });
    const room = fakeRoom({ participants: [a], localSpeaking });
    const src = await source(room);
    const w = watch(src);
    await src.connect();
    a.attributes = { [AGENT_STATE]: "listening" };
    room.emit(RoomEvent.ParticipantAttributesChanged, { [AGENT_STATE]: "listening" }, a);
    src.disconnect();
    return w.interrupts();
  };

  assert.equal(await bargeIn(false), 0, "the agent simply finishing its turn is not a barge-in");
  assert.equal(await bargeIn(true), 1, "speaking -> listening while the user talks is a barge-in");
});

test("livekit: Disconnected returns to idle", async () => {
  const room = fakeRoom({ participants: [agent({ [AGENT_STATE]: "listening" })] });
  const src = await source(room);
  const w = watch(src);
  await src.connect();
  room.emit(RoomEvent.Disconnected);
  assert.equal(w.states.at(-1), "idle");
});
