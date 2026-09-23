#!/usr/bin/env node
// The npm package names in the install samples (apps/site/snippets/install/*.sh)
// must be packages this repo builds. Published coordinates that don't exist yet
// (SwiftPM URL, Maven group) stay as <PLACEHOLDER>s in swift-package.swift /
// gradle.kts -- one file each, filled in once the names are decided.
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const ours = new Set(
  readdirSync(join(root, "packages"), { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => {
      try {
        return JSON.parse(readFileSync(join(root, "packages", d.name, "package.json"), "utf8")).name;
      } catch {
        return null;
      }
    })
    .filter(Boolean)
);
const THIRD_PARTY = new Set(["livekit-client"]);
const dir = join(root, "apps/site/snippets/install");

// Install lines this check knows how to read. `npm i` alone used to be the
// whole list, and a `pnpm add @sinua/invented` line was skipped by the
// `continue` below and reported as a pass -- so the set is explicit, and
// anything that looks like an install but matches none of it is an error
// rather than a silent skip.
const INSTALL = [
  /^\s*npm\s+i(?:nstall)?\s+(.+)$/,
  /^\s*pnpm\s+(?:add|install)\s+(.+)$/,
  /^\s*yarn\s+add\s+(.+)$/,
  /^\s*bun\s+(?:add|install)\s+(.+)$/,
];
const MANAGER = /^\s*(npm|pnpm|yarn|bun)\s+\S+/;

let bad = 0;
// The denominator. A check that cannot say how much it looked at can report
// success having looked at nothing, which is what this one did.
let files = 0;
let installLines = 0;
const checked = new Set();
const mentioned = new Map(); // name -> the file it was seen in

for (const f of readdirSync(dir).filter((f) => f.endsWith(".sh"))) {
  files++;
  for (const line of readFileSync(join(dir, f), "utf8").split("\n")) {
    for (const m of line.matchAll(/@sinua\/[a-z0-9-]+/g)) {
      if (!mentioned.has(m[0])) mentioned.set(m[0], f);
    }

    const hit = INSTALL.map((re) => re.exec(line)).find(Boolean);
    if (!hit) {
      // Not an install line at all is fine; a manager invocation we do not
      // parse is not -- that is the shape that slipped through before.
      if (MANAGER.test(line) && /\s@?[a-z0-9@/-]+\s*$/.test(line) && !line.trim().startsWith("#")) {
        bad++;
        console.error(`${f}: cannot read this install line, so its names went unchecked: ${line.trim()}`);
      }
      continue;
    }
    installLines++;
    for (const name of hit[1].split(/\s+/).filter((t) => t && !t.startsWith("-") && !t.startsWith("."))) {
      checked.add(name);
      if (!ours.has(name) && !THIRD_PARTY.has(name)) {
        bad++;
        console.error(`${f}: "${name}" is not a package in packages/ (or a known third-party one)`);
      }
    }
  }
}

// Every one of our names that appears anywhere in the directory must have been
// reached by a parsed install line. This is the guard that makes the census
// mean something: a name can no longer be present and unexamined.
for (const [name, file] of mentioned) {
  if (!checked.has(name)) {
    bad++;
    console.error(`${file}: "${name}" appears here but no install line this check can read installs it`);
  }
}

if (files === 0 || installLines === 0) {
  console.error(
    `install snippets: examined ${files} file(s) and ${installLines} install line(s) -- ` +
      `nothing to check, which means the snippets moved, not that they are correct`
  );
  process.exit(1);
}

console.log(
  `install snippets: ${checked.size} name(s) on ${installLines} install line(s) across ` +
    `${files} file(s), checked against ${[...ours].join(", ")}`
);
process.exit(bad ? 1 : 0);
