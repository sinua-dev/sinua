// Public files never cite the private design notes ("design note 31", "design-07"): the
// notes live in the private repo, so a reader can't follow the reference. Code comments are
// exempt; this checks what users read: docs, the CHANGELOG, READMEs and spec data.
//   node scripts/public-citations.mjs
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const CITATION = /design[ -]notes? ?\d+|design-\d{2}\b/i;
const files = execFileSync("git", ["ls-files", "docs/*.md", "CHANGELOG.md", "README.md", "packages/*/README.md", "spec/*.json", "spec/**/*.json"], { cwd: root, encoding: "utf8" })
  .split("\n")
  .filter(Boolean);
const hits = [];
for (const f of new Set(files)) {
  readFileSync(join(root, f), "utf8")
    .split("\n")
    .forEach((line, i) => {
      const m = line.match(CITATION);
      if (m) hits.push(`${f}:${i + 1}: "${m[0]}"`);
    });
}
if (hits.length) {
  console.error(`public-citations: ${hits.length} design-note citation(s) in public files (say what it is, or link a public doc):\n  ${hits.join("\n  ")}`);
  process.exit(1);
}
console.log(`public-citations: ${new Set(files).size} public files, no design-note citations`);
