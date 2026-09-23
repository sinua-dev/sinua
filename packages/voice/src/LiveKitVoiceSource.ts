import {
  ConnectionState,
  Room,
  RoomEvent,
  Track,
  type Participant,
  type RemoteParticipant,
  type RemoteTrack,
  type RemoteTrackPublication,
} from "livekit-client";
import { AudioAnalysis } from "./analysis.js";
import {
  AGENT_STATE_ATTRIBUTE,
  agentStateFromAttributes,
  isInferredBargeIn,
  isPrimaryAgent,
  publishesForAgent,
} from "./livekitAgent.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * `VoiceSource` for a LiveKit Room with a LiveKit Agents voice agent in it.
 * Unlike the three vendor adapters, this one handles no vendor protocol and
 * -- on its primary path -- no credential at all: the app already has a
 * connected `Room`, the agent's audio is already a subscribed remote track,
 * and the agent already publishes its lifecycle as a participant attribute.
 * This adapter just points the existing `AudioAnalysis` graph at that track
 * and forwards that attribute. Built on `livekit-client` directly (not the
 * React hooks), so it is framework-agnostic like the rest of `audio/`.
 *
 * API verified fresh from `livekit/client-sdk-js` source, `livekit/protocol`
 * and `livekit/components-js` (`useVoiceAssistant`/`useAgent` are the
 * reference for agent discovery) -- citations in
 * the vendor's public docs; the pure rules live in
 * `./livekitAgent.ts`.
 *
 * Two ways in:
 * - `{ room }` -- attach to the app's Room. Never connects, publishes, or
 *   disconnects it; `disconnect()` only removes this adapter's listeners
 *   and analyser. Does NOT `attach()` the agent track to an element either:
 *   the app already renders that audio, a second attach would double it.
 * - `{ url, token }` -- own Room (the Studio demo): connects, publishes the
 *   mic so the agent hears the user, calls `startAudio()` (a user gesture
 *   is present), attaches the agent track so it is audible, and disconnects
 *   on teardown. The token is a short-lived, room-scoped JWT the developer
 *   mints server-side (`lk token create` or their token server) -- already
 *   the production shape, nothing dev-only about it.
 *
 * State: `lk.agent.state` is read at discovery and on
 * `RoomEvent.ParticipantAttributesChanged` -- an identity mapping onto
 * `AgentState`, guarded to the five known values. Until the agent publishes
 * one: `initializing`. If it never does (older agents), an energy fallback
 * on its track: audible -> `speaking`, else `listening`.
 */
export type LiveKitVoiceSourceOptions =
  | { room: Room }
  | { url: string; token: string; publishMicrophone?: boolean };

const UPDATE_MS = 1000 / 30; // ~30fps metering, same as every other VoiceSource
const WATCHDOG_ZERO_FRAMES = 30; // ~1s of exact-zero RMS while the agent should be audible
const AGENT_JOIN_TIMEOUT_MS = 20_000; // components-js `useAgent`'s default "failed" timeout
const CONNECT_TIMEOUT_MS = 20_000;
const SPEAKING_LEVEL = 0.05; // energy fallback only, when no `lk.agent.state` was ever published

export class LiveKitVoiceSource implements VoiceSource {
  private readonly opts: LiveKitVoiceSourceOptions;
  private readonly ownsRoom: boolean;
  private room: Room | null = null;
  private agent: RemoteParticipant | null = null;
  private agentStateSeen = false;
  private agentResolver: (() => void) | null = null;

  private track: RemoteTrack | null = null;
  private audioEl: HTMLMediaElement | null = null;
  private ctx: AudioContext | null = null;
  private sourceNode: MediaStreamAudioSourceNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private zeroStreak = 0;

  private connected = false;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private interruptCb: (() => void) | null = null;
  private state: AgentState = "idle";

  constructor(opts: LiveKitVoiceSourceOptions) {
    this.opts = opts;
    this.ownsRoom = !("room" in opts);
  }

  onMetrics(cb: (m: VoiceMetrics) => void): void {
    this.metricsCb = cb;
  }

  onStateChange(cb: (s: AgentState) => void): void {
    this.stateCb = cb;
  }

  onInterrupt(cb: () => void): void {
    this.interruptCb = cb;
  }

  async connect(): Promise<void> {
    if (!("room" in this.opts) && (!this.opts.url || !this.opts.token)) {
      throw new Error('LiveKitVoiceSource: needs a server URL and a token ("wss://… <token>"), or an existing Room');
    }
    this.setState("initializing");
    try {
      const room = "room" in this.opts ? this.opts.room : new Room();
      this.room = room;
      this.ctx = new (globalThis.AudioContext ||
        (globalThis as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
      await this.ctx.resume().catch(() => undefined);
      this.bind(room);

      if (!("room" in this.opts)) {
        await room.connect(this.opts.url, this.opts.token);
        if (this.opts.publishMicrophone !== false) await room.localParticipant.setMicrophoneEnabled(true);
        await room.startAudio().catch(() => undefined);
      } else if (room.state !== ConnectionState.Connected) {
        await this.waitForConnected(room);
      }

      const existing = this.findAgent(room);
      if (existing) this.adoptAgent(existing);
      else if (this.ownsRoom) await this.waitForAgent();
      // Attached-Room path with no agent yet: keep listening -- the app may
      // dispatch the agent later.

      this.connected = true;
      this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
    } catch (err) {
      this.teardown();
      this.setState("idle");
      throw err;
    }
  }

  disconnect(): void {
    this.teardown();
    this.setState("idle");
  }

  // --- room events (class fields so they can be `off`ed by reference) ---

  private onParticipantConnected = (p: RemoteParticipant): void => {
    if (!this.agent && isPrimaryAgent(p.kind, p.attributes)) {
      this.adoptAgent(p);
    } else if (this.agent && publishesForAgent(p.kind, p.attributes, this.agent.identity)) {
      this.scanPublications(p);
    }
  };

  private onParticipantDisconnected = (p: RemoteParticipant): void => {
    if (this.agent && p.identity === this.agent.identity) {
      this.agent = null;
      this.agentStateSeen = false;
      this.detachTrack();
      if (this.connected) this.setState("initializing");
    }
  };

  private onAttributesChanged = (changed: Record<string, string>, participant: Participant): void => {
    if (!this.agent) {
      // Attributes can land after the participant itself did.
      if (isPrimaryAgent(participant.kind, participant.attributes)) {
        this.adoptAgent(participant as RemoteParticipant);
      }
      return;
    }
    if (participant.identity === this.agent.identity && AGENT_STATE_ATTRIBUTE in changed) {
      const s = agentStateFromAttributes(participant.attributes);
      if (s) {
        this.agentStateSeen = true;
        if (isInferredBargeIn(this.state, s, this.room?.localParticipant.isSpeaking ?? false)) this.interruptCb?.();
        this.setState(s);
      }
    }
  };

  private onTrackSubscribed = (
    track: RemoteTrack,
    _publication: RemoteTrackPublication,
    participant: RemoteParticipant
  ): void => {
    if (track.kind !== Track.Kind.Audio) return;
    if (!this.agent && isPrimaryAgent(participant.kind, participant.attributes)) {
      this.adoptAgent(participant);
    }
    if (!this.agent) return;
    const isAgent = participant.identity === this.agent.identity;
    const isWorker = publishesForAgent(participant.kind, participant.attributes, this.agent.identity);
    if (isAgent || isWorker) this.attachTrack(track);
  };

  private onTrackUnsubscribed = (track: Track): void => {
    if (this.track && track === this.track) this.detachTrack();
  };

  private onDisconnected = (): void => {
    if (this.connected || this.room) this.disconnect();
  };

  private bind(room: Room): void {
    room.on(RoomEvent.ParticipantConnected, this.onParticipantConnected);
    room.on(RoomEvent.ParticipantDisconnected, this.onParticipantDisconnected);
    room.on(RoomEvent.ParticipantAttributesChanged, this.onAttributesChanged);
    room.on(RoomEvent.TrackSubscribed, this.onTrackSubscribed);
    room.on(RoomEvent.TrackUnsubscribed, this.onTrackUnsubscribed);
    room.on(RoomEvent.Disconnected, this.onDisconnected);
  }

  private unbind(room: Room): void {
    room.off(RoomEvent.ParticipantConnected, this.onParticipantConnected);
    room.off(RoomEvent.ParticipantDisconnected, this.onParticipantDisconnected);
    room.off(RoomEvent.ParticipantAttributesChanged, this.onAttributesChanged);
    room.off(RoomEvent.TrackSubscribed, this.onTrackSubscribed);
    room.off(RoomEvent.TrackUnsubscribed, this.onTrackUnsubscribed);
    room.off(RoomEvent.Disconnected, this.onDisconnected);
  }

  private waitForConnected(room: Room): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        cleanup();
        reject(new Error("LiveKit Room did not connect within 20s"));
      }, CONNECT_TIMEOUT_MS);
      const onConnected = () => {
        cleanup();
        resolve();
      };
      const onDisconnected = () => {
        cleanup();
        reject(new Error("LiveKit Room disconnected before the adapter could attach"));
      };
      const cleanup = () => {
        clearTimeout(timer);
        room.off(RoomEvent.Connected, onConnected);
        room.off(RoomEvent.Disconnected, onDisconnected);
      };
      room.on(RoomEvent.Connected, onConnected);
      room.on(RoomEvent.Disconnected, onDisconnected);
    });
  }

  private waitForAgent(): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.agentResolver = null;
        reject(new Error("No agent joined the LiveKit room within 20s (is an agent dispatched to this room?)"));
      }, AGENT_JOIN_TIMEOUT_MS);
      this.agentResolver = () => {
        clearTimeout(timer);
        this.agentResolver = null;
        resolve();
      };
    });
  }

  private findAgent(room: Room): RemoteParticipant | null {
    for (const p of room.remoteParticipants.values()) {
      if (isPrimaryAgent(p.kind, p.attributes)) return p;
    }
    return null;
  }

  private adoptAgent(p: RemoteParticipant): void {
    this.agent = p;
    const s = agentStateFromAttributes(p.attributes);
    if (s) {
      this.agentStateSeen = true;
      this.setState(s);
    }
    this.scanPublications(p);
    if (this.room) {
      for (const other of this.room.remoteParticipants.values()) {
        if (publishesForAgent(other.kind, other.attributes, p.identity)) this.scanPublications(other);
      }
    }
    this.agentResolver?.();
  }

  /** Tracks subscribed before we started listening never fire TrackSubscribed for us. */
  private scanPublications(p: RemoteParticipant): void {
    for (const pub of p.trackPublications.values()) {
      const track = pub.track;
      if (pub.isSubscribed && track && track.kind === Track.Kind.Audio) {
        this.attachTrack(track);
        return;
      }
    }
  }

  /** Same graph `livekit-client`'s own `createAudioAnalyser` builds, feeding the unchanged `AudioAnalysis`. */
  private attachTrack(track: RemoteTrack): void {
    if (!this.ctx) return;
    if (this.track === track && this.analysis) return;
    this.detachTrack();
    this.track = track;
    const analyser = this.ctx.createAnalyser();
    analyser.fftSize = 512; // see LocalMicVoiceSource for why not 256
    analyser.smoothingTimeConstant = 0; // AudioAnalysis does its own attack/release
    const source = this.ctx.createMediaStreamSource(new MediaStream([track.mediaStreamTrack]));
    source.connect(analyser);
    this.sourceNode = source;
    this.analysis = new AudioAnalysis(analyser);
    this.zeroStreak = 0;
    if (this.ownsRoom) {
      // Audible in the demo; an attached Room's app renders the audio itself.
      this.audioEl = track.attach();
    }
  }

  private detachTrack(): void {
    this.sourceNode?.disconnect();
    this.sourceNode = null;
    this.analysis = null;
    if (this.track && this.audioEl) {
      this.track.detach(this.audioEl);
    }
    this.audioEl = null;
    this.track = null;
  }

  private tick(): void {
    if (!this.analysis || !this.track) return;

    // Watchdog (see LocalMicVoiceSource / OpenAIRealtimeVoiceSource): a remote WebRTC
    // track's AnalyserNode can silently go flat -- only counted while the
    // agent says it is speaking, since silence legitimately reads zero.
    if (this.state === "speaking" && this.analysis.rawRms() === 0) {
      this.zeroStreak++;
      if (this.zeroStreak > WATCHDOG_ZERO_FRAMES) {
        const track = this.track;
        this.detachTrack();
        this.attachTrack(track);
        return;
      }
    } else {
      this.zeroStreak = 0;
    }

    const metrics = this.analysis.read();
    this.metricsCb?.(metrics);

    if (this.connected && !this.agentStateSeen) {
      this.setState(metrics.level > SPEAKING_LEVEL ? "speaking" : "listening");
    }
  }

  private setState(s: AgentState): void {
    if (this.state === s) return;
    this.state = s;
    this.stateCb?.(s);
  }

  private teardown(): void {
    this.connected = false;
    if (this.intervalId != null) clearInterval(this.intervalId);
    this.intervalId = null;
    this.agentResolver = null;
    this.detachTrack();
    const room = this.room;
    this.room = null;
    if (room) {
      this.unbind(room);
      if (this.ownsRoom) void room.disconnect().catch(() => undefined);
    }
    this.agent = null;
    this.agentStateSeen = false;
    void this.ctx?.close().catch(() => undefined);
    this.ctx = null;
  }
}
