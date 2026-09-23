// The emitted React Native components (packages/react-native/src/generated/*.tsx),
// asserted over the generator's output text rather than by rendering them.
//
// Why here and not in packages/react-native: node strips types from .ts but does
// not transform JSX, and these components call React.useEffect, so importing or
// calling one from a node test needs a JSX transform and a test-only renderer in
// a package that is about to be published. The voice-adapters seat covered the
// mapping (SinuaParams.ts) there instead and left the components at 0% on
// purpose; this file is the other half, so that 0% is a deliberate boundary and
// not an untested gap.
//
// What is worth pinning is the only logic the components add over the mapping:
// the spec-mismatch branch, and the PARAM_KEYS list. Every assertion runs across
// all five components, because a generator regression lands on one object at a
// time.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { buildModels } from "../model.mjs";
import { render } from "../generate.mjs";
import { flatProps } from "../emitters/common.mjs";
import { naming, outputs, catalogPath } from "../config.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const catalog = JSON.parse(readFileSync(join(ROOT, catalogPath), "utf8"));
const models = buildModels(catalog, naming);
const { files } = render(ROOT, { naming, outputs });

// By the React Native output directory only. This used to fall back to any
// path ending in `/<Type>.tsx` -- which the web package emits too, so the RN
// file was chosen only because `reactNative` precedes `react` in
// generate.mjs's EMITTERS object literal, an order nothing pins. Since the two
// renderers now share one template (emitters/react-component.mjs), that
// fallback would have left the `<View` assertion below as the only thing
// between this file and silently testing the web renderer.
const componentText = (m) => {
  const want = join(ROOT, outputs.reactNative, `${m.typeName}.tsx`);
  const key = [...files.keys()].find((p) => p === want);
  assert.ok(key, `no emitted ${m.typeName}.tsx`);
  return files.get(key);
};
const paramsText = () => {
  const key = [...files.keys()].find((p) => p.includes("react-native") && p.endsWith(`${naming.typePrefix}Params.ts`));
  assert.ok(key, "no emitted params file");
  return files.get(key);
};

/** The object name the component hands to the spec guard. */
const guardedObject = (text) => /SpecError\(props\.spec,\s*"([^"]+)"\)/.exec(text)?.[1] ?? null;

/** The PARAM_KEYS array literal the component deletes before spreading. */
const paramKeys = (text) => {
  const m = /const PARAM_KEYS = (\[[^\]]*\]);/.exec(text);
  assert.ok(m, "no PARAM_KEYS array in the emitted component");
  return JSON.parse(m[1]);
};

/** The field names of `export interface <Type>Params { … }` in the params file. */
const interfaceKeys = (text, typeName) => {
  const start = text.indexOf(`export interface ${typeName}Params {`);
  assert.ok(start >= 0, `no ${typeName}Params interface`);
  const body = text.slice(start, text.indexOf("\n}", start));
  return [...body.matchAll(/^\s{2}(\w+)\?:/gm)].map((m) => m[1]);
};

test("each component guards its own catalog object", () => {
  // A SinuaOrb asking sinuaSpecError for "ring" is exactly the copy/paste
  // mistake a per-object generator can make, and nothing else would catch it.
  for (const m of models) {
    assert.equal(guardedObject(componentText(m)), m.object, `${m.typeName} guards the wrong object`);
  }
  // And no two components guard the same one.
  const guarded = models.map((m) => guardedObject(componentText(m)));
  assert.equal(new Set(guarded).size, models.length, `duplicate guarded objects: ${guarded}`);
});

test("a spec error is attributed to the component and routed through onError", () => {
  for (const m of models) {
    const text = componentText(m);
    assert.match(text, new RegExp(`\\(onError \\?\\? console\\.error\\)\\(\`${m.typeName}: `), `${m.typeName}: error not prefixed/routed`);
  }
});

test("a spec for another object draws nothing instead of falling through", () => {
  for (const m of models) {
    const text = componentText(m);
    const bail = text.indexOf("if (specError) return <View");
    const draw = text.indexOf("<SinuaView {...rest} spec={spec}");
    assert.ok(bail >= 0, `${m.typeName}: no empty-view bail-out`);
    assert.ok(draw >= 0, `${m.typeName}: no spec render path`);
    assert.ok(bail < draw, `${m.typeName}: the bail-out must come before the render, or a mismatched spec still draws`);
  }
});

test("PARAM_KEYS equals the object's params interface key set", () => {
  // The real defect this catches: a param added to the interface but missed in
  // PARAM_KEYS leaks through as an unknown prop on SinuaView -- no type error,
  // and nothing else fails. The two lists are built in different files
  // (emitters/react-native.mjs vs emitters/ts-params.mjs) from independent
  // expressions over the same model, so they agree by construction, not by a
  // check. This is the check.
  const params = paramsText();
  for (const m of models) {
    assert.deepEqual(
      paramKeys(componentText(m)).slice().sort(),
      interfaceKeys(params, m.typeName).slice().sort(),
      `${m.typeName}: PARAM_KEYS and ${m.typeName}Params disagree`,
    );
  }
});

test("PARAM_KEYS is the model's own flat props plus its material groups", () => {
  // Pinned against the model as well as the interface, so the test still fails
  // if both emitters drift the same way.
  for (const m of models) {
    const expected = [...flatProps(m).map((p) => p.name), ...m.groups.map((g) => g.name)];
    assert.deepEqual(paramKeys(componentText(m)).slice().sort(), expected.slice().sort(), `${m.typeName}: PARAM_KEYS is not the model`);
    assert.ok(expected.length > 0, `${m.typeName}: no params at all, which cannot be right`);
  }
});
