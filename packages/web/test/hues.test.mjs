// Per-vertex stroke colour (Polyline.hues) on the Web painter: the call shape.
// A recording 2D context stands in for both the target and the scratch layer;
// Chrome checks the pixels (scripts/materials, docs/fx-view.md).
import { test } from "node:test";
import assert from "node:assert/strict";
import { drawFrame, drawPacked } from "../dist/index.js";
import { frameWithOverrides, frameWithOverridesPacked } from "@sinua/core";

function recorder(name, log) {
  let state = { alpha: 1, comp: "source-over", filter: "none", m: [1, 0, 0, 1, 0, 0] };
  const stack = [];
  const ctx = {
    canvas: { width: 256, height: 256 },
    save() { stack.push({ ...state }); },
    restore() { state = stack.pop(); },
    scale(s) { state.m = [state.m[0] * s, 0, 0, state.m[3] * s, state.m[4], state.m[5]]; },
    setTransform(a, b, c, d, e, f) {
      state.m = typeof a === "object" ? [a.a, a.b, a.c, a.d, a.e, a.f] : [a, b, c, d, e, f];
    },
    getTransform() { const [a, b, c, d, e, f] = state.m; return { a, b, c, d, e, f }; },
    translate() {},
    beginPath() {}, moveTo() {}, lineTo() {}, closePath() {}, arc(...a) { log.push([name, "arc", a]); },
    clearRect(...a) { log.push([name, "clear", a]); },
    fill() { log.push([name, "fill", this.fillStyle]); },
    stroke() { log.push([name, "stroke", this.strokeStyle, { alpha: state.alpha, cap: this.lineCap }]); },
    drawImage(src, ...a) { log.push([name, "drawImage", { alpha: state.alpha, comp: state.comp, filter: state.filter, m: state.m, box: a }]); },
    createLinearGradient(...a) { const g = { a, stops: [] }; g.addColorStop = (o, c) => g.stops.push([o, c]); return g; },
    createRadialGradient() { return { addColorStop() {} }; },
    set globalAlpha(v) { state.alpha = v; }, get globalAlpha() { return state.alpha; },
    set globalCompositeOperation(v) { state.comp = v; }, get globalCompositeOperation() { return state.comp; },
    set filter(v) { state.filter = v; }, get filter() { return state.filter; },
    set shadowBlur(v) {}, set shadowOffsetX(v) {}, set shadowOffsetY(v) {}, set shadowColor(v) {},
    fillStyle: "", strokeStyle: "", lineWidth: 1, lineCap: "butt", lineJoin: "miter",
  };
  return ctx;
}

const log = [];
// The painter's scratch layer: one OffscreenCanvas, created lazily and reused.
globalThis.OffscreenCanvas = class {
  constructor(w, h) { this.width = w; this.height = h; this.ctx = recorder("layer", log); }
  getContext() { return this.ctx; }
};

const pts = [{ x: 10, y: 10 }, { x: 20, y: 10 }, { x: 20, y: 10 }, { x: 30, y: 20 }];
const poly = (extra = {}) => ({ points: pts, white: 0.5, a: 0.5, w: 2, saturation: 1, hue: 180, ...extra });
const base = { dots: [], lines: [], colorMode: "ink" };

test("without hues: one stroked path on the target at alpha a (unchanged)", () => {
  log.length = 0;
  const target = recorder("target", log);
  drawFrame(target, { ...base, polylines: [poly()] }, false, 4);
  assert.deepEqual(log.map((l) => l.slice(0, 2)), [["target", "stroke"]]);
  assert.equal(log[0][2], "hsla(180, 100%, 50%, 0.5)");
});

test("with hues: gradient segments at alpha 1 in the layer, one composite at a", () => {
  log.length = 0;
  const target = recorder("target", log);
  drawFrame(target, { ...base, polylines: [poly({ hues: [0, 120, 120, 240] })] }, false, 4);
  const layer = log.filter((l) => l[0] === "layer");
  const strokes = layer.filter((l) => l[1] === "stroke");
  assert.equal(strokes.length, 2, "two real segments");
  assert.deepEqual(strokes[0][2].stops, [[0, "hsla(0, 100%, 50%, 1)"], [1, "hsla(120, 100%, 50%, 1)"]]);
  assert.deepEqual(strokes[1][2].stops, [[0, "hsla(120, 100%, 50%, 1)"], [1, "hsla(240, 100%, 50%, 1)"]]);
  assert.equal(strokes[0][3].cap, "round");
  assert.equal(strokes[0][3].alpha, 1, "segments at alpha 1");
  const dot = layer.filter((l) => l[1] === "fill");
  assert.deepEqual(dot.map((l) => l[2]), ["hsla(120, 100%, 50%, 1)"], "zero-length segment: a dot in vertex i's colour");
  assert.equal(layer.find((l) => l[1] === "arc")[2][2], 1, "dot diameter w");
  const comps = log.filter((l) => l[0] === "target");
  assert.deepEqual(comps.map((l) => l[1]), ["drawImage"], "nothing drawn on the target but the composite");
  assert.equal(comps[0][2].alpha, 0.5, "composited once at a");
  assert.deepEqual(comps[0][2].m, [1, 0, 0, 1, 0, 0], "composite in device space");
  // bbox: user (10..30, 10..20) +- w/2 at scale 4 -> device 36..124 x 36..84, +2 px.
  assert.deepEqual(comps[0][2].box, [34, 34, 92, 52, 34, 34, 92, 52]);
  assert.deepEqual(layer.find((l) => l[1] === "clear")[2], [34, 34, 92, 52], "only the bbox is cleared");
});

test("dark theme mirrors each vertex ink; fixed colour mode doesn't", () => {
  // white 0.2: "ink" mirrors lightness to 80% on dark, "fixed" keeps 20%.
  for (const [colorMode, want] of [["ink", "hsla(0, 100%, 80%, 1)"], ["fixed", "hsla(0, 100%, 20%, 1)"]]) {
    log.length = 0;
    drawFrame(recorder("target", log), { ...base, colorMode, polylines: [poly({ white: 0.2, hues: [0, 0, 0, 0] })] }, true, 1);
    const stop = log.find((l) => l[1] === "stroke")[2].stops[0][1];
    assert.equal(stop, want, colorMode);
  }
});

test("effect run: blur and additive blend applied at the composite, not per segment", () => {
  log.length = 0;
  const target = recorder("target", log);
  const frame = { ...base, polylines: [poly({ hues: [0, 120, 120, 240] })], effects: [{ target: 2, start: 0, count: 1, blur: 1.5, blend: 1 }] };
  drawFrame(target, frame, false, 4);
  const comp = log.find((l) => l[0] === "target" && l[1] === "drawImage")[2];
  assert.equal(comp.filter, "blur(6px)");
  assert.equal(comp.comp, "lighter");
  assert.equal(comp.alpha, 0.5);
  for (const s of log.filter((l) => l[0] === "layer" && l[1] === "stroke")) assert.equal(s[3].alpha, 1);
});

test("an outer globalAlpha (cross-dissolve) multiplies into the composite", () => {
  log.length = 0;
  const target = recorder("target", log);
  target.globalAlpha = 0.4;
  drawFrame(target, { ...base, polylines: [poly({ hues: [0, 120, 120, 240] })] }, false, 4);
  assert.equal(log.find((l) => l[1] === "drawImage")[2].alpha, 0.2);
});

test("hues of the wrong length fall back to the single hue", () => {
  log.length = 0;
  drawFrame(recorder("target", log), { ...base, polylines: [poly({ hues: [0, 120] })] }, false, 4);
  assert.deepEqual(log.map((l) => l.slice(0, 2)), [["target", "stroke"]]);
});

test("packed v4 (engine holo frame) paints exactly like its object frame", () => {
  const packed = frameWithOverridesPacked("tracking", 64, 0.6, { holoStrength: 1 });
  const frame = frameWithOverrides("tracking", 64, 0.6, { holoStrength: 1 });
  assert.equal(packed.data[0], 4, "a frame with hues is packed v4");
  assert.ok(frame.polylines.some((p) => p.hues && p.hues.length), "the engine emits hues");
  log.length = 0;
  drawFrame(recorder("target", log), frame, false, 4);
  const want = JSON.stringify(log);
  assert.ok(log.some((l) => l[1] === "drawImage"), "per-vertex path taken");
  log.length = 0;
  drawPacked(recorder("target", log), packed, false, 4);
  assert.equal(JSON.stringify(log), want);
});
