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

// FX Spec 1.12: a character recipe carried in the spec (design note 12).
import { readFileSync } from "node:fs";
const pip = JSON.parse(
  readFileSync(new URL("../../../spec/examples/custom-character.fxspec.json", import.meta.url), "utf8"),
);
const chirpRecipe = JSON.parse(
  readFileSync(new URL("../../../spec/characters/chirp.json", import.meta.url), "utf8"),
);

test("a recipe in the spec resolves to its key and draws", () => {
  const r = resolveFxSpec(pip, "speaking");
  assert.ok(r.ok, JSON.stringify(r.diagnostics));
  assert.match(r.state, /^recipe:pip:[0-9a-f]{16}$/);
  const f = frameWithOverrides(r.state, 64, 1.3, r.overrides);
  assert.equal(f.colorMode, "fixed");
  assert.ok(f.fills.length > 20);
});

test("a copy of chirp's recipe draws chirp's frame", () => {
  const { $comment, ...recipe } = chirpRecipe;
  const r = resolveFxSpec({ fxSpec: "1.12", object: "character", pattern: "twin", recipe: { ...recipe, id: "twin" } });
  assert.ok(r.ok, JSON.stringify(r.diagnostics));
  const o = { mouthTalk: 1, audioLevel: 0.7, flutterGain: 0.3, turnYaw: 0.5 };
  assert.deepEqual(frameWithOverrides(r.state, 64, 0.7, o), frameWithOverrides("chirp", 64, 0.7, o));
});

test("a recipe needs 1.12 and its errors point into it", () => {
  assert.equal(resolveFxSpec({ ...pip, fxSpec: "1.11" }).ok, false);
  const bad = structuredClone(pip);
  bad.recipe.parts[2].segments = 1000;
  const r = resolveFxSpec(bad);
  assert.equal(r.ok, false);
  assert.ok(r.diagnostics.some((d) => d.path === "/recipe/parts/2/segments"), JSON.stringify(r.diagnostics));
});

test("paletteOverrides: the FX Spec's rules, for a built-in and for a file's recipe", async () => {
  const { paletteOverrides, characterRecipe } = await import("../dist/index.js");
  const red = paletteOverrides("buzzy", { body: "#E63946" });
  assert.equal(red.overrides["palette.body.w"], 1);
  assert.ok("palette.bodyDark.l" in red.overrides, "the tones follow");
  assert.deepEqual(red.diagnostics, []);
  const typo = paletteOverrides("cuppa", { mugg: "#000000" });
  assert.deepEqual(typo.overrides, {});
  assert.match(typo.diagnostics[0].message, /did you mean `mug`/);
  const recipe = JSON.parse(readFileSync(new URL("../../../spec/examples/custom-character.fxspec.json", import.meta.url), "utf8")).recipe;
  const bean = characterRecipe("bean");
  assert.equal(bean.id, "bean");
  assert.ok("body" in bean.palette, "the recipe's palette names are the slots");
  assert.equal(characterRecipe("working"), null);
  const own = Object.keys(recipe.palette)[0];
  const r = paletteOverrides(recipe.id, { [own]: "#123456" }, recipe);
  assert.deepEqual(r.diagnostics, []);
  assert.equal(r.overrides[`palette.${own}.w`], 1);
});
