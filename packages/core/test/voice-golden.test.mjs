// Guards spec/voice-golden.json against drift on the Web side: the
// spec-written AnalyserNode reference still reproduces the Chrome bytes,
// and today's VoiceOverrides still produces the recorded override maps.
// The native pipelines are tested against the same file (packages/ios
// SinuaVoiceTests, packages/android dev.sinua.voice unit tests).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { specByteFrequencyData } from "../scripts/voice-spectrum-reference.mjs";
import { VoiceOverrides } from "../dist/index.js";

const golden = JSON.parse(readFileSync(new URL("../../../spec/voice-golden.json", import.meta.url), "utf8"));

test("spec-written AnalyserNode reference reproduces the recorded Chrome bytes (±1)", () => {
  const b = Buffer.from(golden.analysis.samplesF32Base64, "base64");
  const samples = new Float32Array(b.buffer, b.byteOffset, b.length / 4);
  // float64 reference vs Chrome's float32: at most one byte apart at a floor() boundary.
  for (const f of golden.analysis.frames.filter((_, i) => i % 4 === 0)) {
    const ref = specByteFrequencyData(samples, f.end);
    f.bytes.forEach((b, k) => assert.ok(Math.abs(ref[k] - b) <= 1, `frame end ${f.end} bin ${k}: ${ref[k]} vs ${b}`));
  }
});

test("VoiceOverrides reproduces the recorded override maps", () => {
  const { steps, configs, expected } = golden.overrides;
  const inf = (v) => (v === "inf" ? Infinity : v);
  for (const [name, c] of Object.entries(configs)) {
    const v = new VoiceOverrides({
      ...(c.audioStrength != null && { audioStrength: c.audioStrength }),
      ...(c.levelEaseRate != null && { levelEaseRate: inf(c.levelEaseRate) }),
      ...(c.bandEaseRate != null && { bandEaseRate: inf(c.bandEaseRate) }),
      ...(c.historyCount != null && { history: { count: c.historyCount, hz: c.historyHz } }),
      ...(c.interrupt != null && { interrupt: c.interrupt }),
      ...(c.mutedTint != null && { mutedTint: c.mutedTint }),
      ...(c.mutedHue != null && { mutedHue: c.mutedHue }),
    });
    steps.forEach((st, i) => {
      if (st.metrics) v.push(st.metrics);
      if (st.state) v.setState(st.state);
      if (st.interrupt) v.interrupt();
      if (st.historyCount) v.setHistoryCount(st.historyCount);
      if (st.muted !== undefined) v.muted = st.muted;
      if (st.reset) v.reset();
      assert.deepEqual(v.overrides(st.dt), expected[name][i], `${name} step ${i}`);
    });
  }
});
