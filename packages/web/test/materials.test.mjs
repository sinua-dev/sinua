// Effect runs on the Web painter: blur, additive, the order of the draw calls.
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

test("polylines draw first as one stroked path, then lines, then dots", () => {
  const { ctx, log } = recorder();
  drawFrame(ctx, {
    ...base,
    dots: [dot(5)],
    lines: [{ x1: 0, y1: 0, x2: 4, y2: 4, white: 0.2, a: 1, w: 1, saturation: 0, hue: 0 }],
    polylines: [{ points: [{ x: 0, y: 0 }, { x: 5, y: 5 }, { x: 8, y: 2 }], white: 0.1, a: 1, w: 1, saturation: 0, hue: 0 }],
    effects: [{ target: 0, start: 0, count: 1, blur: 0, blend: 0 }],
  }, false, 4);
  const draws = log.filter((l) => l[0] === "fill" || l[0] === "stroke").map((l) => l[0]);
  assert.deepEqual(draws, ["stroke", "stroke", "fill"], "polyline, line, dot");
  assert.equal(log.filter((l) => l[0] === "begin").length, 3, "one path per element");
});
