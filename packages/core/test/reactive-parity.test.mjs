// Reactive-binding parity: packages/core's reactive.ts (bindReactiveInput)
// and the Rust port FX Spec `bindings` run on (core_engine::reactive) must
// agree on spec/reactive-vectors.json -- the targets table (ranges,
// companions) and every mapped value. Regenerate the vectors only via
// crates/core_engine/tests/reactive_vectors.rs.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { REACTIVE_TARGETS, bindReactiveInput } from "../dist/index.js";

const vectors = JSON.parse(
  readFileSync(fileURLToPath(new URL("../../../spec/reactive-vectors.json", import.meta.url)), "utf8")
);

test("REACTIVE_TARGETS matches the Rust table", () => {
  const ts = Object.fromEntries(
    Object.entries(REACTIVE_TARGETS).map(([k, v]) => [
      k,
      { range: [...v.range], defaultOutput: [...(v.defaultOutput ?? v.range)], companions: { ...v.companions } },
    ])
  );
  assert.deepEqual(ts, vectors.targets);
});

test(`bindReactiveInput matches ${vectors.cases.length} Rust binding cases`, () => {
  let n = 0;
  for (const c of vectors.cases) {
    for (const r of c.results) {
      const got = bindReactiveInput({
        value: r.value,
        target: c.target,
        ...(c.inputRange ? { input: c.inputRange } : {}),
        ...(c.outputRange ? { output: c.outputRange } : {}),
        ...(c.curve ? { curve: c.curve } : {}),
      });
      assert.deepEqual(Object.keys(got).sort(), Object.keys(r.overrides).sort(), `${c.target} @ ${r.value}`);
      for (const [k, v] of Object.entries(r.overrides)) {
        assert.ok(Math.abs(got[k] - v) < 1e-12, `${c.target} @ ${r.value}: ${k} ${got[k]} vs ${v}`);
      }
      n++;
    }
  }
  assert.ok(n > 200);
});
