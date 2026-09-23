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
// Layout v2 (only for frames with fills or effect runs -- every other frame
// is still v1): header of 11, adding [nFills, nFillPoints, nStops,
// nEffects]; v1's sections unchanged; then fills x14 (white a saturation hue
// blur blend pointCount gradKind(-1 none/0 linear/1 radial) x0 y0 x1 y1 r
// stopCount), fill points x2, stops x5 (offset white a saturation hue),
// effects x5 (target start count blur blend).
// Layout v3 (only when some fill has holes -- liquid bands): v2 plus
// nHoleRings at header[11] (header 12), fills x15 (+ holeRingCount), and
// after the fill points a hole-ring table (nHoleRings pointCounts) then the
// hole points x2; stops and effects as in v2.
// Layout v4 (only when some polyline has per-vertex `hues`): v3 exactly
// (every section, possibly empty) plus nPolyHues at header[12] (header 13),
// and after the effects a table of nPolylines hue counts (0 or that
// polyline's pointCount), then the hues back to back.

import {
  frame_packed,
  frame_packed_with_overrides,
  frame_from_fx_spec_packed,
} from "../pkg/sinua_core_inline.js";
import type {
  ColorMode, Dot, EffectRun, Fill, FxContext, FxSpec, GradientStop, Line, OrbFrame, OrbSize, OrbState, Polyline,
} from "./index.js";

/** The newest layout this reader understands (it reads v1-v4). */
export const PACKED_LAYOUT_VERSION = 4;
const HEADER = 7;
const HEADER_2 = 11;
const HEADER_3 = 12;
const HEADER_4 = 13;

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
  /** v2 (0 counts on v1): fills, their points, gradient stops, effect runs. */
  fillCount: number;
  /** Fill record stride: 14 (v2) or 15 (v3, + holeRingCount at field 14). */
  fillStride: number;
  /** v3 (0 otherwise): hole rings; `holeRings` = the ring pointCount table, `holePoints` = their x, y pairs. */
  holeRingCount: number;
  holeRings: number;
  holePoints: number;
  fillPointCount: number;
  stopCount: number;
  effectCount: number;
  fills: number;
  fillPoints: number;
  stops: number;
  effects: number;
  /** v4 (0 / unused otherwise): per-vertex polyline hues; `polyHueCounts` = the per-polyline count table, `polyHues` = the hues back to back. */
  polyHueCount: number;
  polyHueCounts: number;
  polyHues: number;
}

/** Wrap a raw packed buffer; `null` when the engine returned no frame. */
export function asPacked(data: Float64Array): PackedFrame | null {
  const version = data[0];
  if (version !== 1 && version !== 2 && version !== 3 && version !== 4) {
    throw new Error(`@sinua/core: packed layout v${version}, this reader knows v1-v${PACKED_LAYOUT_VERSION}`);
  }
  if (data[6] === 0) return null;
  const v4 = version === 4;
  const v3 = version === 3 || v4;
  const v2 = version === 2 || v3;
  const dotCount = data[2];
  const lineCount = data[3];
  const polylineCount = data[4];
  const dots = v4 ? HEADER_4 : v3 ? HEADER_3 : v2 ? HEADER_2 : HEADER;
  const lines = dots + dotCount * 8;
  const polylines = lines + lineCount * 9;
  const points = polylines + polylineCount * 6;
  const fillCount = v2 ? data[7] : 0;
  const fillPointCount = v2 ? data[8] : 0;
  const stopCount = v2 ? data[9] : 0;
  const effectCount = v2 ? data[10] : 0;
  const fillStride = v3 ? 15 : 14;
  const holeRingCount = v3 ? data[11] : 0;
  const fills = points + data[5] * 2;
  const fillPoints = fills + fillCount * fillStride;
  const holeRings = fillPoints + fillPointCount * 2;
  const holePoints = holeRings + holeRingCount;
  let holePointTotal = 0;
  for (let k = 0; k < holeRingCount; k++) holePointTotal += data[holeRings + k];
  const stops = holePoints + holePointTotal * 2;
  const effects = stops + stopCount * 5;
  const polyHueCounts = effects + effectCount * 5;
  return {
    polyHueCount: v4 ? data[12] : 0,
    polyHueCounts,
    polyHues: polyHueCounts + polylineCount,
    fillStride,
    holeRingCount,
    holeRings,
    holePoints,
    fillCount,
    fillPointCount,
    stopCount,
    effectCount,
    fills,
    fillPoints,
    stops,
    effects,
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
  if (p.polyHueCount > 0) {
    for (let i = 0, h = p.polyHues; i < p.polylineCount; i++) {
      const n = d[p.polyHueCounts + i];
      if (n > 0) {
        polylines[i].hues = Array.from(d.subarray(h, h + n));
        h += n;
      }
    }
  }
  const frame: OrbFrame = { dots, lines, polylines, colorMode: p.colorMode };
  if (p.fillCount > 0) {
    const fills: Fill[] = new Array(p.fillCount);
    let ring = p.holeRings;
    let hq = p.holePoints;
    for (let i = 0, o = p.fills, q = p.fillPoints, st = p.stops; i < p.fillCount; i++, o += p.fillStride) {
      const n = d[o + 6];
      const points = new Array(n);
      for (let j = 0; j < n; j++, q += 2) points[j] = { x: d[q], y: d[q + 1] };
      const nh = p.fillStride === 15 ? d[o + 14] : 0;
      const holes: { x: number; y: number }[][] = [];
      for (let h = 0; h < nh; h++) {
        const m = d[ring++];
        const hr = new Array(m);
        for (let j = 0; j < m; j++, hq += 2) hr[j] = { x: d[hq], y: d[hq + 1] };
        holes.push(hr);
      }
      const ns = d[o + 13];
      let gradient: Fill["gradient"] = null;
      if (d[o + 7] >= 0) {
        const gs: GradientStop[] = new Array(ns);
        for (let j = 0; j < ns; j++, st += 5) {
          gs[j] = { offset: d[st], white: d[st + 1], a: d[st + 2], saturation: d[st + 3], hue: d[st + 4] };
        }
        gradient = { kind: d[o + 7] as 0 | 1, x0: d[o + 8], y0: d[o + 9], x1: d[o + 10], y1: d[o + 11], r: d[o + 12], stops: gs };
      }
      fills[i] = {
        points, white: d[o], a: d[o + 1], saturation: d[o + 2], hue: d[o + 3],
        gradient, blur: d[o + 4], blend: d[o + 5] as 0 | 1,
      };
      if (holes.length) fills[i].holes = holes;
    }
    frame.fills = fills;
  }
  if (p.effectCount > 0) {
    const effects: EffectRun[] = new Array(p.effectCount);
    for (let i = 0, o = p.effects; i < p.effectCount; i++, o += 5) {
      effects[i] = { target: d[o] as 0 | 1 | 2, start: d[o + 1], count: d[o + 2], blur: d[o + 3], blend: d[o + 4] as 0 | 1 };
    }
    frame.effects = effects;
  }
  return frame;
}

/**
 * Paint straight from the buffer, no objects: the visitor gets each
 * primitive's section offset into `p.data` in paint-contract order (fills,
 * polylines, lines, dots). Effect runs: `p.effects` / `p.effectCount`
 * (5 fields each), or `unpackFrame(p).effects`. A polyline's points are
 * `data[pointsAt .. pointsAt + 2 * count]` as x, y pairs.
 */
export interface PackedVisitor {
  /**
   * v3: `holes` when this fill has hole rings -- `data[o + 14]` of them; ring k's
   * pointCount at `data[holes.ringsAt + k]`, its points back to back from
   * `holes.pointsAt` (x, y pairs). Paint outer + holes as one even-odd path.
   * v2: a fill's 14 fields at `at`; its points at `pointsAt` (x, y pairs,
   * `data[at + 6]` of them); its stops at `stopsAt` (`data[at + 13]` x 5,
   * none when `data[at + 7]` is -1). Visited first: fills paint under
   * everything.
   */
  fill?(data: Float64Array, at: number, pointsAt: number, stopsAt: number, holes?: { ringsAt: number; pointsAt: number }): void;
  /**
   * v4: `huesAt` when this polyline has per-vertex hues -- `count` of them
   * from `data[huesAt]`, one per point (paint contract: per-segment
   * gradient strokes in one layer, composited once at the stroke's alpha).
   */
  polyline?(data: Float64Array, styleAt: number, pointsAt: number, count: number, huesAt?: number): void;
  line?(data: Float64Array, at: number): void;
  dot?(data: Float64Array, at: number): void;
}

export function readPacked(p: PackedFrame, v: PackedVisitor): void {
  const d = p.data;
  if (v.fill) {
    let ring = p.holeRings;
    let hq = p.holePoints;
    for (let i = 0, o = p.fills, q = p.fillPoints, st = p.stops; i < p.fillCount; i++, o += p.fillStride) {
      const nh = p.fillStride === 15 ? d[o + 14] : 0;
      v.fill(d, o, q, st, nh ? { ringsAt: ring, pointsAt: hq } : undefined);
      for (let h = 0; h < nh; h++) hq += d[ring++] * 2;
      q += d[o + 6] * 2;
      st += (d[o + 7] >= 0 ? d[o + 13] : 0) * 5;
    }
  }
  if (v.polyline) {
    for (let i = 0, o = p.polylines, q = p.points, h = p.polyHues; i < p.polylineCount; i++, o += 6) {
      const n = d[o + 5];
      const nh = p.polyHueCount > 0 ? d[p.polyHueCounts + i] : 0;
      v.polyline(d, o, q, n, nh > 0 ? h : undefined);
      q += n * 2;
      h += nh;
    }
  }
  if (v.line) for (let i = 0, o = p.lines; i < p.lineCount; i++, o += 9) v.line(d, o);
  if (v.dot) for (let i = 0, o = p.dots; i < p.dotCount; i++, o += 8) v.dot(d, o);
}
