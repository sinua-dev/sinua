// Regenerates spec/voice-golden.json -- the parity vectors every native
// voice pipeline (packages/ios SinuaVoice, packages/android
// dev.sinua.voice) is tested against. See docs/audio-pipeline.md, *Native*.
//
// 1. A deterministic fixture signal (voice-spectrum-reference.mjs).
// 2. Real Chrome renders it through an AnalyserNode (fftSize 512, smoothing
//    0 -- the Studio's settings) in an OfflineAudioContext, reading
//    getByteFrequencyData every 1536 frames (12 render quanta). The
//    spec-written reference must match Chrome bit-exact or this aborts.
// 3. Those bytes go through the Web Studio's real analysis.ts (transpiled on
//    the fly) -> expected {level, bands} per tick.
// 4. A scripted feed goes through the real VoiceOverrides (dist/) under four
//    configs -> expected override maps.
//
// Needs Playwright + a Chromium: PLAYWRIGHT=/path/to/node_modules/playwright
// CHROME=/path/to/chrome node scripts/gen-voice-golden.mjs   (after npm run build)
import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { specByteFrequencyData, voiceFixture } from "./voice-spectrum-reference.mjs";
import { VoiceOverrides } from "../dist/index.js";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT ?? "playwright");
const HOP = 1536;

const { sampleRate, samples } = voiceFixture();

const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : {});
const page = await browser.newPage();
const chromeFrames = await page.evaluate(
  async ({ data, sampleRate, hop }) => {
    const ctx = new OfflineAudioContext(1, data.length, sampleRate);
    const buf = ctx.createBuffer(1, data.length, sampleRate);
    buf.copyToChannel(new Float32Array(data), 0);
    const src = ctx.createBufferSource();
    src.buffer = buf;
    const an = ctx.createAnalyser();
    an.fftSize = 512;
    an.smoothingTimeConstant = 0;
    src.connect(an);
    an.connect(ctx.destination);
    src.start(0);
    const frames = [];
    for (let end = hop; end < data.length; end += hop) {
      ctx.suspend(end / sampleRate).then(() => {
        const d = new Uint8Array(an.frequencyBinCount);
        an.getByteFrequencyData(d);
        frames.push({ end, bytes: Array.from(d) });
        ctx.resume();
      });
    }
    await ctx.startRendering();
    return frames;
  },
  { data: Array.from(samples), sampleRate, hop: HOP }
);
const chromeVersion = browser.version();
await browser.close();

// Chrome computes in float32, the reference in float64, so a value sitting
// on a floor() boundary can land one byte apart. Anything more is a real
// mismatch.
let refDiffBins = 0;
for (const f of chromeFrames) {
  const ref = specByteFrequencyData(samples, f.end);
  f.bytes.forEach((b, k) => {
    const d = Math.abs(ref[k] - b);
    if (d > 1) throw new Error(`spec reference != Chrome at end ${f.end} bin ${k}: ${ref[k]} vs ${b}`);
    if (d) refDiffBins++;
  });
}

const analysisSrc = readFileSync(root + "packages/voice/src/analysis.ts", "utf8");
const js = ts.transpileModule(analysisSrc, { compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 } }).outputText;
const { AudioAnalysis } = await import("data:text/javascript," + encodeURIComponent(js));
let current = null;
const analysis = new AudioAnalysis({ frequencyBinCount: 256, getByteFrequencyData: (a) => a.set(current) });
const frames = chromeFrames.map((f) => {
  current = f.bytes;
  const m = analysis.read();
  return { end: f.end, bytes: f.bytes, level: m.level, bands: m.bands };
});

let seed = 7;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const states = ["initializing", "listening", "thinking", "speaking", "listening", "idle", "listening"];
const steps = [];
for (let i = 0; i < 150; i++) {
  const st = { dt: i === 60 ? 0.25 : i === 61 ? 0 : 1 / 60 + (rnd() - 0.5) * 0.004 };
  if (i % 2 === 0) st.metrics = { level: rnd(), bands: Array.from({ length: i > 120 ? 8 : 16 }, rnd) };
  if (i % 20 === 0) st.state = states[(i / 20) % states.length];
  if (i === 30 || i === 90 || i === 91) st.interrupt = true;
  if (i === 80) st.historyCount = 12;
  if (i === 100) st.muted = true;
  if (i === 115) st.muted = false;
  if (i === 130) st.reset = true;
  steps.push(st);
}
// "inf" stands for Infinity (not representable in JSON).
const configs = {
  default: {},
  orb: { bandEaseRate: "inf" },
  signal: { audioStrength: 0, historyCount: 20, historyHz: 12, interrupt: { window: 0.6, duration: 0.5, strength: 0.7, tint: 1, hue: 200 }, mutedTint: 0.4, mutedHue: 8 },
  nointerrupt: { interrupt: false, levelEaseRate: "inf" },
};
const toWeb = (c) => ({
  ...(c.audioStrength != null && { audioStrength: c.audioStrength }),
  ...(c.levelEaseRate != null && { levelEaseRate: c.levelEaseRate === "inf" ? Infinity : c.levelEaseRate }),
  ...(c.bandEaseRate != null && { bandEaseRate: c.bandEaseRate === "inf" ? Infinity : c.bandEaseRate }),
  ...(c.historyCount != null && { history: { count: c.historyCount, hz: c.historyHz } }),
  ...(c.interrupt != null && { interrupt: c.interrupt }),
  ...(c.mutedTint != null && { mutedTint: c.mutedTint }),
  ...(c.mutedHue != null && { mutedHue: c.mutedHue }),
});
const expected = {};
for (const [name, c] of Object.entries(configs)) {
  const v = new VoiceOverrides(toWeb(c));
  expected[name] = steps.map((st) => {
    if (st.metrics) v.push(st.metrics);
    if (st.state) v.setState(st.state);
    if (st.interrupt) v.interrupt();
    if (st.historyCount) v.setHistoryCount(st.historyCount);
    if (st.muted !== undefined) v.muted = st.muted;
    if (st.reset) v.reset();
    return v.overrides(st.dt);
  });
}

const golden = {
  about:
    "Native voice-pipeline parity vectors. Regenerate with packages/core/scripts/gen-voice-golden.mjs; see docs/audio-pipeline.md, *Native*.",
  analysis: {
    source: `${chromeVersion} OfflineAudioContext AnalyserNode (fftSize 512, smoothingTimeConstant 0, min/max dB -100/-30) + packages/voice/src/analysis.ts`,
    sampleRate,
    fftSize: 512,
    hop: HOP,
    samplesF32Base64: Buffer.from(samples.buffer).toString("base64"),
    referenceVsChromeOneByteBins: refDiffBins,
    frames,
  },
  overrides: { source: "packages/core/src/voice.ts VoiceOverrides", steps, configs, expected },
};
writeFileSync(root + "spec/voice-golden.json", JSON.stringify(golden));
console.log(`spec/voice-golden.json: ${frames.length} analysis frames (spec reference vs Chrome: ${refDiffBins}/${frames.length * 256} bins off by 1, none more), ${steps.length} override steps x ${Object.keys(configs).length} configs`);
