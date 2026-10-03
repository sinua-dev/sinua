// @sinua/web/catalog (FX Spec 1.13, design note 26): the subpath's copy of Sinua's
// catalog pack is spec/catalog/catalog-1.json exactly, it loads, and specs can name
// its items; the main entry doesn't carry it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolveFxSpec, unloadCatalog } from "@sinua/core";
import { SINUA_CATALOG, loadSinuaCatalog } from "../dist/catalog.js";

const file = JSON.parse(readFileSync(new URL("../../../spec/catalog/catalog-1.json", import.meta.url), "utf8"));

test("the subpath's pack is spec/catalog/catalog-1.json", () => {
  assert.deepEqual(SINUA_CATALOG, file);
});

test("it loads, and a spec wears and paints from it", () => {
  assert.deepEqual(loadSinuaCatalog(), []);
  assert.deepEqual(loadSinuaCatalog(), [], "loading again is harmless");
  const r = resolveFxSpec(JSON.stringify({ fxSpec: "1.13", object: "character", pattern: "bean", cosmetics: ["catalog:crown"], palette: "catalog:berry" }));
  assert.ok(r.ok && r.diagnostics.length === 0, JSON.stringify(r.diagnostics));
  assert.ok(r.state.startsWith("recipe:bean:"));
  assert.ok(unloadCatalog("catalog"));
  const gone = resolveFxSpec(JSON.stringify({ fxSpec: "1.13", object: "character", pattern: "bean", cosmetics: ["catalog:crown"] }));
  assert.ok(gone.ok && gone.state === "bean" && gone.diagnostics.some((d) => d.path === "/cosmetics/0"));
});

test("the main entry doesn't bundle the pack", () => {
  const main = readFileSync(new URL("../dist/index.js", import.meta.url), "utf8");
  assert.ok(!main.includes("catalog.js") && !main.includes("halo-ring"));
});
