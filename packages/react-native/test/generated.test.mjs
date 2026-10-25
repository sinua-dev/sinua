// The generated typed components (scripts/codegen), React Native copy.
//
// `src/generated/SinuaParams.ts` is emitted from spec/parameters.json and is
// byte-identical to `packages/web/src/generated/SinuaParams.ts`. The web copy
// has had a test since it landed; this one had none, so the same contract was
// only ever checked on one of the two copies. These are the web test's cases
// (packages/web/test/generated.test.mjs) run against this copy, plus an
// assertion that the two copies really are identical -- if codegen ever emits
// them differently, that stops being an assumption and starts being a failure.
//
// Its element-registry case has no counterpart here: custom elements are a Web
// thing, so React Native's emitter produces no SinuaElements.
//
// The generated `.tsx` components stay at 0% coverage in this package **by
// design** -- not an oversight, and not a gap to be closed here. Two measured
// reasons: node 24 strips types from `.ts` but does not transform JSX, so
// importing them fails with ERR_UNKNOWN_FILE_EXTENSION; and they call
// React.useEffect, so a direct call throws `Invalid hook call` and there is no
// renderer in this package (react-test-renderer is deprecated for React 19).
// Rather than add a JSX transform and a test-only renderer to a package we
// publish, their shape is pinned where it actually varies: the emitter tests in
// `scripts/codegen/test/`, over the emitted text. Decided 2026-09-20.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const {
  sinuaOrbOverrides,
  sinuaSignalOverrides,
  sinuaSpecError,
} = await import("../src/generated/SinuaParams.ts");

const here = dirname(fileURLToPath(import.meta.url));
const RN_COPY = join(here, "../src/generated/SinuaParams.ts");
const WEB_COPY = join(here, "../../web/src/generated/SinuaParams.ts");

test("a material group becomes its engine keys", () => {
  assert.deepEqual(sinuaOrbOverrides("glowing", { glow: { strength: 0.6 } }), { glowStrength: 0.6 });
});

test("a list prop spreads over the indexed keys on the patterns that take one", () => {
  assert.deepEqual(sinuaSignalOverrides("dune", { envelope: [0.2, 0.5] }), { envelope0: 0.2, envelope1: 0.5 });
  // A scalar prop keeps its key.
  assert.deepEqual(sinuaSignalOverrides("dune", { progress: 0.4 }), { progress: 0.4 });
});

test("choices and booleans map to engine numbers", () => {
  const o = sinuaOrbOverrides("glowing", { glow: { strength: 0.5, mode: "blur" } });
  assert.equal(o.glowStrength, 0.5);
  assert.equal(typeof o.glowMode, "number");
});

// There is one mapping function per object and they are generated
// independently, so a codegen regression can land on one object alone.
test("every object's own parameters map to their engine keys", () => {
  assert.deepEqual(sinuaSignalOverrides("veil", { layerCount: 9, churn: 0.5 }), { layerCount: 9, churn: 0.5 });
  // The shared material groups resolve the same way on every object.
  for (const fn of [sinuaOrbOverrides, sinuaSignalOverrides]) {
    assert.deepEqual(fn("veil", { glow: { strength: 0.3 } }), { glowStrength: 0.3 });
  }
});

test("a spec for another object is reported, the right one passes", () => {
  assert.match(sinuaSpecError({ object: "signal" }, "orb"), /not a "orb"/);
  assert.equal(sinuaSpecError({ object: "orb" }, "orb"), null);
});

test("this copy is byte-identical to @sinua/web's", (t) => {
  if (!existsSync(WEB_COPY)) {
    return t.skip("packages/web is not present; run this from the repo to check the two copies");
  }
  assert.equal(
    readFileSync(RN_COPY, "utf8"),
    readFileSync(WEB_COPY, "utf8"),
    "the two generated copies have diverged -- re-run `node scripts/codegen/generate.mjs`",
  );
});
