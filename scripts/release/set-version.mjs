#!/usr/bin/env node
// One version for every published artefact: the root VERSION file. This writes it
// into each copy the build reads, and `--check` fails (naming each copy) if any
// disagrees -- so a release can't ship packages at two versions.
//
//   node scripts/release/set-version.mjs            # write VERSION everywhere
//   node scripts/release/set-version.mjs 0.1.0      # set VERSION, then write
//   node scripts/release/set-version.mjs --check    # CI: every copy equals VERSION
//
// The packages release in lockstep, so a sibling peer (`@sinua/core` for web and
// voice) is pinned to the exact version. The SwiftPM distribution manifests are
// generated from VERSION by scripts/release/build.sh, so they're not a stored copy.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const at = (p) => root + p;
const args = process.argv.slice(2);
const check = args.includes("--check");
const given = args.find((a) => !a.startsWith("--"));

// semver 2.0: MAJOR.MINOR.PATCH, optional -prerelease (beta.N here).
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
if (given && !SEMVER.test(given)) {
  console.error(`not a semver version: ${given}`);
  process.exit(2);
}
if (given && !check) writeFileSync(at("VERSION"), `${given}\n`);
const version = readFileSync(at("VERSION"), "utf8").trim();
if (!SEMVER.test(version)) {
  console.error(`VERSION holds "${version}", not a semver version`);
  process.exit(1);
}

const PACKAGES = ["core", "web", "voice", "snippets", "react-native"];
const PEERS = { web: ["@sinua/core"], voice: ["@sinua/core"] };
const mismatches = [];
const writes = new Map();

function edit(path, fn) {
  const before = writes.get(path) ?? readFileSync(at(path), "utf8");
  writes.set(path, fn(before));
}

// npm: each package's version, and the exact sibling peer.
for (const pkg of PACKAGES) {
  const path = `packages/${pkg}/package.json`;
  const json = JSON.parse(readFileSync(at(path), "utf8"));
  if (json.version !== version) mismatches.push(`${path}: version ${json.version}`);
  for (const peer of PEERS[pkg] ?? []) {
    const have = json.peerDependencies?.[peer];
    if (have !== version) mismatches.push(`${path}: peerDependencies["${peer}"] ${have}`);
  }
  edit(path, (text) => {
    let out = text.replace(/("version":\s*")[^"]*(")/, `$1${version}$2`);
    for (const peer of PEERS[pkg] ?? []) {
      // only inside peerDependencies: the devDependency stays `file:../core`
      out = out.replace(/("peerDependencies":\s*\{[^}]*?"@sinua\/core":\s*")[^"]*(")/, `$1${version}$2`);
      void peer;
    }
    return out;
  });
}

// Gradle: sinua.version (publish.gradle.kts has no default, so a missing line fails the build).
{
  const path = "packages/android/gradle.properties";
  const text = readFileSync(at(path), "utf8");
  const m = /^sinua\.version=(.*)$/m.exec(text);
  if (!m || m[1].trim() !== version) mismatches.push(`${path}: sinua.version ${m ? m[1].trim() : "(missing)"}`);
  edit(path, (t) =>
    /^sinua\.version=/m.test(t)
      ? t.replace(/^sinua\.version=.*$/m, `sinua.version=${version}`)
      : `${t.replace(/\n*$/, "\n")}\n# The release version: written by scripts/release/set-version.mjs from VERSION.\nsinua.version=${version}\n`,
  );
}

// The install lines a reader copies: Maven coordinates, and the SwiftPM
// distribution repositories' `from:` version.
for (const path of ["packages/android/README.md", "apps/site/snippets/install/gradle.kts"]) {
  const text = readFileSync(at(path), "utf8");
  for (const m of text.matchAll(/"dev\.sinua:(sinua-[a-z]+):([^"]+)"/g))
    if (m[2] !== version) mismatches.push(`${path}: dev.sinua:${m[1]}:${m[2]}`);
  edit(path, (t) => t.replace(/("dev\.sinua:sinua-[a-z]+:)[^"]+(")/g, `$1${version}$2`));
}
const SWIFT_FROM = /(\.package\(url: "https:\/\/github\.com\/sinua-dev\/sinua-swift[a-z-]*", from: ")([^"]+)(")/g;
for (const path of [
  "packages/ios/README.md",
  "packages/ios-livekit/README.md",
  "packages/ios-openai/README.md",
  "apps/site/snippets/install/swift-package.swift",
]) {
  const text = readFileSync(at(path), "utf8");
  for (const m of text.matchAll(SWIFT_FROM)) if (m[2] !== version) mismatches.push(`${path}: from: "${m[2]}"`);
  edit(path, (t) => t.replace(SWIFT_FROM, `$1${version}$3`));
}

if (check) {
  if (mismatches.length) {
    console.error(`set-version --check: ${mismatches.length} copy(ies) differ from VERSION (${version}):`);
    for (const m of mismatches) console.error(`  ${m}`);
    console.error("fix: node scripts/release/set-version.mjs");
    process.exit(1);
  }
  console.log(`set-version --check: every copy is ${version}`);
} else {
  for (const [path, text] of writes) writeFileSync(at(path), text);
  console.log(`set-version: wrote ${version} to ${writes.size} file(s)`);
}
