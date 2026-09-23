// The paint contract, moved verbatim from the Web Studio's drawFrame.ts
// (the Studio now re-exports it from here). Shared by SinuaView, the Studio's
// live canvas and its PNG exporter, so they can never drift.
import { readPacked, unpackFrame, type OrbFrame, type PackedFrame } from "@sinua/core";

/**
 * Ink color for a dot/line: grayscale by default (`saturation` 0, which is
 * every ported mode's value, via `..Default::default()` on the Rust side --
 * this branch is byte-identical to the pre-color behavior for all 9 of
 * them), HSL when a mode (currently only `"glowing"`, see
 * `orbs::modes::aurora`) sets `saturation` above 0. `white` doubles as
 * lightness in the colored case, same dark-theme mirroring as the
 * grayscale path.
 */
export function ink(white: number, saturation: number, hue: number, alpha: number, dark: boolean): string {
  const w = Math.min(1, Math.max(0, white));
  const lightness = dark ? 1 - w : w;
  if (saturation <= 0) {
    const g = Math.round(lightness * 255);
    return `rgba(${g},${g},${g},${alpha})`;
  }
  const s = Math.min(1, Math.max(0, saturation));
  return `hsla(${hue}, ${Math.round(s * 100)}%, ${Math.round(lightness * 100)}%, ${alpha})`;
}

/**
 * Paint contract (spec/orbs-spec.json, plus `Polyline`'s doc comment in
 * crates/core_engine/src/primitives.rs): polylines draw first, then lines,
 * then dots; plain source-over fills; ink mirrors on dark themes --
 * grey = round((dark ? 1 - white : white) * 255). Shared by the live
 * `OrbCanvas` and the PNG exporter (`export/image.ts`) so they can never
 * drift from each other. `scale` is applied via `ctx.scale`, not baked into
 * the coordinates, so the same call draws crisp at any output resolution.
 */
export function drawFrame(ctx: CanvasRenderingContext2D, frame: OrbFrame, dark: boolean, scale: number): void {
  // Materials phase 1 (fills, effect runs): its own path, so every frame
  // without them keeps the exact calls below (byte-identical output).
  const fx = frame as PaintFrame;
  if ((fx.fills && fx.fills.length) || (fx.effects && fx.effects.length)) {
    drawFrameWithMaterials(ctx, fx, dark, scale);
    return;
  }
  // Paint contract, colour mode (2026-09-18): "ink" frames mirror lightness
  // on dark themes; "fixed" frames keep `white` as-is in both themes (a
  // brand colour looks the same everywhere). The background stays themed.
  const mirror = dark && frame.colorMode !== "fixed";
  ctx.save();
  ctx.scale(scale, scale);
  // One stroked path per polyline, with round caps and joins -- never one
  // stroke per segment, which is exactly the seam problem (wedge gaps
  // outside every bend, double-painted notches inside) this primitive
  // exists to fix. Cap/join are reset afterwards so `Line`s keep the
  // upstream contract's butt-capped look.
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const p of frame.polylines) {
    if (p.points.length < 2) continue;
    if (hasHues(p)) {
      strokePolylineHues(ctx, p, mirror, scale, 0, 0);
      continue;
    }
    ctx.strokeStyle = ink(p.white, p.saturation, p.hue, p.a, mirror);
    ctx.lineWidth = p.w;
    ctx.beginPath();
    ctx.moveTo(p.points[0].x, p.points[0].y);
    for (let i = 1; i < p.points.length; i++) ctx.lineTo(p.points[i].x, p.points[i].y);
    ctx.stroke();
  }
  ctx.lineCap = "butt";
  ctx.lineJoin = "miter";
  for (const l of frame.lines) {
    ctx.strokeStyle = ink(l.white, l.saturation, l.hue, l.a, mirror);
    ctx.lineWidth = l.w;
    ctx.beginPath();
    ctx.moveTo(l.x1, l.y1);
    ctx.lineTo(l.x2, l.y2);
    ctx.stroke();
  }
  for (const d of frame.dots) {
    ctx.fillStyle = ink(d.white, d.saturation, d.hue, d.a, mirror);
    ctx.beginPath();
    ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
}

/**
 * The general-purpose transition between any two states that DON'T share a
 * point lattice (see `@sinua/core`'s `frameTransition` doc comment for
 * the one trio that gets a real morph instead): draws `frameA` at
 * `1 - blend` opacity, then `frameB` on top at `blend` opacity, via
 * `ctx.globalAlpha` -- a plain cross-fade of two independently-rendered
 * frames, not a per-point blend. Works for all 17 states uniformly since it
 * never looks inside either frame's `Dot`/`Line` data.
 */
export function drawCrossDissolve(
  ctx: CanvasRenderingContext2D,
  frameA: OrbFrame,
  frameB: OrbFrame,
  blend: number,
  dark: boolean,
  scale: number
): void {
  const b = Math.min(1, Math.max(0, blend));
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  ctx.save();
  ctx.globalAlpha = 1 - b;
  drawFrame(ctx, frameA, dark, scale);
  ctx.restore();
  ctx.save();
  ctx.globalAlpha = b;
  drawFrame(ctx, frameB, dark, scale);
  ctx.restore();
}

/**
 * `drawFrame` straight from the engine's packed transport
 * (`frameWithOverridesPacked`, @sinua/core `packed.ts`): the same paint
 * contract and the same canvas calls in the same order, without building an
 * object per dot. Pixel-identical to `drawFrame(unpackFrame(p))` (checked in
 * Chrome, docs/fx-view.md).
 */
export function drawPacked(ctx: CanvasRenderingContext2D, p: PackedFrame, dark: boolean, scale: number): void {
  // Packed v2 (fills / effect runs): the object path. Plain v1 frames -- every
  // frame without materials -- keep the no-object path below, unchanged.
  if (p.data[0] !== 1) {
    const frame = unpackFrame(p);
    if (frame) drawFrame(ctx, frame, dark, scale);
    return;
  }
  const mirror = dark && p.colorMode !== "fixed";
  ctx.save();
  ctx.scale(scale, scale);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  readPacked(p, {
    polyline(d, o, q, n) {
      if (n < 2) return;
      ctx.strokeStyle = ink(d[o], d[o + 3], d[o + 4], d[o + 1], mirror);
      ctx.lineWidth = d[o + 2];
      ctx.beginPath();
      ctx.moveTo(d[q], d[q + 1]);
      for (let j = 1; j < n; j++) ctx.lineTo(d[q + 2 * j], d[q + 2 * j + 1]);
      ctx.stroke();
    },
  });
  ctx.lineCap = "butt";
  ctx.lineJoin = "miter";
  readPacked(p, {
    line(d, o) {
      ctx.strokeStyle = ink(d[o + 4], d[o + 7], d[o + 8], d[o + 5], mirror);
      ctx.lineWidth = d[o + 6];
      ctx.beginPath();
      ctx.moveTo(d[o], d[o + 1]);
      ctx.lineTo(d[o + 2], d[o + 3]);
      ctx.stroke();
    },
    dot(d, o) {
      ctx.fillStyle = ink(d[o + 4], d[o + 6], d[o + 7], d[o + 5], mirror);
      ctx.beginPath();
      ctx.arc(d[o], d[o + 1], d[o + 3], 0, Math.PI * 2);
      ctx.fill();
    },
  });
  ctx.restore();
}

// ------------------------------------------------ materials phase 1 --
// Contract agreed with families (docs/engine.md paint contract):
// - Fill: an implicitly closed polygon (nonzero winding), solid ink or a
//   linear/radial gradient whose stop alphas are RELATIVE (final alpha =
//   stop.a x fill.a); pad beyond the ends; ink mirroring and colorMode apply
//   per stop exactly as to solid ink.
// - EffectRun: elements [start, start+count) of dots (0) / lines (1) /
//   polylines (2) are composited with Gaussian blur sigma (engine units)
//   and/or additive blend. Fills carry their own blur/blend.
// - Draw order: fills -> polylines -> lines -> dots; effects never reorder.
// Web specifics (docs/fx-view.md, *Fills and effects*): blur uses the
// canvas shadow (sigma = shadowBlur / 2 per the HTML spec, device pixels,
// so x scale) because Safari has no ctx.filter; additive = "lighter".

export interface FxGradientStop {
  offset: number;
  white: number;
  a: number;
  saturation: number;
  hue: number;
}

export interface FxFillGradient {
  /** 0 linear (x0,y0 -> x1,y1), 1 radial (centre x0,y0, radius r). */
  kind: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  r: number;
  stops: FxGradientStop[];
}

export interface FxFill {
  points: { x: number; y: number }[];
  /** Materials phase 2: inner rings, painted even-odd with `points` as one path (absent = none). */
  holes?: { x: number; y: number }[][];
  white: number;
  a: number;
  saturation: number;
  hue: number;
  gradient?: FxFillGradient | null;
  /** Gaussian sigma, engine units; 0 = none. */
  blur: number;
  /** 0 normal, 1 additive. */
  blend: number;
}

export interface FxEffectRun {
  /** 0 dots, 1 lines, 2 polylines. */
  target: number;
  start: number;
  count: number;
  blur: number;
  blend: number;
}

/** An `OrbFrame` with the phase-1 material lists (absent = none). */
export type PaintFrame = OrbFrame & { fills?: FxFill[]; effects?: FxEffectRun[] };

/** Far enough that the element itself never lands on the canvas; only its shadow does. */
const SHADOW_OFFSET = 10000;

function runsFor(effects: FxEffectRun[] | undefined, target: number, n: number): (FxEffectRun | undefined)[] | null {
  if (!effects || effects.length === 0) return null;
  let map: (FxEffectRun | undefined)[] | null = null;
  for (const e of effects) {
    if (e.target !== target || e.count <= 0) continue;
    if (!map) map = new Array(n);
    const end = Math.min(n, e.start + e.count);
    for (let i = Math.max(0, e.start); i < end; i++) map[i] = e;
  }
  return map;
}

/**
 * Draws `geom` (which builds a path and fills/strokes it) with an optional
 * Gaussian blur and additive blend. Blur: the canvas shadow of the element
 * drawn SHADOW_OFFSET away, so only the blurred copy lands; the shadow takes
 * the element's alpha, so `shadowColor` is the opaque ink. Nothing to do
 * when both are off (the caller doesn't even save/restore then).
 */
function withEffect(
  ctx: CanvasRenderingContext2D,
  scale: number,
  blur: number,
  blend: number,
  opaqueInk: string,
  geom: () => void
): void {
  if (!(blur > 0) && blend !== 1) {
    geom();
    return;
  }
  ctx.save();
  if (blend === 1) ctx.globalCompositeOperation = "lighter";
  if (blur > 0) {
    ctx.shadowColor = opaqueInk;
    ctx.shadowBlur = 2 * blur * scale;
    ctx.shadowOffsetX = SHADOW_OFFSET * scale;
    ctx.shadowOffsetY = 0;
    ctx.translate(-SHADOW_OFFSET, 0);
  }
  geom();
  ctx.restore();
}

function gradientStyle(ctx: CanvasRenderingContext2D, f: FxFill, g: FxFillGradient, mirror: boolean): CanvasGradient {
  const grad =
    g.kind === 1 ? ctx.createRadialGradient(g.x0, g.y0, 0, g.x0, g.y0, Math.max(0, g.r)) : ctx.createLinearGradient(g.x0, g.y0, g.x1, g.y1);
  for (const st of g.stops) {
    grad.addColorStop(Math.min(1, Math.max(0, st.offset)), ink(st.white, st.saturation, st.hue, st.a * f.a, mirror));
  }
  return grad;
}

function fillPath(ctx: CanvasRenderingContext2D, f: FxFill): void {
  ctx.beginPath();
  for (const ring of [f.points, ...(f.holes ?? [])]) {
    if (ring.length < 3) continue;
    ctx.moveTo(ring[0].x, ring[0].y);
    for (let i = 1; i < ring.length; i++) ctx.lineTo(ring[i].x, ring[i].y);
    ctx.closePath();
  }
}

/** Fill the current path: even-odd when the fill has holes (the contract), else the default rule. */
function fillRule(ctx: CanvasRenderingContext2D, f: FxFill): void {
  if (f.holes && f.holes.length) ctx.fill("evenodd");
  else ctx.fill();
}

let warnedGradientBlur = false;

function drawFill(ctx: CanvasRenderingContext2D, f: FxFill, mirror: boolean, scale: number): void {
  if (f.points.length < 3) return;
  const g = f.gradient && f.gradient.stops.length >= 2 ? f.gradient : null;
  if (!g) {
    withEffect(ctx, scale, f.blur, f.blend, ink(f.white, f.saturation, f.hue, 1, mirror), () => {
      ctx.fillStyle = ink(f.white, f.saturation, f.hue, f.a, mirror);
      fillPath(ctx, f);
      fillRule(ctx, f);
    });
    return;
  }
  // A gradient fill: a shadow has one colour, so a *blurred* gradient needs
  // ctx.filter (Chromium/Firefox). Where it's missing (Safari) the gradient
  // is drawn sharp -- documented fallback.
  const canFilter = f.blur > 0 && typeof (ctx as { filter?: unknown }).filter === "string";
  ctx.save();
  if (f.blend === 1) ctx.globalCompositeOperation = "lighter";
  if (canFilter) ctx.filter = `blur(${f.blur * scale}px)`;
  else if (f.blur > 0 && !warnedGradientBlur) {
    warnedGradientBlur = true;
    console.warn("Sinua: blurred gradient fills need ctx.filter (not in Safari); drawing them sharp.");
  }
  ctx.fillStyle = gradientStyle(ctx, f, g, mirror);
  fillPath(ctx, f);
  fillRule(ctx, f);
  ctx.restore();
}

function drawFrameWithMaterials(ctx: CanvasRenderingContext2D, frame: PaintFrame, dark: boolean, scale: number): void {
  const mirror = dark && frame.colorMode !== "fixed";
  ctx.save();
  ctx.scale(scale, scale);
  for (const f of frame.fills ?? []) drawFill(ctx, f, mirror, scale);

  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  const polyRuns = runsFor(frame.effects, 2, frame.polylines.length);
  frame.polylines.forEach((p, i) => {
    if (p.points.length < 2) return;
    const e = polyRuns?.[i];
    if (hasHues(p)) {
      strokePolylineHues(ctx, p, mirror, scale, e?.blur ?? 0, e?.blend ?? 0);
      return;
    }
    withEffect(ctx, scale, e?.blur ?? 0, e?.blend ?? 0, ink(p.white, p.saturation, p.hue, 1, mirror), () => {
      ctx.strokeStyle = ink(p.white, p.saturation, p.hue, p.a, mirror);
      ctx.lineWidth = p.w;
      ctx.beginPath();
      ctx.moveTo(p.points[0].x, p.points[0].y);
      for (let j = 1; j < p.points.length; j++) ctx.lineTo(p.points[j].x, p.points[j].y);
      ctx.stroke();
    });
  });
  ctx.lineCap = "butt";
  ctx.lineJoin = "miter";
  const lineRuns = runsFor(frame.effects, 1, frame.lines.length);
  frame.lines.forEach((l, i) => {
    const e = lineRuns?.[i];
    withEffect(ctx, scale, e?.blur ?? 0, e?.blend ?? 0, ink(l.white, l.saturation, l.hue, 1, mirror), () => {
      ctx.strokeStyle = ink(l.white, l.saturation, l.hue, l.a, mirror);
      ctx.lineWidth = l.w;
      ctx.beginPath();
      ctx.moveTo(l.x1, l.y1);
      ctx.lineTo(l.x2, l.y2);
      ctx.stroke();
    });
  });
  const dotRuns = runsFor(frame.effects, 0, frame.dots.length);
  frame.dots.forEach((d, i) => {
    const e = dotRuns?.[i];
    withEffect(ctx, scale, e?.blur ?? 0, e?.blend ?? 0, ink(d.white, d.saturation, d.hue, 1, mirror), () => {
      ctx.fillStyle = ink(d.white, d.saturation, d.hue, d.a, mirror);
      ctx.beginPath();
      ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
      ctx.fill();
    });
  });
  ctx.restore();
}

// ------------------------------------------- per-vertex stroke colour --
// Contract (docs/engine.md "Per-vertex stroke colour", agreed with
// families): with `hues`, every segment i-1 -> i is a round-capped stroke
// with a linear gradient between its vertices' inks at alpha 1 (a
// zero-length segment is a dot of diameter w in vertex i's colour); all
// segments go into ONE layer, composited once at `a` with the element's
// blur/blend -- so the round-cap overlaps between segments never
// double-paint (the seam `Polyline` exists to avoid). Canvas has no
// stroke-following gradient, hence the layer: a scratch canvas the size of
// the target, drawn only inside the stroke's bounding box.
// Blur at the composite needs `ctx.filter`; a shadow (this painter's usual
// blur, see `withEffect`) has a single colour. Where `filter` is missing
// (Safari: disabled by default through 27.x, caniuse) the layer is drawn
// sharp -- the same documented fallback as blurred gradient fills.

type PolylineLike = OrbFrame["polylines"][number] & { hues?: number[] };

function hasHues(p: PolylineLike): boolean {
  return !!p.hues && p.hues.length > 0 && p.hues.length === p.points.length;
}

type Ctx2D = CanvasRenderingContext2D;
let scratch: { canvas: { width: number; height: number }; ctx: Ctx2D } | null = null;
let warnedHueBlur = false;

/** A reused offscreen canvas at least `w` x `h`, or null where none can be made (no DOM, no OffscreenCanvas). */
function scratchCanvas(target: Ctx2D, w: number, h: number): { canvas: { width: number; height: number }; ctx: Ctx2D } | null {
  if (!scratch) {
    let canvas: OffscreenCanvas | HTMLCanvasElement | null = null;
    if (typeof OffscreenCanvas !== "undefined") {
      canvas = new OffscreenCanvas(w, h);
    } else {
      const doc = (target.canvas as HTMLCanvasElement | undefined)?.ownerDocument ?? (typeof document !== "undefined" ? document : null);
      canvas = doc ? doc.createElement("canvas") : null;
    }
    const c2 = canvas ? (canvas.getContext("2d") as Ctx2D | null) : null;
    if (!canvas || !c2) return null;
    scratch = { canvas, ctx: c2 };
  }
  if (scratch.canvas.width < w) scratch.canvas.width = w;
  if (scratch.canvas.height < h) scratch.canvas.height = h;
  return scratch;
}

function drawHueSegments(o: Ctx2D, p: PolylineLike, colors: string[]): void {
  o.lineCap = "round";
  o.lineWidth = p.w;
  const pts = p.points;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1];
    const b = pts[i];
    if (Math.hypot(b.x - a.x, b.y - a.y) < 1e-9) {
      o.fillStyle = colors[i];
      o.beginPath();
      o.arc(b.x, b.y, p.w / 2, 0, Math.PI * 2);
      o.fill();
      continue;
    }
    const g = o.createLinearGradient(a.x, a.y, b.x, b.y);
    g.addColorStop(0, colors[i - 1]);
    g.addColorStop(1, colors[i]);
    o.strokeStyle = g;
    o.beginPath();
    o.moveTo(a.x, a.y);
    o.lineTo(b.x, b.y);
    o.stroke();
  }
}

/** One per-vertex polyline: segments into the scratch layer, then one composite at `a` with blur/blend. */
function strokePolylineHues(ctx: Ctx2D, p: PolylineLike, mirror: boolean, scale: number, blur: number, blend: number): void {
  const colors = p.hues!.map((h) => ink(p.white, p.saturation, h, 1, mirror));
  const m = ctx.getTransform();
  // Device-pixel bounding box of the stroke (+ blur spread), clipped to the target.
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const q of p.points) {
    x0 = Math.min(x0, q.x); y0 = Math.min(y0, q.y);
    x1 = Math.max(x1, q.x); y1 = Math.max(y1, q.y);
  }
  const pad = p.w / 2;
  let dx0 = Infinity, dy0 = Infinity, dx1 = -Infinity, dy1 = -Infinity;
  for (const [ux, uy] of [[x0 - pad, y0 - pad], [x1 + pad, y0 - pad], [x0 - pad, y1 + pad], [x1 + pad, y1 + pad]]) {
    const dx = m.a * ux + m.c * uy + m.e;
    const dy = m.b * ux + m.d * uy + m.f;
    dx0 = Math.min(dx0, dx); dy0 = Math.min(dy0, dy);
    dx1 = Math.max(dx1, dx); dy1 = Math.max(dy1, dy);
  }
  const spread = 2 + (blur > 0 ? 3 * blur * scale : 0);
  const bx = Math.max(0, Math.floor(dx0 - spread));
  const by = Math.max(0, Math.floor(dy0 - spread));
  const bw = Math.min(ctx.canvas.width, Math.ceil(dx1 + spread)) - bx;
  const bh = Math.min(ctx.canvas.height, Math.ceil(dy1 + spread)) - by;
  if (bw <= 0 || bh <= 0) return;

  const s = scratchCanvas(ctx, ctx.canvas.width, ctx.canvas.height);
  if (!s) {
    // No offscreen canvas at all: segments straight onto the target (overlaps may show at a < 1).
    ctx.save();
    ctx.globalAlpha *= p.a;
    if (blend === 1) ctx.globalCompositeOperation = "lighter";
    drawHueSegments(ctx, p, colors);
    ctx.restore();
    return;
  }
  const o = s.ctx;
  o.setTransform(1, 0, 0, 1, 0, 0);
  o.clearRect(bx, by, bw, bh);
  o.setTransform(m);
  drawHueSegments(o, p, colors);

  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha *= p.a; // keeps an outer alpha (drawCrossDissolve)
  if (blend === 1) ctx.globalCompositeOperation = "lighter";
  if (blur > 0) {
    if (typeof (ctx as { filter?: unknown }).filter === "string") ctx.filter = `blur(${blur * scale}px)`;
    else if (!warnedHueBlur) {
      warnedHueBlur = true;
      console.warn("Sinua: blurred per-vertex strokes need ctx.filter (not in Safari); drawing them sharp.");
    }
  }
  ctx.drawImage(s.canvas as CanvasImageSource, bx, by, bw, bh, bx, by, bw, bh);
  ctx.restore();
}
