import { AudioAnalysis } from "./analysis.js";
import { resolveCredential, type CredentialOptions } from "./credential.js";
import { openAILiveRefusal } from "./insecureCredential.js";
import { OpenAILiveSession, liveAnswerSdp } from "./openaiLive.js";
import {
  DEFAULT_RECONNECT_ATTEMPTS,
  FatalConnectError,
  isFatalConnectError,
  isFatalRealtimeError,
  isRetryableHttpStatus,
  reconnectDelayMs,
  type ReconnectPolicy,
} from "./realtimeReconnect.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * `VoiceSource` for OpenAI's GPT-Live (`gpt-live-1`) over WebRTC
 * (developers.openai.com `guides/voice-webrtc?api=live`, `guides/live-migration`,
 * `guides/live-conversations`; read 2026-09-29).
 *
 * ## Connection -- your server opens the session
 *
 * GPT-Live has no client credential (`ek_`): only your server can open a
 * session, with its project key, via `POST /v1/live/sessions`. So:
 *   1. this source gathers ICE, then POSTs `{ "sdp": "<offer>" }` as JSON to
 *      your `sessionUrl` (with `Authorization: Bearer <credential>` when you
 *      pass one -- your own token, never `sk-…`);
 *   2. your server posts `{ session: {...}, transport: { type: "webrtc", sdp } }`
 *      to OpenAI and answers with OpenAI's 201 JSON unchanged (a bare SDP text
 *      answer works too). `createOpenAILiveSession` in `@sinua/voice/server` does it;
 *   3. events flow over the `oai-events` data channel; the session is live at
 *      `session.started` (the client never sends `session.start`).
 * The model, voice, instructions, delegation and any prior conversation are
 * set by your server when it opens the session -- there is no transcript replay
 * from the client.
 *
 * ## State
 *
 * GPT-Live sends no turn events, so state comes from `OpenAILiveSession`
 * (./openaiLive.ts): the model's audio level drives `speaking`, open backend
 * delegations drive `thinking`, and user speech over the model's audio that
 * stops it is a barge-in.
 *
 * ## Ending and reconnecting
 *
 * `disconnect()` goes `idle` at once, then sends `session.close` and keeps the
 * call until `session.closed` (up to 5 s) so the final usage is confirmed. A
 * `session.closed` for `expired` or `connection_lost`, or a call that drops
 * without one, is replaced by a new session (up to `reconnect.maxAttempts`,
 * a fresh credential each time); `close_requested`, `remote_hangup` and
 * `content` end in `idle`.
 */
export interface OpenAILiveVoiceSourceOptions extends CredentialOptions {
  /**
   * Your endpoint that opens the GPT-Live session (required). A relative path
   * resolves against the page. Never `api.openai.com`: the session needs your
   * project key, which stays on your server.
   */
  sessionUrl: string;
  /** Reconnect policy after a drop or an expired session; `false` ends in `idle` instead. */
  reconnect?: ReconnectPolicy | false;
}

const DATA_CHANNEL_LABEL = "oai-events";
const UPDATE_MS = 1000 / 30;
const WATCHDOG_ZERO_FRAMES = 30;
const ICE_GATHERING_TIMEOUT_MS = 10_000; // OpenAI's own GPT-Live browser sample
const STARTED_TIMEOUT_MS = 15_000;
const CLOSE_TIMEOUT_MS = 5_000;
const RECONNECTING_REASONS = new Set(["expired", "connection_lost"]);

export class OpenAILiveVoiceSource implements VoiceSource {
  private readonly credentials: CredentialOptions;
  private readonly sessionUrl: string;
  private readonly reconnectPolicy: ReconnectPolicy | null;
  private readonly session = new OpenAILiveSession();

  private ctx: AudioContext | null = null;
  private pc: RTCPeerConnection | null = null;
  private dc: RTCDataChannel | null = null;
  private mic: MediaStream | null = null;
  private remote: MediaStream | null = null;
  private audioEl: HTMLAudioElement | null = null;
  private sourceNode: MediaStreamAudioSourceNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private connectionCb: ((connected: boolean) => void) | null = null;
  private sessionUp = false;
  private muted = false;
  private connected = false;
  private wantConnected = false;
  private reconnecting = false;
  private lastBandCount = 0;
  private zeroStreak = 0;
  /** Resolves the wait for `session.started` of the call being opened. */
  private startedWaiter: { resolve: () => void; reject: (e: Error) => void } | null = null;
  /** A graceful close in progress: the timer that gives up on `session.closed`. */
  private closeTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(opts: OpenAILiveVoiceSourceOptions) {
    this.credentials = { credential: opts.credential, credentialUrl: opts.credentialUrl };
    this.sessionUrl = opts.sessionUrl;
    this.reconnectPolicy = opts.reconnect === false ? null : opts.reconnect ?? {};
    this.session.onClosed = (reason) => this.onSessionClosed(reason);
  }

  onMetrics(cb: (m: VoiceMetrics) => void): void {
    this.metricsCb = cb;
  }

  onStateChange(cb: (s: AgentState) => void): void {
    this.session.onState = cb;
  }

  onInterrupt(cb: () => void): void {
    this.session.onInterrupt = cb;
  }

  onConnectionChange(cb: (connected: boolean) => void): void {
    this.connectionCb = cb;
  }

  /** Muted, the mic track is disabled: WebRTC sends silence and the session stays up. */
  setMuted(muted: boolean): void {
    this.muted = muted;
    for (const t of this.mic?.getAudioTracks() ?? []) t.enabled = !muted;
  }

  /** `session.started`'s id for the current call, once live (e.g. for your server's sideband). */
  get sessionId(): string | null {
    return this.session.sessionId;
  }

  private setSessionUp(up: boolean): void {
    if (up === this.sessionUp) return;
    this.sessionUp = up;
    this.connectionCb?.(up);
  }

  async connect(): Promise<void> {
    this.finishClose(); // a graceful close still draining from a previous disconnect()
    const url = resolveUrl(this.sessionUrl);
    const refusal = openAILiveRefusal(url, null);
    if (refusal) throw new FatalConnectError(refusal);
    this.wantConnected = true;
    this.session.connecting();
    try {
      const token = await this.resolveToken();
      await this.acquireLocal();
      await this.openSession(url, token);
      this.connected = true;
      this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
      this.setSessionUp(true);
    } catch (err) {
      this.wantConnected = false;
      this.teardown();
      this.session.stopped();
      throw err;
    }
  }

  /** Your token for this (re)connect, or null when you configured none (cookie auth). */
  private async resolveToken(): Promise<string | null> {
    if (!this.credentials.credential && !this.credentials.credentialUrl) return null;
    const { credential } = await resolveCredential("OpenAILiveVoiceSource", this.credentials);
    const refusal = openAILiveRefusal(resolveUrl(this.sessionUrl), credential);
    if (refusal) throw new FatalConnectError(refusal);
    return credential;
  }

  private async acquireLocal(): Promise<void> {
    this.mic = await navigator.mediaDevices.getUserMedia({ audio: true });
    this.setMuted(this.muted);
    this.ctx = new (globalThis.AudioContext ||
      (globalThis as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
    await this.ctx.resume().catch(() => undefined);
  }

  /** One GPT-Live session: a new peer connection + data channel over the kept mic track. */
  private async openSession(url: string, token: string | null): Promise<void> {
    const mic = this.mic;
    if (!mic) throw new Error("OpenAILiveVoiceSource: no microphone stream");
    const pc = new RTCPeerConnection();
    this.pc = pc;
    let startedTimer: ReturnType<typeof setTimeout> | null = null;
    try {
      pc.ontrack = (e) => {
        if (this.pc === pc) this.attachRemote(e.streams[0]);
      };
      pc.onconnectionstatechange = () => {
        if (this.pc === pc && (pc.connectionState === "failed" || pc.connectionState === "closed")) {
          this.onDrop(`peer connection ${pc.connectionState}`);
        }
      };
      pc.addTrack(mic.getAudioTracks()[0]);

      // Created, with its listener, before the offer (OpenAI's sequence).
      const dc = pc.createDataChannel(DATA_CHANNEL_LABEL);
      this.dc = dc;
      dc.onmessage = (e) => {
        this.session.handle(String(e.data), performance.now());
        if (this.session.isStarted) this.startedWaiter?.resolve();
      };
      dc.onclose = () => {
        if (this.dc === dc) this.onDrop("data channel closed");
      };
      const started = new Promise<void>((resolve, reject) => {
        startedTimer = setTimeout(() => reject(new Error("GPT-Live: no session.started within 15s")), STARTED_TIMEOUT_MS);
        this.startedWaiter = { resolve, reject };
      });
      started.catch(() => undefined);

      await pc.setLocalDescription(await pc.createOffer());
      await iceGatheringComplete(pc);
      const offer = pc.localDescription?.sdp;
      if (!offer) throw new Error("OpenAILiveVoiceSource: no local SDP offer");
      const headers: Record<string, string> = { "Content-Type": "application/json" };
      if (token) headers.Authorization = `Bearer ${token}`;
      const res = await fetch(url, { method: "POST", headers, body: JSON.stringify({ sdp: offer }) });
      const body = await safeText(res);
      if (!res.ok) {
        const message = `OpenAILiveVoiceSource: ${url} returned ${res.status}: ${body.slice(0, 500)}`;
        throw isRetryableHttpStatus(res.status) ? new Error(message) : new FatalConnectError(message);
      }
      const answer = liveAnswerSdp(body);
      if (!answer) {
        throw new FatalConnectError(
          `OpenAILiveVoiceSource: ${url} answered ${res.status} without an SDP answer (expected OpenAI's { session, transport: { sdp } } JSON or the SDP text)`,
        );
      }
      await pc.setRemoteDescription({ type: "answer", sdp: answer.sdp });
      if (!this.session.isStarted) await started;
    } catch (err) {
      if (this.pc === pc) this.closeSession();
      throw err;
    } finally {
      if (startedTimer != null) clearTimeout(startedTimer);
      this.startedWaiter = null;
    }
  }

  /** `session.closed` from the server. */
  private onSessionClosed(reason: string): void {
    this.startedWaiter?.reject(new FatalConnectError(`OpenAILiveVoiceSource: the session closed before it started (${reason})`));
    if (this.closeTimer != null) {
      this.finishClose(); // the answer to our session.close
      return;
    }
    if (!this.connected || !this.wantConnected) return;
    if (RECONNECTING_REASONS.has(reason)) {
      void this.reconnect(`session closed (${reason})`);
    } else {
      console.warn(`GPT-Live session closed (${reason})`);
      this.giveUp();
    }
  }

  private onDrop(reason: string): void {
    if (this.closeTimer != null) {
      this.finishClose();
      return;
    }
    if (!this.connected || !this.wantConnected || this.reconnecting) return;
    void this.reconnect(reason);
  }

  private async reconnect(reason: string): Promise<void> {
    if (this.reconnecting) return;
    this.reconnecting = true;
    this.connected = false;
    const fatal = this.session.fatalCode;
    this.closeSession();
    this.metricsCb?.({ level: 0, bands: new Array<number>(this.lastBandCount).fill(0) });
    this.session.connecting();

    const policy = this.reconnectPolicy;
    if (!policy || isFatalRealtimeError(fatal)) {
      console.warn(`GPT-Live ${reason}; not reconnecting (${!policy ? "reconnect disabled" : `fatal error ${fatal}`})`);
      this.giveUp();
      return;
    }
    const attempts = policy.maxAttempts ?? DEFAULT_RECONNECT_ATTEMPTS;
    const url = resolveUrl(this.sessionUrl);
    for (let attempt = 1; attempt <= attempts; attempt++) {
      await new Promise((r) => setTimeout(r, reconnectDelayMs(attempt, policy)));
      if (!this.wantConnected) return;
      try {
        const token = await this.resolveToken();
        if (!this.wantConnected) return;
        await this.openSession(url, token);
        if (!this.wantConnected) {
          this.closeSession();
          return;
        }
        this.connected = true;
        this.reconnecting = false;
        return;
      } catch (err) {
        if (!this.wantConnected) return;
        console.warn(`GPT-Live reconnect ${attempt}/${attempts} after ${reason} failed:`, err);
        if (isFatalConnectError(err)) break;
      }
    }
    console.error("GPT-Live: gave up reconnecting");
    this.giveUp();
  }

  private giveUp(): void {
    this.reconnecting = false;
    this.wantConnected = false;
    this.teardown();
    this.session.stopped();
    this.setSessionUp(false);
  }

  /**
   * Idle at once; the session itself is closed gracefully: `session.close`,
   * then the call is kept (mic silent, playback stopped) until `session.closed`
   * confirms the final usage, or 5 s pass.
   */
  disconnect(): void {
    this.wantConnected = false;
    this.reconnecting = false;
    const dc = this.dc;
    const graceful = this.connected && this.session.isStarted && dc?.readyState === "open";
    this.connected = false;
    if (this.intervalId != null) clearInterval(this.intervalId);
    this.intervalId = null;
    this.session.stopped();
    this.setSessionUp(false);
    if (!graceful || !dc) {
      this.teardown();
      return;
    }
    for (const t of this.mic?.getAudioTracks() ?? []) t.enabled = false;
    this.audioEl?.pause();
    this.metricsCb?.({ level: 0, bands: new Array<number>(this.lastBandCount).fill(0) });
    try {
      dc.send(JSON.stringify({ type: "session.close" }));
    } catch {
      this.teardown();
      return;
    }
    this.closeTimer = setTimeout(() => {
      console.warn("GPT-Live: no session.closed within 5s; final usage unconfirmed");
      this.finishClose();
    }, CLOSE_TIMEOUT_MS);
  }

  /** Ends a graceful close (answered, timed out, or cut short by a new connect). */
  private finishClose(): void {
    if (this.closeTimer == null) return;
    clearTimeout(this.closeTimer);
    this.closeTimer = null;
    this.teardown();
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

  private attachAnalyser(): void {
    if (!this.ctx || !this.remote) return;
    this.sourceNode?.disconnect();
    const analyser = this.ctx.createAnalyser();
    analyser.fftSize = 512;
    analyser.smoothingTimeConstant = 0;
    const source = this.ctx.createMediaStreamSource(this.remote);
    source.connect(analyser);
    this.sourceNode = source;
    this.analysis = new AudioAnalysis(analyser);
    this.zeroStreak = 0;
  }

  private tick(): void {
    const now = performance.now();
    if (!this.analysis) {
      this.session.tick(0, now); // delegation timeouts still run
      return;
    }
    // Watchdog (see OpenAIRealtimeVoiceSource): only while the model is audible.
    const track = this.remote?.getAudioTracks()[0];
    if (this.session.state === "speaking" && this.analysis.rawRms() === 0) {
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
    this.session.tick(metrics.level, now);
  }

  private closeSession(): void {
    const dc = this.dc;
    this.dc = null;
    if (dc) {
      dc.onmessage = null;
      dc.onclose = null;
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
    this.startedWaiter?.reject(new Error("GPT-Live: the call closed before session.started"));
    this.startedWaiter = null;
    this.sourceNode?.disconnect();
    this.sourceNode = null;
    this.analysis = null;
    if (this.audioEl) this.audioEl.srcObject = null;
    this.remote = null;
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

/** A relative session URL resolves against the page, so the host check sees the real host. */
function resolveUrl(url: string): string {
  try {
    return new URL(url, (globalThis as { location?: { href?: string } }).location?.href).href;
  } catch {
    return url;
  }
}

/** GPT-Live takes one complete offer (no trickle ICE): wait for gathering, up to 10 s. */
function iceGatheringComplete(pc: RTCPeerConnection): Promise<void> {
  if (pc.iceGatheringState === "complete") return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pc.removeEventListener("icegatheringstatechange", onState);
      reject(new Error("OpenAILiveVoiceSource: ICE gathering did not complete within 10s"));
    }, ICE_GATHERING_TIMEOUT_MS);
    function onState() {
      if (pc.iceGatheringState !== "complete") return;
      clearTimeout(timer);
      pc.removeEventListener("icegatheringstatechange", onState);
      resolve();
    }
    pc.addEventListener("icegatheringstatechange", onState);
  });
}

async function safeText(res: Response): Promise<string> {
  try {
    return await res.text();
  } catch {
    return "";
  }
}
