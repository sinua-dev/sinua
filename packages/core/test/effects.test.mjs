// One-shot effects through the wasm (docs/fx-view.md, *One-shot effects*):
// spec/effect-vectors.json, summarised the way every platform summarises it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { effectInfo, frameWithOverrides } from "../dist/index.js";

const vectors = JSON.parse(readFileSync(new URL("../../../spec/effect-vectors.json", import.meta.url), "utf8"));
const r6 = (x) => Math.round(x * 1e6) / 1e6;

function summary(f) {
  let alpha = 0, hue = 0, nh = 0, sx = 0, n = 0;
  for (const d of f.dots) { alpha += d.a; if (d.saturation > 0) { hue += d.hue; nh++; } sx += d.x; n++; }
  for (const l of f.lines) { alpha += l.a; if (l.saturation > 0) { hue += l.hue; nh++; } sx += l.x1 + l.x2; n += 2; }
  for (const p of f.polylines) { alpha += p.a; if (p.saturation > 0) { hue += p.hue; nh++; } for (const q of p.points) { sx += q.x; n++; } }
  return { dots: f.dots.length, lines: f.lines.length, polylines: f.polylines.length, alpha: r6(alpha), hue: nh ? r6(hue / nh) : 0, cx: n ? r6(sx / n) : 0 };
}

test("each effect renders as the engine vectors say", () => {
  assert.ok(vectors.cases.length >= 72);
  for (const c of vectors.cases) {
    const code = effectInfo(c.effect).code;
    const f = frameWithOverrides(c.pattern, vectors.size, vectors.t, { effectCode: code, effectAge: c.age, effectReduced: c.reduced ? 1 : 0 });
    const got = summary(f);
    for (const k of ["dots", "lines", "polylines"]) assert.equal(got[k], c.summary[k], `${c.pattern} ${c.effect} ${c.age} ${c.reduced} ${k}`);
    for (const k of ["alpha", "hue", "cx"]) assert.ok(Math.abs(got[k] - c.summary[k]) < 1e-5, `${c.pattern} ${c.effect} ${c.age} ${c.reduced} ${k}: ${got[k]} vs ${c.summary[k]}`);
  }
});

test("effect info names the three and nothing else", () => {
  assert.deepEqual(effectInfo("success"), { code: 1, duration: 0.9, words: "Done" });
  assert.equal(effectInfo("error").words, "Something went wrong");
  assert.equal(effectInfo("celebrate").duration, 1.4);
  assert.equal(effectInfo("confetti"), null);
});
