import { AudioAnalysis } from "./analysis.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

const UPDATE_MS = 1000 / 30; // ~30fps sampling, decoupled from the render loop
const WATCHDOG_ZERO_FRAMES = 30; // ~1s of exact-zero RMS before re-attaching

/**
 * The one `VoiceSource` that needs no vendor API at all -- the browser's
 * own microphone via `getUserMedia`. Real audio, not a fake, so it's the
 * fastest way to prove the whole pipeline (mic -> AnalyserNode -> opts ->
 * engine) works before wiring an actual OpenAI Realtime / Gemini Live
 * connection. A future `OpenAIRealtimeVoiceSource` reuses this exact analysis path
 * against a *remote* `MediaStreamTrack` instead of the local mic's.
 */
export class LocalMicVoiceSource implements VoiceSource {
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private sourceNode: MediaStreamAudioSourceNode | null = null;
  private analyserNode: AnalyserNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private zeroStreak = 0;

  onMetrics(cb: (m: VoiceMetrics) => void): void {
    this.metricsCb = cb;
  }

  onStateChange(cb: (s: AgentState) => void): void {
    this.stateCb = cb;
  }

  async connect(): Promise<void> {
    this.stateCb?.("initializing");
    this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    this.ctx = new (globalThis.AudioContext || (globalThis as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
    this.attach();
    this.stateCb?.("listening");

    this.intervalId = setInterval(() => this.tick(), UPDATE_MS);
  }

  disconnect(): void {
    if (this.intervalId != null) clearInterval(this.intervalId);
    this.intervalId = null;
    this.sourceNode?.disconnect();
    this.stream?.getTracks().forEach((t) => t.stop());
    this.ctx?.close();
    this.ctx = null;
    this.stream = null;
    this.sourceNode = null;
    this.analyserNode = null;
    this.analysis = null;
    this.stateCb?.("idle");
  }

  private attach(): void {
    if (!this.ctx || !this.stream) return;
    this.sourceNode?.disconnect();
    const analyser = this.ctx.createAnalyser();
    // 512, not 256 -- more raw bins in the voice-relevant range gives
    // AudioAnalysis's log-spaced bands (see its own doc comment) enough
    // resolution to actually diverge instead of averaging near-identical
    // values per band.
    analyser.fftSize = 512;
    // We do our own attack/release smoothing in AudioAnalysis -- leaving
    // the AnalyserNode's own smoothing at 0 avoids double-smoothing.
    analyser.smoothingTimeConstant = 0;
    const source = this.ctx.createMediaStreamSource(this.stream);
    source.connect(analyser);
    this.sourceNode = source;
    this.analyserNode = analyser;
    this.analysis = new AudioAnalysis(analyser);
    this.zeroStreak = 0;
  }

  private tick(): void {
    if (!this.analysis || !this.analyserNode || !this.stream) return;

    // Watchdog: a live track's AnalyserNode can silently start reading
    // flat/zero data with no error (a known real Chrome/Android issue for
    // WebRTC tracks specifically, but cheap enough to guard here too since
    // this same code path is reused by the future WebRTC adapter). If the
    // track still reports "live" but we've read exact zero for ~1s, tear
    // down and recreate the source/analyser pair.
    const track = this.stream.getAudioTracks()[0];
    if (this.analysis.rawRms() === 0) {
      this.zeroStreak++;
      if (this.zeroStreak > WATCHDOG_ZERO_FRAMES && track?.readyState === "live") {
        this.attach();
        return;
      }
    } else {
      this.zeroStreak = 0;
    }

    const metrics = this.analysis.read();
    this.metricsCb?.(metrics);
    this.stateCb?.(metrics.level > 0.08 ? "speaking" : "listening");
  }
}
