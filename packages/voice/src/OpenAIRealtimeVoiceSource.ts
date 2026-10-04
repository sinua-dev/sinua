import { AudioAnalysis } from "./analysis.js";
import { canRefreshCredential, resolveCredential, type CredentialOptions, type CredentialProvider } from "./credential.js";
import { openAICredentialRefusal } from "./insecureCredential.js";
import {
  DEFAULT_RECONNECT_ATTEMPTS,
  FatalConnectError,
  TranscriptLog,
  isFatalConnectError,
  isFatalRealtimeError,
  httpError,
  reconnectDelayMs,
  resolveHeaders,
  type ReconnectPolicy,
  type RequestHeaders,
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
 * Only an ephemeral key (`ek_...`) is accepted, minted by *your backend*
 * via `client_secrets` (`@sinua/voice/server`'s `mintOpenAIRealtimeCredential`,
 * or `npx @sinua/voice dev-proxy` locally). The browser never sees a
 * long-lived key; the `ek_` expires (600s default) and is scoped to one
 * Realtime session, whose model, voice and instructions the backend set when
 * it minted it. Pass `credentialUrl` or a `credential` provider so every
 * connect and reconnect gets a fresh one; a raw API key is refused.
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
 * credential from `credentialUrl` or the `credential` provider (your backend
 * mints a new `ek_`). A pasted `ek_` alone can't be reused (single session,
 * expiring), so without a provider a drop still ends in `idle`. Fatal failures (HTTP 400/401/403, or a fatal
 * `error.code` such as `invalid_api_key`/`insufficient_quota` seen before
 * the drop) give up at once. On success the finalized transcript is
 * replayed as `conversation.item.create` text items so the model keeps its
 * context (assistant turns always; user turns only if the session has input
 * transcription on). Give-up ends in `idle`.
 */
export interface OpenAIRealtimeVoiceSourceOptions extends CredentialOptions {
  /** @deprecated Pass the same function as `credential`. */
  getCredential?: CredentialProvider;
  /** Reconnect policy after a drop; `false` restores the old drop-to-`idle` behaviour. */
  reconnect?: ReconnectPolicy | false;
  /**
   * Extra headers on the request to `callsUrl` (for your own endpoint: a CSRF token, your
   * auth); a function is called per request. Never logged or put in an error.
   */
  headers?: RequestHeaders;
  /** The `fetch` to use (a wrapped one, a test's). Default the global `fetch`. */
  fetch?: typeof fetch;
  /** Replay the finalized transcript into a replacement session. Default true. */
  replayTranscript?: boolean;
  /** Realtime model id. `gpt-realtime` is the alias OpenAI's own WebRTC guide uses. */
  model?: string;
  /**
   * Where the SDP offer goes. Default OpenAI's `https://api.openai.com/v1/realtime/calls`,
   * with an `ek_` from your backend. Point it at your own endpoint to open the OpenAI
   * session server-side (with your key, tools and transcripts on OpenAI's sideband
   * connection): the credential is then your own short-lived token, any shape except a
   * raw `sk-…` key. The answer must be the SDP text with a 2xx status.
   */
  callsUrl?: string;
  /**
   * WARP (developers.openai.com `guides/realtime-webrtc-warp`): the event channel is
   * pre-negotiated (`negotiated: true, id: 1`) and its id goes along as `dcid=1`, saving a
   * round trip at startup. A page can't set WebRTC field trials, so this is the part a
   * browser can do (Chrome has DTLS 1.3 on by default; SNAP needs your origin-trial
   * token). A `callsUrl` backend must forward `dcid` to OpenAI unchanged. Default false.
   */
  warp?: boolean;
  /**
   * @deprecated Ignored: the voice is fixed when your backend mints the `ek_`
   * (`mintOpenAIRealtimeCredential({ voice })`).
   */
  voice?: string;
  /**
   * @deprecated Ignored: instructions are fixed when your backend mints the
   * `ek_` (`mintOpenAIRealtimeCredential({ instructions })`).
   */
  instructions?: string;
}

const CALLS_URL = "https://api.openai.com/v1/realtime/calls";
const DATA_CHANNEL_LABEL = "oai-events";
/** WARP's pre-negotiated event channel id (any free id works; the same one goes in `dcid`). */
export const WARP_DATA_CHANNEL_ID = 1;
const UPDATE_MS = 1000 / 30; // ~30fps, decoupled from the render loop -- same as LocalMicVoiceSource
const WATCHDOG_ZERO_FRAMES = 30; // ~1s of exact-zero RMS *while the model should be audible*
const SPEAKING_LEVEL = 0.05; // energy floor for the no-output_audio_buffer-events fallback
const SPEAKING_TAIL_FRAMES = 30; // ~1 s below the floor after response.done before leaving `speaking` (design note 30, V3)
const DATA_CHANNEL_OPEN_TIMEOUT_MS = 15_000;

export class OpenAIRealtimeVoiceSource implements VoiceSource {
  private readonly credentials: CredentialOptions;
  private readonly reconnectPolicy: ReconnectPolicy | null;
  private readonly replayTranscript: boolean;
  private readonly headers: RequestHeaders | undefined;
  private readonly fetchFn: typeof fetch | undefined;
  private readonly transcript = new TranscriptLog();
  private readonly model: string;
  private readonly callsUrl: string;
  private readonly warp: boolean;

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
  private connectionCb: ((connected: boolean) => void) | null = null;
  /** What `onConnectionChange` last said: true from a successful connect through reconnects, until disconnect or giving up. */
  private sessionUp = false;
  private muted = false;

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
    this.credentials = {
      credential: opts.getCredential ?? opts.credential,
      credentialUrl: opts.credentialUrl,
    };
    if (opts.voice !== undefined || opts.instructions !== undefined) {
      console.warn(
        "OpenAIRealtimeVoiceSource: `voice` and `instructions` are ignored -- the session's config is fixed when your " +
          "backend mints the ek_ (mintOpenAIRealtimeCredential from @sinua/voice/server).",
      );
    }
    this.reconnectPolicy = opts.reconnect === false ? null : opts.reconnect ?? {};
    this.headers = opts.headers;
    this.fetchFn = opts.fetch;
    this.replayTranscript = opts.replayTranscript ?? true;
    this.model = opts.model ?? "gpt-realtime";
    this.callsUrl = opts.callsUrl ?? CALLS_URL;
    this.warp = opts.warp ?? false;
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

  onConnectionChange(cb: (connected: boolean) => void): void {
    this.connectionCb = cb;
  }

  /**
   * Muted, the mic track is disabled: WebRTC sends silence and the call stays up.
   * The mic is kept across reconnects, so the mute holds through them.
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    for (const t of this.mic?.getAudioTracks() ?? []) t.enabled = !muted;
  }

  private setSessionUp(up: boolean): void {
    if (up === this.sessionUp) return;
    this.sessionUp = up;
    this.connectionCb?.(up);
  }

  async connect(): Promise<void> {
    if (!this.credentials.credential && !this.credentials.credentialUrl) {
      throw new Error("OpenAIRealtimeVoiceSource: a credential or credentialUrl is required");
    }
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
      this.setSessionUp(true);
    } catch (err) {
      this.wantConnected = false;
      this.teardown();
      this.setState("idle");
      throw err;
    }
  }

  /** A fresh `ek_` for this (re)connect, from `credentialUrl`, the provider or a pasted value. */
  private async resolveKey(): Promise<string> {
    const { credential } = await resolveCredential("OpenAIRealtimeVoiceSource", this.credentials);
    // Runs on every (re)connect, and on whatever the provider returns, so a
    // backend that starts handing out raw keys is caught too. Fatal by
    // construction: a refused credential is never worth retrying.
    const refusal = openAICredentialRefusal(credential, this.callsUrl);
    if (refusal) throw new FatalConnectError(refusal);
    return credential;
  }

  /** Mic, AudioContext -- kept across reconnects. */
  private async acquireLocal(): Promise<void> {
    this.mic = await navigator.mediaDevices.getUserMedia({ audio: true });
    this.setMuted(this.muted);
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

      const dc = pc.createDataChannel(
        DATA_CHANNEL_LABEL,
        this.warp ? { negotiated: true, id: WARP_DATA_CHANNEL_ID } : undefined
      );
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
      const url = new URL(this.callsUrl, (globalThis as { location?: { href?: string } }).location?.href);
      url.searchParams.set("model", this.model);
      if (this.warp) url.searchParams.set("dcid", String(WARP_DATA_CHANNEL_ID));
      const res = await (this.fetchFn ?? fetch)(url.href, {
        method: "POST",
        body: offer.sdp,
        headers: {
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/sdp",
          ...(await resolveHeaders(this.headers)),
        },
      });
      if (!res.ok) throw httpError("OpenAI", this.callsUrl, res.status, await safeText(res));
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
    const hasFreshCredential = canRefreshCredential(this.credentials);
    if (!policy || !hasFreshCredential || isFatalRealtimeError(this.fatalCode)) {
      console.warn(
        `OpenAI Realtime ${reason}; not reconnecting (${
          !policy ? "reconnect disabled" : !hasFreshCredential ? "a pasted ek_ can't be reused -- pass credentialUrl or a credential provider" : `fatal error ${this.fatalCode}`
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
    this.setSessionUp(false);
  }

  disconnect(): void {
    this.wantConnected = false;
    this.reconnecting = false;
    this.teardown();
    this.setState("idle");
    this.setSessionUp(false);
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
