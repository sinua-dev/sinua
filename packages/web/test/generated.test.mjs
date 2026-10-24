// The generated typed components (scripts/codegen): the pure mapping a
// component hands to SinuaView, and the element registry it defines. The pixels
// are SinuaView's, so the contract worth locking here is the mapping.
import { test } from "node:test";
import assert from "node:assert/strict";
import { sinuaOrbOverrides, sinuaRingOverrides, sinuaSpecError } from "../dist/generated/SinuaParams.js";
import { Sinua_ELEMENT_TAGS } from "../dist/generated/SinuaElements.js";

test("a material group becomes its engine keys", () => {
  assert.deepEqual(sinuaOrbOverrides("glowing", { glow: { strength: 0.6 } }), { glowStrength: 0.6 });
});

test("a list prop spreads over the indexed keys on the patterns that take one", () => {
  assert.deepEqual(sinuaRingOverrides("tracking", { progress: [0.2, 0.5] }), { progress0: 0.2, progress1: 0.5 });
  // A pattern that takes a single value keeps the scalar key.
  assert.deepEqual(sinuaRingOverrides("completing", { progress: 0.4 }), { progress: 0.4 });
});

test("choices and booleans map to engine numbers", () => {
  const o = sinuaOrbOverrides("glowing", { glow: { strength: 0.5, mode: "blur" } });
  assert.equal(o.glowStrength, 0.5);
  assert.equal(typeof o.glowMode, "number");
});

test("a spec for another object is reported, the right one passes", () => {
  assert.match(sinuaSpecError({ object: "ring" }, "orb"), /not a "orb"/);
  assert.equal(sinuaSpecError({ object: "orb" }, "orb"), null);
});

test("one element definition per object, tags from the config prefix", () => {
  assert.deepEqual(Object.keys(Sinua_ELEMENT_TAGS), ["sinua-orb", "sinua-signal", "sinua-ring", "sinua-core", "sinua-beacon"]);
  const orb = Sinua_ELEMENT_TAGS["sinua-orb"];
  assert.equal(orb.object, "orb");
  assert.ok(orb.groups.includes("glow"));
  assert.deepEqual(orb.toOverrides("glowing", { glow: { strength: 0.6 } }), { glowStrength: 0.6 });
});
