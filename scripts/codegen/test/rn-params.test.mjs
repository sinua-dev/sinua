// The generated React Native mapping (plain .ts, loaded through node's type stripping).
import { test } from "node:test";
import assert from "node:assert/strict";

const P = await import("../../../packages/react-native/src/generated/SinuaParams.ts");

test("signal: scalar progress, the envelope list, groups, and an orb boolean", () => {
  assert.deepEqual(P.sinuaSignalOverrides("dune", { progress: 0.4, ink: 1 }), { progress: 0.4, ink: 1 });
  assert.deepEqual(P.sinuaSignalOverrides("dune", { envelope: [0.2, 0.5] }), { envelope0: 0.2, envelope1: 0.5 });
  assert.equal(P.sinuaSignalOverrides("dune", { envelope: new Array(64).fill(0).map((_, i) => (i === 63 ? 1 : 0)) }).envelope63, 1);
  assert.deepEqual(P.sinuaOrbOverrides("composing", { faceOn: true }), { faceOn: 1 });
  assert.deepEqual(P.sinuaSignalOverrides("veil", { glow: { strength: 0.6, blend: "additive" } }), { glowStrength: 0.6, glowBlend: 1 });
  assert.deepEqual(P.sinuaSignalOverrides("veil", {}), {});
});

test("orb: the renamed flat prop and a material stay apart", () => {
  const o = P.sinuaOrbOverrides("working", { orbitParticles: 4, noise: { strength: 0.5 } });
  assert.equal(o.particles, 4);
  assert.equal(o.noiseStrength, 0.5);
});

test("unknown choice throws", () => {
  assert.throws(() => P.sinuaOrbOverrides("working", { glow: { mode: "nope" } }), /unknown value/);
});

test("spec object check", () => {
  assert.equal(P.sinuaSpecError('{"object":"signal"}', "signal"), null);
  assert.match(P.sinuaSpecError({ object: "orb" }, "signal"), /describes a "orb"/);
  assert.equal(P.sinuaSpecError("not json", "signal"), null);
});
