// node --test scripts/codegen/test
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, mkdtempSync, cpSync, writeFileSync, mkdirSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { buildModels } from "../model.mjs";
import { render, run } from "../generate.mjs";
import { naming, outputs, catalogPath } from "../config.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "../../..");
const catalog = JSON.parse(readFileSync(join(ROOT, catalogPath), "utf8"));
const models = buildModels(catalog, naming);
const byObject = Object.fromEntries(models.map((m) => [m.object, m]));
const prop = (obj, path) => byObject[obj].props.find((p) => p.path === path);

test("one component per catalog object, in catalog order, names from config", () => {
  assert.deepEqual(models.map((m) => m.object), catalog.objects.map((o) => o.id));
  for (const m of models) {
    assert.equal(m.typeName, naming.typePrefix + m.object[0].toUpperCase() + m.object.slice(1));
    assert.equal(m.tagName, naming.tagPrefix + m.object);
    assert.deepEqual(m.patterns.map((p) => p.id), catalog.objects.find((o) => o.id === m.object).patterns.map((p) => p.id));
  }
});

test("every non-deprecated catalog param of every pattern becomes a prop", () => {
  for (const obj of catalog.objects) {
    for (const p of obj.patterns) {
      for (const e of p.params) {
        const d = catalog.definitions[e.ref];
        if (d.deprecated) continue;
        const pr = prop(obj.id, d.path);
        assert.ok(pr, `${obj.id}.${d.path}`);
        assert.ok(pr.patterns.includes(p.id), `${obj.id}.${d.path} lists ${p.id}`);
      }
    }
  }
});

test("per-object path conflicts merge into one prop with per-pattern ranges", () => {
  const period = prop("orb", "period");
  assert.equal(period.type, "number");
  assert.equal(period.ranges.concluding.max, 60);
  assert.equal(period.ranges.confirming.max, 30);
  assert.equal(period.max, 60);
  const progress = prop("ring", "progress");
  assert.equal(progress.type, "number|number[]");
  assert.deepEqual(progress.listPatterns, ["tracking"]);
  assert.deepEqual(progress.engineKeys, ["progress0", "progress1", "progress2", "progress3"]);
  assert.equal(progress.maxItems, 4);
  for (const path of ["dotSize", "ringCount"]) assert.ok(prop("beacon", path), path);
});

test("number[] needs bounds; missing ones throw", () => {
  const seg = prop("ring", "segment");
  assert.deepEqual([seg.type, seg.minItems, seg.maxItems, seg.engineKeys.length], ["number[]", 1, 24, 24]);
  const broken = structuredClone(catalog);
  delete broken.definitions["segment@segmented"].maxItems;
  assert.throws(() => buildModels(broken, naming), /minItems\/maxItems/);
});

test("choices become enum cases with engine values", () => {
  const blend = prop("orb", "glow.blend");
  assert.deepEqual(blend.choices.map((c) => [c.caseName, c.value]), [["normal", 0], ["additive", 1]]);
});

test("no flat prop shares a name with a material group (orb orbitParticles vs particles.*)", () => {
  const p = prop("orb", "orbitParticles");
  assert.equal(p.name, "orbitParticles");
  assert.equal(p.key, "particles");
  assert.equal(p.attr, "orbit-particles");
  const broken = structuredClone(catalog);
  broken.definitions["particles@orbits"].path = "particles";
  assert.throws(() => buildModels(broken, naming), /both a prop and a group/);
});

test("defaults per pattern and size, attributes, events", () => {
  const glow = prop("orb", "glow.strength");
  for (const pat of byObject.orb.patterns) assert.deepEqual(Object.keys(glow.defaultByPattern[pat.id]), pat.sizes.map(String));
  assert.equal(prop("ring", "segment").attribute, false);
  assert.equal(prop("ring", "strokeWidth").attr, "stroke-width");
  assert.equal(prop("orb", "glow.strength").attr, "glow-strength");
  assert.deepEqual(models[0].events.map((e) => e.id), ["frame", "error"]);
});

test("generation is deterministic", () => {
  const a = render(ROOT).files;
  const b = render(ROOT).files;
  assert.deepEqual([...a.entries()], [...b.entries()]);
});

test("checked-in files are up to date", () => {
  assert.equal(run({ check: true, log: () => {} }), true);
});

test("the prefix lives only in config: another prefix renames every type and tag", () => {
  const alt = { typePrefix: "Acme", tagPrefix: "acme-" };
  const { files } = render(ROOT, { naming: alt, outputs });
  const all = [...files.entries()];
  assert.ok(all.some(([p]) => p.endsWith("AcmeRing.swift")));
  for (const [path, text] of all) {
    // Module/package names (SinuaView, SinuaVoice, dev.sinua) are not the brand prefix.
    const leftovers = text.replace(/Sinua(View|Voice)/g, "").match(/Devin|devin-/g);
    assert.equal(leftovers, null, `${path} still says Devin`);
  }
  // …and nothing in the generator hard-codes it.
  const src = ["model.mjs", "generate.mjs", ...readdirSync(join(HERE, "../emitters")).map((f) => `emitters/${f}`)];
  for (const f of src) {
    const text = readFileSync(join(HERE, "..", f), "utf8").replace(/Sinua(View|Voice)/g, "");
    assert.equal(/Devin|devin-/.test(text), false, f);
  }
});

test("--check flags a stale file and an orphaned generated file", () => {
  const tmp = mkdtempSync(join(tmpdir(), "codegen-"));
  mkdirSync(join(tmp, "spec"));
  cpSync(join(ROOT, catalogPath), join(tmp, catalogPath));
  const cfg = { naming, outputs };
  assert.equal(run({ root: tmp, cfg, log: () => {} }), true);
  assert.equal(run({ root: tmp, cfg, check: true, log: () => {} }), true);
  const ring = join(tmp, outputs.swift, "SinuaRing.swift");
  writeFileSync(ring, readFileSync(ring, "utf8") + "\n// edit");
  const lines = [];
  assert.equal(run({ root: tmp, cfg, check: true, log: (l) => lines.push(l) }), false);
  assert.match(lines.join("\n"), /stale: .*SinuaRing\.swift/);
  run({ root: tmp, cfg, log: () => {} });
  const orphan = join(tmp, outputs.kotlin, "SinuaOld.kt");
  writeFileSync(orphan, readFileSync(join(tmp, outputs.kotlin, "SinuaRing.kt"), "utf8"));
  const hand = join(tmp, outputs.kotlin, "HandWritten.kt");
  writeFileSync(hand, "// mine\n");
  assert.equal(run({ root: tmp, cfg, check: true, log: () => {} }), false);
  run({ root: tmp, cfg, log: () => {} });
  assert.deepEqual(readdirSync(join(tmp, outputs.kotlin)).filter((f) => f.startsWith("SinuaOld") || f.startsWith("Hand")), ["HandWritten.kt"]);
});
