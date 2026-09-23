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

/**
 * On paper the engine's ink is a mid-tone, and the brand violet at its profile
 * lightness reads washed out; a darker colour holds the same hue against a
 * light background. Dark backgrounds need nothing.
 */
const contrast = (theme: "light" | "dark"): Record<string, number> => (theme === "light" ? { colorLightness: -0.22 } : {});

export function LiveVisual({
  pattern,
  size = 64,
  state,
  level = 0.5,
  overrides,
  maxFps,
  className,
  label,
}: {
  pattern: string;
  size?: 20 | 32 | 64;
  /** With a state, families' profile is applied; without one, the pattern's own defaults. */
  state?: VoiceState;
  level?: number;
  /** Engine keys on top of the pattern's defaults, e.g. a ring's progress. */
  overrides?: Record<string, number>;
  maxFps?: number;
  className?: string;
  label: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const theme = useSiteTheme();
  const visual = useMemo(() => (state ? voiceStateVisual(pattern, state, level) : null), [pattern, state, level]);
  const opts = useMemo(
    () => ({
      pattern,
      size,
      ...(visual ? { speed: visual.speed } : {}),
      overrides: { ...(visual?.overrides ?? {}), ...overrides, ...contrast(theme) },
      theme,
      maxFps,
    }),
    [pattern, size, visual, overrides, theme, maxFps]
  );

  useEffect(() => {
    let live = true;
    import("@sinua/web")
      .then(({ mount }) => {
        if (!live || !canvas.current) return;
        handle.current = mount(canvas.current, opts);
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    handle.current?.update(opts);
  }, [opts]);

  return <canvas ref={canvas} className={className} aria-label={label} />;
}
