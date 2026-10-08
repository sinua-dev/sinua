// Checked-in copies of spec files must follow their source (0.1.0-beta.8's lesson: after an
// edit, copies in the runtime and the Studio stayed stale). One table, source -> copies.
//   node scripts/sync-copies.mjs           check: each copy equals its source byte for byte, and
//                                          no other tracked file is an unlisted copy of a spec file
//   node scripts/sync-copies.mjs --write   rewrite the copies from their sources
// The Studio runs its own (scripts/studio/sync-copies.mjs in sinua-studio) for its copies.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
export const COPIES = [
  // The five framework examples import one spec (apps/examples/web-component/*/).
  ["spec/examples/voice-assistant.fxspec.json", ["apps/examples/web-component/spec.json"]],
];

const write = process.argv.includes("--write");
const read = (f) => readFileSync(join(root, f));
const failures = [];
for (const [source, copies] of COPIES) {
  const want = read(source);
  for (const copy of copies) {
    if (read(copy).equals(want)) continue;
    if (write) {
      writeFileSync(join(root, copy), want);
      console.log(`sync-copies: wrote ${copy} from ${source}`);
    } else failures.push(`${copy} differs from ${source} (run: node scripts/sync-copies.mjs --write)`);
  }
}

// A new copy nobody listed: any tracked JSON outside spec/ with a spec file's exact bytes.
const sha = (b) => createHash("sha1").update(b).digest("hex");
const files = execFileSync("git", ["ls-files", "*.json"], { cwd: root, encoding: "utf8" }).split("\n").filter(Boolean);
const specs = new Map(files.filter((f) => f.startsWith("spec/")).map((f) => [sha(read(f)), f]));
const listed = new Set(COPIES.flatMap(([, c]) => c));
for (const f of files) {
  if (f.startsWith("spec/") || listed.has(f) || /(^|\/)(package(-lock)?|tsconfig[^/]*)\.json$/.test(f)) continue;
  const src = specs.get(sha(read(f)));
  if (src) failures.push(`${f} is a copy of ${src}: add it to COPIES in scripts/sync-copies.mjs`);
}

if (failures.length) {
  console.error(`sync-copies: ${failures.length} problem(s):\n  ${failures.join("\n  ")}`);
  process.exit(1);
}
const n = COPIES.reduce((k, [, c]) => k + c.length, 0);
console.log(`sync-copies: ${n} ${n === 1 ? "copy follows its source" : "copies follow their sources"}; no unlisted copies`);
