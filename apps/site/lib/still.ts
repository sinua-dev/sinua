/**
 * One still frame of a pattern, painted exactly the way `mount` paints a live
 * one (square engine space, centred, DPR capped at 2), for grids where a live
 * view per tile would cost a render loop each. The instant is the one with the
 * most visible ink among a few samples: some patterns are periodic with quiet
 * phases (confirming's ping fades out between pulses), so a fixed t can leave a
 * tile nearly blank. The Studio's state grid picks its thumbnails the same way.
 */
import { frameWithOverrides, type OrbFrame, type OrbSize, type OrbState } from "@sinua/core";
import { DPR_CAP, drawFrame } from "@sinua/web";

const SAMPLE_T = [0.45, 1.1, 1.7, 2.6, 3.4];

/** How much ink a frame puts on the canvas: alpha-weighted dot area plus line and polyline presence. */
function inkScore(f: OrbFrame): number {
  let s = 0;
  for (const d of f.dots) s += d.a * d.r * d.r;
  for (const l of f.lines) s += l.a * l.w;
  for (const p of f.polylines) s += p.a * p.w * p.points.length * 0.25;
  return s;
}

/** The fullest of a few instants, for one pattern at one size with these overrides. */
export function stillFrame(pattern: string, size: OrbSize, overrides: Record<string, number>): OrbFrame | null {
  let best: OrbFrame | null = null;
  let bestScore = -1;
  for (const t of SAMPLE_T) {
    const f = frameWithOverrides(pattern as OrbState, size, t, overrides);
    if (!f) continue;
    const s = inkScore(f);
    if (s > bestScore) {
      best = f;
      bestScore = s;
    }
  }
  return best;
}

/** The box ratio `mount` gives a box-layout pattern (its `aspect` input's range). */
export const boxAspect = (w: number, h: number) => Math.min(8, Math.max(0.125, w / Math.max(1, h)));

/**
 * Paints `frame` into `canvas` at its CSS size, as mount would: centred in a square,
 * or, for a box-layout pattern (`fill`), `size * aspect` by `size` over the whole box.
 */
export function paintStill(canvas: HTMLCanvasElement, frame: OrbFrame, size: OrbSize, dark: boolean, fill = false): void {
  const box = canvas.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return;
  const dpr = Math.min(window.devicePixelRatio || 1, DPR_CAP);
  canvas.width = Math.max(1, Math.round(box.width * dpr));
  canvas.height = Math.max(1, Math.round(box.height * dpr));
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  const side = fill ? canvas.height : Math.min(canvas.width, canvas.height);
  ctx.save();
  if (!fill) ctx.translate((canvas.width - side) / 2, (canvas.height - side) / 2);
  drawFrame(ctx, frame, dark, side / size);
  ctx.restore();
}
