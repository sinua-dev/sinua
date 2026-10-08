#!/usr/bin/env node
// The size budget (spec/size-budget.json): fails when a shipped artefact grows past its limit,
// and prints the "Size:" line every CHANGELOG entry carries (docs/publishing.md, *Size budget*).
//
//   node scripts/size-budget.mjs --web                    @sinua/core's inline.js (after its build)
//   node scripts/size-budget.mjs --android <libcore_engine.so>   arm64, release variant
//   node scripts/size-budget.mjs --ios <libcore_engine.a>        device slice, release variant
//   node scripts/size-budget.mjs --voice                  each @sinua/voice web entry, min+gzip (after its build)
//   add --line to print the CHANGELOG line; --budget <file> reads other limits (tests)
//
// The native limits hold for the release variant only (SINUA_NATIVE_RELEASE=1): a default
// build.sh build carries the Studio / dev exports and is ~100 KB larger.
import { readFileSync, statSync, existsSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i < 0 ? undefined : args[i + 1];
};
const budget = JSON.parse(readFileSync(opt("--budget") ?? join(root, "spec/size-budget.json"), "utf8"));
const fmt = (n) => n.toLocaleString("en-US");
const size = (p) => {
  if (!existsSync(p)) throw new Error(`${p}: not found (build it first)`);
  return statSync(p).size;
};

const parts = [];
let over = false;
function check(label, file, bytes, limit, extra = "") {
  const left = limit - bytes;
  const ok = left >= 0;
  over ||= !ok;
  console.log(`${ok ? "ok  " : "OVER"} ${label}: ${fmt(bytes)} B${extra} (limit ${fmt(limit)}, ${ok ? `${fmt(left)} left` : `${fmt(-left)} over`})  ${file}`);
}

if (args.includes("--web")) {
  const file = join(root, "packages/core/pkg/sinua_core_inline.js");
  const bytes = size(file);
  const gz = gzipSync(readFileSync(file), { level: 9 }).length;
  check("web @sinua/core inline.js", file, bytes, budget.webInline, `, gzip -9 ${fmt(gz)}`);
  const dev = join(root, "packages/core/pkg-dev/sinua_core_inline.js");
  if (existsSync(dev)) console.log(`     (info) @sinua/core/dev inline.js: ${fmt(size(dev))} B, no limit`);
  parts.push(`web ${fmt(bytes)} B (gzip ${fmt(gz)})`);
}
if (args.includes("--voice")) {
  // Each @sinua/voice web entry the way an app bundles it: minified, @sinua/core and the vendor
  // SDK left to the app (they're shared or the app's own choice), gzip -9.
  const { build } = await import(join(root, "packages/core/node_modules/esbuild/lib/main.js"));
  const exportsMap = JSON.parse(readFileSync(join(root, "packages/voice/package.json"), "utf8")).exports;
  const voiceParts = [];
  for (const [entry, limit] of Object.entries(budget.voice ?? {})) {
    const file = join(root, "packages/voice", exportsMap[entry].default);
    size(file);
    const out = await build({ entryPoints: [file], bundle: true, minify: true, format: "esm", platform: "browser", write: false, logLevel: "silent", external: ["@sinua/core", "livekit-client"] });
    const gz = gzipSync(out.outputFiles[0].contents, { level: 9 }).length;
    check(`@sinua/voice${entry === "." ? "" : entry.slice(1)} (min+gzip)`, file, gz, limit);
    voiceParts.push(`${entry === "." ? "voice" : entry.slice(2)} ${fmt(gz)}`);
  }
  parts.push(`@sinua/voice gzip ${voiceParts.join(", ")} B`);
}
if (opt("--android")) {
  const file = opt("--android");
  const bytes = size(file);
  check("Android arm64 libcore_engine.so", file, bytes, budget.androidSo);
  parts.push(`Android arm64 .so ${fmt(bytes)} B`);
}
if (opt("--ios")) {
  const file = opt("--ios");
  const bytes = size(file);
  check("iOS device libcore_engine.a", file, bytes, budget.iosA);
  parts.push(`iOS .a ${fmt(bytes)} B`);
}
if (!parts.length) {
  console.error("usage: size-budget.mjs [--web] [--voice] [--android <so>] [--ios <a>] [--line] [--budget <file>]");
  process.exit(2);
}
if (args.includes("--line")) console.log(`Size: ${parts.join(", ")} (release builds).`);
if (over) {
  console.error("Over the size budget (spec/size-budget.json). Trim, move optional parts out of the core, or bring the numbers to the user: only the user raises a limit.");
  process.exit(1);
}
