// Build hygiene across every publishable package.
//
// Why this exists: `@sinua/web` shipped 14 pre-rename files -- `DevinOrb.js`,
// `DevinParams.js` and friends -- in its npm tarball. `tsc` never deletes an
// output whose source is gone, and no package cleaned `dist`, so the Sinua
// rename left them behind and every check we had was blind to it: the tests
// passed, the build passed, the bundle worked. It surfaced only because
// somebody instrumented coverage and noticed files that shouldn't exist.
//
// So there are two assertions here, and they are deliberately both:
//
//   1. every package's `build` removes `dist` first -- the structural fix;
//   2. no `dist` output has a missing source -- the actual condition, which
//      catches a package that lost its clean step, or a `dist` built before
//      one was added.
//
// (1) alone would pass while a stale tree sat on disk; (2) alone would pass on
// a machine that happened to have built recently.
//
// This lives in `packages/core` because it is the package CI always builds and
// tests, and it reaches sideways to its siblings. Run from an installed copy
// there are no siblings, so it skips rather than fails.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";

const packages = join(dirname(fileURLToPath(import.meta.url)), "../..");

/** The publishable packages. `snippets` is studio-ui-ux's; the rest are voice-adapters'. */
const PACKAGES = ["core", "web", "voice", "react-native", "snippets"];

/** Extensions `tsc` emits, longest first so `.d.ts` wins over `.ts`. */
const OUTPUTS = [".d.ts.map", ".d.ts", ".js.map", ".js"];
const SOURCES = [".ts", ".tsx"];

function walk(dir, acc = []) {
  if (!existsSync(dir)) return acc;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, acc);
    else acc.push(p);
  }
  return acc;
}

for (const pkg of PACKAGES) {
  const root = join(packages, pkg);

  test(`${pkg}: build removes dist before compiling`, (t) => {
    if (!existsSync(join(root, "package.json"))) return t.skip(`packages/${pkg} is not present`);
    const { scripts = {} } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
    assert.ok(
      /(^|&&\s*)rm -rf dist\b/.test(scripts.build ?? ""),
      `packages/${pkg}'s build must start by removing dist, else a deleted source leaves its ` +
        `output behind and it ships. Got: ${JSON.stringify(scripts.build)}`,
    );
  });

  test(`${pkg}: every dist output still has a source`, (t) => {
    const dist = join(root, "dist");
    const src = join(root, "src");
    if (!existsSync(dist) || !existsSync(src)) return t.skip(`packages/${pkg} has no built dist`);

    const orphans = [];
    for (const file of walk(dist)) {
      const rel = relative(dist, file);
      const ext = OUTPUTS.find((e) => rel.endsWith(e));
      if (!ext) {
        orphans.push(`${rel} (not a tsc output -- should it be in dist at all?)`);
        continue;
      }
      const base = rel.slice(0, -ext.length);
      if (!SOURCES.some((e) => existsSync(join(src, base + e)))) orphans.push(rel);
    }

    assert.deepEqual(
      orphans,
      [],
      `packages/${pkg}/dist holds output with no source in src/ -- a deleted or renamed file ` +
        `left it behind and \`files\` would ship it. Rebuild the package (its build now cleans dist).`,
    );
  });
}
