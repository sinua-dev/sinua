// No runtime import cycle inside `@sinua/core`'s own sources.
//
// There was one: `fxPlayer.ts` imported three values from `./index.js` while
// `index.ts` re-exported `./fxPlayer.js` -- a loop through the package's own
// barrel file. It was benign, because the player only calls those functions
// inside a method, so both modules are fully evaluated first. It is still the
// shape that makes evaluation order bundler-dependent, in the one package whose
// history is two silent bundler-specific breaks (`initSync` tree-shaken, then
// `element-define.js` dropped by `sideEffects: false`) -- neither of which any
// green test noticed.
//
// `import type` / `export type` are erased by tsc, so they are not edges: that
// is exactly why `packed.ts` referring back to `./index.js` is fine and does not
// show up here.
//
// This is a source-shape check, deliberately: dependency-cruiser found the
// original cycle, but it is not a dependency of this repo and only ran because
// somebody invoked it by hand.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const src = join(dirname(fileURLToPath(import.meta.url)), "../src");

/** `./x.js` -> `x`, ignoring anything that is not a sibling module. */
const localName = (spec) => {
  const m = /^\.\/([\w.-]+)\.js$/.exec(spec);
  return m ? m[1] : null;
};

/**
 * Value imports/re-exports only; `import type` and `export type` are erased.
 *
 * Scanned statement by statement rather than with one regex over the file: a
 * lazy `[\s\S]*?` lets a declaration like `export function frame(...)` -- which
 * has no `from` -- start a match and swallow everything up to the *next*
 * module specifier, silently eating the `export * from "./fxPlayer.js"` this
 * check exists to find. (It did exactly that on the first draft.)
 */
const DECLARATION =
  /^\s*export\s+(?:async\s+)?(?:function|class|interface|type|const|let|var|enum|abstract|declare|default|namespace)\b/;

function valueEdges(source) {
  const out = new Set();
  const lines = source
    .replace(/\/\*[\s\S]*?\*\//g, "") // block comments
    .split("\n")
    .filter((l) => !/^\s*\/\//.test(l)); // line comments

  for (let i = 0; i < lines.length; i++) {
    if (!/^\s*(?:import|export)\b/.test(lines[i]) || DECLARATION.test(lines[i])) continue;
    // A statement may wrap over a `{ ... }` clause; join until the specifier.
    let stmt = lines[i];
    for (let j = i + 1; j < lines.length && !/from\s*["']/.test(stmt) && j - i < 20; j++) stmt += " " + lines[j];
    const m = /^\s*(import|export)\s+(type\s+)?([\s\S]*?)from\s*["']([^"']+)["']/.exec(stmt);
    if (!m) continue;
    const [, , isType, clause, spec] = m;
    if (isType) continue;
    const name = localName(spec);
    if (!name) continue;
    // `import { type A, type B } from "x"` is also fully erased.
    const named = clause.match(/\{([\s\S]*)\}/);
    if (named) {
      const parts = named[1].split(",").map((s) => s.trim()).filter(Boolean);
      if (parts.length > 0 && parts.every((p) => p.startsWith("type "))) continue;
    }
    out.add(name);
  }
  return out;
}

function graph() {
  const g = new Map();
  for (const f of readdirSync(src).filter((f) => f.endsWith(".ts"))) {
    g.set(f.replace(/\.ts$/, ""), valueEdges(readFileSync(join(src, f), "utf8")));
  }
  return g;
}

/** Every cycle, each as the path that closes it. */
function cycles(g) {
  const found = [];
  const seen = new Set();
  const walk = (node, path) => {
    const at = path.indexOf(node);
    if (at !== -1) {
      const loop = [...path.slice(at), node];
      // One entry per distinct set of modules: the same loop is reachable from
      // each of its members, and reporting it once reads better.
      const key = [...new Set(loop)].sort().join(",");
      if (!seen.has(key)) {
        seen.add(key);
        found.push(loop.join(" -> "));
      }
      return;
    }
    for (const next of g.get(node) ?? []) walk(next, [...path, node]);
  };
  for (const node of g.keys()) walk(node, []);
  return found;
}

test("no runtime import cycle among packages/core/src modules", () => {
  assert.deepEqual(
    cycles(graph()),
    [],
    "a value import loops back through another module -- move the shared values into a " +
      "module both can import, rather than importing from the barrel",
  );
});
