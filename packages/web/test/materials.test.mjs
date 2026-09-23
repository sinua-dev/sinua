// Materials phase 1 on the Web painter: fills, gradients, effect runs.
// A recording 2D context checks the calls; Chrome checks the pixels (see docs/fx-view.md).
import { test } from "node:test";
import assert from "node:assert/strict";
import { drawFrame } from "../dist/index.js";

function recorder() {
  const log = [];
  let state = { comp: "source-over", shadowBlur: 0, shadowOffsetX: 0, shadowColor: "", filter: "none" };
  const stack = [];
  const ctx = {
    canvas: { width: 256, height: 256 },
    save() { stack.push({ ...state }); },
    restore() { state = stack.pop(); },
    scale() {}, translate(x) { log.push(["translate", x]); },
    beginPath() { log.push(["begin"]); }, moveTo() {}, lineTo() {}, closePath() { log.push(["close"]); },
    arc() {},
    fill() { log.push(["fill", { ...state, style: this.fillStyle }]); },
    stroke() { log.push(["stroke", { ...state, style: this.strokeStyle }]); },
    createLinearGradient(...a) { const g = { kind: "linear", a, stops: [] }; g.addColorStop = (o, c) => g.stops.push([o, c]); return g; },
    createRadialGradient(...a) { const g = { kind: "radial", a, stops: [] }; g.addColorStop = (o, c) => g.stops.push([o, c]); return g; },
    set globalCompositeOperation(v) { state.comp = v; }, get globalCompositeOperation() { return state.comp; },
    set shadowBlur(v) { state.shadowBlur = v; }, set shadowOffsetX(v) { state.shadowOffsetX = v; }, set shadowOffsetY(v) {},
    set shadowColor(v) { state.shadowColor = v; },
    set filter(v) { state.filter = v; }, get filter() { return state.filter; },
    fillStyle: "", strokeStyle: "", lineWidth: 1, lineCap: "butt", lineJoin: "miter",
  };
  return { ctx, log };
}

const dot = (x, extra = {}) => ({ x, y: 10, z: 0, r: 2, white: 0.2, a: 0.8, saturation: 0, hue: 0, ...extra });
const base = { dots: [], lines: [], polylines: [], colorMode: "ink" };
const square = [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 10 }, { x: 0, y: 10 }];

test("fills draw first, as closed paths, before polylines/lines/dots", () => {
  const { ctx, log } = recorder();
  const frame = {
    ...base,
    dots: [dot(5)],
    polylines: [{ points: [{ x: 0, y: 0 }, { x: 5, y: 5 }], white: 0.1, a: 1, w: 1, saturation: 0, hue: 0 }],
    fills: [{ points: square, white: 0.3, a: 0.5, saturation: 0, hue: 0, gradient: null, blur: 0, blend: 0 }],
  };
  drawFrame(ctx, frame, false, 4);
  const draws = log.filter((l) => l[0] === "fill" || l[0] === "stroke").map((l) => l[0]);
  assert.deepEqual(draws, ["fill", "stroke", "fill"], "fill, polyline stroke, dot fill");
  assert.equal(log.findIndex((l) => l[0] === "close"), 1, "the fill path is closed");
  assert.equal(log.find((l) => l[0] === "fill")[1].style, "rgba(77,77,77,0.5)");
});

test("gradient stops go through ink with alpha = stop.a x fill.a, mirrored on dark (not when fixed)", () => {
  const g = { kind: 1, x0: 5, y0: 5, x1: 0, y1: 0, r: 8, stops: [{ offset: 0, white: 0, a: 1, saturation: 0, hue: 0 }, { offset: 1, white: 0, a: 0.5, saturation: 0, hue: 0 }] };
  const f = { points: square, white: 0, a: 0.5, saturation: 0, hue: 0, gradient: g, blur: 0, blend: 0 };
  for (const [colorMode, want] of [["ink", "rgba(255,255,255,0.5)"], ["fixed", "rgba(0,0,0,0.5)"]]) {
    const { ctx, log } = recorder();
    drawFrame(ctx, { ...base, colorMode, fills: [f] }, true, 4);
    const style = log.find((l) => l[0] === "fill")[1].style;
    assert.equal(style.kind, "radial");
    assert.deepEqual(style.a, [5, 5, 0, 5, 5, 8], "circle at x0,y0 from 0 to r");
    assert.deepEqual(style.stops, [[0, want], [1, want.replace("0.5)", "0.25)")]]);
  }
});

test("effect runs: additive only for its range; blur = shadowBlur 2*sigma*scale with the element drawn off-canvas", () => {
  const { ctx, log } = recorder();
  const frame = { ...base, dots: [dot(1), dot(2), dot(3), dot(4)], effects: [{ target: 0, start: 1, count: 2, blur: 1.5, blend: 1 }] };
  drawFrame(ctx, frame, false, 4);
  const fills = log.filter((l) => l[0] === "fill").map((l) => l[1]);
  assert.deepEqual(fills.map((f) => f.comp), ["source-over", "lighter", "lighter", "source-over"]);
  assert.deepEqual(fills.map((f) => f.shadowBlur), [0, 12, 12, 0]);
  assert.equal(fills[1].shadowOffsetX, 40000, "offset back into view, device px");
  assert.equal(fills[1].shadowColor, "rgba(51,51,51,1)", "opaque ink: the shadow takes the element's alpha");
  assert.equal(log.filter((l) => l[0] === "translate" && l[1] === -10000).length, 2);
});

test("sigma 0 (e.g. low power: blurScale 0) draws no shadow at all", () => {
  const { ctx, log } = recorder();
  drawFrame(ctx, { ...base, dots: [dot(1)], effects: [{ target: 0, start: 0, count: 1, blur: 0, blend: 0 }] }, false, 4);
  const f = log.find((l) => l[0] === "fill")[1];
  assert.equal(f.shadowBlur, 0);
  assert.equal(log.filter((l) => l[0] === "translate").length, 0);
});

test("a blurred gradient fill uses ctx.filter when present (sigma x scale px)", () => {
  const { ctx, log } = recorder();
  const g = { kind: 0, x0: 0, y0: 0, x1: 10, y1: 0, r: 0, stops: [{ offset: 0, white: 0, a: 1, saturation: 0, hue: 0 }, { offset: 1, white: 1, a: 1, saturation: 0, hue: 0 }] };
  drawFrame(ctx, { ...base, fills: [{ points: square, white: 0, a: 1, saturation: 0, hue: 0, gradient: g, blur: 2, blend: 1 }] }, false, 4);
  const f = log.find((l) => l[0] === "fill")[1];
  assert.equal(f.filter, "blur(8px)");
  assert.equal(f.comp, "lighter");
  assert.equal(f.style.kind, "linear");
});

test("low power sheds blur in the resolver: no effect runs, the painter takes the plain path", async () => {
  const { resolveFxSpec, frameWithOverrides, resolvedOpts } = await import("@sinua/core");
  const spec = JSON.stringify({
    fxSpec: "1.8", object: "orb", pattern: "glowing",
    materials: { glow: { strength: 1, mode: "blur" } },
    performance: { lowPower: { disable: ["blur"] } },
  });
  const full = resolveFxSpec(spec, {});
  const low = resolveFxSpec(spec, { lowPower: true });
  assert.ok(full.ok && low.ok, JSON.stringify(full.diagnostics));
  const frameOf = (r) => frameWithOverrides(r.state, r.size, 0.6 * resolvedOpts(r.state, r.size).speed * r.speed, r.overrides);
  assert.ok((frameOf(full).effects ?? []).length > 0, "blur glow emits effect runs");
  assert.equal((frameOf(low).effects ?? []).length, 0, "low power: no runs at all");
  assert.ok(low.disabledMaterials.includes("blur"));
});

test("holes: outer + hole rings as one path, filled even-odd; no holes keeps the default rule", () => {
  const hole = [{ x: 3, y: 3 }, { x: 7, y: 3 }, { x: 7, y: 7 }, { x: 3, y: 7 }];
  for (const [holes, rule, closes] of [[[hole], "evenodd", 2], [undefined, undefined, 1]]) {
    const { ctx, log } = recorder();
    const fills = [];
    ctx.fill = (r) => fills.push(r);
    drawFrame(ctx, { ...base, fills: [{ points: square, holes, white: 0.3, a: 1, saturation: 0, hue: 0, gradient: null, blur: 0, blend: 0 }] }, false, 4);
    assert.equal(log.filter((l) => l[0] === "close").length, closes);
    assert.equal(log.filter((l) => l[0] === "begin").length, 1, "one path");
    assert.deepEqual(fills, [rule]);
  }
});
