// The parameter catalog through wasm equals the checked-in spec/parameters.json,
// and checkOverrides warns (never blocks) -- docs/parameters.md, Parameter catalog.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parameterCatalog, checkOverrides, frameWithOverrides } from "../dist-dev/dev-entry.js";

const full = JSON.parse(readFileSync(fileURLToPath(new URL("../../../spec/parameters.json", import.meta.url)), "utf8"));
// The runtime carries the catalog without its words (design note 10).
const strip = (v) =>
  Array.isArray(v) ? v.map(strip)
  : v && typeof v === "object" ? Object.fromEntries(Object.entries(v).filter(([k]) => k !== "description").map(([k, x]) => [k, strip(x)]))
  : v;
const file = strip(full);

test("parameterCatalog() equals spec/parameters.json without the descriptions", () => {
  const c = parameterCatalog();
  assert.deepEqual(c, file);
  assert.deepEqual(c.objects.map((o) => [o.id, o.component, o.patterns.length]), [
    ["orb", "SinuaOrb", 18], ["signal", "SinuaSignal", 5], ["ring", "SinuaRing", 6], ["core", "SinuaCore", 2], ["beacon", "SinuaBeacon", 5], ["character", "SinuaCharacter", 7],
  ]);
  for (const o of c.objects) for (const p of o.patterns) for (const r of p.params) assert.ok(c.definitions[r.ref], `${p.id}: ${r.ref}`);
  assert.equal(c.definitions["glowStrength@shared"].path, "glow.strength");
  assert.equal(c.definitions["progress@nested"].type, "number[]");
});

test("checkOverrides: warnings, did-you-mean, ranges, renames; the frame is unchanged", () => {
  assert.match(checkOverrides("breathing", 64, { lanse: 6 })[0].message, /unknown key `lanse` for orb\/breathing \(did you mean `lanes`\?\)/);
  assert.equal(checkOverrides("breathing", 64, { lanse: 6 })[0].severity, "warning");
  assert.match(checkOverrides("connecting", 64, { lineWidth: 1 })[0].message, /did you mean `lineW`/);
  // The taxonomy retrofit's old names are plain unknown keys since the FX Spec 1.8 floor.
  assert.match(checkOverrides("glowing", 64, { nodeN: 90 })[0].message, /unknown key `nodeN`/);
  assert.deepEqual(checkOverrides("tracking", 64, { progress1: 2, glowStrength: 0.4, audioLevel: 0.2 }), []);
  assert.deepEqual(frameWithOverrides("breathing", 64, 1, { lanse: 6 }), frameWithOverrides("breathing", 64, 1, {}));
});

test("binders accept the catalog paths (FX Spec 1.7 names)", async () => {
  const { bindReactiveInput, reactiveTargetKey } = await import("../dist/index.js");
  assert.equal(reactiveTargetKey("glow.strength"), "glowStrength");
  assert.deepEqual(
    bindReactiveInput({ value: 0.5, target: "progress[1]" }),
    bindReactiveInput({ value: 0.5, target: "progress1" }),
  );
});
