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
 * then dots; plain source-over; ink mirrors on dark themes --
 * grey = round((dark ? 1 - white : white) * 255). Shared by the live
 * `OrbCanvas` and the PNG exporter (`export/image.ts`) so they can never
 * drift from each other. `scale` is applied via `ctx.scale`, not baked into
 * the coordinates, so the same call draws crisp at any output resolution.
 */
export function drawFrame(ctx: CanvasRenderingContext2D, frame: OrbFrame, dark: boolean, scale: number): void {
  // Effect runs (blur, additive, light): their own path, so every frame
  // without them keeps the exact calls below (byte-identical output).
  const fx = frame as PaintFrame;
  if (fx.effects && fx.effects.length) {
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
  // Packed v2 (effect runs): the object path. Plain v1 frames -- every
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

// ----------------------------------------------------- effect runs --
// Contract (docs/engine.md paint contract):
// - EffectRun: elements [start, start+count) of dots (0) / lines (1) are
//   composited with Gaussian blur sigma (engine units) and/or additive
//   blend. `blend` 3 on dots is light: a soft radial glow per dot, added up
//   (see `drawLight`).
// - Draw order: polylines -> lines -> dots; effects never reorder.
// Web specifics (docs/fx-view.md): blur uses the canvas shadow
// (sigma = shadowBlur / 2 per the HTML spec, device pixels, so x scale)
// because Safari has no ctx.filter; additive = "lighter".

export interface FxEffectRun {
  /** 0 dots, 1 lines. */
  target: number;
  start: number;
  count: number;
  blur: number;
  blend: number;
}

/** An `OrbFrame` with its effect runs (absent = none). */
export type PaintFrame = OrbFrame & { effects?: FxEffectRun[] };

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

function drawFrameWithMaterials(ctx: CanvasRenderingContext2D, frame: PaintFrame, dark: boolean, scale: number): void {
  const mirror = dark && frame.colorMode !== "fixed";
  ctx.save();
  ctx.scale(scale, scale);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const p of frame.polylines) {
    if (p.points.length < 2) continue;
    ctx.strokeStyle = ink(p.white, p.saturation, p.hue, p.a, mirror);
    ctx.lineWidth = p.w;
    ctx.beginPath();
    ctx.moveTo(p.points[0].x, p.points[0].y);
    for (let j = 1; j < p.points.length; j++) ctx.lineTo(p.points[j].x, p.points[j].y);
    ctx.stroke();
  }
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
    if (e?.blend === LIGHT) {
      drawLight(ctx, d, dark, mirror);
      return;
    }
    withEffect(ctx, scale, e?.blur ?? 0, e?.blend ?? 0, ink(d.white, d.saturation, d.hue, 1, mirror), () => {
      ctx.fillStyle = ink(d.white, d.saturation, d.hue, d.a, mirror);
      ctx.beginPath();
      ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
      ctx.fill();
    });
  });
  ctx.restore();
}

// ---------------------------------------------------------------- light --
// `blend` 3 on dots (light volumes, docs/engine.md *Light*): each dot is a flat, hard-edged disc
// -- a plain `arc`, exactly like an orb's dots, so it is as sharp at 900 px as at 140 -- of
// `LIGHT_CORE` of the dot's radius `r` (`r` is sized for glow, so the disc is a fraction of it).
// On a dark background the discs add up ("lighter"), so overlapping dots still read as light. A
// light background can't take added light (it is already white), so there the disc, no brighter
// than the colour, is laid on normally. The same fraction and colours on every platform.

const LIGHT = 3;
const LIGHT_CORE = 0.5;

function drawLight(ctx: CanvasRenderingContext2D, d: OrbFrame["dots"][number], dark: boolean, mirror: boolean): void {
  const lightness = mirror ? 1 - d.white : d.white;
  const l = Math.round(lightness * 100);
  const tone = dark ? l : Math.max(10, l - 22);
  const prevOp = ctx.globalCompositeOperation, prevA = ctx.globalAlpha;
  ctx.globalCompositeOperation = dark ? "lighter" : "source-over";
  ctx.globalAlpha = Math.min(1, Math.max(0, d.a));
  ctx.fillStyle = `hsl(${Math.round(d.hue)}, ${Math.round(d.saturation * 100)}%, ${tone}%)`;
  ctx.beginPath();
  ctx.arc(d.x, d.y, d.r * LIGHT_CORE, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = prevOp;
  ctx.globalAlpha = prevA;
}
