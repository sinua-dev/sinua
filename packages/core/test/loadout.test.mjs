// Loadouts (FX Spec 1.13, design note 25) through wasm: the shared vectors
// (spec/loadout-vectors.json, also checked by Rust, iOS and Android), what fits,
// thumbnails, and the soft change on its own clock beside a state change.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { FxSpecPlayer, StateTransition, WEAR_S, applyLoadout, cosmeticsFor, frameStill, resolveFxSpec } from "../dist/index.js";

const read = (f) => readFileSync(fileURLToPath(new URL(`../../../spec/${f}`, import.meta.url)), "utf8");
const vectors = JSON.parse(read("loadout-vectors.json"));
const spec = read(vectors.spec);

test("the shared loadout vectors hold", () => {
  for (const c of vectors.cases) {
    const r = applyLoadout(spec, c.loadout);
    const out = JSON.parse(r.spec);
    assert.deepEqual((out.cosmetics ?? []).map((x) => x.id), c.wear, `${c.name}: worn`);
    assert.deepEqual(out.palette ?? null, c.palette, `${c.name}: palette`);
    assert.deepEqual(out.params?.eyeStyle ?? null, c.eyeStyle, `${c.name}: eyeStyle`);
    assert.deepEqual(r.diagnostics.map((d) => d.path).sort(), c.warnings, `${c.name}: warnings`);
    assert.ok(r.diagnostics.every((d) => d.severity === "warning"));
    assert.ok(resolveFxSpec(r.spec).ok, `${c.name}: still draws`);
  }
  for (const f of vectors.fits) {
    const got = Object.fromEntries(cosmeticsFor(spec, f.character).map((x) => [x.id, x.reason]));
    assert.deepEqual(got, f.expect, f.character);
    for (const x of cosmeticsFor(spec, f.character)) assert.equal(x.fits, x.why === "", x.id);
  }
});

test("a thumbnail is a still frame, the same every time", () => {
  const a = frameStill(spec, 64, { loadout: { wear: ["round-glasses"], palette: "sunset" } });
  assert.ok(a && a.fills.length > 0);
  assert.deepEqual(frameStill(spec, 64, { loadout: { wear: ["round-glasses"], palette: "sunset" } }), a);
  assert.notDeepEqual(frameStill(spec, 64, { loadout: { wear: ["round-glasses"], palette: "sunset" }, turnYaw: 0.5 }), a);
  assert.equal(frameStill("{", 64), null);
});

test("a loadout change eases on its own clock, beside a state change", () => {
  const player = new FxSpecPlayer(spec, { loadout: { wear: [] } });
  player.frame(0, 0.016);
  const plain = player.frame(0.1, 0.016).frame;
  player.setLoadout({ wear: ["party-hat"] });
  // A state change starts in the middle of it: neither cuts the other.
  player.frame(0.15, 0.05);
  player.setState("listening");
  const mid = player.frame(0.2, 0.05);
  assert.ok(mid.frame);
  // Past the loadout's 0.35 s the hat is on; the state change (0.6 s) still runs.
  let out;
  for (let i = 0; i < 6; i++) out = player.frame(0.25 + i * 0.05, 0.05);
  assert.ok(out.previous === null || out.blend <= 1);
  assert.ok(out.frame.fills.length > plain.fills.length, "the hat is worn");
  assert.deepEqual(player.loadoutDiagnostics, []);
});

test("StateTransition: the wear clock and the state clock are separate", () => {
  const tr = new StateTransition();
  const bean = { state: "bean", speed: 1, overrides: {} };
  tr.frames(bean, 64, 1);
  tr.start(0.6, "easeInOut");
  tr.advance(0.1);
  tr.wear();
  assert.ok(tr.wearing && tr.active);
  tr.advance(WEAR_S);
  assert.ok(!tr.wearing && tr.active, "the loadout change ended first, the state change runs on");
  tr.wear();
  tr.cancel();
  assert.ok(!tr.wearing && !tr.active, "cancel (reduced motion) stops both");
});

test("a stale loadout warns and the character still draws", () => {
  const player = new FxSpecPlayer(spec, { loadout: { loadout: 3, wear: ["top-hat"], palette: "neon" } });
  assert.ok(player.frame(0, 0.016).frame);
  assert.deepEqual(player.loadoutDiagnostics.map((d) => d.path).sort(), ["/loadout/loadout", "/loadout/palette", "/loadout/wear/0"]);
});
