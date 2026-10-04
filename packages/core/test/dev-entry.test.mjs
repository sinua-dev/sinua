// @sinua/core/dev (design note 33): the default entry ships no Studio / dev
// exports, never loads the dev wasm, and the dev entry is the whole API on one
// (dev) wasm -- the Studio aliases @sinua/core to it and must run one engine.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const read = (p) => readFileSync(here(p), "utf8");
const DEV_EXPORTS = [
  "conversation_at_json", "conversation_samples_json", "conversation_sample_json",
  "check_overrides_json", "estimate_cost_json", "fx_spec_cost_json", "liquid_suitability_json",
  "parameter_catalog_json", "frame_json", "frame_json_with_overrides", "frame_from_fx_spec_json",
];
const DEV_API = [
  "parameterCatalog", "checkOverrides", "estimateCost", "fxSpecCost", "liquidSuitability",
  "conversationAt", "conversationSampleNames", "conversationSample", "SimulatedVoiceSource",
  "frameViaJson", "frameWithOverridesViaJson", "frameFromFxSpecViaJson",
];

test(`the default wasm has none of the ${DEV_EXPORTS.length} dev exports; the dev wasm has them all`, () => {
  const main = read("../pkg/sinua_core.d.ts"), dev = read("../pkg-dev/sinua_core.d.ts");
  for (const f of DEV_EXPORTS) {
    assert.ok(!main.includes(`function ${f}(`), `pkg/ exports ${f}`);
    assert.ok(dev.includes(`function ${f}(`), `pkg-dev/ lacks ${f}`);
  }
});

test("dist/ loads only the default wasm except through dev.js; dist-dev/ only the dev wasm", () => {
  let checked = 0;
  for (const f of readdirSync(here("../dist")).filter((f) => f.endsWith(".js"))) {
    const s = read(`../dist/${f}`);
    if (f === "dev.js" || f === "conversation.js") assert.ok(s.includes("../pkg-dev/"), f);
    else assert.ok(!s.includes("pkg-dev"), `dist/${f} reaches the dev wasm`);
    checked++;
  }
  for (const f of readdirSync(here("../dist-dev")).filter((f) => f.endsWith(".js"))) {
    assert.ok(!read(`../dist-dev/${f}`).includes('"../pkg/'), `dist-dev/${f} reaches the default wasm`);
  }
  assert.ok(checked > 10, `only ${checked} modules`);
});

test(`the ${DEV_API.length} dev functions are in @sinua/core/dev only, which also has the whole default API`, async () => {
  const main = await import("../dist/index.js");
  const dev = await import("../dist-dev/dev-entry.js");
  for (const k of DEV_API) {
    assert.equal(main[k], undefined, `@sinua/core still exports ${k}`);
    assert.ok(dev[k] !== undefined, `@sinua/core/dev lacks ${k}`);
  }
  const missing = Object.keys(main).filter((k) => !(k in dev));
  assert.deepEqual(missing, [], "the dev entry is a superset");
});
