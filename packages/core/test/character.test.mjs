// The character family through wasm (FX Spec 1.11, crates/core_engine/src/character/):
// a character is fills only, in a fixed palette, and a state change reaches it as
// `stateAge` so it can blink at the end of the user's turn.
import { test } from "node:test";
import assert from "node:assert/strict";
import { frameWithOverrides, resolveFxSpec, voiceStateProfile, StateTransition } from "../dist/index.js";

test("buzzy is fills only, in a fixed palette", () => {
  const f = frameWithOverrides("buzzy", 64, 0.3, {});
  assert.equal(f.colorMode, "fixed");
  assert.equal(f.dots.length + f.lines.length + f.polylines.length, 0);
  assert.ok(f.fills.length > 20);
});

test("the turn blink needs stateAge, and StateTransition supplies it after a change", () => {
  const thinking = { ...voiceStateProfile("buzzy", "thinking").overrides, look: 0 };
  const eyeArea = (f) => f.fills.reduce((a, x) => a + x.points.length, 0);
  const blink = frameWithOverrides("buzzy", 64, 0.1, { ...thinking, stateAge: 0.09 });
  const open = frameWithOverrides("buzzy", 64, 0.1, { ...thinking, stateAge: 1 });
  assert.notDeepEqual(blink, open);
  assert.ok(eyeArea(blink) > 0 && eyeArea(open) > 0);

  const tr = new StateTransition();
  assert.equal(tr.stateAge, null, "the first state is not a change");
  const side = (state) => ({ state: "buzzy", speed: 1, overrides: voiceStateProfile("buzzy", state).overrides });
  tr.start(0.6, "easeInOut"); // nothing on screen yet: still not a change
  tr.frames(side("listening"), 64, 0);
  assert.equal(tr.stateAge, null);
  tr.start(0.6, "easeInOut");
  tr.advance(0.05);
  assert.ok(Math.abs(tr.stateAge - 0.05) < 1e-9);
});

test("a character spec needs 1.11 and takes hue, not colour", () => {
  const base = { object: "character", pattern: "buzzy" };
  assert.equal(resolveFxSpec({ fxSpec: "1.10", ...base }).ok, false);
  assert.equal(resolveFxSpec({ fxSpec: "1.11", ...base, color: "#ff0000" }).ok, false);
  const r = resolveFxSpec({ fxSpec: "1.11", ...base, params: { hue: 150 } });
  assert.ok(r.ok, JSON.stringify(r.diagnostics));
  assert.equal(r.overrides.hue, 150);
});
