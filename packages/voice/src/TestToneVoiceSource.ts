import { AudioAnalysis } from "./analysis.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "@sinua/core";

/**
 * A `VoiceSource` that needs no microphone permission and no vendor API --
 * a synthesized tone with a speech-like on/off burst pattern, run through
 * the exact same `AnalyserNode`/`AudioAnalysis` path `LocalMicVoiceSource`
 * uses. Exists for demoing (a "play a test sound" button)
 * and for verifying the analysis pipeline end-to-end without needing a
 * live mic or a real conversation to talk into.
 *
 * The oscillator is routed to `destination` through a zero-gain node
 * rather than left disconnected -- some browsers only keep an audio graph
 * actively processing if it has a live path to the destination, so a
 * fully-disconnected `AnalyserNode` can end up reading stale data.
 */
export class TestToneVoiceSource implements VoiceSource {
  private ctx: AudioContext | null = null;
  private oscillators: OscillatorNode[] = [];
  private envelope: GainNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private burstTimeoutId: ReturnType<typeof setTimeout> | null = null;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;

  onMetrics(cb: (m: VoiceMetrics) => void): void {
    this.metricsCb = cb;
  }

  onStateChange(cb: (s: AgentState) => void): void {
    this.stateCb = cb;
  }

  async connect(): Promise<void> {
    this.stateCb?.("initializing");

    const ctx = new (globalThis.AudioContext || (globalThis as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();

    const envelope = ctx.createGain();
    envelope.gain.value = 0;

    // A single pure tone concentrates all its energy in one FFT bin, so
    // every band except one reads near-zero regardless of how many bands
    // AudioAnalysis splits into -- not representative of real voice, and
    // it would make the log-spaced-band fix (see analysis.ts) look like it
    // did nothing in Test Tone mode specifically. Three oscillators at a
    // speech-like fundamental/formant-ish spread, each with independent
    // gain, give the test signal actual spectral structure to differentiate.
    const partials: Array<{ freq: number; gain: number }> = [
      { freq: 160, gain: 1.0 },
      { freq: 520, gain: 0.5 },
      { freq: 1400, gain: 0.28 },
    ];
    const oscillators = partials.map(({ freq, gain }) => {
      const osc = ctx.createOscillator();
      osc.type = "sine";
      osc.frequency.value = freq;
      const partialGain = ctx.createGain();
      partialGain.gain.value = gain;
      osc.connect(partialGain);
      partialGain.connect(envelope);
      osc.start();
      return osc;
    });

    const analyser = ctx.createAnalyser();
    analyser.fftSize = 512; // matches LocalMicVoiceSource -- see its comment
    analyser.smoothingTimeConstant = 0;

    const silent = ctx.createGain();
    silent.gain.value = 0;

    envelope.connect(analyser);
    envelope.connect(silent);
    silent.connect(ctx.destination);

    this.ctx = ctx;
    this.oscillators = oscillators;
    this.envelope = envelope;
    this.analysis = new AudioAnalysis(analyser);

    this.scheduleBursts();
    this.stateCb?.("listening");
    this.intervalId = setInterval(() => this.tick(), 1000 / 30);
  }

  disconnect(): void {
    if (this.intervalId != null) clearInterval(this.intervalId);
    if (this.burstTimeoutId != null) clearTimeout(this.burstTimeoutId);
    this.intervalId = null;
    this.burstTimeoutId = null;
    for (const osc of this.oscillators) {
      osc.stop();
      osc.disconnect();
    }
    this.oscillators = [];
    this.ctx?.close();
    this.ctx = null;
    this.envelope = null;
    this.analysis = null;
    this.stateCb?.("idle");
  }

  private scheduleBursts(): void {
    const burst = () => {
      if (!this.ctx || !this.envelope) return;
      const now = this.ctx.currentTime;
      const burstLen = 0.3 + Math.random() * 0.5;
      const gapLen = 0.2 + Math.random() * 0.4;
      this.envelope.gain.cancelScheduledValues(now);
      this.envelope.gain.setValueAtTime(0, now);
      this.envelope.gain.linearRampToValueAtTime(0.6, now + 0.03);
      this.envelope.gain.linearRampToValueAtTime(0, now + burstLen);
      this.burstTimeoutId = setTimeout(burst, (burstLen + gapLen) * 1000);
    };
    burst();
  }

  private tick(): void {
    if (!this.analysis) return;
    const metrics = this.analysis.read();
    this.metricsCb?.(metrics);
    this.stateCb?.(metrics.level > 0.08 ? "speaking" : "listening");
  }
}
