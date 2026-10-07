// DOM-free half of `OpenAILiveVoiceSource`: the state rules and the session
// response parsing, kept apart so they can be tested under plain node
// (spec/openai-live-cases.json; iOS and Android port the same rules).
//
// GPT-Live has none of Realtime's turn events (no `speech_started/stopped`,
// no `response.created/done`, no `output_audio_buffer.*`): OpenAI's migration
// guide says to drive the speaking indicator from the audio player. So:
// - `speaking` is the remote track's level (above 0.05), left after ~300 ms of quiet;
// - `thinking` is an open delegation (backend work) while the model is quiet,
//   from `session.delegation.created` (or a nested `response.created`) until
//   its nested terminal Responses event, the `session.commentary.appended` that
//   delivers a client delegation's result, or 30 s without news. That
//   acknowledgment reaches the client even when your server sent the commentary
//   on its sideband, but carries no `delegation_id` (seen live, 2026-09-30), so
//   it closes the oldest open client delegation. `session.thinking.appended` is
//   quiet progress and keeps thinking;
// - a barge-in is user speech (`session.input_transcript.delta`) while the
//   model is audible, followed by the model going quiet within 1 s of the user's
//   latest words -- GPT-Live is full duplex, so a "mhm" under continuing speech
//   is not one, and the model may talk on for a while before it stops.
import type { AgentState, TranscriptUpdate } from "@sinua/core";
import { isFatalRealtimeError } from "./realtimeReconnect.js";
import { TranscriptAssembler } from "./transcript.js";

export const LIVE_SPEAKING_LEVEL = 0.05;
// Speaking ends after a quiet tail that grows with how long the agent has been speaking
// (design note 31, V7; telephony's variable hangover): a short reply hands back quickly, a
// long answer survives the pauses people leave. In 30 Hz frames: the base, plus so many per
// second of the speaking stretch so far, up to the max. After ~300 ms when the user just
// spoke (a barge-in stays instant).
export const LIVE_SPEAKING_TAIL_FRAMES = 21;
export const LIVE_SPEAKING_TAIL_PER_SECOND = 4;
export const LIVE_SPEAKING_TAIL_MAX_FRAMES = 51;
export const LIVE_BARGE_IN_TAIL_FRAMES = 9;
export const LIVE_BARGE_IN_WINDOW_MS = 1000;
export const LIVE_DELEGATION_TIMEOUT_MS = 30_000;

/** The quiet frames that end a speaking stretch `ms` long (see the constants above). */
export function liveSpeakingTail(ms: number): number {
  return Math.min(LIVE_SPEAKING_TAIL_MAX_FRAMES, Math.max(LIVE_SPEAKING_TAIL_FRAMES, Math.round(LIVE_SPEAKING_TAIL_FRAMES + (LIVE_SPEAKING_TAIL_PER_SECOND * ms) / 1000)));
}

const num = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) ? v : undefined);

const TERMINAL_RESPONSE_EVENTS = new Set(["response.completed", "response.failed", "response.incomplete", "response.cancelled"]);

type Json = Record<string, unknown>;

export class OpenAILiveSession {
  state: AgentState = "idle";
  /** `session.started`'s `session.id`, once seen. */
  sessionId: string | null = null;
  /** A fatal `error.code` seen this session (e.g. `insufficient_quota`): don't reconnect. */
  fatalCode: string | null = null;
  /** `session.closed`'s `reason`, once seen. */
  closedReason: string | null = null;
  onState: ((s: AgentState) => void) | null = null;
  onInterrupt: (() => void) | null = null;
  onClosed: ((reason: string) => void) | null = null;
  /** The transcript (design note 39): `segments` timing, reveal synced to the audio unless `syncToAudio: false`. */
  readonly transcript: TranscriptAssembler;

  private started = false;
  private quietFrames = 0;
  /** When the current speaking stretch began (ms). */
  private speakingSince = 0;
  /** When user speech was heard over the model's audio; null when not armed. */
  private bargeInAt: number | null = null;
  /** Open delegations, oldest first: id -> last news (ms) and whether the app's backend answers it. */
  private readonly delegations = new Map<string, { at: number; client: boolean }>();

  constructor(opts: { syncToAudio?: boolean } = {}) {
    this.transcript = new TranscriptAssembler("segments", opts.syncToAudio ?? true);
  }

  /** Transcript updates for both speakers (see `TranscriptAssembler`). */
  set onTranscript(cb: ((u: TranscriptUpdate) => void) | null) {
    this.transcript.onUpdate = cb;
  }

  connecting(): void {
    this.reset();
    this.setState("initializing");
  }

  stopped(): void {
    this.reset();
    this.setState("idle");
  }

  get isStarted(): boolean {
    return this.started;
  }

  /** A server event from the `oai-events` data channel, at `now` ms. */
  handle(text: string, now: number): void {
    let ev: Json;
    try {
      const parsed: unknown = JSON.parse(text);
      if (!parsed || typeof parsed !== "object") return;
      ev = parsed as Json;
    } catch {
      return;
    }
    switch (ev.type) {
      case "session.started": {
        const id = (ev.session as Json | undefined)?.id;
        if (typeof id === "string") this.sessionId = id;
        this.started = true;
        this.setState("listening");
        break;
      }
      case "session.input_transcript.delta":
        // The window runs from the user's latest words: in full duplex the model may talk on for
        // a while after the user started, and stop only later (seen live, design note 39).
        if (this.state === "speaking") this.bargeInAt = now;
        if (this.state === "speaking") this.transcript.hold();
        if (typeof ev.delta === "string") this.transcript.userDelta(ev.delta, now, num(ev.start_ms), num(ev.end_ms));
        break;
      case "session.output_transcript.delta":
        if (typeof ev.delta === "string") this.transcript.assistantDelta(ev.delta, now, num(ev.start_ms), num(ev.end_ms));
        break;
      case "session.delegation.created": {
        const d = ev.delegation as Json | undefined;
        if (typeof d?.id === "string") this.open(d.id, now, d.target === "client");
        break;
      }
      case "response.event": {
        const id = ev.delegation_id;
        const inner = (ev.event as Json | undefined)?.type;
        if (typeof id !== "string" || typeof inner !== "string") break;
        if (TERMINAL_RESPONSE_EVENTS.has(inner)) this.close(id);
        else if (inner === "response.created" || this.delegations.has(id)) this.open(id, now, false);
        break;
      }
      case "session.commentary.appended": {
        // The result of a client delegation was delivered: it names its delegation if
        // the server says so, else it's the oldest one the app's backend still owes.
        const id = typeof ev.delegation_id === "string" ? ev.delegation_id : [...this.delegations].find(([, d]) => d.client)?.[0];
        if (id) this.close(id);
        break;
      }
      case "session.closed":
        this.closedReason = typeof ev.reason === "string" ? ev.reason : "unknown";
        this.onClosed?.(this.closedReason);
        break;
      case "error": {
        const code = (ev.error as Json | undefined)?.code;
        if (isFatalRealtimeError(code)) this.fatalCode = code as string;
        break;
      }
      default:
        break;
    }
  }

  /** 30 Hz with the remote track's current level, at `now` ms. */
  tick(level: number, now: number): void {
    if (!this.started) return;
    this.expire(now);
    if (level > LIVE_SPEAKING_LEVEL) {
      this.quietFrames = 0;
      if (this.state !== "speaking") this.speakingSince = now;
      this.setState("speaking");
    } else if (this.state === "speaking") {
      this.quietFrames++;
      const userSpoke = this.bargeInAt != null && now - this.bargeInAt <= LIVE_BARGE_IN_WINDOW_MS;
      if (this.quietFrames >= (userSpoke ? LIVE_BARGE_IN_TAIL_FRAMES : liveSpeakingTail(now - this.speakingSince))) {
        const armed = this.bargeInAt;
        this.bargeInAt = null;
        this.quietFrames = 0;
        const interrupted = armed != null && now - armed <= LIVE_BARGE_IN_WINDOW_MS;
        if (interrupted) this.transcript.cut();
        else this.transcript.speakingEnded();
        this.setState(this.delegations.size > 0 ? "thinking" : "listening");
        if (interrupted) this.onInterrupt?.();
      }
    } else {
      this.settle();
    }
    this.transcript.tick(now, level, this.state === "speaking");
  }

  private open(id: string, now: number, client: boolean): void {
    const known = this.delegations.get(id);
    this.delegations.set(id, { at: now, client: known ? known.client : client });
    this.settle();
  }

  private close(id: string): void {
    if (this.delegations.delete(id)) this.settle();
  }

  private expire(now: number): void {
    for (const [id, d] of this.delegations) {
      if (now - d.at > LIVE_DELEGATION_TIMEOUT_MS) this.delegations.delete(id);
    }
  }

  /** Listening <-> thinking by open delegations; speaking is left alone. */
  private settle(): void {
    if (!this.started || this.state === "speaking") return;
    this.setState(this.delegations.size > 0 ? "thinking" : "listening");
  }

  private reset(): void {
    this.transcript.stop();
    this.started = false;
    this.sessionId = null;
    this.fatalCode = null;
    this.closedReason = null;
    this.quietFrames = 0;
    this.bargeInAt = null;
    this.delegations.clear();
  }

  private setState(s: AgentState): void {
    if (this.state === s) return;
    this.state = s;
    this.onState?.(s);
  }
}

/**
 * The SDP answer from your session endpoint's 2xx body: OpenAI's own
 * `POST /v1/live/sessions` JSON (`{ session: { id }, transport: { sdp } }`),
 * passed through unchanged, or the bare SDP text. Null if it is neither.
 */
export function liveAnswerSdp(body: string): { sdp: string; sessionId: string | null } | null {
  const text = body.trim();
  if (text.startsWith("v=")) return { sdp: body, sessionId: null };
  try {
    const json = JSON.parse(text) as Json;
    const sdp = (json.transport as Json | undefined)?.sdp;
    if (typeof sdp !== "string" || !sdp.trim().startsWith("v=")) return null;
    const id = (json.session as Json | undefined)?.id;
    return { sdp, sessionId: typeof id === "string" ? id : null };
  } catch {
    return null;
  }
}
