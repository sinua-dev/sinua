// What `npm pack` ships: the inlined wasm glue must be in the tarball
// (wasm-pack's pkg/.gitignore once made npm drop pkg/ entirely).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

test("npm pack ships the inlined wasm and no raw .wasm / ignore files", () => {
  const out = execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: ROOT, encoding: "utf8" });
  const files = JSON.parse(out)[0].files.map((f) => f.path);
  for (const f of ["dist/index.js", "dist/index.d.ts", "pkg/sinua_core.js", "pkg/sinua_core.d.ts", "pkg/sinua_core_inline.js", "pkg/sinua_core_inline.d.ts"]) {
    assert.ok(files.includes(f), `${f} is packed`);
  }
  assert.equal(files.filter((f) => f.endsWith(".wasm") || f.endsWith(".gitignore")).length, 0);
});

test("the glue has no import.meta.url wasm fallback (bundlers would emit an unused asset)", () => {
  const glue = readFileSync(join(ROOT, "pkg/sinua_core.js"), "utf8");
  assert.equal(/new URL\([^)]*\.wasm/.test(glue), false);
  const inline = readFileSync(join(ROOT, "pkg/sinua_core_inline.js"), "utf8");
  assert.match(inline, /initSync\(\{ module: inflateSync\(decode\(WASM_DEFLATE_BASE64\)/);
  assert.equal(/from\s*["']fflate["']/.test(inline), false, "fflate is bundled in, not a bare import");
});

test("pkg/ has no package.json: wasm-pack's `sideEffects` list let bundlers drop the inline init", () => {
  assert.equal(existsSync(join(ROOT, "pkg/package.json")), false);
  // The full proof (esbuild / Vite / webpack production bundles, run) is bundler-check/.
});
