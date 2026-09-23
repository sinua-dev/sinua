#!/usr/bin/env node
// The npm tarballs a release publishes: @sinua/core, @sinua/web, @sinua/voice.
//
// The packages stay `private: true` in the tree (packages/core/test/manifests.test.mjs
// ties that to the `<REPO_URL>` placeholder), so each is packed from a scratch copy
// with `private` removed -- the tree is never edited. `--publishable` also refuses a
// tarball that still carries a placeholder, which is what the real release runs with.
// @sinua/snippets and @sinua/react-native don't publish (release decision 0.5).
//
//   node scripts/release/npm-pack.mjs <out dir> [--publishable]
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, readdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

const root = fileURLToPath(new URL("../../", import.meta.url));
const [outArg, flag] = process.argv.slice(2);
if (!outArg) {
  console.error("usage: npm-pack.mjs <out dir> [--publishable]");
  process.exit(2);
}
const out = resolve(outArg);
mkdirSync(out, { recursive: true });
const version = readFileSync(join(root, "VERSION"), "utf8").trim();
const PUBLISHED = ["core", "web", "voice"];

const made = [];
for (const pkg of PUBLISHED) {
  const src = join(root, "packages", pkg);
  const json = JSON.parse(readFileSync(join(src, "package.json"), "utf8"));
  if (json.version !== version) throw new Error(`${pkg}: version ${json.version}, VERSION says ${version} (run set-version.mjs)`);
  const tmp = mkdtempSync(join(tmpdir(), `sinua-pack-${pkg}-`));
  try {
    // Everything but node_modules: `files` in package.json decides what ships.
    for (const entry of readdirSync(src)) if (entry !== "node_modules") cpSync(join(src, entry), join(tmp, entry), { recursive: true });
    delete json.private;
    const text = JSON.stringify(json, null, 2) + "\n";
    if (flag === "--publishable" && /<REPO_URL>|<[A-Z_]+>/.test(text))
      throw new Error(`${json.name}: package.json still has a placeholder (${text.match(/<[A-Z_]+>/)[0]}); fill it before publishing`);
    writeFileSync(join(tmp, "package.json"), text);
    const res = JSON.parse(execFileSync("npm", ["pack", "--json", "--ignore-scripts", "--pack-destination", out], { cwd: tmp, encoding: "utf8" }));
    made.push(`${res[0].filename} (${res[0].entryCount} files, ${Math.round(res[0].size / 1024)} KB)`);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}
console.log(`npm-pack: ${version}\n  ${made.join("\n  ")}`);
