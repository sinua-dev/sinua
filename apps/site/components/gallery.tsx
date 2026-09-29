"use client";
/**
 * The pattern gallery, laid out like the Studio's workbench: a bar of pickers,
 * the pattern tiles, and an inspector with a large preview, the basic
 * parameters and the code. It views and copies; it doesn't save anything.
 *
 * The visuals come from `mount` (@sinua/web), the parameters and descriptions
 * from `parameterCatalog()` (spec/parameters.json), the states from families'
 * profile (lib/voice-state), the code from @sinua/snippets (the Studio's own
 * exporter) and the controls from @sinua/design (the Studio's own controls).
 *
 * No audio device is ever opened: levels are simulated.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { parameterCatalog, type OrbSize } from "@sinua/core";
import { buildSnippets, buildTypedSnippets, toTypedProps } from "@sinua/snippets";
import type { FxHandle } from "@sinua/web";
import { Icon, PgSlider, PgTabs, Snippet } from "@sinua/design";
import { useSiteTheme } from "@/lib/use-site-theme";
import { PointerToggle } from "./landing/look";
import { StillVisual } from "./still-visual";
import { previewInputs, shapeOf } from "@/lib/layout";
import { CopyPrompt, CopySpec } from "./copy-prompt";
import { specFile, specText } from "@/lib/spec-file";
import { PROFILE_SOURCE, SIMULATED_NOTE, stateCaveat, voiceStateVisual, VOICE_STATES, type VoiceState } from "@/lib/voice-state";

type Catalog = ReturnType<typeof parameterCatalog>;
type Params = Record<string, number>;
type Mode = "off" | VoiceState | "cycle";

const SIZES: OrbSize[] = [20, 32, 64];

/** Where each platform tab sends you to get it installed. */
const PLATFORM_DOCS: Record<string, string> = {
  react: "/docs/platforms/react/",
  web: "/docs/platforms/vanilla/",
  swiftui: "/docs/platforms/swiftui/",
  compose: "/docs/platforms/compose/",
  rn: "/docs/platforms/react-native/",
};

/** A pattern's `basic` parameters, from the catalog's definitions (`param@scope`). */
function basicParams(catalog: Catalog, objectId: string, patternId: string) {
  const object = catalog.objects.find((o) => o.id === objectId);
  const pattern = object?.patterns.find((p) => p.id === patternId);
  const defs = catalog.definitions as unknown as Record<string, {
    key: string; label: string; description: string; min: number; max: number; step: number;
    tier: string; type: string; choices: string[] | null; category: string;
  }>;
  return (pattern?.params ?? [])
    .map((p) => {
      const def = defs[(p as { ref: string }).ref];
      const perSize = (p as { default?: Record<string, number> }).default;
      return def && def.tier === "basic" && def.type === "number" && !def.choices
        ? { ...def, ref: (p as { ref: string }).ref, defaults: perSize ?? {} }
        : null;
    })
    .filter((p): p is NonNullable<typeof p> => p !== null);
}

interface FxProps {
  pattern: string;
  size: OrbSize;
  state: VoiceState;
  level: number;
  /** Off: the bare pattern at its catalog defaults, exactly what the Studio draws. */
  states: boolean;
  overrides?: Params;
  theme: "light" | "dark";
  paused: boolean;
  maxFps?: number;
  pointer?: boolean;
}

/** One live visual. Mounts once, then every change goes through `update`. */
function useFx({ pattern, size, state, level, states, overrides, theme, paused, maxFps, pointer }: FxProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const visual = useMemo(() => (states ? voiceStateVisual(pattern, state, level) : null), [states, pattern, state, level]);
  const opts = useMemo(
    () => ({
      pattern,
      size,
      ...(visual ? { speed: visual.speed } : {}),
      overrides: { ...previewInputs(pattern), ...(visual?.overrides ?? {}), ...overrides },
      theme,
      paused,
      maxFps,
      pointer,
    }),
    [pattern, size, visual, overrides, theme, paused, maxFps, pointer]
  );

  // Mount reads the latest options: an update that lands while the engine chunk is
  // still loading (the site theme resolving, say) has no handle to reach.
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
        console.error("Sinua: the engine chunk failed to load; the %s will stay blank.", "gallery visual", err);
      });
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
    // Mount once; `opts` changes are applied below.
  }, []);

  useEffect(() => {
    handle.current?.update(opts);
  }, [opts]);

  return canvas;
}

/** The live thumbnail: only mounted while its tile is hovered, focused or selected. */
function LiveThumb({ label, ...fx }: FxProps & { label: string }) {
  const canvas = useFx(fx);
  return <canvas ref={canvas} className="gl-tile-canvas" data-shape={shapeOf(fx.pattern)} aria-label={`${label} preview`} />;
}

/**
 * A pattern tile in the grid: the Studio's `.tile`. It shows the engine's still frame
 * (in the chosen voice state, when one is on) and runs live only while someone points
 * at it, focuses it or has it selected: 34 render loops would say nothing the stills don't.
 */
function Tile({ label, selected, onSelect, ...fx }: FxProps & { label: string; selected: boolean; onSelect: () => void }) {
  const [hot, setHot] = useState(false);
  const still = useMemo(
    () => (fx.states ? voiceStateVisual(fx.pattern, fx.state, fx.level).overrides : {}),
    [fx.states, fx.pattern, fx.state, fx.level]
  );
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      data-pattern={fx.pattern}
      className="tile fx-tile"
      onPointerEnter={() => setHot(true)}
      onPointerLeave={() => setHot(false)}
      onFocus={() => setHot(true)}
      onBlur={() => setHot(false)}
    >
      <span className="tile-thumb">
        {hot || selected ? (
          <LiveThumb label={label} {...fx} />
        ) : (
          <StillVisual pattern={fx.pattern} size={fx.size} overrides={still} className="gl-tile-canvas" label={`${label} preview`} themeContrast={false} />
        )}
      </span>
      <span className="tile-label">{label}</span>
    </button>
  );
}

/** The inspector's large preview. */
function Preview(fx: FxProps & { label: string }) {
  const canvas = useFx(fx);
  return <canvas ref={canvas} className="gl-preview" data-shape={shapeOf(fx.pattern)} aria-label={`${fx.label}, large preview`} />;
}

export function Gallery() {
  const catalog = useMemo(() => parameterCatalog(), []);
  const all = useMemo(
    () =>
      catalog.objects.flatMap((o) =>
        o.patterns.map((p) => ({ object: o.id, objectLabel: o.label, component: o.component, pattern: p.id, label: p.label, description: p.description }))
      ),
    [catalog]
  );
  const [family, setFamily] = useState<string>("all");
  const [size, setSize] = useState<OrbSize>(64);
  // Off by default: the grid shows each pattern plain, as the Studio draws it.
  const [mode, setMode] = useState<Mode>("off");
  const [cycled, setCycled] = useState<VoiceState>("idle");
  const [paused, setPaused] = useState(false);
  const [pointer, setPointer] = useState(true);
  const theme = useSiteTheme();
  const [selected, setSelected] = useState<string>("glowing");
  // A docs page links here filtered: /gallery/?family=ring (&pattern=tracking).
  useEffect(() => {
    const q = new URLSearchParams(window.location.search);
    const f = q.get("family");
    const p = q.get("pattern");
    if (f && (f === "all" || catalog.objects.some((o) => o.id === f))) {
      setFamily(f);
      // Arriving filtered, the panel should show that family, not the default pattern.
      const first = all.find((x) => x.object === f);
      if (first && !p) setSelected(first.pattern);
    }
    if (p && all.some((x) => x.pattern === p)) setSelected(p);
  }, [catalog, all]);
  const [params, setParams] = useState<Params>({});
  // null = whatever the first tab is (the typed component when it can express the design).
  const [platform, setPlatform] = useState<string | null>(null);

  // A simulated level: no microphone is ever opened here.
  const [level, setLevel] = useState(0.45);
  useEffect(() => {
    if (mode !== "cycle") return;
    const id = setInterval(() => setCycled((s) => VOICE_STATES[(VOICE_STATES.indexOf(s) + 1) % VOICE_STATES.length]), 3000);
    return () => clearInterval(id);
  }, [mode]);
  const states = mode !== "off";
  const state: VoiceState = mode === "cycle" ? cycled : mode === "off" ? "listening" : mode;

  const shown = family === "all" ? all : all.filter((p) => p.object === family);
  const current = all.find((p) => p.pattern === selected) ?? all[0];
  const caveat = stateCaveat(current.pattern);
  const controls = useMemo(() => basicParams(catalog, current.object, current.pattern), [catalog, current]);
  useEffect(() => setParams({}), [selected]);

  const visual = states ? voiceStateVisual(current.pattern, state, level) : null;
  // The typed component is the primary form; the view form stays as "advanced"
  // and is the only one that can carry an engine key the catalog has no prop for.
  const typed = useMemo(() => {
    const design = toTypedProps(current.object, current.pattern, { ...(visual?.overrides ?? {}), ...params }, catalog);
    return Object.keys(design.leftover).length ? null : buildTypedSnippets({ design, size, speed: visual?.speed ?? 1 });
  }, [catalog, current, size, params, visual]);

  const snippets = useMemo(
    () =>
      buildSnippets({
        state: current.pattern,
        size,
        overrides: { ...(visual?.overrides ?? {}), ...params },
        speed: visual?.speed ?? 1,
        specFile: `${current.object}-${current.pattern}.fxspec.json`,
      }),
    // The snippet is the design as it stands, so it follows every control.
    [current, size, state, level, params, states, visual?.speed] // eslint-disable-line react-hooks/exhaustive-deps
  );
  const tabs = [
    ...(typed ?? []).map((t) => ({ ...t, id: `typed-${t.id}`, label: `${t.label} (${current.component})` })),
    ...snippets.code.map((t) => ({ ...t, label: `${t.label} · SinuaView` })),
  ];
  const tab = tabs.find((t) => t.id === platform) ?? tabs[0];

  // The design only: the values changed here, where the catalog says they go. A voice
  // state isn't baked in (it used to be, and made a file the resolver rejects): the
  // view is handed the state at run time, and the engine applies its profile.
  const download = useCallback(() => {
    const text = specText(specFile(current.object, current.pattern, size, params));
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${current.object}-${current.pattern}.fxspec.json`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }, [current, size, params]);

  const fx = { size, state, level, states, theme, paused };
  const families = [{ id: "all", label: "All", count: all.length }, ...catalog.objects.map((o) => ({ id: o.id, label: o.label, count: o.patterns.length }))];

  return (
    <div className="gl">
      <header className="gl-head">
        <h1 className="lp-h2">Pattern gallery</h1>
        <p className="lp-lead">
          Every pattern at its defaults, live. Turn on a voice state to see each one react to a conversation; levels are simulated, and no microphone is
          opened.
        </p>
      </header>

      <div className="gl-bar">
        <nav className="seg gl-seg" aria-label="Family">
          {families.map((f) => (
            <button key={f.id} type="button" className="seg-btn" aria-pressed={family === f.id} onClick={() => setFamily(f.id)}>
              {f.label} <span className="lp-count">{f.count}</span>
            </button>
          ))}
        </nav>
        <div className="gl-bar-right">
          <nav className="seg seg-sm" aria-label="Engine size">
            {SIZES.map((s) => (
              <button key={s} type="button" className="seg-btn" aria-pressed={size === s} onClick={() => setSize(s)}>
                {s}px
              </button>
            ))}
          </nav>
          <button type="button" className="icon-btn" aria-pressed={paused} aria-label={paused ? "Play" : "Pause"} title={paused ? "Play" : "Pause"} onClick={() => setPaused((p) => !p)}>
            <Icon name={paused ? "play" : "pause"} />
          </button>
        </div>
      </div>

      <div className="gl-bench">
        <div id="gallery-grid" className="tiles gl-tiles" data-count={shown.length}>
          {shown.map((p) => (
            <Tile key={p.pattern} pattern={p.pattern} label={p.label} {...fx} maxFps={30} selected={p.pattern === selected} onSelect={() => setSelected(p.pattern)} />
          ))}
        </div>

        <aside className="gl-inspector" aria-label={`${current.label} inspector`}>
          <div className="gl-preview-wrap">
            <PointerToggle on={pointer} onChange={setPointer} />
            <Preview key={current.pattern} pattern={current.pattern} label={current.label} {...fx} overrides={params} pointer={pointer} />
          </div>
          <div className="gl-about">
            <h2 className="lp-h3">
              {current.label} <span className="lp-muted">{current.objectLabel}</span>
            </h2>
            <p>{current.description}</p>
            <Link className="lp-link" href={`/docs/catalog/${current.object}/`}>
              {current.objectLabel} in the docs
            </Link>
          </div>

          <PgTabs
            label="Voice state"
            options={[{ value: "off", label: "Off" }, ...VOICE_STATES.map((s) => ({ value: s, label: s[0].toUpperCase() + s.slice(1) })), { value: "cycle", label: "Cycle" }] as { value: Mode; label: string }[]}
            value={mode}
            onChange={setMode}
            hint={states ? `States come from ${PROFILE_SOURCE}, applied by the engine. ${SIMULATED_NOTE.speedRamp}` : "Off is the bare pattern, as the Studio draws it."}
          />
          {states ? <PgSlider label="Simulated level" value={level} min={0} max={1} step={0.01} display={level.toFixed(2)} onChange={setLevel} /> : null}
          {states && caveat ? (
            <p className="gl-caveat">
              <b>Known caveat ({caveat.status}):</b> {caveat.issue}
            </p>
          ) : null}

          {controls.length ? (
            <div className="gl-controls">
              {controls.map((c) => {
                const value = params[c.key] ?? c.defaults[String(size)] ?? visual?.overrides[c.key] ?? c.min;
                const rounded = Math.round(value * 100) / 100;
                return (
                  <PgSlider
                    key={c.ref}
                    label={c.label}
                    value={value}
                    min={c.min}
                    max={c.max}
                    step={c.step}
                    display={String(rounded)}
                    onChange={(v) => setParams((p) => ({ ...p, [c.key]: v }))}
                    modified={c.key in params}
                    onReset={() =>
                      setParams((p) => {
                        const next = { ...p };
                        delete next[c.key];
                        return next;
                      })
                    }
                  />
                );
              })}
            </div>
          ) : null}
        </aside>
      </div>

      <div className="gl-code" data-snippet={tab.id}>
        <Snippet tabs={tabs} value={tab.id} onChange={setPlatform} />
        <div className="gl-code-actions">
          <CopyPrompt object={current.object} pattern={current.pattern} label={current.label} size={size} changed={params} />
          <CopySpec object={current.object} pattern={current.pattern} size={size} changed={params} />
          <button type="button" className="tb-btn copy-btn" onClick={download}>
            <Icon name="download" />
            Download {current.object}-{current.pattern}.fxspec.json
          </button>
          <Link className="lp-link" href={PLATFORM_DOCS[tab.id.replace(/^typed-/, "")] ?? "/docs/getting-started/installation/"}>
            How to set up {tab.label.split(" ")[0]}
          </Link>
        </div>
      </div>
    </div>
  );
}
