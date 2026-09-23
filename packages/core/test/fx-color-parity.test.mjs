// FX Spec color parity: the wasm runtime must reproduce spec/fx-color-vectors.json,
// the vectors every colour picker is held to as well. That makes "a hex picked in
// a tool renders as that color from a spec" a tested property rather than ports
// that happen to agree. Regenerate the vectors only via
// crates/core_engine/tests/fx_color_vectors.rs.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { fxColorToHsl, resolveFxSpec } from "../dist/index.js";

const vectors = JSON.parse(
  readFileSync(fileURLToPath(new URL("../../../spec/fx-color-vectors.json", import.meta.url)), "utf8")
);
const near = (a, b) => Math.abs(a - b) < 1e-12;

test("wasm runtime matches the color and gradient vectors", () => {
  for (const v of vectors.colors) {
    const c = fxColorToHsl(v.input);
    assert.ok(c, v.input);
    assert.equal(c.hex, v.hex);
    assert.equal(c.achromatic, v.h === null);
    if (v.h !== null) assert.ok(near(c.h, v.h), `${v.input}: h`);
    const r = resolveFxSpec({ fxSpec: "1.8", object: "orb", pattern: "working", color: v.input });
    assert.ok(r.ok, JSON.stringify(r.diagnostics));
    assert.equal(r.overrides.colorHue, v.colorHue, `${v.input}: colorHue`);
    assert.equal(r.overrides.colorSaturation, v.colorSaturation, `${v.input}: colorSaturation`);
  }
  for (const g of vectors.gradients) {
    const r = resolveFxSpec({
      fxSpec: "1.8", object: "orb", pattern: "working", gradient: { stops: g.stops, path: g.path },
    });
    const got = Object.fromEntries(Object.entries(r.overrides).filter(([k]) => /^gradientHue\d?$/.test(k)));
    assert.deepEqual(got, g.engine, `${g.stops.join(",")} ${g.path}`);
  }
});
