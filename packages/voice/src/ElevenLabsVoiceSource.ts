import { PcmAudioGraph } from "./PcmAudioGraph.js";
import { base64ToBytes, bytesToBase64, float32ToPcm16, parseAudioFormat, pcm16ToFloat32, ulawToFloat32 } from "./pcm.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * `VoiceSource` for ElevenLabs Conversational AI (the "Agents platform")
 * over its raw WebSocket. Same transport *shape* as Gemini Live (PCM over a
 * socket, caller decodes and schedules playback -- hence the shared
 * `PcmAudioGraph`), different protocol: its own message vocabulary, a
 * `ping`/`pong` keep-alive, `event_id`-based interruption, audio formats
 * negotiated per agent (PCM at several rates, or μ-law), and the reason it
 * earns its own adapter: a server-side **`vad_score`** event -- the
 * textbook case for this project's rule of preferring a vendor's own VAD
 * over an energy heuristic.
 *
 * Wire protocol, verified against the current docs and ElevenLabs' own
 * shipped SDK (`elevenlabs/packages`, `packages/client/src/`), not recalled
 * -- from the vendor's public docs:
 *   - `wss://api.elevenlabs.io/v1/convai/conversation?agent_id=…`, opened
 *     with the `"convai"` subprotocol exactly as the SDK does; a private
 *     agent uses a signed URL instead (see *Credentials*);
 *   - first client message `{"type":"conversation_initiation_client_data"}`;
 *     first server message `conversation_initiation_metadata` carries
 *     `agent_output_audio_format` / `user_input_audio_format` (e.g.
 *     `pcm_44100` / `pcm_16000`) -- the graph is built only after it;
 *   - mic: `{"user_audio_chunk": <base64 PCM16 at the input rate>}`;
 *   - agent audio: `audio` (`audio_event.audio_base_64`, `event_id`);
 *     `interruption` (`interruption_event.event_id`), `ping`
 *     (`ping_event.event_id`) -> reply `{"type":"pong","event_id"}`,
 *     `vad_score` (`vad_score_event.vad_score`, 0..1, on by default),
 *     `user_transcript` (finalized user utterance), `agent_response`.
 *
 * ## State mapping (LiveKit `AgentState` vocabulary, ./types.ts)
 *
 * - `vad_score` above 0.5 -> the user is talking -> `listening`; a fall back
 *   below 0.5 with nothing queued -> `thinking` (the "user stopped, agent
 *   working" moment OpenAI's `speech_stopped` gives and Gemini cannot).
 *   0.5 is this adapter's threshold on the documented probability, not a
 *   vendor constant.
 * - `user_transcript` is the *finalized* utterance, i.e. the user has
 *   stopped -> `thinking` unless the agent is already audible.
 * - a chunk received but not yet audible -> `thinking`; audible -> `speaking`
 *   (the `PcmAudioGraph` gate). The reply's end is the server's signal:
 *   `agent_response_complete` (or an `audio` chunk with `is_final`). Drained
 *   *after* it -> `listening`; drained *before* it (a network stall mid-reply)
 *   -> `thinking`. A reply with no audio (text-only / empty) ends `thinking` at
 *   once. Chunks trailing a `complete` don't reopen the reply; the user's next
 *   turn (speech, transcript, barge-in) does.
 * - `interruption` -> clear playback (30 ms fade) -> `listening`; audio
 *   chunks whose `event_id` is below the interruption's are dropped, the
 *   SDK's own rule for late chunks of the cut-off response.
 * - while a reply is open, 10 s without audio falls back to `listening` (a lost
 *   `complete`);
 * - a `thinking` that produces no audio for 4 s falls back to `listening`
 *   (a guard against VAD flicker on noise, not vendor semantics).
 * - close code 1000 = the agent ended the conversation -> `idle`; any other
 *   close -> `idle` with the reason logged. No auto-reconnect: an ElevenLabs
 *   conversation is not resumable.
 *
 * ## Credentials
 *
 * - `agent_id` (anything not starting with `wss://`) -> a **public** agent.
 *   No secret is involved at all: the API key never appears anywhere, so
 *   this path needs no dev-only caveat.
 * - `wss://…` -> a **signed URL** for a private agent, minted by *your
 *   backend* (`GET /v1/convai/conversation/get-signed-url?agent_id=…` with
 *   the `xi-api-key` header -> `{ signed_url }`, valid 15 minutes; an open
 *   socket outlives it). The production shape for private agents.
 */
export interface ElevenLabsVoiceSourceOptions {
  /** A public agent's `agent_id`, or a signed `wss://` URL for a private agent. */
  credential: string;
  /** Optional `conversation_config_override` payload (agent prompt/voice overrides, if the agent allows them). */
  overrides?: Record<string, unknown>;
}

const WS_URL = "wss://api.elevenlabs.io/v1/convai/conversation";
const SUBPROTOCOL = "convai";
const VAD_THRESHOLD = 0.5;
const UPDATE_MS = 1000 / 30; // ~30fps metering, same as every other VoiceSource
const METADATA_TIMEOUT_MS = 15_000;
const THINKING_TIMEOUT_MS = 4_000;
const STALL_TIMEOUT_MS = 10_000; // a reply that went quiet without agent_response_complete

interface ServerEvent {
  type?: string;
  conversation_initiation_metadata_event?: {
    conversation_id?: string;
    agent_output_audio_format?: string;
    user_input_audio_format?: string;
  };
  audio_event?: { audio_base_64?: string; event_id?: number; is_final?: boolean };
  interruption_event?: { event_id?: number };
  ping_event?: { event_id?: number; ping_ms?: number };
  vad_score_event?: { vad_score?: number };
  user_transcription_event?: { user_transcript?: string };
}

export class ElevenLabsVoiceSource implements VoiceSource {
  private readonly credential: string;
  private readonly overrides: Record<string, unknown> | undefined;

  private readonly graph = new PcmAudioGraph();
  private graphReady = false;
  /** Audio that arrived between the metadata and the graph coming up. */
  private pendingAudio: ServerEvent[] = [];
  private outputCodec: "pcm" | "ulaw" = "pcm";
  private outputRate = 16000;
  private lastInterruptEventId = 0;
  private userSpeaking = false;
  private thinkingSince = 0;
  /** No reply in flight: `agent_response_complete` / a final chunk / an interruption since the last one began. */
  private responseDone = true;
  /** The reply was closed this turn: trailing chunks don't reopen it; the user's next turn does. */
  private closedThisTurn = false;
  private lastAudioAt = 0;

  private ws: WebSocket | null = null;
  private wantConnected = false;
  private connected = false;
  private streaming = false;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private interruptCb: (() => void) | null = null;
  private state: AgentState = "idle";

  constructor(opts: ElevenLabsVoiceSourceOptions) {
    this.credential = opts.credential.trim();
    this.overrides = opts.overrides;
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
    if (!this.credential) throw new Error("ElevenLabsVoiceSource: an agent id or signed URL is required");
    this.wantConnected = true;
    this.setState("initializing");
    try {
      // Permission prompt first, then the socket: the agent may greet the
      // user immediately, and its formats are only known after metadata.
      const mic = await PcmAudioGraph.requestMic();
      const meta = await this.openSocket();
      const input = parseAudioFormat(meta.user_input_audio_format);
      const output = parseAudioFormat(meta.agent_output_audio_format);
      if (input.codec !== "pcm") {
        throw new Error(
          `ElevenLabs agent expects ${meta.user_input_audio_format} input; this adapter sends PCM only (configure the agent for a pcm_* input format)`
        );
      }
      this.outputCodec = output.codec;
      this.outputRate = output.rate;
      await this.graph.start({
        mic,
        inputRate: input.rate,
        outputRate: output.rate,
        onChunk: (samples) => this.onCaptureChunk(samples),
      });
      this.graphReady = true;
      for (const ev of this.pendingAudio) this.enqueueAudio(ev);
      this.pendingAudio = [];
      this.streaming = true;
      this.connected = true;
      if (this.state === "initializing") this.setState("listening");
      this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
    } catch (err) {
      this.teardown();
      this.setState("idle");
      throw err;
    }
  }

  disconnect(): void {
    this.wantConnected = false;
    this.teardown();
    this.setState("idle");
  }

  private buildUrl(): string {
    return this.credential.startsWith("wss://")
      ? this.credential
      : `${WS_URL}?agent_id=${encodeURIComponent(this.credential)}`;
  }

  /** Opens the socket, sends the initiation message, resolves with the metadata event's formats. */
  private openSocket(): Promise<NonNullable<ServerEvent["conversation_initiation_metadata_event"]>> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(this.buildUrl(), [SUBPROTOCOL]);
      ws.binaryType = "arraybuffer"; // synchronous decode, message order preserved
      this.ws = ws;
      let settled = false;
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        reject(new Error("ElevenLabs did not send conversation_initiation_metadata within 15s"));
        ws.close();
      }, METADATA_TIMEOUT_MS);

      ws.onopen = () => {
        const init: Record<string, unknown> = { type: "conversation_initiation_client_data" };
        if (this.overrides) init.conversation_config_override = this.overrides;
        ws.send(JSON.stringify(init));
      };
      ws.onmessage = (e) => {
        const ev = parseServerEvent(e.data);
        if (!ev) return;
        if (ev.type === "conversation_initiation_metadata" && !settled) {
          settled = true;
          clearTimeout(timer);
          resolve(ev.conversation_initiation_metadata_event ?? {});
          return;
        }
        this.onServerEvent(ev);
      };
      ws.onerror = () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        reject(new Error("ElevenLabs WebSocket failed to connect (check the agent id / signed URL and network)"));
      };
      ws.onclose = (ev) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(new Error(`ElevenLabs closed during setup (code ${ev.code}${ev.reason ? `: ${ev.reason}` : ""})`));
          return;
        }
        if (this.ws === ws) this.onSocketClosed(ev);
      };
    });
  }

  private onCaptureChunk(samples: Float32Array): void {
    if (!this.streaming || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify({ user_audio_chunk: bytesToBase64(float32ToPcm16(samples)) }));
  }

  private onServerEvent(ev: ServerEvent): void {
    switch (ev.type) {
      case "ping": {
        // Keep-alive; the SDK answers immediately with the same event_id.
        const id = ev.ping_event?.event_id;
        if (id != null && this.ws?.readyState === WebSocket.OPEN) {
          this.ws.send(JSON.stringify({ type: "pong", event_id: id }));
        }
        break;
      }
      case "audio": {
        // Late chunks of an interrupted response carry an event_id below
        // the interruption's -- dropped, per the SDK's own rule.
        const id = ev.audio_event?.event_id ?? 0;
        if (id < this.lastInterruptEventId) return;
        if (!this.graphReady) {
          this.pendingAudio.push(ev);
          return;
        }
        this.enqueueAudio(ev);
        break;
      }
      case "agent_response":
        // The reply's text: a reply is under way (its audio may still be coming).
        if (!this.closedThisTurn) this.responseDone = false;
        this.lastAudioAt = Date.now();
        break;
      case "agent_response_complete":
        // The server's end of the agent's turn. With nothing left to play (a
        // text-only or empty reply, or audio already drained) it ends now;
        // otherwise tick() ends it when playback drains.
        this.responseDone = true;
        this.closedThisTurn = true;
        if (this.state === "thinking" && this.graph.playbackState() === "drained" && this.pendingAudio.length === 0) {
          this.setState("listening");
        }
        break;
      case "interruption": {
        const id = ev.interruption_event?.event_id;
        if (id != null) this.lastInterruptEventId = id;
        // An interrupt only if there was output to cut (audible, scheduled,
        // or still waiting for the graph).
        if (this.state === "speaking" || this.graph.playbackState() !== "drained" || this.pendingAudio.length > 0) {
          this.interruptCb?.();
        }
        this.graph.clearPlayback(true);
        this.pendingAudio = [];
        this.responseDone = true;
        this.closedThisTurn = false;
        this.setState("listening");
        break;
      }
      case "vad_score": {
        // The vendor VAD: probability the user is speaking, 0..1.
        const speaking = (ev.vad_score_event?.vad_score ?? 0) > VAD_THRESHOLD;
        if (speaking && !this.userSpeaking) {
          this.userSpeaking = true;
          this.closedThisTurn = false;
          if (this.state !== "speaking") this.setState("listening");
        } else if (!speaking && this.userSpeaking) {
          this.userSpeaking = false;
          if (this.state === "listening" && this.graph.playbackState() === "drained") this.setState("thinking");
        }
        break;
      }
      case "user_transcript":
        // Finalized utterance: the user has stopped, the agent is working.
        this.closedThisTurn = false;
        if (this.state !== "speaking") this.setState("thinking");
        break;
      default:
        // agent_response_correction / client_tool_call /
        // internal_tentative_agent_response: no bearing on the visual.
        break;
    }
  }

  private enqueueAudio(ev: ServerEvent): void {
    const b64 = ev.audio_event?.audio_base_64;
    if (!b64) return;
    const bytes = base64ToBytes(b64);
    const samples = this.outputCodec === "ulaw" ? ulawToFloat32(bytes) : pcm16ToFloat32(bytes);
    this.graph.enqueue(samples, this.outputRate);
    this.lastAudioAt = Date.now();
    if (ev.audio_event?.is_final) {
      this.responseDone = true;
      this.closedThisTurn = true;
    } else if (!this.closedThisTurn) {
      this.responseDone = false;
    }
    // Received, not audible yet -- tick() promotes to `speaking` once the
    // playback clock reaches the burst.
    if (this.state !== "speaking") this.setState("thinking");
  }

  private tick(): void {
    const metrics = this.graph.read();
    if (metrics) this.metricsCb?.(metrics);
    if (!this.connected) return;
    const playback = this.graph.playbackState();
    if (playback === "audible") {
      this.setState("speaking");
    } else if (playback === "drained") {
      if (this.state === "speaking") {
        // Drained mid-reply is a stall (more audio is coming), not the end of the turn.
        this.setState(this.responseDone ? "listening" : "thinking");
      } else if (this.state === "thinking" && this.responseDone && Date.now() - this.thinkingSince > THINKING_TIMEOUT_MS) {
        this.setState("listening");
      } else if (this.state === "thinking" && !this.responseDone && Date.now() - this.lastAudioAt > STALL_TIMEOUT_MS) {
        this.responseDone = true;
        this.setState("listening");
      }
    }
  }

  private onSocketClosed(ev: CloseEvent): void {
    if (!this.wantConnected) return;
    if (ev.code === 1000) console.info("ElevenLabs: the agent ended the conversation");
    else console.warn(`ElevenLabs socket closed (code ${ev.code}${ev.reason ? `: ${ev.reason}` : ""})`);
    this.disconnect();
  }

  private setState(s: AgentState): void {
    if (this.state === s) return;
    this.state = s;
    if (s === "thinking") this.thinkingSince = Date.now();
    this.stateCb?.(s);
  }

  private teardown(): void {
    this.connected = false;
    this.streaming = false;
    if (this.intervalId != null) clearInterval(this.intervalId);
    this.intervalId = null;
    if (this.ws) {
      const ws = this.ws;
      this.ws = null;
      ws.onopen = null;
      ws.onmessage = null;
      ws.onerror = null;
      ws.onclose = null;
      try {
        ws.close(1000, "User ended conversation");
      } catch {
        /* already closed */
      }
    }
    this.graph.stop();
    this.graphReady = false;
    this.pendingAudio = [];
    this.lastInterruptEventId = 0;
    this.userSpeaking = false;
    this.responseDone = true;
    this.closedThisTurn = false;
  }
}

function parseServerEvent(data: unknown): ServerEvent | null {
  let text: string;
  if (typeof data === "string") text = data;
  else if (data instanceof ArrayBuffer) text = new TextDecoder().decode(data);
  else return null;
  try {
    return JSON.parse(text) as ServerEvent;
  } catch {
    return null;
  }
}
