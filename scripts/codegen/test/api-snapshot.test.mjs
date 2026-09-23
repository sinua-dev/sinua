// The generated surface's public-API snapshot (scripts/codegen/api-snapshot.mjs).
//
// The case this gate exists for is a public rename that every other gate lets
// through: edit the catalog, regenerate, and `generate.mjs --check` is green by
// construction. So the red proof below renames a real parameter (`bandMul` ->
// `bandWidth`) in a copy of the catalog,
// runs the shipped generator on it, and requires the snapshot to name the
// change on every output it reaches.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { render } from "../generate.mjs";
import { catalogPath } from "../config.mjs";
import { extractContract, loadSnapshot, diffContracts, generatedFiles, census } from "../api-snapshot.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const files = generatedFiles();
const K = "packages/android/view/src/main/kotlin/dev/sinua/view/generated/";
const S = "packages/ios/Sources/Sinua/Generated/";
const W = "packages/web/src/generated/";

/** The contract of one file, with its contents replaced. */
const contractOf = (path, contents) => extractContract(new Map([[path, contents]]))[path];

/** `s` with each `[from, to]` applied once; fails if a replacement didn't land. */
function edit(s, ...pairs) {
  let out = s;
  for (const [from, to] of pairs) {
    assert.ok(out.includes(from), `fixture edit did not land: ${JSON.stringify(from)}`);
    out = out.replace(from, to);
  }
  assert.notEqual(out, s);
  return out;
}

test("the checked-in snapshot matches the checked-in generated files, all of them", () => {
  const actual = extractContract(files);
  assert.deepEqual(diffContracts(loadSnapshot(), actual), []);
  const { files: n, decls, members } = census(actual);
  assert.equal(n, render().files.size, "every generated file is in the contract");
  assert.ok(decls > 0 && members > decls, `a non-trivial denominator (${decls} declarations, ${members} members)`);
});

test("red proof: renaming a public parameter in the catalog fails on every output it reaches", () => {
  const root = mkdtempSync(join(tmpdir(), "api-snapshot-"));
  try {
    const catalog = readFileSync(join(REPO, catalogPath), "utf8");
    mkdirSync(join(root, "spec"), { recursive: true });
    const renamed = catalog.replaceAll('"path": "bandMul"', '"path": "bandWidth"');
    assert.notEqual(renamed, catalog, "the rename landed in the catalog copy");
    writeFileSync(join(root, catalogPath), renamed);

    const regenerated = new Map([...render(root).files].map(([abs, c]) => [relative(root, abs), c]));
    const diffs = diffContracts(loadSnapshot(), extractContract(regenerated));
    const touched = (file) =>
      diffs.some((d) => d.file === file && d.removed.some((m) => m.includes("bandMul")) && d.added.some((m) => m.includes("bandWidth")));
    for (const file of [
      `${S}SinuaOrb.swift`,
      `${K}SinuaOrb.kt`,
      `${W}SinuaParams.ts`,
      "packages/react-native/src/generated/SinuaParams.ts",
      `${W}SinuaElements.ts`,
    ])
      assert.ok(touched(file), `${file}: the rename is not reported`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("formatting, comments and default values don't change the contract", () => {
  const cases = [
    [
      `${S}SinuaRing.swift`,
      ["    public var hue: Double?", "    /* moved */\n    public   var   hue :Double?"],
      ["size: SinuaSize = .s64,", "size: SinuaSize = .s32 ,"],
    ],
    [`${K}SinuaRing.kt`, ["    val gap: Double? = null,", "    val gap:Double?=0.5 , // note"]],
    [`${W}SinuaParams.ts`, ["export interface SinuaRingParams {", "export interface SinuaRingParams   {\n  // a comment\n"]],
  ];
  for (const [path, ...pairs] of cases) {
    const original = files.get(path);
    assert.deepEqual(contractOf(path, edit(original, ...pairs)), contractOf(path, original), path);
  }
});

test("a type change is a contract change", () => {
  const path = `${K}SinuaRing.kt`;
  const d = diffContracts({ [path]: contractOf(path, files.get(path)) }, {
    [path]: contractOf(path, edit(files.get(path), ["    val gap: Double? = null,", "    val gap: Float? = null,"])),
  });
  assert.ok(d.some((x) => x.added.some((m) => m.includes("val gap:Float?"))), "Double? -> Float? is reported");
});

test("a file the extractor finds nothing in is an error, not an empty snapshot", () => {
  assert.throws(() => contractOf(`${S}SinuaRing.swift`, "// Generated\nimport SwiftUI\n"), /no public declarations found in .*SinuaRing\.swift/);
});

test("order is part of the contract where calls are positional, and not where it isn't", () => {
  const kt = `${K}SinuaRing.kt`;
  const swapped = edit(
    files.get(kt),
    ["    val gap: Double? = null,", "    val __SWAP__: Double? = null,"],
    ["    val hue: Double? = null,", "    val gap: Double? = null,"],
    ["    val __SWAP__: Double? = null,", "    val hue: Double? = null,"],
  );
  assert.notDeepEqual(contractOf(kt, swapped), contractOf(kt, files.get(kt)), "swapping two data-class params changes it");

  const ts = `${W}SinuaParams.ts`;
  const src = files.get(ts);
  const start = src.indexOf("export interface SinuaRingParams");
  const end = src.indexOf("\n}\n", start);
  const block = src.slice(start, end);
  const lines = block.split("\n");
  const gap = lines.findIndex((l) => l.trim() === "gap?: number;");
  const hue = lines.findIndex((l) => l.trim() === "hue?: number;");
  assert.ok(gap > 0 && hue > 0, "both members found in SinuaRingParams");
  [lines[gap], lines[hue]] = [lines[hue], lines[gap]];
  const reordered = src.slice(0, start) + lines.join("\n") + src.slice(end);
  assert.notEqual(reordered, src);
  assert.deepEqual(contractOf(ts, reordered), contractOf(ts, src), "interface member order is not API");
});
