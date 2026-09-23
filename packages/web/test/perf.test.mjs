// The shared frame pacer and the low-power policy (pure).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createFramePacer, performanceFor, DEFAULT_LOW_POWER } from "../dist/index.js";

function drawsPerSecond(maxFps, hz, seconds = 10, jitterMs = 0) {
  const pace = createFramePacer(maxFps);
  let draws = 0;
  let seed = 1;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647 - 0.5) * 2;
  const frames = Math.round(hz * seconds);
  for (let i = 0; i < frames; i++) if (pace(1000 + (i * 1000) / hz + rnd() * jitterMs)) draws++;
  return draws / seconds;
}

test("30 fps on 60 Hz and 120 Hz displays is exactly 30", () => {
  assert.ok(Math.abs(drawsPerSecond(30, 60) - 30) <= 0.1, `${drawsPerSecond(30, 60)}`);
  assert.ok(Math.abs(drawsPerSecond(30, 120) - 30) <= 0.1, `${drawsPerSecond(30, 120)}`);
});

test("24 fps on 60 Hz averages 24 with no drift over 10 s, even with vsync jitter", () => {
  assert.ok(Math.abs(drawsPerSecond(24, 60) - 24) <= 0.2, `${drawsPerSecond(24, 60)}`);
  assert.ok(Math.abs(drawsPerSecond(30, 60, 10, 0.8) - 30) <= 0.3, `${drawsPerSecond(30, 60, 10, 0.8)}`);
  assert.ok(Math.abs(drawsPerSecond(60, 120, 10, 0.5) - 60) <= 0.3, `${drawsPerSecond(60, 120, 10, 0.5)}`);
});

test("no cap (undefined, 0, NaN) draws every frame; a cap above the display rate draws every frame", () => {
  for (const cap of [undefined, 0, NaN, -5]) assert.equal(drawsPerSecond(cap, 60), 60);
  assert.equal(drawsPerSecond(120, 60), 60);
});

test("performanceFor: low power default, spec 1.2 block priority, option caps further", () => {
  assert.deepEqual(performanceFor({ lowPower: false }), { maxFps: null, overrides: {} });
  assert.deepEqual(performanceFor({ lowPower: true }), { maxFps: 30, overrides: { glowStrength: 0, particleStrength: 0 } });
  assert.equal(DEFAULT_LOW_POWER.maxFps, 30);
  // 1.2 with a lowPower block: the resolver already capped and shed -- no default on top.
  assert.deepEqual(performanceFor({ lowPower: true, specHandlesLowPower: true, specMaxFps: 20 }), { maxFps: 20, overrides: {} });
  // 1.2 maxFps only (no lowPower block): its cap, plus the host default under low power.
  assert.deepEqual(performanceFor({ lowPower: false, specMaxFps: 60, optionMaxFps: 24 }), { maxFps: 24, overrides: {} });
  assert.deepEqual(performanceFor({ lowPower: true, specMaxFps: 24 }), { maxFps: 24, overrides: { glowStrength: 0, particleStrength: 0 } });
  assert.deepEqual(performanceFor({ lowPower: true, optionMaxFps: 15 }), { maxFps: 15, overrides: { glowStrength: 0, particleStrength: 0 } });
});
