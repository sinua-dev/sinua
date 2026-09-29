"use client";
/**
 * One design, four platforms. The four frames draw the same pattern; the code
 * under them is the Studio's own exporter (@sinua/snippets) for each platform,
 * and picking a platform's code lights up its frame.
 *
 * The frames on this page are drawn by the web build. The claim they stand for
 * is made by tests, not by this page: `frames` is the size of the frozen set the
 * web, iOS and Android builds each check (spec/sinua-golden.json, counted at
 * build time).
 *
 * One engine runs, in the first frame; every frame it draws is copied into the
 * other three (`onFrame` fires after each paint). Four views would compute the
 * same frame four times and drift apart by their start times.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { Snippet } from "@sinua/design";
import type { FxHandle } from "@sinua/web";
import { designTabs, type TabId } from "@/lib/design-tabs";
import { useSiteTheme } from "@/lib/use-site-theme";
import { contrast } from "./live-visual";
import { TINT } from "./swatches";
import { PlatformIcon, type Platform } from "./platform-icons";

const PATTERN = { object: "orb", id: "connecting" };
const LOOK = { ...TINT, colorHue: 172 };

const PLATFORMS: { id: Platform; label: string; device: string; tabs: TabId[] }[] = [
  { id: "web", label: "Web", device: "browser", tabs: ["typed-react", "web"] },
  { id: "ios", label: "iOS", device: "phone-ios", tabs: ["swiftui"] },
  { id: "android", label: "Android", device: "phone-android", tabs: ["compose"] },
  { id: "rn", label: "React Native", device: "phone-rn", tabs: ["typed-rn"] },
];

export function Platforms({ frames }: { frames: number }) {
  const tabs = useMemo(() => designTabs(PATTERN.object, PATTERN.id, LOOK, PLATFORMS.flatMap((p) => p.tabs)), []);
  const [tab, setTab] = useState(tabs[0].id);
  const active = PLATFORMS.find((p) => (p.tabs as string[]).includes(tab))?.id;
  const canvases = useRef<(HTMLCanvasElement | null)[]>([]);
  useMirroredVisual(canvases);

  return (
    <section className="lp-section" aria-labelledby="lp-platforms">
      <header className="lp-section-head">
        <h2 id="lp-platforms" className="lp-h2">
          The same frame on every platform
        </h2>
        <p className="lp-lead">
          One Rust engine is compiled into each platform, so a design is never ported four times. The web, iOS and Android builds each check the same{" "}
          {frames} frozen frames in CI; React Native runs the iOS and Android binaries as they are.
        </p>
      </header>

      <div className="lp-devices">
        {PLATFORMS.map((p, i) => (
          <button key={p.id} type="button" className="lp-device" data-device={p.device} aria-pressed={active === p.id} onClick={() => setTab(p.tabs[0])}>
            <span className="lp-device-screen">
              <canvas
                ref={(el) => {
                  canvases.current[i] = el;
                }}
                className="lp-device-visual"
                aria-label={`${PATTERN.id} on ${p.label}`}
              />
            </span>
            <span className="lp-device-label">
              <PlatformIcon platform={p.id} />
              {p.label}
            </span>
          </button>
        ))}
      </div>
      <p className="lp-footnote">
        All four are drawn here by the web build.{" "}
        <a className="lp-link lp-link-small" href="https://github.com/sinua-dev/sinua/blob/main/docs/testing.md">
          How parity is tested
        </a>
      </p>

      <Snippet tabs={tabs} value={tab} onChange={setTab} />
    </section>
  );
}

/** Mounts the engine on the first canvas and copies each drawn frame into the rest. */
function useMirroredVisual(canvases: { current: (HTMLCanvasElement | null)[] }) {
  const theme = useSiteTheme();
  const handle = useRef<FxHandle | null>(null);
  const options = useMemo(() => ({ pattern: PATTERN.id, overrides: { ...LOOK, ...contrast(theme) }, theme, maxFps: 30 }), [theme]);
  const latest = useRef(options);
  latest.current = options;

  useEffect(() => {
    let live = true;
    const [source, ...mirrors] = canvases.current;
    if (!source) return;
    const dpr = () => Math.min(window.devicePixelRatio || 1, 2);
    // The engine's square is centred in whatever box a canvas has: copy square to square.
    const copy = () => {
      const side = Math.min(source.width, source.height);
      if (!side) return;
      const sx = (source.width - side) / 2;
      const sy = (source.height - side) / 2;
      for (const m of mirrors) {
        if (!m) continue;
        const w = Math.round(m.clientWidth * dpr());
        const h = Math.round(m.clientHeight * dpr());
        if (m.width !== w) m.width = w;
        if (m.height !== h) m.height = h;
        const ctx = m.getContext("2d");
        if (!ctx) continue;
        const t = Math.min(w, h);
        ctx.clearRect(0, 0, w, h);
        ctx.drawImage(source, sx, sy, side, side, (w - t) / 2, (h - t) / 2, t, t);
      }
    };
    import("@sinua/web")
      .then(({ mount }) => {
        if (!live) return;
        handle.current = mount(source, { ...latest.current, onFrame: copy });
      })
      .catch((err: unknown) => console.error("Sinua: the engine chunk failed to load; the %s will stay blank.", "platform frames", err));
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
  }, [canvases]);

  useEffect(() => {
    handle.current?.update(options);
  }, [options]);
}
