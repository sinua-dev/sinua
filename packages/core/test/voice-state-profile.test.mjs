// The built-in voice-state profile through wasm (FX Spec 1.8): one shape
// stays on screen and the agent's state changes how it behaves.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parameterCatalog, resolveFxSpec, voiceStateProfile, VOICE_STATES } from "../dist/index.js";

const file = JSON.parse(
  readFileSync(fileURLToPath(new URL("../../../spec/voice-state-profile.json", import.meta.url)), "utf8"),
);

test("every voice state has a profile, and other state names have none", () => {
  for (const state of VOICE_STATES) {
    const p = voiceStateProfile("glowing", state);
    assert.ok(p, state);
    assert.ok(p.speed > 0, state);
    assert.equal(typeof p.overrides.ink, "number", `${state} sets ink`);
  }
  assert.equal(voiceStateProfile("glowing", "recording"), null);
  // A name with no tuned entry still gets the generic state -- the language
  // is meant to work on any pattern, including ones added later.
  assert.deepEqual(
    voiceStateProfile("notAPattern", "idle"),
    { ...voiceStateProfile("working", "idle") },
    "an untuned pattern gets the generic state",
  );
});

test("listening draws the shape in, speaking pushes it out", () => {
  const listening = voiceStateProfile("breathing", "listening");
  const speaking = voiceStateProfile("breathing", "speaking");
  assert.ok(listening.overrides.audioStrength < 0, "the inhale");
  assert.ok(speaking.overrides.audioStrength > 0, "the swell");
  assert.equal(listening.audioInput, "micLevel");
  assert.equal(speaking.audioInput, "agentVolume");
  assert.equal(voiceStateProfile("breathing", "thinking").audioInput, null);
});

test("the engine's values are the checked-in file's values", () => {
  // The Studio diffs "tuned by you" against the engine, so the two must agree.
  const engine = voiceStateProfile("muted", "listening");
  const fromFile = { ...file.states.listening.overrides, ...file.patterns.muted.states.listening.overrides };
  for (const [k, v] of Object.entries(fromFile)) assert.equal(engine.overrides[k], v, k);
  assert.equal(parameterCatalog().profileVersion, file.profileVersion);
});

test("a 1.8 spec inherits the profile; the file's own values win", () => {
  const spec = JSON.stringify({
    fxSpec: "1.8",
    object: "orb",
    pattern: "glowing",
    states: { idle: {}, listening: { ink: 0.4 } },
  });
  const idle = resolveFxSpec(spec, { state: "idle" });
  assert.equal(idle.overrides.ink, voiceStateProfile("glowing", "idle").overrides.ink);
  assert.ok(idle.speed < 1, "idle is slower");
  assert.equal(resolveFxSpec(spec, { state: "listening" }).overrides.ink, 0.4, "the file wins");
});

test("a null in an entry opts its keys out of the profile", () => {
  // docs/fx-spec.md: a `null` in a patch removes the key and opts it out of the
  // profile too -- a whole section's null included.
  const spec = JSON.stringify({
    fxSpec: "1.8", object: "orb", pattern: "working",
    materials: { glow: { strength: 0.3 } },
    states: { thinking: { materials: { glow: null } } },
  });
  assert.ok("glowStrength" in voiceStateProfile("working", "thinking").overrides, "precondition: the profile has glow");
  const r = resolveFxSpec(spec, { state: "thinking" });
  assert.ok(r.ok, JSON.stringify(r.diagnostics));
  assert.equal(r.overrides.glowStrength, undefined);
});
