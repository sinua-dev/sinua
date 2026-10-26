// A light-volume dot (`blend` 3) is a plain, hard-edged disc, like an orb's: one `arc` at half of
// the dot's radius, filled with a flat HSL colour, added up on dark and laid on normally on light.
import { test } from "node:test";
import assert from "node:assert/strict";
import { drawFrame } from "../dist/index.js";

function recorder() {
  const calls = [];
  let comp = "source-over";
  const ctx = {
    save() {}, restore() {}, scale() {}, translate() {},
    beginPath() {}, moveTo() {}, lineTo() {}, closePath() {}, stroke() {}, fillRect() {},
    arc(x, y, r) { calls.push({ op: "arc", x, y, r, comp, alpha: this.globalAlpha, style: this.fillStyle }); },
    fill() {},
    drawImage() { calls.push({ op: "drawImage" }); },
    createRadialGradient() { throw new Error("a light dot is not a gradient sprite any more"); },
    set globalCompositeOperation(v) { comp = v; }, get globalCompositeOperation() { return comp; },
    globalAlpha: 1, fillStyle: "", strokeStyle: "", lineWidth: 1, lineCap: "butt", lineJoin: "miter",
  };
  return { ctx, calls };
}

const frame = {
  dots: [{ x: 10, y: 20, z: 0, r: 4, white: 0.6, a: 0.8, saturation: 0.7, hue: 180 }],
  lines: [], polylines: [], colorMode: "fixed",
  effects: [{ target: 0, start: 0, count: 1, blur: 0, blend: 3 }],
};

test("a light dot is one arc at half its radius, with a flat colour (no sprite)", () => {
  const { ctx, calls } = recorder();
  drawFrame(ctx, frame, true, 1);
  assert.equal(calls.filter((c) => c.op === "drawImage").length, 0);
  const [arc] = calls;
  assert.deepEqual([arc.x, arc.y, arc.r], [10, 20, 2]);
  assert.equal(arc.style, "hsl(180, 70%, 60%)");
  assert.equal(arc.alpha, 0.8);
});

test("on dark the discs add up; on light they are laid on normally and no brighter than the colour", () => {
  const dark = recorder();
  drawFrame(dark.ctx, frame, true, 1);
  assert.equal(dark.calls[0].comp, "lighter");
  const light = recorder();
  drawFrame(light.ctx, frame, false, 1);
  assert.equal(light.calls[0].comp, "source-over");
  assert.equal(light.calls[0].style, "hsl(180, 70%, 38%)");
});
