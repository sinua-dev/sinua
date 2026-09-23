// The web and React Native components come from one template
// (emitters/react-component.mjs). This pins WHAT may differ between them:
// take both renderers' emitted text, undo each documented axis of divergence,
// and the two must then be identical. Anything else that differs -- a new
// import, a renamed prop, an `if (native)` in the template -- fails here
// rather than passing review as "the RN file looks the same".
//
// It keeps its value if the template is ever un-shared again: at that point
// it becomes the only check that two hand-kept copies still agree.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { buildModels } from "../model.mjs";
import { render } from "../generate.mjs";
import { naming, outputs, catalogPath } from "../config.mjs";
import { RENDERER as WEB } from "../emitters/react.mjs";
import { RENDERER as RN } from "../emitters/react-native.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const models = buildModels(JSON.parse(readFileSync(join(ROOT, catalogPath), "utf8")), naming);
const { files } = render(ROOT, { naming, outputs });
const P = naming.typePrefix;

const emitted = (dir, name) => {
  const text = files.get(join(ROOT, dir, name));
  assert.ok(text, `nothing emitted at ${dir}/${name}`);
  return text;
};

/** Undo one renderer's documented choices, leaving neutral placeholders. */
const neutral = (text, r) =>
  text
    .replace(r.preamble.join("\n"), "<<PREAMBLE>>")
    .replace(`from "${r.viewModule}"`, 'from "<<VIEW>>"')
    .replace(`${r.effect}(`, "<<EFFECT>>(")
    .replace(r.blankView, "<<BLANK>>")
    // Only the generated-sibling specifiers ("./..."): RN's suffix is the empty
    // string, and a plain replaceAll of `${suffix}";` would then tag EVERY
    // string literal ending a line -- which is how the first version of this
    // test failed on the view import.
    .replace(/(from "\.\/[^"]+?)(?:\.js)?";/g, '$1<<X>>";');

test("the five documented axes are the renderers' ONLY differences, per component", () => {
  for (const m of models) {
    const web = emitted(outputs.react, `${m.typeName}.tsx`);
    const rn = emitted(outputs.reactNative, `${m.typeName}.tsx`);
    assert.notEqual(web, rn, `${m.typeName}: the two renderers emitted identical text, so the premise is gone`);
    assert.equal(neutral(web, WEB), neutral(rn, RN), `${m.typeName}: web and RN differ beyond the Renderer fields`);
  }
});

test("each axis is actually present in the output, so the substitutions above are not vacuous", () => {
  // Without this, a Renderer field that no longer appears in the template would
  // make `neutral` a no-op for it, and the equality test would still pass.
  for (const [name, r, dir] of [["web", WEB, outputs.react], ["rn", RN, outputs.reactNative]]) {
    const t = emitted(dir, `${models[0].typeName}.tsx`);
    assert.ok(t.includes(r.preamble.join("\n")), `${name}: preamble missing`);
    assert.ok(t.includes(`from "${r.viewModule}"`), `${name}: viewModule missing`);
    // Not an axis any more: both packages name the cross-fade `crossFade` (it was
    // `crossFadeSeconds` on the web until 2026-09-21). Pin that, so the split can't return.
    // Whole word: a bare substring count also matches `crossFadeSeconds` and was blind to it.
    assert.equal((t.match(/\bcrossFade\b/g) ?? []).length, 2, `${name}: crossFade should appear exactly twice (Omit list + prop)`);
    assert.ok(!t.includes("crossFadeSeconds"), `${name}: the old web name crossFadeSeconds is back`);
    assert.ok(t.includes(`${r.effect}(`), `${name}: effect missing`);
    assert.ok(t.includes(`if (specError) return ${r.blankView};`), `${name}: blankView missing`);
    assert.ok(t.includes(`from "./${P}Params${r.moduleSuffix}";`), `${name}: moduleSuffix missing`);
  }
});

test("the index files differ only by module suffix and the web's custom-element tail", () => {
  const web = emitted(outputs.react, "index.ts");
  const rn = emitted(outputs.reactNative, "index.ts");
  const tail = WEB.indexTail(P);
  assert.equal(tail.length, 1, "the web index adds exactly one line");
  assert.ok(web.includes(tail[0]), "the web index carries the custom-element exports");
  assert.deepEqual(RN.indexTail(P), [], "the RN index adds nothing");
  assert.equal(web.replace(`${tail[0]}\n`, "").replaceAll('.js";', '";'), rn);
});
