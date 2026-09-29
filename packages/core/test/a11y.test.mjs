// State-aware accessibility through the wasm (docs/fx-view.md, *Accessibility*):
// spec/a11y-announce-vectors.json, driven with the host loop every platform runs
// (a step on each change and at recheckAt), and the accessible-name cases.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { accessibleName, announceStep, fxSpecAccessibility } from "../dist/index.js";

const vectors = JSON.parse(readFileSync(new URL("../../../spec/a11y-announce-vectors.json", import.meta.url), "utf8"));

function run(steps, end) {
  let s = null;
  const said = [];
  let pending = null;
  let i = 0;
  let words = null;
  for (;;) {
    const next = i < steps.length ? steps[i][0] : null;
    const t = next != null && pending != null ? Math.min(next, pending) : next ?? pending;
    if (t == null || t > end) break;
    if (next === t) words = steps[i++][1];
    const out = announceStep(s, words, t);
    s = out.state;
    pending = out.recheckAt;
    if (out.announce != null) said.push([t, out.announce]);
  }
  return said;
}

test("the announcer says the same things at the same moments as the engine vectors", () => {
  assert.ok(vectors.cases.length >= 6);
  for (const c of vectors.cases) assert.deepEqual(run(c.steps, c.end), c.said, c.name);
});

test("accessible names match the engine vectors", () => {
  for (const n of vectors.names) assert.equal(accessibleName(n.name, n.state, n.spec, n.app), n.expect, `${n.name} ${n.state}`);
});

test("the spec's accessibility block reads leniently", () => {
  const a = fxSpecAccessibility(JSON.stringify({ fxSpec: "1.9", object: "orb", pattern: "glowing", accessibility: { name: "Coach", states: { listening: "Go" }, announce: false } }));
  assert.deepEqual(a, { name: "Coach", states: { listening: "Go" }, announce: false });
  assert.deepEqual(fxSpecAccessibility("{}"), { name: null, states: {}, announce: null });
});
