"use client";
/**
 * Every pattern, grouped by family the way the Studio's top bar groups them.
 * The list, the labels and the one-line descriptions all come from the engine's
 * parameter catalog, so a new pattern appears here without anyone listing it.
 * Each family wears one hue, set through the same colour keys an app would use.
 */
import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { StillVisual } from "../still-visual";
import Link from "next/link";
import { parameterCatalog } from "@/lib/catalog";
import { LiveVisual } from "./live-visual";
import { TINT } from "./swatches";

/** One hue per family, so switching families reads at a glance. */
const FAMILY_HUE: Record<string, number> = { orb: 265, signal: 172, ring: 38, beacon: 345, core: 200, character: 232 };
const tint = (family: string) => ({ ...TINT, colorHue: FAMILY_HUE[family] ?? 265 });

/** Values that make a pattern read at a glance: a ring needs progress, a meter needs a level. */
const SHOWCASE: Record<string, Record<string, number>> = {
  completing: { progress: 0.68 },
  tracking: { progress0: 0.72, progress1: 0.48, progress2: 0.3 },
  stepping: { progress: 0.6 },
  measuring: { progress: 0.62 },
  progressing: { progress: 0.55 },
  reconnecting: { quality: 0.5 },
  signaling: { audioLevel: 0.7 },
  waveform: { audioLevel: 0.7 },
  scrolling: { audioLevel: 0.6 },
  metering: { audioLevel: 0.75 },
  speaking: { audioLevel: 0.6, audioStrength: 0.45 },
};

/** Patterns whose point is a value moving: their live previews sweep it instead of holding one. */
const SWEEPS: Record<string, string> = { progressing: "progress" };

/**
 * A value that sweeps 0 → 1 over `seconds`, holds a moment at 1 and starts again,
 * while `on` and `where` is on screen; null when off. 30 updates a second: the engine
 * draws at its own rate, the page only feeds it the value.
 */
function useSweep(on: boolean, where: RefObject<Element | null>, seconds = 5): number | null {
  const [value, setValue] = useState(0);
  useEffect(() => {
    if (!on) return;
    let raf = 0;
    let last = 0;
    let visible = true;
    const t0 = performance.now();
    const io =
      where.current && typeof IntersectionObserver !== "undefined"
        ? new IntersectionObserver((e) => (visible = e[e.length - 1]?.isIntersecting ?? true))
        : null;
    if (io && where.current) io.observe(where.current);
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick);
      if (!visible || now - last < 33) return;
      last = now;
      const s = ((now - t0) / 1000) % (seconds + 0.8);
      setValue(Math.min(1, s / seconds));
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      io?.disconnect();
    };
  }, [on, where, seconds]);
  return on ? value : null;
}

/** The showcase values, with a sweeping value in place of the fixed one while it runs. */
function showcase(pattern: string, sweep: number | null): Record<string, number> {
  const key = SWEEPS[pattern];
  return key && sweep != null ? { ...SHOWCASE[pattern], [key]: sweep } : (SHOWCASE[pattern] ?? {});
}

export function Families() {
  const objects = useMemo(() => parameterCatalog().objects, []);
  const [family, setFamily] = useState(objects[0].id as string);
  const current = objects.find((o) => o.id === family) ?? objects[0];
  const [picked, setPicked] = useState<Record<string, string>>({});
  const patternId = picked[current.id] ?? current.patterns[0].id;
  const pattern = current.patterns.find((p) => p.id === patternId) ?? current.patterns[0];
  const total = objects.reduce((n, o) => n + o.patterns.length, 0);
  const detail = useRef<HTMLElement>(null);
  const sweep = useSweep(pattern.id in SWEEPS, detail);

  return (
    <section className="lp-section" aria-labelledby="lp-families">
      <header className="lp-section-head">
        <h2 id="lp-families" className="lp-h2">
          {total} patterns in {objects.length} families
        </h2>
        <p className="lp-lead">
          Orbs for an agent's inner life, signals for sound, rings for progress, beacons for attention, cores for text that is on its way and characters that give your assistant a face. Point at one to
          set it moving; every frame below is drawn by the engine.
        </p>
      </header>

      <nav className="seg lp-family-seg" aria-label="Family">
        {objects.map((o) => (
          <button key={o.id} type="button" className="seg-btn" aria-pressed={o.id === current.id} onClick={() => setFamily(o.id)}>
            {o.label} <span className="lp-count">{o.patterns.length}</span>
          </button>
        ))}
      </nav>

      <div className="lp-bench">
        <div className="tiles lp-tiles" style={{ ["--cols" as string]: current.patterns.length > 6 ? 6 : current.patterns.length }}>
          {current.patterns.map((p) => (
            <FamilyTile
              key={p.id}
              pattern={p.id}
              label={p.label}
              overrides={{ ...(p.id === pattern.id ? showcase(p.id, sweep) : SHOWCASE[p.id]), ...tint(current.id) }}
              selected={p.id === pattern.id}
              onSelect={() => setPicked({ ...picked, [current.id]: p.id })}
            />
          ))}
        </div>

        <aside ref={detail} className="lp-stage lp-detail" aria-live="polite">
          <LiveVisual key={pattern.id} pattern={pattern.id} overrides={{ ...showcase(pattern.id, sweep), ...tint(current.id) }} className="lp-detail-visual" label={pattern.label} />
          <div className="lp-detail-text">
            <h3 className="lp-h3">
              {pattern.label} <span className="lp-muted">{current.label}</span>
            </h3>
            <p>{pattern.description}</p>
            <Link className="lp-link" href={`/gallery/?family=${current.id}&pattern=${pattern.id}`}>
              Tune it in the gallery
            </Link>
          </div>
        </aside>
      </div>
    </section>
  );
}

/**
 * A tile is a still frame until someone points at it, focuses it or picks it;
 * only then does it run. Eighteen live tiles would each run a render loop to
 * tell a visitor nothing the still frame doesn't.
 */
function FamilyTile({
  pattern,
  label,
  overrides,
  selected,
  onSelect,
}: {
  pattern: string;
  label: string;
  overrides: Record<string, number>;
  selected: boolean;
  onSelect: () => void;
}) {
  const [hot, setHot] = useState(false);
  const live = hot || selected;
  return (
    <button
      type="button"
      className="tile"
      aria-pressed={selected}
      onClick={onSelect}
      onPointerEnter={() => setHot(true)}
      onPointerLeave={() => setHot(false)}
      onFocus={() => setHot(true)}
      onBlur={() => setHot(false)}
    >
      <span className="tile-thumb">
        {live ? (
          <LiveVisual pattern={pattern} size={64} maxFps={30} overrides={overrides} className="lp-tile-canvas" label={label} />
        ) : (
          <StillVisual pattern={pattern} size={64} overrides={overrides} className="lp-tile-canvas" label={label} />
        )}
      </span>
      <span className="tile-label">{label}</span>
    </button>
  );
}
