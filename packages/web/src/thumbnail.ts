// Thumbnails for a picker screen (FX Spec 1.13, design note 25): the engine draws a
// character with a loadout in a still pose (`frameStill`), and this paints it into an
// image with the views' own painter.

import { frameStill, type FxSpec, type Loadout, type OrbSize } from "@sinua/core";
import { drawFrame } from "./paint.js";

export interface ThumbnailOptions {
  /** The loadout to show (what the end user would wear). */
  loadout?: Loadout;
  /** CSS pixels across (the image is `size * scale` pixels). Default 128. */
  size?: number;
  /** Pixel density. Default 2. */
  scale?: number;
  /** Paint for a dark background. Default false. */
  dark?: boolean;
  /** The head turned this far, radians (0 = facing; about ±0.5 shows another angle). */
  turnYaw?: number;
}

/**
 * A character thumbnail as a PNG `Blob`: `spec` (with a `wardrobe`) wearing `loadout`,
 * still (no blink, no glance), transparent around it. `null` if the spec doesn't resolve.
 */
export async function characterThumbnail(spec: FxSpec | string | object, opts: ThumbnailOptions = {}): Promise<Blob | null> {
  const size = opts.size ?? 128;
  const scale = opts.scale ?? 2;
  const frame = frameStill(spec as FxSpec | string, size as OrbSize, { loadout: opts.loadout, turnYaw: opts.turnYaw });
  if (!frame) return null;
  const px = Math.round(size * scale);
  if (typeof OffscreenCanvas !== "undefined") {
    const canvas = new OffscreenCanvas(px, px);
    drawFrame(canvas.getContext("2d") as unknown as CanvasRenderingContext2D, frame, !!opts.dark, scale);
    return canvas.convertToBlob({ type: "image/png" });
  }
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = px;
  drawFrame(canvas.getContext("2d")!, frame, !!opts.dark, scale);
  return new Promise((done) => canvas.toBlob((b) => done(b), "image/png"));
}
