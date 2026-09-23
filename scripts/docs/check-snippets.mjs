#!/usr/bin/env node
// Checks the docs site's code samples (apps/site/snippets/), both directions.
//
// Forward -- is every sample correct?
//   - spec/*.fxspec.json: resolves with the engine for the base design and
//     every lifecycle state, low power on and off -- ok, and zero
//     diagnostics (a sample that warns teaches the wrong thing);
//   - ts/*.ts: type-checks against @sinua/core's types (tsc -p).
// Other snippet folders (native, web platforms) are checked by their
// owners' builds. Needs packages/core built.
//
// Reverse -- is every sample reachable? Added 2026-09-20.
// Nothing used to ask whether a snippet was
// included by anything, and the cost of that gap was not a pile of dead files:
// it was that **three sessions measuring this tree produced three different
// orphan counts** (13, 10 and 12), all of them wrong. With the references
// resolved properly there are **no orphans at all** -- every content file is on
// a page. The check exists so the next answer is computed, not estimated.
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const dir = join(root, "apps/site/snippets");
const { resolveFxSpec } = await import(join(root, "packages/core/dist/index.js"));

// Sample inputs every binding in the samples can read.
const inputs = { micMuted: 0, micLevel: 0.5, agentVolume: 0.5, uploaded: 40, steps: 6200, waterMl: 1800, activeMinutes: 12, heartRate: 110, level: 0.5 };
let failures = 0;
let resolved = 0;
const specDir = join(dir, "spec");
for (const f of readdirSync(specDir).filter((f) => f.endsWith(".fxspec.json")).sort()) {
  const text = readFileSync(join(specDir, f), "utf8");
  const base = resolveFxSpec(text, { inputs });
  for (const state of [undefined, ...(base.stateKeys ?? [])]) {
    for (const lowPower of [false, true]) {
      const r = resolveFxSpec(text, { state, inputs, lowPower });
      resolved++;
      if (!r.ok || r.diagnostics.length) {
        failures++;
        console.error(`${f} state=${state ?? "(base)"} lowPower=${lowPower}:`, JSON.stringify(r.diagnostics));
      }
    }
  }
}
console.log(`spec snippets: ${resolved} resolutions, ${failures} with diagnostics`);

const tsc = join(root, "packages/core/node_modules/.bin/tsc");
if (!existsSync(tsc)) {
  console.error("packages/core has no tsc; run npm ci there first");
  process.exit(1);
}
try {
  execFileSync(tsc, ["-p", join(dir, "tsconfig.json")], { stdio: "inherit" });
  console.log("ts snippets: type-check ok");
} catch {
  failures++;
}

// ---------------------------------------------------------------------------
// Reverse pass: every snippet must be reachable from a page.
//
// A snippet counts as referenced when either:
//   1. an MDX <include> resolves to it -- the path is relative to the .mdx
//      file, so it is resolved, never substring-matched; or
//   2. its snippets-relative path appears as a quoted string in the site's own
//      code or MDX (that is how <Demo spec="spec/x.fxspec.json" /> reaches one)
//      or in scripts/.
//
// Matching is on the snippets-relative path, NEVER on a bare basename. That
// distinction is the whole point: a basename like `tsconfig.json` or
// `react.tsx` occurs all over any tree, so a loose matcher reports references
// that are not there -- and a matcher that only looks for `snippets/<path>`
// misses `<Demo spec="...">`. Both mistakes were made while measuring this.
const SKIP_DIRS = new Set(["build", "node_modules", ".gradle", ".build", "dist", "Pods", ".next", "out"]);

// Files that exist so the compile harnesses work. No page will ever include
// them, and their absence would break the checks above.
const SCAFFOLDING = {
  "Package.swift": "SwiftPM manifest for the Swift samples' compile harness",
  "Package.resolved": "its pinned dependencies",
  "android/build.gradle.kts": "the Kotlin samples' compile harness",
  "tsconfig.json": "ts/*.ts type-check (this script)",
  "tsconfig.web.json": "the Web samples' type-check (check-code-snippets.sh)",
  "tsconfig.rn.json": "the React Native samples' type-check",
};

const walk = (d, out = []) => {
  for (const e of readdirSync(d, { withFileTypes: true })) {
    if (SKIP_DIRS.has(e.name)) continue;
    const p = join(d, e.name);
    if (e.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
};

const snippets = walk(dir).map((p) => relative(dir, p)).sort();
const referenced = new Set();

// (1) MDX includes, resolved against the page they appear on.
const contentDir = join(root, "apps/site/content");
for (const page of walk(contentDir).filter((p) => p.endsWith(".mdx"))) {
  const text = readFileSync(page, "utf8");
  for (const m of text.matchAll(/<include[^>]*>([^<]+)<\/include>/g)) {
    const abs = resolve(dirname(page), m[1].trim());
    if (abs.startsWith(dir)) referenced.add(relative(dir, abs));
  }
}

// (2) quoted snippets-relative paths in the site's code/MDX and in scripts/.
const sources = [
  ...walk(join(root, "apps/site")).filter((p) => /\.(tsx?|mjs|mdx)$/.test(p) && !p.startsWith(dir)),
  ...walk(join(root, "scripts")).filter((p) => /\.(mjs|sh)$/.test(p)),
];
const haystack = sources.map((p) => readFileSync(p, "utf8")).join("\n");
for (const f of snippets) {
  if (haystack.includes(`"${f}"`) || haystack.includes(`'${f}'`) || haystack.includes(`snippets/${f}`)) {
    referenced.add(f);
  }
}

// Scaffolding is counted as scaffolding even when something happens to mention
// it, so the three numbers always add up to the total. They are printed because
// the point of this pass is that the count stops being a matter of opinion.
const scaffolding = snippets.filter((f) => f in SCAFFOLDING);
const onPage = snippets.filter((f) => !(f in SCAFFOLDING) && referenced.has(f));
const orphans = snippets.filter((f) => !(f in SCAFFOLDING) && !referenced.has(f));
console.log(
  `snippet reachability: ${snippets.length} files = ${onPage.length} on a page ` +
    `+ ${scaffolding.length} compile-harness scaffolding + ${orphans.length} unreferenced`,
);
if (orphans.length) {
  failures++;
  console.error(
    `\n${orphans.length} snippet file(s) no page includes. Either reference them from a page, ` +
      `delete them, or -- if one is compile-harness scaffolding -- add it to SCAFFOLDING in this file with a reason:`,
  );
  for (const f of orphans) console.error(`  apps/site/snippets/${f}`);
}
// The allow-list must not rot either: an entry whose file is gone is a lie.
for (const f of Object.keys(SCAFFOLDING)) {
  if (!snippets.includes(f)) {
    failures++;
    console.error(`SCAFFOLDING lists apps/site/snippets/${f}, which does not exist -- remove the entry`);
  }
}

process.exit(failures ? 1 : 0);
