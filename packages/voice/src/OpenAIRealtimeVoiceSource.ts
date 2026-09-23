import { AudioAnalysis } from "./analysis.js";
import { insecureCredentialRefusal } from "./insecureCredential.js";
import {
  DEFAULT_RECONNECT_ATTEMPTS,
  FatalConnectError,
  TranscriptLog,
  isFatalConnectError,
  isFatalRealtimeError,
  isRetryableHttpStatus,
  reconnectDelayMs,
  type ReconnectPolicy,
} from "./realtimeReconnect.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * `VoiceSource` for OpenAI's Realtime API over its WebRTC transport -- the
 * first *real* vendor adapter (see docs/audio-pipeline.md's transport
 * table: WebRTC hands us a live remote `MediaStreamTrack`, so the exact
 * `AnalyserNode` -> `AudioAnalysis` path `LocalMicVoiceSource` already uses
 * attaches to the model's voice unchanged; nothing is decoded or scheduled
 * by hand here, unlike the PCM-over-WebSocket shape Gemini Live needs).
 *
 * Handshake, verified against the current (2026) docs and OpenAI's own
 * shipped code rather than recalled -- developers.openai.com
 * `guides/voice-webrtc` + `api-reference/realtime-sessions/create-realtime-
 * client-secret`, `openai/openai-realtime-console` (`App.jsx`, `server.js`),
 * `openai/openai-agents-js` (`openaiRealtimeWebRtc.ts`):
 *   1. an ephemeral client secret (`ek_...`) comes from
 *      `POST /v1/realtime/client_secrets` (needs the real API key);
 *   2. the browser POSTs its SDP offer to `POST /v1/realtime/calls` with
 *      `Authorization: Bearer ek_...` / `Content-Type: application/sdp` and
 *      gets the answer SDP back as text;
 *   3. JSON events flow both ways over a data channel labelled
 *      `"oai-events"`; the model's audio arrives as a normal remote track
 *      (`pc.ontrack`).
 *
 * ## Credentials -- read this before wiring it anywhere real
 *
 * `credential` is either:
 * - an ephemeral key (`ek_...`) minted by *your backend* via
 *   `client_secrets` -- the production shape. The browser never sees a
 *   long-lived key; the `ek_` expires (600s default) and is scoped to one
 *   Realtime session. This is the only path a shipped product should use.
 * - anything else, treated as a raw API key: the adapter then mints the
 *   `ek_` itself, from the browser. **DEV-ONLY DEMO WIRING.** It exists so
 *   the local Studio can be pointed at a real model without a backend
 *   (this project has none). The
 *   key lives in this instance's memory only: never persisted, never
 *   logged, never put in a URL. Do not copy this path into an app.
 *
 * ## State mapping (LiveKit `AgentState` vocabulary, see ./types.ts)
 *
 * Vendor VAD/lifecycle events drive state, not the energy heuristic the
 * mic/test-tone sources fall back on -- OpenAI actually exposes them
 * (`input_audio_buffer.speech_started/stopped` under `server_vad`,
 * `response.created/done`), which the original research flagged as the
 * right default whenever a vendor signal exists. The WebRTC-only
 * `output_audio_buffer.started/stopped/cleared` events say when the
 * model's audio is *audible* (started) and fully drained (stopped), which
 * is exactly the "gate on playback, not on chunk receipt" rule
 * docs/audio-pipeline.md calls for. They are documented inconsistently
 * (present in OpenAI's May-2025 reference addition, Azure's reference, and
 * community threads; absent from one current reference page), so they are
 * treated as optional: if none has been seen this session, `speaking` is
 * entered when remote-track energy rises during a response and left ~300ms
 * after it falls following `response.done`. Same mapping LiveKit's
 * `realtime_model.py` and Pipecat's `openai/realtime/llm.py` use for their
 * user-started/stopped-speaking and TTS-started/stopped frames.
 *
 * ## Reconnect
 *
 * OpenAI Realtime has no session resumption (docs/audio-pipeline.md,
 * *Reconnect*): a dropped call (peer connection `failed`/`closed`, or the
 * data channel closing) is replaced by a **new** session. State goes
 * `initializing` while that happens, and one zeroed metrics reading is sent
 * so visuals go quiet instead of freezing. The mic, `AudioContext` and
 * audio element are kept. Up to `reconnect.maxAttempts` (3) tries, 100 ms
 * then exponential backoff (LiveKit's policy). Each try gets a *fresh*
 * credential: `getCredential()` when given (production: your backend mints
 * a new `ek_`), else a raw dev key re-mints. A pasted `ek_` alone can't be
 * reused (single session, expiring), so without `getCredential` a drop
 * still ends in `idle`. Fatal failures (HTTP 400/401/403, or a fatal
 * `error.code` such as `invalid_api_key`/`insufficient_quota` seen before
 * the drop) give up at once. On success the finalized transcript is
 * replayed as `conversation.item.create` text items so the model keeps its
 * context (assistant turns always; user turns only if the session has input
 * transcription on). Give-up ends in `idle`.
 */
export interface OpenAIRealtimeVoiceSourceOptions {
  /**
   * An `ek_...` ephemeral key (production shape) or, dev-only, a raw API
   * key. May be empty when `getCredential` is given.
   */
  credential?: string;
  /**
   * Opt in to a raw, long-lived API key (which this adapter would mint an
   * `ek_` from, in the browser). Without it `connect()` refuses one before
   * touching the mic or opening a session -- see `insecureCredential.ts`.
   * Local demos only (the Studio sets it); never in a shipped app. It applies
   * to whatever `getCredential` returns too.
   */
  allowInsecureApiKey?: boolean;
  /**
   * Production shape: returns a fresh credential (normally an `ek_` minted
   * by your backend) and is called for **every** connect and reconnect, so
   * an expired key is never reused. Takes precedence over `credential`.
   */
  getCredential?: () => Promise<string>;
  /** Reconnect policy after a drop; `false` restores the old drop-to-`idle` behaviour. */
  reconnect?: ReconnectPolicy | false;
  /** Replay the finalized transcript into a replacement session. Default true. */
  replayTranscript?: boolean;
  /** Realtime model id. `gpt-realtime` is the alias OpenAI's own WebRTC guide uses. */
  model?: string;
  /** Output voice; `marin` is the current default in OpenAI's samples. */
  voice?: string;
  /** Optional system instructions for the session. */
  instructions?: string;
}

const CALLS_URL = "https://api.openai.com/v1/realtime/calls";
const CLIENT_SECRETS_URL = "https://api.openai.com/v1/realtime/client_secrets";
const DATA_CHANNEL_LABEL = "oai-events";
const UPDATE_MS = 1000 / 30; // ~30fps, decoupled from the render loop -- same as LocalMicVoiceSource
const WATCHDOG_ZERO_FRAMES = 30; // ~1s of exact-zero RMS *while the model should be audible*
const SPEAKING_LEVEL = 0.05; // energy floor for the no-output_audio_buffer-events fallback
const SPEAKING_TAIL_FRAMES = 9; // ~300ms below the floor after response.done before leaving `speaking`
const DATA_CHANNEL_OPEN_TIMEOUT_MS = 15_000;

/** Shown when in-browser minting fails, so the user knows the supported alternative. */
const MINT_HINT = [
  "Mint an ephemeral key server-side and paste that (ek_...) instead:",
  'curl -X POST https://api.openai.com/v1/realtime/client_secrets -H "Authorization: Bearer $OPENAI_API_KEY" -H "Content-Type: application/json" -d \'{"session":{"type":"realtime","model":"gpt-realtime"}}\'',
].join("\n");

export class OpenAIRealtimeVoiceSource implements VoiceSource {
  private readonly credential: string;
  private readonly allowInsecureApiKey: boolean | undefined;
  private readonly getCredential: (() => Promise<string>) | undefined;
  private readonly reconnectPolicy: ReconnectPolicy | null;
  private readonly replayTranscript: boolean;
  private readonly transcript = new TranscriptLog();
  private readonly model: string;
  private readonly voice: string;
  private readonly instructions: string | undefined;

  private ctx: AudioContext | null = null;
  private pc: RTCPeerConnection | null = null;
  private dc: RTCDataChannel | null = null;
  private mic: MediaStream | null = null;
  private remote: MediaStream | null = null;
  // Kept referenced (not appended to the DOM) so the remote track has a
  // sink -- Chrome won't pump a remote WebRTC track through Web Audio
  // alone -- and the user actually hears the model. Same as every
  // reference implementation's `audioElement.srcObject = e.streams[0]`.
  private audioEl: HTMLAudioElement | null = null;
  private sourceNode: MediaStreamAudioSourceNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private interruptCb: (() => void) | null = null;

  private state: AgentState = "idle";
  private connected = false;
  /** False once the caller disconnects (or we give up): every async step re-checks it. */
  private wantConnected = false;
  private reconnecting = false;
  /** A fatal `error.code` seen on this session; a drop after it isn't retried. */
  private fatalCode: string | null = null;
  private lastBandCount = 0;
  private zeroStreak = 0;
  private quietFrames = 0;
  /** True between `response.created` and `response.done`. */
  private responseActive = false;
  /** Once any `output_audio_buffer.*` event arrives, the energy fallback stands down. */
  private sawOutputBufferEvents = false;

  constructor(opts: OpenAIRealtimeVoiceSourceOptions) {
    this.credential = (opts.credential ?? "").trim();
    this.allowInsecureApiKey = opts.allowInsecureApiKey;
    this.getCredential = opts.getCredential;
    this.reconnectPolicy = opts.reconnect === false ? null : opts.reconnect ?? {};
    this.replayTranscript = opts.replayTranscript ?? true;
    this.model = opts.model ?? "gpt-realtime";
    this.voice = opts.voice ?? "marin";
    this.instructions = opts.instructions;
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
    if (!this.credential && !this.getCredential) throw new Error("OpenAIRealtimeVoiceSource: a credential is required");
    this.wantConnected = true;
    this.fatalCode = null;
    this.transcript.clear();
    this.setState("initializing");
    try {
      const key = await this.resolveKey();
      await this.acquireLocal();
      await this.openSession(key);
      this.connected = true;
      this.setState("listening");
      this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
    } catch (err) {
      this.wantConnected = false;
      this.teardown();
      this.setState("idle");
      throw err;
    }
  }

  /** A fresh key for this (re)connect: the caller's callback, a pasted `ek_`, or a dev-only mint. */
  private async resolveKey(): Promise<string> {
    const credential = this.getCredential ? (await this.getCredential()).trim() : this.credential;
    if (!credential) throw new FatalConnectError("OpenAIRealtimeVoiceSource: getCredential() returned an empty credential");
    // Runs on every (re)connect, and on whatever `getCredential` returns, so a
    // backend that starts handing out raw keys is caught too. Fatal by
    // construction: a refused credential is never worth retrying.
    const refusal = insecureCredentialRefusal({
      vendor: "OpenAIRealtimeVoiceSource",
      isEphemeral: credential.startsWith("ek_"),
      allowInsecureApiKey: this.allowInsecureApiKey,
      ephemeralShape: "ek_…",
      mintHint: "POST https://api.openai.com/v1/realtime/client_secrets",
    });
    if (refusal) throw new FatalConnectError(refusal);
    return credential.startsWith("ek_") ? credential : this.mintClientSecret(credential);
  }

  /** Mic, AudioContext -- kept across reconnects. */
  private async acquireLocal(): Promise<void> {
    this.mic = await navigator.mediaDevices.getUserMedia({ audio: true });
    this.ctx = new (globalThis.AudioContext ||
      (globalThis as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
    // The user just clicked "connect", so resuming under an autoplay
    // policy is allowed here; a suspended context would read all zeros.
    await this.ctx.resume().catch(() => undefined);
  }

  /** One Realtime call: a new peer connection + data channel over the kept mic track. */
  private async openSession(key: string): Promise<void> {
    const mic = this.mic;
    if (!mic) throw new Error("OpenAIRealtimeVoiceSource: no microphone stream");
    const pc = new RTCPeerConnection();
    this.pc = pc;
    let openTimer: ReturnType<typeof setTimeout> | null = null;
    try {
      pc.ontrack = (e) => {
        if (this.pc === pc) this.attachRemote(e.streams[0]);
      };
      pc.onconnectionstatechange = () => {
        // "disconnected" can be transient in WebRTC; only the terminal
        // states count as a drop.
        if (this.pc === pc && (pc.connectionState === "failed" || pc.connectionState === "closed")) {
          this.onDrop(`peer connection ${pc.connectionState}`);
        }
      };
      pc.addTrack(mic.getAudioTracks()[0]);

      const dc = pc.createDataChannel(DATA_CHANNEL_LABEL);
      this.dc = dc;
      dc.onmessage = (e) => this.onServerEvent(String(e.data));
      dc.onclose = () => {
        if (this.dc === dc) this.onDrop("data channel closed");
      };
      // Armed before the remote description lands: the channel can open
      // the moment ICE completes, which may be before the fetch resolves.
      const opened = new Promise<void>((resolve, reject) => {
        openTimer = setTimeout(
          () => reject(new Error("OpenAI Realtime data channel did not open within 15s")),
          DATA_CHANNEL_OPEN_TIMEOUT_MS
        );
        dc.onopen = () => {
          if (openTimer != null) clearTimeout(openTimer);
          resolve();
        };
      });
      // If the SDP exchange below throws first, nobody awaits `opened`;
      // don't let its timeout surface later as an unhandled rejection.
      opened.catch(() => undefined);

      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      const res = await fetch(`${CALLS_URL}?model=${encodeURIComponent(this.model)}`, {
        method: "POST",
        body: offer.sdp,
        headers: {
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/sdp",
        },
      });
      if (!res.ok) {
        const message = `OpenAI ${CALLS_URL} returned ${res.status}: ${await safeText(res)}`;
        throw isRetryableHttpStatus(res.status) ? new Error(message) : new FatalConnectError(message);
      }
      await pc.setRemoteDescription({ type: "answer", sdp: await res.text() });
      await opened;
    } catch (err) {
      if (openTimer != null) clearTimeout(openTimer);
      if (this.pc === pc) this.closeSession();
      throw err;
    }
  }

  private onDrop(reason: string): void {
    if (!this.connected || !this.wantConnected || this.reconnecting) return;
    void this.reconnect(reason);
  }

  private async reconnect(reason: string): Promise<void> {
    this.reconnecting = true;
    this.connected = false;
    this.closeSession();
    // One zeroed reading, same band count, so visuals settle instead of
    // freezing on the last frame of the dropped call.
    this.metricsCb?.({ level: 0, bands: new Array<number>(this.lastBandCount).fill(0) });
    this.setState("initializing");

    const policy = this.reconnectPolicy;
    const hasFreshCredential = !!this.getCredential || !this.credential.startsWith("ek_");
    if (!policy || !hasFreshCredential || isFatalRealtimeError(this.fatalCode)) {
      console.warn(
        `OpenAI Realtime ${reason}; not reconnecting (${
          !policy ? "reconnect disabled" : !hasFreshCredential ? "a pasted ek_ can't be reused -- pass getCredential" : `fatal error ${this.fatalCode}`
        })`
      );
      this.giveUp();
      return;
    }

    const attempts = policy.maxAttempts ?? DEFAULT_RECONNECT_ATTEMPTS;
    for (let attempt = 1; attempt <= attempts; attempt++) {
      await new Promise((r) => setTimeout(r, reconnectDelayMs(attempt, policy)));
      if (!this.wantConnected) return;
      try {
        const key = await this.resolveKey();
        if (!this.wantConnected) return;
        await this.openSession(key);
        if (!this.wantConnected) {
          this.closeSession();
          return;
        }
        this.connected = true;
        this.reconnecting = false;
        this.setState("listening");
        if (this.replayTranscript) {
          for (const ev of this.transcript.replayEvents()) this.dc?.send(JSON.stringify(ev));
        }
        return;
      } catch (err) {
        if (!this.wantConnected) return;
        console.warn(`OpenAI Realtime reconnect ${attempt}/${attempts} after ${reason} failed:`, err);
        if (isFatalConnectError(err)) break;
      }
    }
    console.error("OpenAI Realtime: gave up reconnecting");
    this.giveUp();
  }

  private giveUp(): void {
    this.reconnecting = false;
    this.wantConnected = false;
    this.teardown();
    this.setState("idle");
  }

  disconnect(): void {
    this.wantConnected = false;
    this.reconnecting = false;
    this.teardown();
    this.setState("idle");
  }

  /**
   * DEV-ONLY: mints the ephemeral key from the browser with a raw API key.
   * The session config goes here (not via a later `session.update`) so a
   * backend-minted `ek_` -- whose config that backend owns -- takes the
   * exact same connect path with nothing overridden. `server_vad` is
   * requested explicitly because the reference documents
   * `speech_started/stopped` as emitted "in `server_vad` mode".
   */
  private async mintClientSecret(apiKey: string): Promise<string> {
    const session: Record<string, unknown> = {
      type: "realtime",
      model: this.model,
      audio: {
        input: { turn_detection: { type: "server_vad" } },
        output: { voice: this.voice },
      },
    };
    if (this.instructions) session.instructions = this.instructions;

    let res: Response;
    try {
      res = await fetch(CLIENT_SECRETS_URL, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${apiKey}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          expires_after: { anchor: "created_at", seconds: 600 },
          session,
        }),
      });
    } catch (err) {
      throw new Error(
        `Could not reach ${CLIENT_SECRETS_URL} from the browser (${describe(err)}).\n${MINT_HINT}`
      );
    }
    if (!res.ok) {
      const message = `OpenAI client_secrets returned ${res.status}: ${await safeText(res)}\n${MINT_HINT}`;
      throw isRetryableHttpStatus(res.status) ? new Error(message) : new FatalConnectError(message);
    }
    const data = (await res.json()) as { value?: unknown };
    if (typeof data.value !== "string" || !data.value) {
      throw new Error("OpenAI client_secrets response had no `value` field");
    }
    return data.value;
  }

  private attachRemote(stream: MediaStream | undefined): void {
    if (!stream) return;
    this.remote = stream;
    if (!this.audioEl) {
      const el = document.createElement("audio");
      el.autoplay = true;
      this.audioEl = el;
    }
    this.audioEl.srcObject = stream;
    void this.audioEl.play().catch(() => undefined);
    this.attachAnalyser();
  }

  /** Same graph as `LocalMicVoiceSource.attach`, on the remote track instead of the mic. */
  private attachAnalyser(): void {
    if (!this.ctx || !this.remote) return;
    this.sourceNode?.disconnect();
    const analyser = this.ctx.createAnalyser();
    analyser.fftSize = 512; // see LocalMicVoiceSource for why not 256
    analyser.smoothingTimeConstant = 0; // AudioAnalysis does its own attack/release
    const source = this.ctx.createMediaStreamSource(this.remote);
    source.connect(analyser);
    this.sourceNode = source;
    this.analysis = new AudioAnalysis(analyser);
    this.zeroStreak = 0;
  }

  private onServerEvent(raw: string): void {
    let ev: { type?: string; error?: { code?: unknown }; transcript?: unknown };
    try {
      ev = JSON.parse(raw) as typeof ev;
    } catch {
      return;
    }
    switch (ev.type) {
      case "input_audio_buffer.speech_started":
        // The user is talking. If we were `speaking`, this is the barge-in
        // moment -- WebRTC sessions auto-truncate and follow up with
        // `output_audio_buffer.cleared`, handled below. Only a barge-in over
        // audible output is an interrupt; speech over `thinking` isn't.
        if (this.state === "speaking") this.interruptCb?.();
        this.setState("listening");
        break;
      case "input_audio_buffer.speech_stopped":
        this.setState("thinking");
        break;
      case "response.created":
        this.responseActive = true;
        this.quietFrames = 0;
        this.setState("thinking");
        break;
      case "output_audio_buffer.started":
        this.sawOutputBufferEvents = true;
        this.setState("speaking");
        break;
      case "output_audio_buffer.stopped":
      case "output_audio_buffer.cleared":
        this.sawOutputBufferEvents = true;
        this.responseActive = false;
        this.setState("listening");
        break;
      case "response.done":
        this.responseActive = false;
        if (this.state === "thinking") {
          // A response that produced no audio (text-only, empty, errored)
          // never gets an output_audio_buffer.stopped -- don't hang here.
          this.setState("listening");
        }
        // `speaking`: either output_audio_buffer.stopped will follow, or
        // tick()'s energy tail ends it if those events aren't available.
        break;
      case "response.output_audio_transcript.done":
        this.transcript.add("assistant", ev.transcript);
        break;
      case "conversation.item.input_audio_transcription.completed":
        // Only arrives when the session has input transcription enabled.
        this.transcript.add("user", ev.transcript);
        break;
      case "error":
        console.error("OpenAI Realtime error event:", ev.error);
        if (isFatalRealtimeError(ev.error?.code)) this.fatalCode = ev.error?.code as string;
        break;
      default:
        break;
    }
  }

  private tick(): void {
    if (!this.analysis) return; // remote track not attached yet

    // Watchdog (see LocalMicVoiceSource): a live remote WebRTC track's
    // AnalyserNode can silently go flat -- the documented Chrome issue is
    // specifically about WebRTC tracks, so this adapter is where it
    // matters most. Only counted while the model *should* be audible; a
    // remote track legitimately reads exact zeros during silence, and
    // re-attaching every second through every quiet stretch would be churn
    // for nothing.
    const track = this.remote?.getAudioTracks()[0];
    if (this.responseActive && this.analysis.rawRms() === 0) {
      this.zeroStreak++;
      if (this.zeroStreak > WATCHDOG_ZERO_FRAMES && track?.readyState === "live") {
        this.attachAnalyser();
        return;
      }
    } else {
      this.zeroStreak = 0;
    }

    const metrics = this.analysis.read();
    this.lastBandCount = metrics.bands.length;
    this.metricsCb?.(metrics);

    // Energy floor / tail -- only load-bearing when the WebRTC-only
    // output_audio_buffer.* events haven't shown up (see the class doc).
    if (metrics.level > SPEAKING_LEVEL) {
      this.quietFrames = 0;
      if (this.state === "thinking" && this.responseActive && !this.sawOutputBufferEvents) {
        this.setState("speaking");
      }
    } else if (this.state === "speaking" && !this.sawOutputBufferEvents && !this.responseActive) {
      this.quietFrames++;
      if (this.quietFrames >= SPEAKING_TAIL_FRAMES) this.setState("listening");
    }
  }

  private setState(s: AgentState): void {
    if (this.state === s) return;
    this.state = s;
    this.stateCb?.(s);
  }

  /** Closes the current call only (peer connection, data channel, analyser); keeps mic/context/element. */
  private closeSession(): void {
    const dc = this.dc;
    this.dc = null;
    if (dc) {
      dc.onmessage = null;
      dc.onclose = null;
      dc.onopen = null;
      try {
        dc.close();
      } catch {
        /* already closed */
      }
    }
    const pc = this.pc;
    this.pc = null;
    if (pc) {
      pc.ontrack = null;
      pc.onconnectionstatechange = null;
      pc.close();
    }
    this.sourceNode?.disconnect();
    this.sourceNode = null;
    this.analysis = null;
    if (this.audioEl) this.audioEl.srcObject = null;
    this.remote = null;
    this.responseActive = false;
    this.sawOutputBufferEvents = false;
    this.quietFrames = 0;
    this.zeroStreak = 0;
  }

  private teardown(): void {
    this.connected = false;
    if (this.intervalId != null) clearInterval(this.intervalId);
    this.intervalId = null;
    this.closeSession();
    this.mic?.getTracks().forEach((t) => t.stop());
    this.mic = null;
    if (this.audioEl) {
      this.audioEl.pause();
      this.audioEl.srcObject = null;
      this.audioEl = null;
    }
    void this.ctx?.close().catch(() => undefined);
    this.ctx = null;
  }
}

async function safeText(res: Response): Promise<string> {
  try {
    return (await res.text()).slice(0, 500);
  } catch {
    return "<no body>";
  }
}

function describe(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
