#!/usr/bin/env node
// Typed components from the parameter catalog -- docs/fx-view.md, *Typed components*.
//   node scripts/codegen/generate.mjs          write every emitter's files
//   node scripts/codegen/generate.mjs --check  exit 1 if any checked-in file is stale
//   node scripts/codegen/generate.mjs --model  print the ComponentModel[] (JSON) the emitters get
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { naming, outputs, catalogPath } from "./config.mjs";
import { buildModels } from "./model.mjs";
import { HEADER_LINES } from "./emitters/common.mjs";
import * as swift from "./emitters/swift.mjs";
import * as kotlin from "./emitters/kotlin.mjs";
import * as reactNative from "./emitters/react-native.mjs";
import * as react from "./emitters/react.mjs";
import * as webComponent from "./emitters/web-component.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const EMITTERS = { swift, kotlin, reactNative, react, webComponent };

/** { absolutePath: contents } for every emitter with an output dir in config. */
export function render(root = ROOT, cfg = { naming, outputs }) {
  const catalog = JSON.parse(readFileSync(join(root, catalogPath), "utf8"));
  const models = buildModels(catalog, cfg.naming);
  // The emitter set and the output map must name the same things, both ways.
  // This used to be `if (!dir) continue`: deleting `outputs.kotlin` made
  // --check exit 0 while eleven generated Kotlin files quietly stopped being
  // managed. A generator that silently emits less than it knows how to is the
  // worst possible thing to have a green check on.
  const emitterNames = Object.keys(EMITTERS);
  const outputNames = Object.keys(cfg.outputs);
  const missing = emitterNames.filter((n) => !cfg.outputs[n]);
  const unknown = outputNames.filter((n) => !EMITTERS[n]);
  if (missing.length || unknown.length) {
    const parts = [];
    if (missing.length) parts.push(`emitters with no output dir: ${missing.join(", ")}`);
    if (unknown.length) parts.push(`outputs with no emitter: ${unknown.join(", ")}`);
    throw new Error(
      `codegen config and emitter set disagree -- ${parts.join("; ")}. ` +
        `Every emitter in EMITTERS needs an entry in config.mjs's \`outputs\` and vice versa.`
    );
  }

  const files = new Map();
  for (const [name, emitter] of Object.entries(EMITTERS)) {
    const dir = cfg.outputs[name];
    for (const f of emitter.emit(models, cfg.naming)) files.set(join(root, dir, f.path), f.contents);
  }
  return { models, files };
}

/** Files in an output dir the generator didn't produce (e.g. after a rename) -- stale too. */
function strays(root, cfg, files) {
  const out = [];
  for (const dir of Object.values(cfg.outputs)) {
    const abs = join(root, dir);
    if (!existsSync(abs)) continue;
    for (const f of readdirSync(abs)) {
      const p = join(abs, f);
      // Only files this generator wrote (its header) count; anything else in the dir is left alone.
      if (!files.has(p) && readFileSync(p, "utf8").slice(0, 300).includes(HEADER_LINES[0])) out.push(p);
    }
  }
  return out;
}

export function run({ check = false, root = ROOT, cfg = { naming, outputs }, log = console.log } = {}) {
  const { files } = render(root, cfg);
  const stale = [];
  for (const [path, contents] of files) {
    const current = existsSync(path) ? readFileSync(path, "utf8") : null;
    if (current === contents) continue;
    stale.push(path);
    if (!check) {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, contents);
    }
  }
  const extra = strays(root, cfg, files);
  if (!check) for (const p of extra) rmSync(p);
  const rel = (p) => p.slice(root.length + 1);
  if (check) {
    for (const p of stale) log(`stale: ${rel(p)}`);
    for (const p of extra) log(`not generated (remove or regenerate): ${rel(p)}`);
    if (stale.length || extra.length) log("run: node scripts/codegen/generate.mjs");
    // State the denominator: a check that only ever counts failures can report
    // success having compared nothing.
    else log(`codegen: ${files.size} file(s) from ${Object.keys(EMITTERS).length} emitter(s) all current`);
    return stale.length + extra.length === 0;
  }
  log(`codegen: ${files.size} files, ${stale.length} written, ${extra.length} removed`);
  return true;
}

if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv.includes("--model")) {
  // The ComponentModel[] every emitter receives, for emitter authors.
  console.log(JSON.stringify(render().models, null, 2));
} else if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const ok = run({ check: process.argv.includes("--check") });
  process.exit(ok ? 0 : 1);
}
