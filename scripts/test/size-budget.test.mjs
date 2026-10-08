// scripts/size-budget.mjs passes under a limit, fails over it, and reads the real budget.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const script = join(root, "scripts/size-budget.mjs");
const dir = mkdtempSync(join(tmpdir(), "size-budget-"));
const lib = join(dir, "libcore_engine.so");
writeFileSync(lib, Buffer.alloc(1000));
const run = (limit, ...extra) => {
  const b = join(dir, `budget-${limit}.json`);
  writeFileSync(b, JSON.stringify({ webInline: 1, androidSo: limit, iosA: 1 }));
  return spawnSync(process.execPath, [script, "--budget", b, "--android", lib, ...extra], { encoding: "utf8" });
};

test("the budget file holds the three limits as positive byte counts", () => {
  const b = JSON.parse(readFileSync(join(root, "spec/size-budget.json"), "utf8"));
  for (const k of ["webInline", "androidSo", "iosA"]) assert.ok(Number.isInteger(b[k]) && b[k] > 0, k);
});

test("every @sinua/voice web entry has a limit, and every limit names a real entry", () => {
  const b = JSON.parse(readFileSync(join(root, "spec/size-budget.json"), "utf8"));
  const entries = Object.keys(JSON.parse(readFileSync(join(root, "packages/voice/package.json"), "utf8")).exports).filter((e) => e !== "./server");
  assert.deepEqual(Object.keys(b.voice).sort(), entries.sort());
  for (const [k, v] of Object.entries(b.voice)) assert.ok(Number.isInteger(v) && v > 0, k);
});

test("passes at or under the limit and prints the CHANGELOG line", () => {
  const r = run(1000, "--line");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /^ok +Android arm64/m);
  assert.match(r.stdout, /Size: Android arm64 \.so 1,000 B \(release builds\)\./);
});

test("fails over the limit and names it", () => {
  const r = run(999);
  assert.equal(r.status, 1);
  assert.match(r.stdout, /OVER Android arm64 libcore_engine\.so: 1,000 B \(limit 999, 1 over\)/);
});

test("a missing file is an error, not a pass", () => {
  const r = spawnSync(process.execPath, [script, "--ios", join(dir, "nope.a")], { encoding: "utf8" });
  assert.notEqual(r.status, 0);
});
