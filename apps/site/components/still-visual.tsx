"use client";
/**
 * A pattern as one still frame: the engine's real output, painted once (and again
 * on a resize or a theme change), with no render loop. Grids use it for every
 * tile that isn't hovered or selected; see lib/still.ts for which instant.
 */
import { useEffect, useMemo, useRef } from "react";
import type { OrbSize } from "@sinua/core";
import { boxAspect, paintStill, stillFrame } from "@/lib/still";
import { previewInputs, shapeOf } from "@/lib/layout";
import { useSiteTheme } from "@/lib/use-site-theme";
import { contrast } from "./landing/live-visual";

export function StillVisual({
  pattern,
  size = 64,
  overrides,
  className,
  label,
  themeContrast = true,
}: {
  pattern: string;
  size?: OrbSize;
  overrides?: Record<string, number>;
  className?: string;
  label: string;
  /** The landing's light-theme contrast correction (LiveVisual's); off to match a view drawn without it. */
  themeContrast?: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const theme = useSiteTheme();
  // Callers spread a fresh object each render; key the frame on its contents.
  const key = JSON.stringify(overrides ?? {});
  const shape = shapeOf(pattern);
  const base = useMemo(
    () => ({ ...previewInputs(pattern), ...(JSON.parse(key) as Record<string, number>), ...(themeContrast ? contrast(theme) : {}) }),
    [pattern, key, theme, themeContrast]
  );
  // A square pattern's frame doesn't depend on the box; a box-layout one is drawn for its box.
  const frame = useMemo(() => (shape === "square" ? stillFrame(pattern, size, base) : null), [shape, pattern, size, base]);

  useEffect(() => {
    const el = canvas.current;
    if (!el) return;
    const paint = () => {
      if (shape === "square") {
        if (frame) paintStill(el, frame, size, theme === "dark");
        return;
      }
      const r = el.getBoundingClientRect();
      const f = stillFrame(pattern, size, { ...base, aspect: boxAspect(r.width, r.height) });
      if (f) paintStill(el, f, size, theme === "dark", true);
    };
    paint();
    const ro = new ResizeObserver(paint);
    ro.observe(el);
    return () => ro.disconnect();
  }, [frame, size, theme, shape, pattern, base]);

  return <canvas ref={canvas} className={className} data-shape={shape} aria-label={label} />;
}
