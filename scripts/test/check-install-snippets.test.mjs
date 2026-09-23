// `scripts/docs/check-install-snippets.mjs` must fail when it examined nothing.
//
// It used to do `if (!m) continue` on any line that was not `npm i[nstall] …`,
// so an empty install/ directory and a `pnpm add @sinua/invented` snippet both
// printed its success line and exited 0. The control case -- a real `npm
// install` of a bogus name -- exited 1, which is exactly why the gate looked
// like it worked.
//
// The script derives its root from its own location, so each case gets a
// fixture tree with the real script copied into it. That is deliberate: the
// test runs the shipped file, not a refactored copy of its logic.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, cpSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const REPO = join(dirname(fileURLToPath(import.meta.url)), "../..");
const SCRIPT = join(REPO, "scripts/docs/check-install-snippets.mjs");

/** A fixture repo holding the real script, our package.json names, and `lines`. */
function run(lines) {
  const root = mkdtempSync(join(tmpdir(), "install-snippets-"));
  try {
    mkdirSync(join(root, "scripts/docs"), { recursive: true });
    mkdirSync(join(root, "apps/site/snippets/install"), { recursive: true });
    cpSync(SCRIPT, join(root, "scripts/docs/check-install-snippets.mjs"));
    for (const p of ["core", "web", "voice", "snippets", "react-native"]) {
      mkdirSync(join(root, "packages", p), { recursive: true });
      cpSync(join(REPO, "packages", p, "package.json"), join(root, "packages", p, "package.json"));
    }
    if (lines !== null) writeFileSync(join(root, "apps/site/snippets/install/web.sh"), lines);
    const r = spawnSync(process.execPath, [join(root, "scripts/docs/check-install-snippets.mjs")], {
      encoding: "utf8",
    });
    return { code: r.status, out: r.stdout + r.stderr };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("an empty install/ directory fails instead of reporting success", () => {
  const { code, out } = run(null);
  assert.equal(code, 1);
  assert.match(out, /nothing to check/);
});

test("a package manager the check cannot read is an error, not a skip", () => {
  const { code, out } = run("pnpm add @sinua/totally-invented\n");
  assert.equal(code, 1);
  assert.match(out, /totally-invented/);
});

test("one of our names installed by an unreadable command is still caught", () => {
  const { code, out } = run("deno add npm:@sinua/core\n");
  assert.equal(code, 1);
  assert.match(out, /no install line this check can read installs it/);
});

test("the control still works: npm install of a bogus name fails", () => {
  const { code, out } = run("npm install @sinua/totally-invented\n");
  assert.equal(code, 1);
  assert.match(out, /is not a package in packages\//);
});

test("a valid snippet passes and the success line states the denominator", () => {
  const { code, out } = run("npm install @sinua/core @sinua/web\n");
  assert.equal(code, 0, out);
  assert.match(out, /2 name\(s\) on 1 install line\(s\) across 1 file\(s\)/);
});
