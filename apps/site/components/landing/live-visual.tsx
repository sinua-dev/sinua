"use client";
/**
 * The landing page's live visuals: the same `mount` path the docs `<Demo>` and
 * the gallery use, so what the page shows is what the engine draws. The engine
 * module is imported dynamically, and `mount` pauses it off screen.
 */
import { useEffect, useMemo, useRef } from "react";
import type { FxHandle } from "@sinua/web";
import { useSiteTheme } from "@/lib/use-site-theme";
import { voiceStateVisual, type VoiceState } from "@/lib/voice-state";
import { previewInputs, shapeOf } from "@/lib/layout";

/**
 * On paper the engine's ink is a mid-tone, and the brand violet at its profile
 * lightness reads washed out; a darker colour holds the same hue against a
 * light background. Dark backgrounds need nothing.
 */
export const contrast = (theme: "light" | "dark"): Record<string, number> => (theme === "light" ? { colorLightness: -0.22 } : {});

export function LiveVisual({
  pattern,
  size = 64,
  state,
  level = 0.5,
  bands,
  overrides,
  maxFps,
  pointer,
  className,
  label,
}: {
  pattern: string;
  size?: 20 | 32 | 64;
  /** With a state, families' profile is applied; without one, the pattern's own defaults. */
  state?: VoiceState;
  level?: number;
  /** A moving spectrum for the audio-driven states (the simulated conversation's); else one is made from `level`. */
  bands?: readonly number[];
  /** Engine keys on top of the pattern's defaults, e.g. a ring's progress. */
  overrides?: Record<string, number>;
  maxFps?: number;
  /** Pointer and touch scatter (mount's `pointer`). */
  pointer?: boolean;
  className?: string;
  label: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const theme = useSiteTheme();
  const visual = useMemo(() => (state ? voiceStateVisual(pattern, state, level, 0, bands) : null), [pattern, state, level, bands]);
  const opts = useMemo(
    () => ({
      pattern,
      size,
      ...(visual ? { speed: visual.speed } : {}),
      overrides: { ...previewInputs(pattern), ...(visual?.overrides ?? {}), ...overrides, ...contrast(theme) },
      theme,
      maxFps,
      pointer,
    }),
    [pattern, size, visual, overrides, theme, maxFps, pointer]
  );

  // The engine chunk loads asynchronously; an update that lands before it (the site
  // theme resolving on the first effect, say) has no handle to reach. So mount reads
  // the latest options, not the ones the first render closed over.
  const latest = useRef(opts);
  latest.current = opts;

  useEffect(() => {
    let live = true;
    import("@sinua/web")
      .then(({ mount }) => {
        if (!live || !canvas.current) return;
        handle.current = mount(canvas.current, latest.current);
      })
      // Without this a failed chunk load left an empty canvas and said
      // nothing -- the same silent-failure shape as the Studio's S9.
      .catch((err: unknown) => {
        console.error("Sinua: the engine chunk failed to load; the %s will stay blank.", "landing visual", err);
      });
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
    // Mounted once; every change below goes through update().
  }, []);

  useEffect(() => {
    handle.current?.update(opts);
  }, [opts]);

  return <canvas ref={canvas} className={className} data-shape={shapeOf(pattern)} aria-label={label} />;
}
