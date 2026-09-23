// The generated React Native mapping (plain .ts, loaded through node's type stripping).
import { test } from "node:test";
import assert from "node:assert/strict";

const P = await import("../../../packages/react-native/src/generated/SinuaParams.ts");

test("ring: scalar progress, list progress on tracking, segment list, booleans, groups", () => {
  assert.deepEqual(P.sinuaRingOverrides("completing", { progress: 0.4, strokeWidth: 0.1 }), { progress: 0.4, strokeWidth: 0.1 });
  assert.deepEqual(P.sinuaRingOverrides("tracking", { progress: [0.2, 0.5], ringCount: 2 }), { progress0: 0.2, progress1: 0.5, ringCount: 2 });
  assert.deepEqual(P.sinuaRingOverrides("tracking", { progress: 0.7 }), { progress0: 0.7 });
  assert.deepEqual(P.sinuaRingOverrides("completing", { progress: [0.3, 0.9] }), { progress: 0.3 });
  assert.deepEqual(P.sinuaRingOverrides("stepping", { segment: [1, 0.5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] }).segment23, 0);
  assert.deepEqual(P.sinuaRingOverrides("measuring", { fill: true, marker: false }), { fill: 1, marker: 0 });
  assert.deepEqual(P.sinuaRingOverrides("loading", { glow: { strength: 0.6, blend: "additive" } }), { glowStrength: 0.6, glowBlend: 1 });
  assert.deepEqual(P.sinuaRingOverrides("loading", {}), {});
});

test("orb: renamed flat prop and the particles material stay apart", () => {
  const o = P.sinuaOrbOverrides("working", { orbitParticles: 4, particles: { count: 12, style: "orbit" } });
  assert.equal(o.particles, 4);
  assert.equal(o.particleCount, 12);
  assert.equal(o.particleStyle, 2);
});

test("unknown choice throws", () => {
  assert.throws(() => P.sinuaOrbOverrides("working", { glow: { mode: "nope" } }), /unknown value/);
});

test("spec object check", () => {
  assert.equal(P.sinuaSpecError('{"object":"ring"}', "ring"), null);
  assert.match(P.sinuaSpecError({ object: "orb" }, "ring"), /describes a "orb"/);
  assert.equal(P.sinuaSpecError("not json", "ring"), null);
});
