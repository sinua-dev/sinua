// The light-volume dot is a flat disc of a fixed fraction of the dot's radius, and the three
// painters must agree on that fraction and on the colour rule (twin rule): the web, Swift and
// Kotlin sources are read and compared.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
const read = (p) => readFileSync(new URL(p, root), "utf8");
const web = read("packages/web/src/paint.ts");
const swift = read("packages/ios/Sources/Sinua/FxPaint.swift");
const kotlin = read("packages/android/view/src/main/kotlin/dev/sinua/view/FxPaint.kt");

test("the three painters draw a light dot at the same fraction of its radius", () => {
  const w = web.match(/const LIGHT_CORE = ([0-9.]+);/)?.[1];
  const s = swift.match(/static let lightCore = ([0-9.]+)/)?.[1];
  const k = kotlin.match(/const val FX_LIGHT_CORE = ([0-9.]+)f/)?.[1];
  assert.ok(w && s && k, `found ${w} ${s} ${k}`);
  assert.equal(Number(w), Number(s));
  assert.equal(Number(w), Number(k));
});

test("the colour rule is the same: the colour on dark, 22 points darker (floor 10) on light", () => {
  assert.match(web, /dark \? l : Math\.max\(10, l - 22\)/);
  assert.match(swift, /dark \? l : max\(0\.10, l - 0\.22\)/);
  assert.match(kotlin, /if \(dark\) l else max\(0\.10, l - 0\.22\)/);
});

test("no painter keeps the old 32-px glow sprite", () => {
  assert.doesNotMatch(web, /lightSprite|createRadialGradient/);
  assert.doesNotMatch(swift, /lightSprite|drawRadialGradient/);
  assert.doesNotMatch(kotlin, /lightSprite|RadialGradient/);
});
