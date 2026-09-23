// The exported text is a contract: the native Studios (Snippets.swift,
// Snippets.kt) print the same characters for the same design, and the docs
// site shows it as copyable code. These cases are the byte-for-byte lock --
// update them only together with the three ports.
import { test } from "node:test";
import assert from "node:assert/strict";
import { buildSnippets, hasLifecycle, snipNum, LIFECYCLE_NOTE } from "../dist/index.js";

const plain = buildSnippets({ state: "breathing", size: 64, overrides: {}, specFile: "orb-breathing.fxspec.json" });
const tuned = buildSnippets({
  state: "working",
  size: 32,
  overrides: { glowStrength: 0.6, glowRadius: 3, colorHue: 251.94999 },
  speed: 1.25,
  specFile: "orb-working.fxspec.json",
});
const code = (s, id) => s.code.find((t) => t.id === id).code;
const file = (s, id) => s.file.find((t) => t.id === id).code;

test("tabs: the five platforms, in order, in both modes", () => {
  assert.deepEqual(plain.code.map((t) => [t.id, t.label]), [
    ["react", "React"], ["web", "Web"], ["swiftui", "SwiftUI"], ["compose", "Compose"], ["rn", "React Native"],
  ]);
  assert.deepEqual(plain.file.map((t) => t.id), plain.code.map((t) => t.id));
});

test("a plain design prints only the pattern (defaults left out)", () => {
  assert.equal(
    code(plain, "react"),
    `import { SinuaView } from "@sinua/web/react";\n\n` +
      `export function Visual() {\n` +
      `  return <SinuaView pattern="breathing" style={{ width: 160, height: 160 }} />;\n` +
      `}\n\n` +
      `// Or use the file: Spec menu → orb-breathing.fxspec.json (File tab).`
  );
  assert.equal(
    code(plain, "web"),
    `import { mount } from "@sinua/web";\n\n` +
      `const fx = mount(document.querySelector("canvas")!, { pattern: "breathing" });\n` +
      `// fx.update({ ... }) changes it later; fx.destroy() when done.\n\n` +
      `// Or use the file: Spec menu → orb-breathing.fxspec.json (File tab).`
  );
});

test("size, speed and sorted overrides per syntax", () => {
  assert.match(code(tuned, "react"), /<SinuaView pattern="working" size=\{32\} overrides=\{\{ colorHue: 251\.95, glowRadius: 3, glowStrength: 0\.6 \}\} speed=\{1\.25\} /);
  assert.match(code(tuned, "web"), /\{ pattern: "working", size: 32, overrides: \{ colorHue: 251\.95, glowRadius: 3, glowStrength: 0\.6 \}, speed: 1\.25 \}/);
  // Swift: a dictionary literal; Kotlin: mapOf with double literals.
  assert.match(code(tuned, "swiftui"), /SinuaView\(pattern: "working", size: 32, overrides: \["colorHue": 251\.95, "glowRadius": 3, "glowStrength": 0\.6\], speed: 1\.25\)\.frame\(width: 160, height: 160\)/);
  assert.match(code(tuned, "compose"), /SinuaView\(pattern = "working", size = 32u, overrides = mapOf\("colorHue" to 251\.95, "glowRadius" to 3\.0, "glowStrength" to 0\.6\), speed = 1\.25, modifier = Modifier\.size\(160\.dp\)\)/);
  assert.match(code(tuned, "rn"), /from "@sinua\/react-native"/);
});

test("file mode loads the exported spec on every platform", () => {
  assert.match(file(plain, "react"), /import spec from "\.\/orb-breathing\.fxspec\.json";[\s\S]*<SinuaView spec=\{spec\}/);
  assert.match(file(plain, "web"), /mount\(document\.querySelector\("canvas"\)!, \{ spec \}\)/);
  assert.match(file(plain, "swiftui"), /Bundle\.main\.url\(forResource: "orb-breathing", withExtension: "fxspec\.json"\)/);
  assert.match(file(plain, "compose"), /context\.assets\.open\("orb-breathing\.fxspec\.json"\)/);
  assert.match(file(plain, "compose"), /import androidx\.compose\.runtime\.remember/);
});

test("snipNum keeps 4 decimals and drops a trailing .0", () => {
  assert.equal(snipNum(0.045), "0.045");
  assert.equal(snipNum(0.30000000000000004), "0.3");
  assert.equal(snipNum(251.94999), "251.95");
  assert.equal(snipNum(3), "3");
});

test("hasLifecycle: only a non-empty states / bindings / performance block", () => {
  assert.equal(hasLifecycle({}), false);
  assert.equal(hasLifecycle({ states: {} }), false);
  assert.equal(hasLifecycle({ states: { speaking: {} } }), true);
  assert.equal(hasLifecycle({ bindings: { glowStrength: { input: "x" } } }), true);
  assert.equal(hasLifecycle({ performance: { maxFps: 30 } }), true);
  assert.match(LIFECYCLE_NOTE, /use the File tab/);
});

// ---- Typed components: the catalog-driven prop form ----------------------
import { parameterCatalog } from "@sinua/core";
import { buildTypedSnippets, toTypedProps } from "../dist/index.js";

const catalog = parameterCatalog();

test("engine overrides become the component's named props", () => {
  const d = toTypedProps("orb", "working", { glowStrength: 0.6, glowRadius: 3 }, catalog);
  assert.equal(d.component, "SinuaOrb");
  assert.deepEqual(d.props, { glow: { strength: 0.6, radius: 3 } });
  assert.deepEqual(d.leftover, {});
});

test("indexed engine keys collapse into one list prop", () => {
  const d = toTypedProps("ring", "tracking", { progress0: 0.2, progress1: 0.5, ringCount: 2 }, catalog);
  assert.deepEqual(d.props.progress, [0.2, 0.5]);
  assert.equal(d.props.ringCount, 2);
});

test("a key the catalog has no prop for is reported, not invented", () => {
  const d = toTypedProps("orb", "working", { notACatalogKey: 1 }, catalog);
  assert.deepEqual(d.leftover, { notACatalogKey: 1 });
  assert.deepEqual(d.props, {});
});

test("the typed snippet prints the component with its props", () => {
  const d = toTypedProps("orb", "working", { glowStrength: 0.6 }, catalog);
  const [react, rn] = buildTypedSnippets({ design: d, size: 32, speed: 1.25 });
  assert.equal(
    react.code,
    `import { SinuaOrb } from "@sinua/web/components";\n\n` +
      `export function Visual() {\n` +
      `  return <SinuaOrb pattern="working" glow={{ strength: 0.6 }} size={32} speed={1.25} style={{ width: 160, height: 160 }} />;\n` +
      `}`
  );
  assert.match(rn.code, /from "@sinua\/react-native"/);
});
