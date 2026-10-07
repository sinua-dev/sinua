import { canRefreshCredential, resolveCredential, type CredentialOptions } from "./credential.js";
import { insecureCredentialRefusal } from "./insecureCredential.js";
import { PcmAudioGraph } from "./PcmAudioGraph.js";
import { FatalConnectError, isFatalConnectError } from "./realtimeReconnect.js";
import { base64ToBytes, bytesToBase64, float32ToPcm16, parsePcmRate, pcm16ToFloat32 } from "./pcm.js";
import { TranscriptAssembler } from "./transcript.js";
import type { AgentState, TranscriptUpdate, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * `VoiceSource` for Gemini Live over its raw WebSocket -- the PCM-over-
 * socket transport *shape* from docs/audio-pipeline.md's table (Amazon Nova
 * Sonic is the same shape over HTTP/2; ElevenLabs is the same shape with a
 * different protocol, see `ElevenLabsVoiceSource`). Unlike
 * `OpenAIRealtimeVoiceSource`, nothing hands us a playable track: the shared
 * `PcmAudioGraph` captures the mic and ships PCM16 @ 16 kHz, and decodes/
 * schedules the model's PCM16 @ 24 kHz chunks onto a playback timeline.
 *
 * Wire protocol, verified against the current reference
 * (ai.google.dev/api/live, camelCase JSON) and Google's own browser sample
 * (`google-gemini/live-api-web-console`), not recalled -- full citations in
 * from the vendor's public docs:
 *   - `wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage
 *     .v1beta.GenerativeService.BidiGenerateContent?key=…` (API key) or
 *     `…BidiGenerateContentConstrained?access_token=…` (ephemeral token);
 *   - first message `{"setup": {...}}` -> `{"setupComplete": {}}`;
 *   - mic: `{"realtimeInput": {"audio": {"data": <base64>, "mimeType":
 *     "audio/pcm;rate=16000"}}}`;
 *   - model: `serverContent.modelTurn.parts[].inlineData.{data, mimeType}`,
 *     plus `interrupted` / `turnComplete` / `generationComplete` /
 *     `waitingForInput` / `inputTranscription`; `goAway.timeLeft`;
 *     `sessionResumptionUpdate.{newHandle, resumable}`.
 *
 * ## The gate: `speaking` follows the playback timeline, not chunk receipt
 *
 * Chunks arrive well before they are audible. `PcmAudioGraph` schedules
 * every decoded chunk at a running cursor and hangs the analyser off the
 * playback gain node, so `AudioAnalysis` can only read what is audible and
 * `tick()` derives `speaking` from the graph's `playbackState()`.
 * `serverContent.interrupted` (user barged in) stops every scheduled source
 * and resets the cursor, as the Live guide instructs.
 *
 * ## State mapping (LiveKit `AgentState` vocabulary, ./types.ts)
 *
 * Gemini exposes **no** server-side user-VAD start/stop message (the
 * reference's `speechState` is deprecated with an undefined replacement;
 * LiveKit's and Pipecat's Gemini adapters both work around the same gap),
 * so, unlike OpenAI, "the user stopped talking" is never signalled. What is
 * signalled and used here: `setupComplete` -> `listening`; `interrupted`
 * -> `listening`; input transcription arriving (Gemini's own ASR heard the
 * user) -> `listening`; a chunk received but not yet audible, or the buffer
 * starved mid-generation -> `thinking`; audio audible -> `speaking`;
 * drained after `turnComplete`/`generationComplete`/`waitingForInput` ->
 * `listening`. `thinking` is therefore short by construction here.
 *
 * ## Session lifetime
 *
 * A connection lives ~10 minutes; the server sends `goAway` first. The
 * setup asks for `sessionResumption`, the latest `newHandle` is kept, and
 * on `goAway` or an unexpected close the adapter reopens the socket with
 * `sessionResumption: {handle}` (mic and playback graphs untouched, state
 * `initializing` meanwhile), the same loop LiveKit/Pipecat run.
 * `contextWindowCompression` is requested so the 15-minute audio-only
 * session cap doesn't apply either.
 *
 * ## Credentials -- read this before wiring it anywhere real
 *
 * Only an ephemeral token (`auth_tokens/…`) is accepted, minted by *your
 * backend* (`POST /v1beta/auth_tokens`; `@sinua/voice/server`'s
 * `mintGeminiLiveCredential`, or `npx @sinua/voice dev-proxy` locally) and sent
 * as `access_token` on the Constrained endpoint. It is single-use by default
 * and locks the model, voice and instructions server-side. Pass
 * `credentialUrl` or a `credential` provider: every reconnect (a drop, or
 * `goAway`) resumes the session with a **new** token. A raw API key is refused.
 */
export interface GeminiLiveVoiceSourceOptions extends CredentialOptions {
  /** Live model id; `gemini-3.8-live` is the current stable default per ai.google.dev/gemini-api/docs/models. */
  model?: string;
  /**
   * @deprecated Set instructions when your backend mints the token
   * (`mintGeminiLiveCredential({ instructions })`), which locks them. Still
   * sent here, but a locked token wins.
   */
  instructions?: string;
  /**
   * Transcripts (`onTranscript`): `true` (default) paces the model's text over its played
   * audio (Gemini sends no word times); `false` shows it as it arrives
   * (docs/audio-pipeline.md, *Transcripts*).
   */
  syncToAudio?: boolean;
}

const WS_URL =
  "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContentConstrained";
const INPUT_RATE = 16000;
const OUTPUT_RATE_FALLBACK = 24000;
const UPDATE_MS = 1000 / 30; // ~30fps metering, same as every other VoiceSource
const SETUP_TIMEOUT_MS = 15_000;
const RECONNECT_ATTEMPTS = 3;
const RECONNECT_DELAY_MS = 500;

interface ServerMessage {
  setupComplete?: unknown;
  serverContent?: {
    modelTurn?: { parts?: Array<{ inlineData?: { data?: string; mimeType?: string } }> };
    turnComplete?: boolean;
    interrupted?: boolean;
    generationComplete?: boolean;
    waitingForInput?: boolean;
    inputTranscription?: { text?: string };
    interimInputTranscription?: { text?: string };
    outputTranscription?: { text?: string };
  };
  sessionResumptionUpdate?: { newHandle?: string; resumable?: boolean };
  goAway?: { timeLeft?: string };
  error?: unknown;
}

export class GeminiLiveVoiceSource implements VoiceSource {
  private readonly credentials: CredentialOptions;
  /** The token for the current socket; replaced on every refresh. */
  private token = "";
  private readonly model: string;
  private readonly instructions: string | undefined;

  private readonly graph = new PcmAudioGraph();
  /** `generationComplete`/`turnComplete`/`waitingForInput` seen since the last audio chunk. */
  private generationDone = true;

  private ws: WebSocket | null = null;
  private resumptionHandle: string | null = null;
  private wantConnected = false; // user intent -- false after disconnect()
  private connected = false;
  private streaming = false; // mic chunks are sent only after setupComplete
  private reconnecting = false;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private interruptCb: (() => void) | null = null;
  /** Transcripts: no times from Gemini; paced over the reply's received audio. */
  private readonly transcript: TranscriptAssembler;
  readonly supportsTranscript = true;
  readonly transcriptTiming = "none" as const;
  private connectionCb: ((connected: boolean) => void) | null = null;
  private sessionUp = false;
  private muted = false;
  private micStream: MediaStream | null = null;
  private state: AgentState = "idle";

  constructor(opts: GeminiLiveVoiceSourceOptions) {
    this.credentials = { credential: opts.credential, credentialUrl: opts.credentialUrl };
    this.transcript = new TranscriptAssembler("none", opts.syncToAudio ?? true);
    if (opts.instructions !== undefined) {
      console.warn(
        "GeminiLiveVoiceSource: `instructions` is deprecated -- set them when your backend mints the token " +
          "(mintGeminiLiveCredential from @sinua/voice/server), which locks them.",
      );
    }
    this.model = opts.model ?? "gemini-3.8-live";
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

  /**
   * Both speakers' live transcript (design note 39). Turn ids keep counting across
   * reconnects; display only, nothing is kept or sent.
   */
  onTranscript(cb: (u: TranscriptUpdate) => void): void {
    this.transcript.onUpdate = cb;
  }

  onConnectionChange(cb: (connected: boolean) => void): void {
    this.connectionCb = cb;
  }

  /**
   * Muted, silence goes out: the capture sends zeroed PCM (the server's turn
   * detection keeps its timing) and the mic track is disabled. The session stays up.
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    for (const t of this.micStream?.getAudioTracks() ?? []) t.enabled = !muted;
  }

  private setSessionUp(up: boolean): void {
    if (up === this.sessionUp) return;
    this.sessionUp = up;
    this.connectionCb?.(up);
  }

  async connect(): Promise<void> {
    if (!this.credentials.credential && !this.credentials.credentialUrl) {
      throw new Error("GeminiLiveVoiceSource: a credential or credentialUrl is required");
    }
    // Before the mic and before the socket: a refused credential must not open
    // a device or a connection.
    await this.refreshToken();
    this.wantConnected = true;
    this.setState("initializing");
    try {
      // Gemini's rates are fixed and known up front, so the whole graph can
      // be up before the socket opens (ElevenLabs negotiates them instead).
      const mic = await PcmAudioGraph.requestMic();
      this.micStream = mic;
      this.setMuted(this.muted);
      await this.graph.start({
        mic,
        inputRate: INPUT_RATE,
        outputRate: OUTPUT_RATE_FALLBACK,
        onChunk: (samples) => this.onCaptureChunk(samples),
      });
      await this.openSocket();
      this.connected = true;
      this.setState("listening");
      this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
      this.setSessionUp(true);
    } catch (err) {
      this.teardown();
      this.setState("idle");
      throw err;
    }
  }

  disconnect(): void {
    this.wantConnected = false;
    this.teardown();
    this.micStream = null;
    this.setState("idle");
    this.setSessionUp(false);
  }

  /** A token for the next socket; an `auth_tokens/…` name is the only accepted shape. */
  private async refreshToken(): Promise<void> {
    const { credential } = await resolveCredential("GeminiLiveVoiceSource", this.credentials);
    const refusal = insecureCredentialRefusal({
      vendor: "GeminiLiveVoiceSource",
      isEphemeral: credential.startsWith("auth_tokens/"),
      ephemeralShape: "auth_tokens/…",
    });
    if (refusal) throw new FatalConnectError(refusal);
    this.token = credential;
  }

  private buildUrl(): string {
    return `${WS_URL}?access_token=${encodeURIComponent(this.token)}`;
  }

  private buildSetup(): Record<string, unknown> {
    const setup: Record<string, unknown> = {
      model: this.model.startsWith("models/") ? this.model : `models/${this.model}`,
      generationConfig: { responseModalities: ["AUDIO"] },
      // Gemini's own ASR of the user -- the vendor-side "the user is being
      // heard" signal this adapter uses instead of an energy heuristic.
      inputAudioTranscription: {},
      // The model's own words, for transcripts (the same Live session; no extra request).
      outputAudioTranscription: {},
      sessionResumption: this.resumptionHandle ? { handle: this.resumptionHandle } : {},
      contextWindowCompression: { slidingWindow: {} },
    };
    if (this.instructions) setup.systemInstruction = { parts: [{ text: this.instructions }] };
    return setup;
  }

  /** Opens the socket, sends `setup`, resolves on `setupComplete`. */
  private openSocket(): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const ws = new WebSocket(this.buildUrl());
      // ArrayBuffer, not Blob, so every frame is decoded synchronously and
      // message order can never be reshuffled by an awaited Blob.text().
      ws.binaryType = "arraybuffer";
      this.ws = ws;
      let settled = false;
      const timer = setTimeout(() => {
        if (settled) return;
        settled = true;
        reject(new Error("Gemini Live setup did not complete within 15s"));
        ws.close();
      }, SETUP_TIMEOUT_MS);

      ws.onopen = () => ws.send(JSON.stringify({ setup: this.buildSetup() }));
      ws.onmessage = (e) => {
        const msg = parseServerMessage(e.data);
        if (!msg) return;
        if (msg.setupComplete !== undefined && !settled) {
          settled = true;
          clearTimeout(timer);
          this.streaming = true;
          resolve();
          return;
        }
        this.onServerMessage(msg);
      };
      ws.onerror = () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        reject(new Error("Gemini Live WebSocket failed to connect (check the credential and network)"));
      };
      ws.onclose = (ev) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(new Error(`Gemini Live closed during setup (code ${ev.code}${ev.reason ? `: ${ev.reason}` : ""})`));
          return;
        }
        if (this.ws === ws) this.onSocketClosed(ev);
      };
    });
  }

  private onCaptureChunk(samples: Float32Array): void {
    if (!this.streaming || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    if (this.muted) samples = new Float32Array(samples.length);
    this.ws.send(
      JSON.stringify({
        realtimeInput: {
          audio: { data: bytesToBase64(float32ToPcm16(samples)), mimeType: `audio/pcm;rate=${INPUT_RATE}` },
        },
      })
    );
  }

  private onServerMessage(msg: ServerMessage): void {
    const sc = msg.serverContent;
    if (sc) {
      if (sc.interrupted) {
        // The Live guide: "stop playing audio and clear queued playback".
        // An interrupt only if there was output to cut -- audible, or
        // already queued on the playback timeline.
        if (this.state === "speaking" || this.graph.playbackState() !== "drained") this.interruptCb?.();
        this.transcript.cut();
        this.graph.clearPlayback(true);
        this.generationDone = true;
        this.setState("listening");
      }
      if ((sc.interimInputTranscription || sc.inputTranscription) && this.state !== "speaking") {
        this.setState("listening");
      }
      const now = performance.now();
      if (sc.inputTranscription?.text) {
        if (this.state === "speaking") this.transcript.hold();
        this.transcript.userDelta(sc.inputTranscription.text, now);
      }
      if (sc.outputTranscription?.text) this.transcript.assistantDelta(sc.outputTranscription.text, now);
      for (const part of sc.modelTurn?.parts ?? []) {
        const inline = part.inlineData;
        if (inline?.data && (inline.mimeType ?? "").startsWith("audio/pcm")) {
          const samples = pcm16ToFloat32(base64ToBytes(inline.data));
          const rate = parsePcmRate(inline.mimeType, OUTPUT_RATE_FALLBACK);
          this.graph.enqueue(samples, rate);
          this.transcript.assistantAudio((samples.length / rate) * 1000);
          this.generationDone = false;
          // Received, not audible yet -- tick() promotes to `speaking` once
          // the playback clock actually reaches the burst.
          if (this.state !== "speaking") this.setState("thinking");
        }
      }
      if (sc.generationComplete || sc.turnComplete || sc.waitingForInput) this.generationDone = true;
    }
    const upd = msg.sessionResumptionUpdate;
    if (upd?.resumable && upd.newHandle) this.resumptionHandle = upd.newHandle;
    if (msg.goAway) {
      console.warn(`Gemini Live goAway (timeLeft ${msg.goAway.timeLeft ?? "?"}) -- reconnecting with the resumption handle`);
      void this.reconnect("goAway");
    }
    if (msg.error) console.error("Gemini Live error message:", msg.error);
  }

  private tick(): void {
    const metrics = this.graph.read();
    if (metrics) this.metricsCb?.(metrics);

    if (!this.connected) return; // reconnecting: stay `initializing`
    this.transcript.tick(performance.now(), metrics?.level ?? 0, this.state === "speaking");
    const playback = this.graph.playbackState();
    if (playback === "audible") {
      this.setState("speaking");
    } else if (playback === "drained") {
      // Drained. Still generating (buffer starved by the network) reads as
      // `thinking`, not `speaking` -- nothing is audible; done reads as
      // `listening`. Both only apply once we were actually in a turn.
      if (this.state === "speaking" || this.state === "thinking") {
        this.setState(this.generationDone ? "listening" : "thinking");
      }
    }
  }

  private onSocketClosed(ev: CloseEvent): void {
    if (!this.wantConnected) return; // user disconnect, already handled
    console.warn(`Gemini Live socket closed (code ${ev.code}${ev.reason ? `: ${ev.reason}` : ""})`);
    void this.reconnect("close");
  }

  private async reconnect(reason: string): Promise<void> {
    if (this.reconnecting || !this.wantConnected) return;
    this.reconnecting = true;
    this.streaming = false;
    this.connected = false;
    this.graph.clearPlayback(false);
    this.generationDone = true;
    this.setState("initializing");
    const old = this.ws;
    this.ws = null;
    if (old) {
      old.onopen = null;
      old.onmessage = null;
      old.onerror = null;
      old.onclose = null;
      try {
        old.close(1000);
      } catch {
        /* already closed */
      }
    }
    for (let attempt = 1; attempt <= RECONNECT_ATTEMPTS && this.wantConnected; attempt++) {
      try {
        // A token is single-use by default: resume with a new one when the
        // caller can mint it (a pasted token is tried as is).
        if (canRefreshCredential(this.credentials)) await this.refreshToken();
        if (!this.wantConnected) break;
        await this.openSocket();
        this.connected = true;
        this.reconnecting = false;
        this.setState("listening");
        return;
      } catch (err) {
        console.warn(`Gemini Live reconnect ${attempt}/${RECONNECT_ATTEMPTS} after ${reason} failed:`, err);
        if (isFatalConnectError(err)) break;
        await new Promise((r) => setTimeout(r, RECONNECT_DELAY_MS));
      }
    }
    this.reconnecting = false;
    if (this.wantConnected) {
      console.error("Gemini Live: gave up reconnecting");
      this.disconnect();
    }
  }

  private setState(s: AgentState): void {
    if (this.state === s) return;
    // The reply is over (drained, cut just before, or a reconnect): its turn ends if it was heard.
    if (s === "listening" || s === "initializing" || s === "idle") this.transcript.speakingEnded();
    this.state = s;
    this.stateCb?.(s);
  }

  private teardown(): void {
    this.connected = false;
    this.streaming = false;
    this.reconnecting = false;
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
        ws.close(1000);
      } catch {
        /* already closed */
      }
    }
    this.graph.stop();
    this.transcript.stop();
    this.generationDone = true;
    this.resumptionHandle = null; // a fresh connect() starts a fresh session
  }
}

function parseServerMessage(data: unknown): ServerMessage | null {
  let text: string;
  if (typeof data === "string") text = data;
  else if (data instanceof ArrayBuffer) text = new TextDecoder().decode(data);
  else return null;
  try {
    return JSON.parse(text) as ServerMessage;
  } catch {
    return null;
  }
}
