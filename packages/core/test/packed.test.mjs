// The packed Float64Array transport (packed.ts / transport.rs) must be
// bit-identical to the JSON path: unpackFrame(packed) deepEqual the JSON
// frame on every spec/sinua-golden.json case, every FX Spec example in
// every state (both power states), and the "no frame" case. readPacked must
// visit the same primitives in paint-contract order.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
// The public frame/frameWithOverrides/frameFromFxSpec now ride the packed
// path, so the JSON reference is the explicit *ViaJson bridge.
import {
  frameViaJson as frame, frameWithOverridesViaJson as frameWithOverrides,
  frameFromFxSpecViaJson as frameFromFxSpec, resolveFxSpec,
  framePacked, frameWithOverridesPacked, frameFromFxSpecPacked, unpackFrame, readPacked,
} from "../dist/index.js";

import * as pub from "../dist/index.js";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const golden = JSON.parse(readFileSync(here("../../../spec/sinua-golden.json"), "utf8"));
const dir = here("../../../spec/examples/");
const INPUTS = { micMuted: 0, micLevel: 0.6, agentVolume: 0.5, steps: 6200, waterMl: 1800, activeMinutes: 12, heartRate: 128 };

test(`packed == JSON on all ${golden.cases.length} golden cases`, () => {
  for (const c of golden.cases) {
    const json = frameWithOverrides(c.state, c.size, c.t, c.overrides);
    const packed = frameWithOverridesPacked(c.state, c.size, c.t, c.overrides);
    assert.deepEqual(unpackFrame(packed), json, c.key);
    assert.deepEqual(pub.frameWithOverrides(c.state, c.size, c.t, c.overrides), json, `${c.key} (public API)`);
    // v4 exactly when a polyline has per-vertex hues, v3 when a fill has
    // holes, v2 with fills or effect runs; v1 otherwise (byte identity).
    const holes = (json.fills ?? []).some((x) => x.holes && x.holes.length);
    const hues = json.polylines.some((p) => p.hues && p.hues.length);
    assert.equal(packed.data[0], hues ? 4 : holes ? 3 : json.fills || json.effects ? 2 : 1, `${c.key}: layout version`);
  }
  assert.deepEqual(unpackFrame(framePacked("working", 64, 1.3)), frame("working", 64, 1.3));
  assert.equal(framePacked("not-a-state", 64, 0), null);
  assert.equal(frameWithOverridesPacked("not-a-state", 64, 0, {}), null);
});

test("packed == JSON for every FX Spec example, state and power state", () => {
  let n = 0;
  for (const f of readdirSync(dir).filter((f) => f.endsWith(".fxspec.json"))) {
    const text = readFileSync(dir + f, "utf8");
    for (const state of [undefined, ...resolveFxSpec(text).stateKeys]) {
      for (const lowPower of [false, true]) {
        const ctx = { state, inputs: INPUTS, lowPower };
        assert.deepEqual(unpackFrame(frameFromFxSpecPacked(text, 1.3, ctx)), frameFromFxSpec(text, 1.3, ctx), `${f} ${state}`);
        n++;
      }
    }
  }
  assert.ok(n >= 26);
  assert.equal(frameFromFxSpecPacked("{", 0), null);
});

test("readPacked visits polylines, lines, dots with their data", () => {
  for (const [state, ov] of [["connecting", {}], ["scrolling", {}], ["tracking", { progress0: 1.4 }]]) {
    const p = frameWithOverridesPacked(state, 64, 1.3, ov);
    const f = frameWithOverrides(state, 64, 1.3, ov);
    const seen = { poly: [], line: [], dot: [] };
    readPacked(p, {
      polyline: (d, s, q, n) => seen.poly.push({ w: d[s + 2], first: [d[q], d[q + 1]], last: [d[q + 2 * n - 2], d[q + 2 * n - 1]], n }),
      line: (d, o) => seen.line.push([d[o], d[o + 1], d[o + 6]]),
      dot: (d, o) => seen.dot.push([d[o], d[o + 1], d[o + 3]]),
    });
    assert.deepEqual(seen.dot, f.dots.map((x) => [x.x, x.y, x.r]), state);
    assert.deepEqual(seen.line, f.lines.map((l) => [l.x1, l.y1, l.w]), state);
    assert.deepEqual(
      seen.poly,
      f.polylines.map((pl) => ({ w: pl.w, first: [pl.points[0].x, pl.points[0].y], last: [pl.points.at(-1).x, pl.points.at(-1).y], n: pl.points.length })),
      state
    );
  }
});

test("readPacked hands v4 polylines their per-vertex hues", () => {
  const p = frameWithOverridesPacked("tracking", 64, 0.6, { holoStrength: 1, glowStrength: 0.8, glowMode: 1 });
  assert.equal(p.data[0], 4);
  const f = unpackFrame(p);
  const seen = [];
  readPacked(p, {
    polyline(d, o, q, n, huesAt) {
      seen.push(huesAt === undefined ? undefined : Array.from(d.subarray(huesAt, huesAt + n)));
    },
  });
  assert.deepEqual(seen, f.polylines.map((pl) => pl.hues));
  assert.ok(seen.some((h) => h && h.length > 2));
});
