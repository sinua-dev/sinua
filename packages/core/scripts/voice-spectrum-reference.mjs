// Web Audio spec AnalyserNode byte spectrum, written straight from the spec
// (WebAudio/web-audio-api index.bs, "FFT Windowing and Smoothing over Time"
// + getByteFrequencyData): Blackman window (alpha 0.16), X[k] = (1/N) sum
// x[n] e^{-2 pi i k n / N}, no smoothing (the Studio sets
// smoothingTimeConstant 0), 20 log10, floor(255/(max-min) (Y - min)) clipped
// to 0..255. Direct DFT in float64 -- slow, but obviously correct, and
// checked bit-exact against Chrome by gen-voice-golden.mjs.
export function specByteFrequencyData(samples, end, fftSize = 512, minDb = -100, maxDb = -30) {
  const N = fftSize;
  const x = new Float64Array(N);
  for (let n = 0; n < N; n++) {
    const i = end - N + n;
    const w = 0.42 - 0.5 * Math.cos((2 * Math.PI * n) / N) + 0.08 * Math.cos((4 * Math.PI * n) / N);
    x[n] = (i >= 0 ? samples[i] : 0) * w;
  }
  const out = new Uint8Array(N / 2);
  for (let k = 0; k < N / 2; k++) {
    let re = 0;
    let im = 0;
    for (let n = 0; n < N; n++) {
      const ph = (-2 * Math.PI * k * n) / N;
      re += x[n] * Math.cos(ph);
      im += x[n] * Math.sin(ph);
    }
    const db = 20 * Math.log10(Math.sqrt(re * re + im * im) / N);
    const b = Math.floor((255 / (maxDb - minDb)) * (db - minDb));
    out[k] = Number.isFinite(b) ? Math.max(0, Math.min(255, b)) : 0;
  }
  return out;
}

/** The deterministic 2 s / 48 kHz fixture: test-tone partials under bursts, a noise stretch, silence. */
export function voiceFixture() {
  const SR = 48000;
  const LEN = SR * 2;
  let seed = 12345;
  const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const x = new Float32Array(LEN);
  const env = new Float64Array(LEN);
  let t0 = 0;
  while (t0 < 0.9) {
    const burst = 0.3 + rnd() * 0.5;
    const gap = 0.2 + rnd() * 0.4;
    for (let i = Math.floor(t0 * SR); i < Math.min(LEN, Math.floor((t0 + burst) * SR)); i++) {
      const t = i / SR - t0;
      env[i] = t < 0.03 ? 0.6 * (t / 0.03) : 0.6 * (1 - (t - 0.03) / (burst - 0.03));
    }
    t0 += burst + gap;
  }
  const partials = [[160, 1.0], [520, 0.5], [1400, 0.28]];
  for (let i = 0; i < LEN; i++) {
    const t = i / SR;
    let s = 0;
    for (const [f, g] of partials) s += g * Math.sin(2 * Math.PI * f * t);
    x[i] = s * env[i];
    if (t >= 1.0 && t < 1.4) x[i] += (rnd() * 2 - 1) * 0.08;
  }
  return { sampleRate: SR, samples: x };
}
