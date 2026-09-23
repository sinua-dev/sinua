"use client";
/**
 * The pattern gallery: every catalog pattern live, in the 4 voice states, with
 * the parameters that shape it and the code to paste. The visuals come from
 * `mount` (@sinua/web), the parameter list from `parameterCatalog()`
 * (spec/parameters.json), the states from families' profile (lib/voice-state)
 * and the code from @sinua/snippets -- the Studio's own exporter, so a
 * snippet here is the snippet the Studio hands out.
 *
 * No audio device is ever opened: levels are simulated.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { parameterCatalog, type OrbSize } from "@sinua/core";
import { buildSnippets, buildTypedSnippets, toTypedProps } from "@sinua/snippets";
import type { FxHandle } from "@sinua/web";
import { useSiteTheme } from "@/lib/use-site-theme";
import { PROFILE_SOURCE, SIMULATED_NOTE, stateCaveat, voiceStateVisual, VOICE_STATES, type VoiceState } from "@/lib/voice-state";

type Catalog = ReturnType<typeof parameterCatalog>;
type Params = Record<string, number>;

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

/** One live visual. Mounts once, then every change goes through `update`. */
function Tile({
  pattern,
  size,
  state,
  level,
  states,
  overrides,
  theme,
  paused,
  maxFps,
  onSelect,
  selected,
}: {
  pattern: string;
  size: OrbSize;
  state: VoiceState;
  level: number;
  /** Off (the default): the bare pattern at its catalog defaults, exactly what the Studio draws. */
  states: boolean;
  overrides?: Params;
  theme: "light" | "dark";
  paused: boolean;
  maxFps?: number;
  onSelect?: () => void;
  selected?: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const visual = useMemo(() => (states ? voiceStateVisual(pattern, state, level) : null), [states, pattern, state, level]);
  const opts = useMemo(
    () => ({
      pattern,
      size,
      ...(visual ? { speed: visual.speed } : {}),
      overrides: { ...(visual?.overrides ?? {}), ...overrides },
      theme,
      paused,
      maxFps,
    }),
    [pattern, size, visual, overrides, theme, paused, maxFps]
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
        console.error("Sinua: the engine chunk failed to load; the %s will stay blank.", "gallery tile", err);
      });
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
    // Mount once per tile; `opts` changes are applied below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    handle.current?.update(opts);
  }, [opts]);



  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      data-pattern={pattern}
      className={`fx-tile flex flex-col items-center gap-1 rounded-xl border p-2 transition-colors ${selected ? "border-fd-primary" : "border-fd-border hover:border-fd-primary/50"}`}
    >
      <canvas ref={canvas} className="aspect-square w-full" aria-label={`${pattern} preview`} />
      <span className="text-xs text-fd-muted-foreground">{pattern}</span>
    </button>
  );
}

export function Gallery() {
  const catalog = useMemo(() => parameterCatalog(), []);
  const all = useMemo(
    () => catalog.objects.flatMap((o) => o.patterns.map((p) => ({ object: o.id, component: o.component, pattern: p.id, label: p.label }))),
    [catalog]
  );
  const [family, setFamily] = useState<string>("all");
  const [size, setSize] = useState<OrbSize>(64);
  const [state, setState] = useState<VoiceState>("listening");
  const [cycle, setCycle] = useState(false);
  // Off by default: the grid shows each pattern plain, as the Studio draws it.
  const [states, setStates] = useState(false);
  const [paused, setPaused] = useState(false);
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
  const [copied, setCopied] = useState(false);

  // A simulated level: no microphone is ever opened here.
  const [level, setLevel] = useState(0.45);
  useEffect(() => {
    if (!cycle) return;
    const id = setInterval(() => setState((s) => VOICE_STATES[(VOICE_STATES.indexOf(s) + 1) % VOICE_STATES.length]), 3000);
    return () => clearInterval(id);
  }, [cycle]);

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

  const download = useCallback(() => {
    const spec = {
      fxSpec: "1.8",
      object: current.object,
      pattern: current.pattern,
      size,
      ...(visual && visual.speed !== 1 ? { speed: visual.speed } : {}),
      params: { ...(visual?.overrides ?? {}), ...params },
    };
    const url = URL.createObjectURL(new Blob([JSON.stringify(spec, null, 2) + "\n"], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${current.object}-${current.pattern}.fxspec.json`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }, [current, size, params, visual]);

  const pill = (on: boolean) =>
    `rounded-full border px-2.5 py-1 text-xs ${on ? "border-fd-primary text-fd-primary" : "border-fd-border text-fd-muted-foreground hover:text-fd-foreground"}`;

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-4 p-4">
      <header className="flex flex-col gap-1">
        <h1 className="text-2xl font-semibold">Pattern gallery</h1>
        <p className="text-sm text-fd-muted-foreground">
          Every pattern at its defaults, live. Turn on voice states to see each one react to a conversation; levels are simulated, and no microphone is opened.
        </p>
      </header>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-fd-border p-2">
        <div role="radiogroup" aria-label="Family" className="flex flex-wrap gap-1">
          {["all", ...catalog.objects.map((o) => o.id)].map((f) => (
            <button key={f} type="button" role="radio" aria-checked={family === f} onClick={() => setFamily(f)} className={pill(family === f)}>
              {f}
            </button>
          ))}
        </div>
        <span className="mx-1 h-4 w-px bg-fd-border" />
        <button
          type="button"
          aria-pressed={states}
          onClick={() => { setStates((v) => !v); setCycle(false); }}
          className={pill(states)}
          title="The state profile (and the caveats that go with it) only applies with voice states on; off is the bare pattern."
        >
          Voice states {states ? "on" : "off"}
        </button>
        {states ? (
          <div role="radiogroup" aria-label="Voice state" className="flex gap-1">
            {VOICE_STATES.map((s) => (
              <button key={s} type="button" role="radio" aria-checked={state === s && !cycle} onClick={() => { setCycle(false); setState(s); }} className={pill(state === s && !cycle)}>
                {s}
              </button>
            ))}
            <button type="button" aria-pressed={cycle} onClick={() => setCycle((c) => !c)} className={pill(cycle)}>
              cycle
            </button>
          </div>
        ) : null}
        <span className="mx-1 h-4 w-px bg-fd-border" />
        <div role="radiogroup" aria-label="Engine size" className="flex gap-1">
          {SIZES.map((s) => (
            <button key={s} type="button" role="radio" aria-checked={size === s} onClick={() => setSize(s)} className={pill(size === s)}>
              {s}px
            </button>
          ))}
        </div>
        <button type="button" aria-pressed={paused} onClick={() => setPaused((p) => !p)} className={pill(paused)}>
          {paused ? "play" : "pause"}
        </button>
        {states ? (
          <label className="ml-auto flex items-center gap-2 text-xs text-fd-muted-foreground">
            <span>level {level.toFixed(2)}</span>
            <input type="range" min={0} max={1} step={0.01} value={level} onChange={(e) => setLevel(Number(e.target.value))} aria-label="Simulated level" />
          </label>
        ) : null}
      </div>

      <div className="grid gap-4 lg:grid-cols-[1fr_22rem]">
        <div id="gallery-grid" className="grid content-start grid-cols-[repeat(auto-fill,minmax(110px,1fr))] items-start gap-2" data-count={shown.length}>
          {shown.map((p) => (
            <Tile
              key={p.pattern}
              pattern={p.pattern}
              size={size}
              state={state}
              level={level}
              states={states}
              theme={theme}
              paused={paused}
              maxFps={30}
              selected={p.pattern === selected}
              onSelect={() => setSelected(p.pattern)}
            />
          ))}
        </div>

        <aside className="flex flex-col gap-3 rounded-xl border border-fd-border p-3">
          <div className="flex items-baseline justify-between gap-2">
            <h2 className="text-lg font-medium">{current.label}</h2>
            <Link className="text-xs underline decoration-fd-border underline-offset-4 hover:decoration-fd-primary" href={`/docs/catalog/${current.object}/`}>
              {current.object} docs
            </Link>
          </div>
          <Tile key={current.pattern} pattern={current.pattern} size={size} state={state} level={level} states={states} overrides={params} theme={theme} paused={paused} selected />
          {states && caveat ? (
            <p className="rounded border border-fd-border p-2 text-xs text-fd-muted-foreground">
              <b>Known caveat ({caveat.status}):</b> {caveat.issue}
            </p>
          ) : null}

          {controls.length ? (
            <div className="flex flex-col gap-1">
              {controls.map((c) => {
                const value = params[c.key] ?? c.defaults[String(size)] ?? visual?.overrides[c.key] ?? c.min;
                return (
                  <label key={c.ref} className="flex items-center gap-2 text-xs text-fd-muted-foreground" title={c.description}>
                    <span className="w-24 shrink-0 truncate">{c.label}</span>
                    <input
                      type="range"
                      className="w-full"
                      min={c.min}
                      max={c.max}
                      step={c.step}
                      value={value}
                      onChange={(e) => setParams((p) => ({ ...p, [c.key]: Number(e.target.value) }))}
                    />
                    <span className="w-10 shrink-0 text-right tabular-nums">{Math.round(value * 100) / 100}</span>
                  </label>
                );
              })}
            </div>
          ) : null}

          <div className="flex flex-wrap gap-1">
            {tabs.map((t) => (
              <button key={t.id} type="button" role="tab" aria-selected={t.id === tab.id} onClick={() => setPlatform(t.id)} className={pill(t.id === tab.id)}>
                {t.label}
              </button>
            ))}
          </div>
          <pre className="max-h-64 overflow-auto rounded bg-fd-muted p-2 text-[11px] leading-relaxed" data-snippet={tab.id}>
            {tab.code}
          </pre>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={pill(false)}
              onClick={() => {
                navigator.clipboard?.writeText(tab.code).then(() => {
                  setCopied(true);
                  setTimeout(() => setCopied(false), 1400);
                }, () => undefined);
              }}
            >
              {copied ? "copied" : "copy code"}
            </button>
            <button type="button" className={pill(false)} onClick={download}>
              download .fxspec.json
            </button>
            <Link className="text-xs text-fd-muted-foreground underline decoration-fd-border underline-offset-4 hover:text-fd-foreground" href={PLATFORM_DOCS[tab.id] ?? "/docs/getting-started/installation/"}>
              How to set up {tab.label}
            </Link>
          </div>

          {states ? (
          <p className="text-[11px] leading-relaxed text-fd-muted-foreground">
            States come from <code>{PROFILE_SOURCE}</code>, applied by the engine. One thing is still view-side: {SIMULATED_NOTE.speedRamp}
          </p>
          ) : (
            <p className="text-[11px] leading-relaxed text-fd-muted-foreground">
              Each pattern at its catalog defaults — the same frame the Studio draws. Turn on <b>Voice states</b> to see it react to a conversation; the tuned state profile, and the caveats that come with it, only apply there.
            </p>
          )}
        </aside>
      </div>
    </div>
  );
}
