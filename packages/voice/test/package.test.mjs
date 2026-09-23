// Package hygiene: each subpath only pulls what it needs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const DIST = join(dirname(fileURLToPath(import.meta.url)), "../dist");
const pkg = JSON.parse(readFileSync(join(DIST, "../package.json"), "utf8"));

/** Every bare specifier reachable from `entry` through static imports/exports. */
function externals(entry, seen = new Set(), out = new Set()) {
  if (seen.has(entry)) return out;
  seen.add(entry);
  const src = readFileSync(join(DIST, entry), "utf8");
  for (const m of src.matchAll(/(?:import|export)\s[^"']*?from\s*["']([^"']+)["']|import\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/g)) {
    const spec = m[1] ?? m[2] ?? m[3];
    if (spec.startsWith(".")) externals(join(dirname(entry), spec), seen, out);
    else out.add(spec);
  }
  return out;
}

test("every subpath is exported with types", () => {
  for (const sub of [".", "./livekit", "./openai", "./gemini", "./elevenlabs", "./mic", "./tone"]) {
    const e = pkg.exports[sub];
    assert.ok(e?.types && e?.default, sub);
    readFileSync(join(DIST, "..", e.default));
    readFileSync(join(DIST, "..", e.types));
  }
});

test("only /livekit reaches livekit-client, and nothing imports @sinua/core at runtime", () => {
  for (const entry of ["index.js", "mic.js", "tone.js", "openai.js", "gemini.js", "elevenlabs.js"]) {
    assert.deepEqual([...externals(entry)], [], `${entry} imports no package at runtime`);
  }
  assert.deepEqual([...externals("livekit.js")], ["livekit-client"]);
  assert.equal(pkg.peerDependenciesMeta["livekit-client"].optional, true);
});
