// The materials cross-platform check compares three renderers, and the list of
// cases it renders is written out three times:
//
//   Web      packages/web/scripts/materials/frames.mjs  -- derived from
//            spec/sinua-golden.json by regex, plus SYNTHETIC
//   Swift    packages/ios/Tests/SinuaTests/MaterialsRenderTests.swift  -- a literal
//   Kotlin   packages/android/view/src/androidTest/.../SinuaViewTest.kt  -- a literal
//
// The README asks a human to keep them in sync. This is that check.
//
// It matters in **both** directions, which is the part worth being careful
// about. The comparison is driven by the Web list, so a case dropped from the
// Swift literal becomes a render that never arrives -- now a named failure in
// compare.cjs, but only because it was expected. A case *added* to the Swift
// literal and nowhere else is invisible to that check entirely: nothing asks
// for it, so nothing misses it. A comparison against a set built by filtering
// can only fail one way; this one compares sets, so it fails both.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const read = (p) => readFileSync(join(root, p), "utf8");

/**
 * The keys the Web side will render -- by **running `frames.mjs`**, not by
 * restating its rule. My first attempt re-derived the selection from the
 * golden file with frames.mjs's own `MATERIALS` regex, and it was wrong:
 * `tracking-64-0.6-gradient3` is selected by `hasHues(frame)`, which can only
 * be known by resolving the frame through the wasm engine. Re-deriving a
 * selection rule is how a check ends up testing the copy instead of the thing.
 */
let cached = null;
function webKeys() {
  if (cached) return cached;
  const out = join(mkdtempSync(join(tmpdir(), "materials-cases-")), "frames.json");
  const script = join(root, "packages/web/scripts/materials/frames.mjs");
  execFileSync(process.execPath, ["--experimental-wasm-modules", script, out], {
    cwd: dirname(script),
    stdio: ["ignore", "ignore", "inherit"],
  });
  cached = new Set(JSON.parse(readFileSync(out, "utf8")).map((f) => f.key));
  return cached;
}

const keysIn = (text) => new Set([...text.matchAll(/"((?:x-)?[a-z][a-z0-9-]*-\d+-[\d.]+-[a-z0-9-]+)"/g)].map((m) => m[1]));

const swiftKeys = () => keysIn(read("packages/ios/Tests/SinuaTests/MaterialsRenderTests.swift"));
const kotlinKeys = () => keysIn(read("packages/android/view/src/androidTest/kotlin/dev/sinua/view/SinuaViewTest.kt"));

/** Reports the differing names, in both directions -- never a count. */
function assertSameSet(a, b, aName, bName) {
  const onlyA = [...a].filter((k) => !b.has(k)).sort();
  const onlyB = [...b].filter((k) => !a.has(k)).sort();
  assert.deepEqual(
    { [`only in ${aName}`]: onlyA, [`only in ${bName}`]: onlyB },
    { [`only in ${aName}`]: [], [`only in ${bName}`]: [] },
    `${aName} and ${bName} render different materials cases`
  );
}

test("materials: the Swift render list matches the Web's", () => {
  const web = webKeys();
  assert.ok(web.size > 15, `only ${web.size} web cases -- the golden file or frames.mjs changed shape`);
  assertSameSet(web, swiftKeys(), "the Web list (frames.mjs + golden)", "MaterialsRenderTests.swift");
});

test("materials: the Kotlin render list matches the Web's", () => {
  assertSameSet(webKeys(), kotlinKeys(), "the Web list (frames.mjs + golden)", "SinuaViewTest.kt");
});

test("materials: the tolerance and the AA exception are the ones docs/fx-view.md decided", () => {
  // compare.cjs is the enforcement point for a contract the user decided, so
  // the numbers are pinned here too. Changing a threshold should have to
  // change this test, deliberately, the way a golden re-baseline does.
  const compare = read("packages/web/scripts/materials/compare.cjs");
  assert.match(compare, /ENV_EXCEPTIONS\[r\.key\]\?\.mean \?\? 2;/, "the default mean limit is no longer 2");
  assert.match(compare, /v\.mean > meanLimit \|\| v\.p99 > 24/, "the tolerance is no longer mean <= limit / p99 <= 24");
  const aa = [...compare.matchAll(/^\s{2}"([^"]+)":\s*"/gm)].map((m) => m[1]);
  assert.deepEqual(aa, ["drifting-64-0.6-particles-liquid"], "the AA exception list changed");
  // Renderer-environment exceptions (user decision 2026-09-21): exactly this case, at exactly 3.
  const env = [...compare.matchAll(/^\s{2}"([^"]+)":\s*\{\s*mean:\s*([0-9.]+)/gm)].map((m) => `${m[1]}=${m[2]}`);
  assert.deepEqual(env, ["tracking-64-0.6-glow-blur-additive=3"], "the environment exception list changed");
});
