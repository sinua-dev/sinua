// The Web fast path for frames: one `Float64Array` per frame instead of a
// JSON string (crates/core_engine/src/transport.rs has the layout and the
// why). wasm-bindgen copies the packed buffer out of linear memory once;
// there is no JSON.stringify/JSON.parse and, with `readPacked`, no object
// per dot either. f64 throughout, so `unpackFrame(p)` is bit-identical to
// the JSON frame (packages/core/test/packed.test.mjs checks every golden case).
//
// Layout v1: header [version, colorMode (0 ink / 1 fixed), nDots, nLines,
// nPolylines, nPoints, found], then dots x8 (x y z r white a saturation
// hue), lines x9 (x1 y1 x2 y2 white a w saturation hue), polylines x6
// (white a w saturation hue pointCount), points x2 (x y).
// Layout v2 (only for frames with effect runs -- every other frame is still
// v1): header of 8, adding [nEffects]; v1's sections unchanged; then
// effects x5 (target start count blur blend).

import {
  frame_packed,
  frame_packed_with_overrides,
  frame_from_fx_spec_packed,
} from "../pkg/sinua_core_inline.js";
import type {
  ColorMode, Dot, EffectRun, FxContext, FxSpec, Line, OrbFrame, OrbSize, OrbState, Polyline,
} from "./index.js";

/** The newest layout this reader understands (it reads v1 and v2). */
export const PACKED_LAYOUT_VERSION = 2;
const HEADER = 7;
const HEADER_2 = 8;

/** A frame as one typed array, plus where each section starts. */
export interface PackedFrame {
  data: Float64Array;
  colorMode: ColorMode;
  dotCount: number;
  lineCount: number;
  polylineCount: number;
  pointCount: number;
  /** Section start indices into `data`. */
  dots: number;
  lines: number;
  polylines: number;
  points: number;
  /** v2 (0 on v1): effect runs, 5 fields each. */
  effectCount: number;
  effects: number;
}

/** Wrap a raw packed buffer; `null` when the engine returned no frame. */
export function asPacked(data: Float64Array): PackedFrame | null {
  const version = data[0];
  if (version !== 1 && version !== 2) {
    throw new Error(`@sinua/core: packed layout v${version}, this reader knows v1-v${PACKED_LAYOUT_VERSION}`);
  }
  if (data[6] === 0) return null;
  const v2 = version === 2;
  const dotCount = data[2];
  const lineCount = data[3];
  const polylineCount = data[4];
  const dots = v2 ? HEADER_2 : HEADER;
  const lines = dots + dotCount * 8;
  const polylines = lines + lineCount * 9;
  const points = polylines + polylineCount * 6;
  const effectCount = v2 ? data[7] : 0;
  return {
    effectCount,
    effects: points + data[5] * 2,
    data,
    colorMode: data[1] === 1 ? "fixed" : "ink",
    dotCount,
    lineCount,
    polylineCount,
    pointCount: data[5],
    dots,
    lines,
    polylines,
    points,
  };
}

/** `frame`, packed. */
export function framePacked(state: OrbState, size: OrbSize, t: number): PackedFrame | null {
  return asPacked(frame_packed(state, size, t));
}

/** `frameWithOverrides`, packed. */
export function frameWithOverridesPacked(
  state: OrbState,
  size: OrbSize,
  t: number,
  overrides: Partial<Record<string, number>>
): PackedFrame | null {
  return asPacked(frame_packed_with_overrides(state, size, t, JSON.stringify(overrides)));
}

/** `frameFromFxSpec`, packed. */
export function frameFromFxSpecPacked(spec: FxSpec | string, elapsed: number, ctx: FxContext = {}): PackedFrame | null {
  const text = typeof spec === "string" ? spec : JSON.stringify(spec);
  return asPacked(frame_from_fx_spec_packed(text, elapsed, JSON.stringify(ctx)));
}

/** Rebuild the object frame (identical to the JSON path's). */
export function unpackFrame(p: PackedFrame | null): OrbFrame | null {
  if (!p) return null;
  const d = p.data;
  const dots: Dot[] = new Array(p.dotCount);
  for (let i = 0, o = p.dots; i < p.dotCount; i++, o += 8) {
    dots[i] = { x: d[o], y: d[o + 1], z: d[o + 2], r: d[o + 3], white: d[o + 4], a: d[o + 5], saturation: d[o + 6], hue: d[o + 7] };
  }
  const lines: Line[] = new Array(p.lineCount);
  for (let i = 0, o = p.lines; i < p.lineCount; i++, o += 9) {
    lines[i] = {
      x1: d[o], y1: d[o + 1], x2: d[o + 2], y2: d[o + 3],
      white: d[o + 4], a: d[o + 5], w: d[o + 6], saturation: d[o + 7], hue: d[o + 8],
    };
  }
  const polylines: Polyline[] = new Array(p.polylineCount);
  for (let i = 0, o = p.polylines, q = p.points; i < p.polylineCount; i++, o += 6) {
    const n = d[o + 5];
    const points = new Array(n);
    for (let j = 0; j < n; j++, q += 2) points[j] = { x: d[q], y: d[q + 1] };
    polylines[i] = { points, white: d[o], a: d[o + 1], w: d[o + 2], saturation: d[o + 3], hue: d[o + 4] };
  }
  const frame: OrbFrame = { dots, lines, polylines, colorMode: p.colorMode };
  if (p.effectCount > 0) {
    const effects: EffectRun[] = new Array(p.effectCount);
    for (let i = 0, o = p.effects; i < p.effectCount; i++, o += 5) {
      effects[i] = { target: d[o] as 0 | 1, start: d[o + 1], count: d[o + 2], blur: d[o + 3], blend: d[o + 4] as 0 | 1 };
    }
    frame.effects = effects;
  }
  return frame;
}

/**
 * Paint straight from the buffer, no objects: the visitor gets each
 * primitive's section offset into `p.data` in paint-contract order
 * (polylines, lines, dots). Effect runs: `p.effects` / `p.effectCount`
 * (5 fields each), or `unpackFrame(p).effects`. A polyline's points are
 * `data[pointsAt .. pointsAt + 2 * count]` as x, y pairs.
 */
export interface PackedVisitor {
  polyline?(data: Float64Array, styleAt: number, pointsAt: number, count: number): void;
  line?(data: Float64Array, at: number): void;
  dot?(data: Float64Array, at: number): void;
}

export function readPacked(p: PackedFrame, v: PackedVisitor): void {
  const d = p.data;
  if (v.polyline) {
    for (let i = 0, o = p.polylines, q = p.points; i < p.polylineCount; i++, o += 6) {
      const n = d[o + 5];
      v.polyline(d, o, q, n);
      q += n * 2;
    }
  }
  if (v.line) for (let i = 0, o = p.lines; i < p.lineCount; i++, o += 9) v.line(d, o);
  if (v.dot) for (let i = 0, o = p.dots; i < p.dotCount; i++, o += 8) v.dot(d, o);
}
