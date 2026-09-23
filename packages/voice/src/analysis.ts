import type { VoiceMetrics } from "@sinua/core";

/**
 * Shared `AnalyserNode` -> `VoiceMetrics` extraction, used by every
 * `VoiceSource` that ends up with a live Web Audio graph (both
 * `LocalMicVoiceSource` and `TestToneVoiceSource` route through this --
 * the point is to exercise the exact same analysis code a real WebRTC
 * voice-model track would use, not a separate "fake" path).
 *
 * Technique choices below are taken directly from real, shipped code
 * (LiveKit's and ElevenLabs' audio-visualizer components), not invented:
 * - Restrict to the voice-relevant bin range (discard DC/sub-bass and the
 *   mostly-noise high end).
 * - `sqrt` compression on the normalized magnitude -- the load-bearing
 *   perceptual step that keeps quiet/moderate speech visibly above zero
 *   instead of everything under conversational volume reading as
 *   near-silent.
 * - Asymmetric attack/release smoothing (fast rise, slow decay) rather
 *   than a single symmetric time constant -- grounded in VU/PPM meter
 *   ballistics (IEC 60268-17), reads as "alive" rather than twitchy.
 * - **Log-spaced band boundaries, not equal-width chunks.** FFT bins are
 *   linear in frequency; a single human voice's energy is fairly
 *   correlated across a *linear* split of a narrow band range, which is
 *   exactly why an earlier version of this (equal-width chunks) made every
 *   bar in `signal`'s bar style look like it was doing "the same
 *   animation" -- a real, user-reported symptom, not a hypothetical one.
 *   Spacing band edges geometrically (each band's width scales with
 *   frequency, same principle Butterchurn and most spectrum visualizers
 *   use) spreads out the low end (where a voice's fundamental/formants
 *   live) and widens the high end (sibilants/consonant noise), so bands
 *   actually diverge instead of all tracking the same broadband envelope.
 */
export class AudioAnalysis {
  private attack: number;
  private release: number;
  private loFrac: number;
  private hiFrac: number;
  private levelState = 0;
  private bandState: number[];

  constructor(
    private analyser: AnalyserNode,
    private bandCount = 16,
    opts: { attack?: number; release?: number; loFrac?: number; hiFrac?: number } = {}
  ) {
    this.attack = opts.attack ?? 0.7;
    this.release = opts.release ?? 0.12;
    this.loFrac = opts.loFrac ?? 0.05;
    this.hiFrac = opts.hiFrac ?? 0.4;
    this.bandState = new Array(bandCount).fill(0);
  }

  /** Call once per animation frame / update tick. */
  read(): VoiceMetrics {
    const data = new Uint8Array(this.analyser.frequencyBinCount);
    this.analyser.getByteFrequencyData(data);

    const lo = Math.max(1, Math.floor(data.length * this.loFrac)); // >=1 so log(lo) is finite
    const hi = Math.max(lo + this.bandCount, Math.floor(data.length * this.hiFrac));

    // Geometric (log-spaced) band edges -- see the class doc comment for
    // why equal-width chunks made every band (and therefore every `signal`
    // bar) track the same broadband envelope.
    const logLo = Math.log(lo);
    const logHi = Math.log(hi);
    const edges: number[] = [];
    for (let b = 0; b <= this.bandCount; b++) {
      edges.push(Math.round(Math.exp(logLo + (logHi - logLo) * (b / this.bandCount))));
    }

    const targets: number[] = [];
    for (let b = 0; b < this.bandCount; b++) {
      const start = edges[b];
      const end = Math.max(start + 1, edges[b + 1]);
      let sum = 0;
      let n = 0;
      for (let i = start; i < end && i < data.length; i++) {
        sum += data[i];
        n++;
      }
      const raw = n > 0 ? sum / n / 255 : 0;
      targets.push(Math.sqrt(Math.max(0, Math.min(1, raw))));
    }

    for (let b = 0; b < this.bandCount; b++) {
      const target = targets[b];
      const current = this.bandState[b];
      const rate = target > current ? this.attack : this.release;
      this.bandState[b] = current + (target - current) * rate;
    }

    const levelTarget = targets.reduce((a, v) => Math.max(a, v), 0);
    const levelRate = levelTarget > this.levelState ? this.attack : this.release;
    this.levelState += (levelTarget - this.levelState) * levelRate;

    return { level: this.levelState, bands: [...this.bandState] };
  }

  /** RMS over the full buffer, unsmoothed -- used only by the watchdog. */
  rawRms(): number {
    const data = new Uint8Array(this.analyser.frequencyBinCount);
    this.analyser.getByteFrequencyData(data);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return Math.sqrt(sum / data.length) / 255;
  }
}
