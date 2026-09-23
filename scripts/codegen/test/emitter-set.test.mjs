// The emitter set and config.mjs's `outputs` must name the same things.
//
// `render()` used to do `if (!dir) continue`, so deleting `outputs.kotlin`
// made `--check` exit 0 while eleven generated Kotlin files quietly stopped
// being managed. That hole was mitigated only by accident: a neighbouring test
// happened to crash on the undefined path, which is not the same as anything
// asserting the set. These tests are the assertion.
import { test } from "node:test";
import assert from "node:assert/strict";
import { sep } from "node:path";
import { render, EMITTERS } from "../generate.mjs";
import { naming, outputs } from "../config.mjs";

const cfg = (outs) => ({ naming, outputs: outs });

test("an emitter with no output dir is a hard error, not a silent skip", () => {
  const { kotlin, ...withoutKotlin } = outputs;
  assert.ok(kotlin, "precondition: outputs.kotlin exists to be removed");
  assert.throws(() => render(undefined, cfg(withoutKotlin)), /emitters with no output dir: kotlin/);
});

test("an output entry with no emitter is a hard error too", () => {
  assert.throws(
    () => render(undefined, cfg({ ...outputs, kotln: "packages/android/nowhere" })),
    /outputs with no emitter: kotln/
  );
});

test("every emitter is named in outputs, and nothing else is", () => {
  assert.deepEqual(Object.keys(EMITTERS).sort(), Object.keys(outputs).sort());
});

test("the real config renders from every emitter", () => {
  const { files } = render();
  assert.equal(Object.keys(EMITTERS).length, 5, "five emitters; update this test deliberately");
  assert.ok(files.size > 0, "the denominator is not zero");
  // Each emitter's output dir is represented, so none of them emitted nothing.
  for (const [name, dir] of Object.entries(outputs)) {
    const wanted = dir.split("/").join(sep);
    const hit = [...files.keys()].some((p) => p.includes(wanted));
    assert.ok(hit, `${name} emitted no files into ${dir}`);
  }
});
