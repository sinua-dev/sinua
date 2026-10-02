import { test } from "node:test";
import assert from "node:assert/strict";
import { fitPath, pathBox, svgPaths } from "../dist/index.js";

const near = (a, b, eps = 0.15) => a.every((v, i) => Math.abs(v - b[i]) <= eps);

test("fitPath: fits into the box, centred, keeping proportions", () => {
  const r = fitPath("M0 0 L20 0 L20 10 L0 10 Z", [0, 0, 200, 200]);
  assert.equal(r.d, "M0 50 L200 50 L200 150 L0 150 Z");
  assert.equal(r.holes, 0);
  assert.deepEqual(r.warnings, []);
  assert.ok(near(pathBox(fitPath("M5 5 L15 25 L25 5 Z").d), [10, 10, 190, 190]));
  assert.ok(near(pathBox(fitPath("M0 0 L10 0 L10 40 Z").d), [77.5, 10, 122.5, 190]), "tall: centred across");
});

test("fitPath: relative and shorthand commands come out absolute", () => {
  const a = fitPath("m10 10 h20 v20 h-20 z", [0, 0, 100, 100]).d;
  const b = fitPath("M10 10 L30 10 L30 30 L10 30 Z", [0, 0, 100, 100]).d;
  assert.equal(a, b);
  assert.match(fitPath("M0 0 C10 0 20 10 20 20 S30 40 40 40", [0, 0, 40, 40]).d, /^M0 0 C10 0 20 10 20 20 C20 30 30 40 40 40$/);
  assert.match(fitPath("M0 0 Q10 20 20 0 T40 0", [0, 0, 40, 20]).d, /Q30 -20 40 0|Q30 -?\d/);
  assert.doesNotMatch(fitPath("M0 0 L10 0 10 10").d, /[HVSTA]/, "implicit line pairs after M");
});

test("fitPath: an arc becomes cubics that stay on the circle", () => {
  // A half circle of radius 50 around (50, 50), kept at scale 1.
  const r = fitPath("M0 50 A50 50 0 0 1 100 50 Z", [0, 0, 100, 50]);
  assert.doesNotMatch(r.d, /A/);
  assert.equal((r.d.match(/C/g) ?? []).length, 2, "two quarter arcs");
  // Sample each cubic: every point within 0.5 of the radius.
  const nums = r.d.match(/-?\d+(\.\d+)?/g).map(Number);
  let cur = [nums[0], nums[1]];
  for (const m of r.d.matchAll(/C(-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+)/g)) {
    const [c1x, c1y, c2x, c2y, x, y] = m.slice(1).map(Number);
    for (let i = 0; i <= 20; i++) {
      const t = i / 20, u = 1 - t;
      const px = u * u * u * cur[0] + 3 * u * u * t * c1x + 3 * u * t * t * c2x + t * t * t * x;
      const py = u * u * u * cur[1] + 3 * u * u * t * c1y + 3 * u * t * t * c2y + t * t * t * y;
      assert.ok(Math.abs(Math.hypot(px - 50, py - 50) - 50) < 0.5, `(${px}, ${py})`);
    }
    cur = [x, y];
  }
});

test("fitPath: later subpaths are holes; limits warn; bad paths throw", () => {
  assert.equal(fitPath("M0 0 H100 V100 H0 Z M40 40 H60 V60 H40 Z").holes, 1);
  const many = "M0 0 " + Array.from({ length: 600 }, (_, i) => `L${i} ${i % 7}`).join(" ");
  assert.match(fitPath(many).warnings.join(), /at most 512/);
  assert.throws(() => fitPath("10 10 L20 20"), /start with a command/);
  assert.throws(() => fitPath(""), /empty/);
  assert.throws(() => fitPath("M0 0 L10"), /number was expected/);
});

test("svgPaths: every <path>, joined; transforms and other shapes are flagged", () => {
  const svg = `<svg viewBox="0 0 24 24"><path d="M0 0 H10 V10 Z"/><g><path fill="red" d='M2 2 h4 v4 z' transform="scale(2)"/></g><circle r="3"/></svg>`;
  const r = svgPaths(svg);
  assert.equal(r.d, "M0 0 H10 V10 Z M2 2 h4 v4 z");
  assert.deepEqual(r.warnings.sort(), ["a path's transform is ignored", "only <path> elements are read (convert shapes to paths)"]);
  assert.throws(() => svgPaths("<svg><rect/></svg>"), /no <path>/);
});
