import {
  fxSpecTransition,
  FxSpecPlayer,
  SharedVoiceSource,
  a11yStateWords,
  accessibleName,
  announceStep,
  effectInfo,
  fxSpecAccessibility,
  fxSpecDeriveState,
  StateTransition,
  VoiceOverrides,
  frameWithOverridesPacked,
  resolveFxSpec,
  resolvedOpts,
  voiceStateProfile,
  expressionOverrides,
  paletteOverrides,
  patternLayout,
  type AnnouncerState,
  type FxAccessibility,
  type FxDiagnostic,
  type FxSpec,
  type OrbFrame,
  type OrbSize,
  type PackedFrame,
  type TransitionSide,
  type OrbState,
  type VoiceOverridesOptions,
  type VoiceSource,
  type VoiceStateProfile,
} from "@sinua/core";
import { drawCrossDissolve, drawFrame, drawPacked } from "./paint.js";
import { createFramePacer, performanceFor, type FxPerformance } from "./perf.js";

/**
 * Everything `mount` / `<SinuaView/>` accepts. Give either `spec` (an FX Spec
 * document or its JSON text) or `state` (+ `size`, `overrides`, `speed`).
 */
/** A spec as given (typed, JSON text, or a widened JSON import) -> what core takes. */
const asSpec = (spec: FxSpec | string | object): FxSpec | string => spec as FxSpec | string;

export interface SinuaViewOptions {
  /**
   * An FX Spec (docs/fx-spec.md). Takes precedence over `state`. `object` is
   * accepted so a JSON import (`import spec from "./x.fxspec.json"`, whose
   * literal types TypeScript widens) needs no cast; the resolver validates it
   * at runtime either way.
   */
  spec?: FxSpec | string | object;
  /** Plain input: a pattern name, e.g. `"speaking"` or `"tracking"` (see `OrbState`). */
  pattern?: OrbState | string;
  /**
   * The spec's lifecycle state (FX Spec 1.7 naming): the `states` key to render.
   * Default: the voice's `AgentState` when a voice is attached ("listening",
   * "speaking", ... -- docs/fx-spec.md's convention), else the top-level
   * design. A key the spec doesn't have renders the top-level design. State
   * changes animate (`FxSpecPlayer`: the spec's `transitions`, default 0.6 s).
   *
   * Deprecated fallback: without a `spec`, `state` is read as `pattern` (its pre-1.7 meaning).
   */
  state?: OrbState | string;
  /** Engine size for plain input. Default 64. */
  size?: OrbSize;
  /** Engine opts overlaid on the preset (plain input), e.g. from the Studio's export. */
  overrides?: Record<string, number>;
  /** Multiplier on the preset's tuned speed (plain input; a spec carries its own `speed`). Default 1. */
  speed?: number;
  /**
   * Makes the visual react to a conversation. A `VoiceSource` is bound for
   * you through `SharedVoiceSource.of(source)`, so several views and a voice
   * button can share one source, and the views show its mute. A source
   * holds one callback of each kind: to listen yourself, subscribe on
   * `SharedVoiceSource.of(source)`, not on the source (or pass a
   * `VoiceOverrides` you feed yourself). The view never connects or
   * disconnects the source.
   */
  voice?: VoiceSource | VoiceOverrides | null;
  /**
   * Words per state for the accessible name and announcements (docs/fx-view.md,
   * *Accessibility*): `{ listening: "Coach is listening" }`. Win over the spec's
   * `accessibility.states` and the built-in words; this is how apps translate.
   */
  labels?: Record<string, string>;
  /** Speak state changes (polite, rate-limited). Default: the spec's `accessibility.announce`, else true. */
  announce?: boolean;
  /** Derive the state from the spec's 1.9 `rules` and `inputs` (default true). Off while a voice is bound. */
  rules?: boolean;
  /** Options for the bound `VoiceOverrides`; defaults follow the Studio per family (orb: raw bands; signal: scrolling history). */
  voiceOptions?: VoiceOverridesOptions;
  /** @deprecated Renamed to `state`. */
  specState?: string;
  /** FX Spec v1.1 app inputs driving the spec's `bindings` (e.g. `{ heartRate: 72 }`). */
  inputs?: Record<string, number>;
  /**
   * Feed the voice level into this spec input (e.g. `"agentVolume"`, as
   * spec/examples/voice-assistant.fxspec.json binds it), so the spec's own
   * binding drives the look. Optional; the engine voice keys are applied either way.
   */
  voiceLevelInput?: string;
  /**
   * Every state change's duration, in seconds (`0` = a cut). Unset: the spec's
   * 1.9 `transitions`, or 0.6 s. A change keeping the pattern interpolates its
   * parameters; a pattern change morphs (the orb lattice trio) or cross-fades.
   */
  crossFade?: number;
  /** `auto` follows `prefers-color-scheme`. Colors with `colorMode: "fixed"` look the same in both. Default `auto`. */
  theme?: "auto" | "light" | "dark";
  /** Stops the clock (the pose freezes; resume continues from it). */
  paused?: boolean;
  /** `auto` follows `prefers-reduced-motion`: a static frame at t = 0.6 (spec/orbs-spec.json `paint`). Default `auto`. */
  reducedMotion?: "auto" | "always" | "never";
  /** Accessible name (`role="img"`). Default: the spec's `name`, else the state. `""` marks the canvas decorative. */
  label?: string;
  /** Called with the diagnostics when a spec doesn't resolve (nothing is drawn). */
  onError?: (diagnostics: FxDiagnostic[]) => void;
  /**
   * Frame-rate cap (display frames are skipped, the clock stays wall time).
   * A spec's `performance.maxFps` (FX Spec 1.2) also caps; the lower wins.
   * Default: display rate.
   */
  maxFps?: number;
  /**
   * Low-power mode. The Web has no reliable OS signal (docs/fx-view.md), so
   * the app sets this -- e.g. from its own setting or `watchLowBattery()`.
   * On: the spec's `performance.lowPower` (1.2), else 30 fps with glow off.
   */
  lowPower?: boolean;
  /** Per drawn frame: time since the previous drawn frame, engine time and paint time (ms). */
  onFrame?: (stats: FxFrameStats) => void;
  /**
   * Pointer and touch scatter: the dots near the pointer are pushed away from it
   * (the engine's `pointerX/Y/Radius/Strength`), easing in and out over about a
   * tenth of a second. Off by default. Not applied under reduced motion or while
   * paused. It doesn't change `touch-action`, so a page still scrolls on touch.
   */
  pointer?: boolean;
  /**
   * Tap to hop (design note 15): a click or a touch on the view plays the `hop`
   * effect, and a character glances toward where it was tapped. Characters only:
   * other families draw nothing for it. Off by default here; `SinuaCharacter`
   * turns it on. Ignored while success / error / celebrate plays; at most two
   * hops a second.
   */
  tap?: boolean;
  /**
   * A character's expression (design note 16): `happy`, `surprised`, `thoughtful`, `sad`,
   * `sleepy`, or `null` / `"none"` for none. It shapes the eyes and the resting mouth; the
   * voice state keeps the gaze, the turn and the talking mouth. It wins over a spec's
   * `expression`; leave it unset to let the spec decide. A change eases over 0.6 s.
   */
  expression?: string | null;
  /**
   * A character's palette, in part (design note 19): slot -> hex or DTCG colour, e.g.
   * `{ body: "#E63946" }`. The slots' tones follow; it wins over a spec's `palette`.
   * FX Spec 1.13 (design note 23): role names (`primary`, `secondary`, `accent`) and a
   * `dark` variant, picked in a dark theme.
   * A change is immediate. Problems (an unknown slot) go to `onError`.
   */
  palette?: Record<string, unknown> | null;
}

export interface FxFrameStats {
  dtMs: number;
  computeMs: number;
  paintMs: number;
}

export interface FxHandle {
  /** Change any option; unspecified ones keep their value. */
  update(options: Partial<SinuaViewOptions>): void;
  pause(): void;
  resume(): void;
  /** Stops the loop and all observers; leaves the canvas as last drawn. */
  destroy(): void;
  /** The bound `VoiceOverrides` (for a level meter / lifecycle label), or null. */
  readonly voice: VoiceOverrides | null;
  /** Seconds the clock has run (pauses excluded). */
  readonly elapsed: number;
  /**
   * Plays a one-shot effect on top of the view: `success`, `error`, `celebrate`, or a
   * character's `hop` (silent; see `tap`)
   * (docs/fx-view.md, *One-shot effects*). A new one replaces a running one; unknown
   * names do nothing. The words are spoken unless `announce` is false.
   */
  trigger(name: string): void;
}

/** spec/orbs-spec.json `paint`: devicePixelRatioCap and the reduced-motion pose. */
export const DPR_CAP = 2;
export const REDUCED_MOTION_T = 0.6;
const MAX_DT_S = 0.1;
/** Pointer strength eases toward its target at this rate per second (about 1/10 s), as in the Studio. */
const POINTER_EASE_RATE = 10;
const REDUCED_MOTION_VOICE_INTERVAL_MS = 1000 / 30;

interface Resolved {
  /** FX Spec 1.2: the spec has a `performance.lowPower` block (the resolver sheds / caps for low power itself). */
  specHandlesLowPower: boolean;
  state: string;
  size: number;
  speed: number;
  presetSpeed: number;
  overrides: Record<string, number>;
  family: string | null;
  name: string | null;
}

/**
 * Resolve the input once per change: validity (diagnostics), size, speeds
 * and naming. A spec's per-frame v1.1 state/inputs resolution happens in
 * `FxSpecPlayer`; plain input renders from this directly.
 */
/** The plain-input pattern: `pattern`, or (deprecated) `state` when there's no spec. */
export function patternOf(o: SinuaViewOptions): string | undefined {
  return o.pattern ?? (o.spec == null ? o.state : undefined);
}

/**
 * The lifecycle state: `state` with a spec, and also with plain input once a
 * `pattern` is given (`pattern` + `state` is the voice-state case, docs/fx-view.md).
 * Without a `pattern`, a spec-less `state` is still the deprecated pattern label.
 */
export function lifecycleStateOf(o: SinuaViewOptions): string | undefined {
  const named = o.spec != null || o.pattern != null ? o.state : undefined;
  return named ?? o.specState;
}

/**
 * How a view with these options lays out: `"box"` for a box-layout pattern (edge
 * signal `playing`: it fills whatever box it gets), else `"square"`.
 * Wrappers use it for their default size (React: no square `aspectRatio` for a box
 * pattern). Invalid input is `"square"`.
 */
export function viewLayout(o: SinuaViewOptions): "box" | "square" {
  const r = resolveInput(o);
  return r.ok ? patternLayout(r.value.state) : "square";
}

function resolveInput(o: SinuaViewOptions): { ok: true; value: Resolved } | { ok: false; diagnostics: FxDiagnostic[] } {
  if (o.spec != null) {
    const r = resolveFxSpec(asSpec(o.spec));
    if (!r.ok) return { ok: false, diagnostics: r.diagnostics };
    const spec = asSpec(o.spec);
    const doc = typeof spec === "string" ? safeParse(spec) : spec;
    const perfBlock = (doc)?.performance;
    return {
      ok: true,
      value: {
        specHandlesLowPower: perfBlock?.lowPower != null,
        state: r.state,
        size: r.size,
        speed: r.speed,
        presetSpeed: resolvedOpts(r.state as OrbState, r.size as OrbSize)?.speed ?? 1,
        overrides: r.overrides,
        family: typeof doc?.object === "string" ? doc.object : null,
        name: typeof doc?.name === "string" ? doc.name : null,
      },
    };
  }
  const pattern = patternOf(o);
  if (!pattern) return { ok: false, diagnostics: [{ severity: "error", path: "", message: "SinuaView needs a `spec` or a `pattern`" }] };
  const size = o.size ?? 64;
  const preset = resolvedOpts(pattern as OrbState, size);
  if (!preset) return { ok: false, diagnostics: [{ severity: "error", path: "/pattern", message: `unknown pattern "${pattern}"` }] };
  return {
    ok: true,
    value: { specHandlesLowPower: false, state: pattern, size, speed: o.speed ?? 1, presetSpeed: preset.speed, overrides: o.overrides ?? {}, family: null, name: null },
  };
}

function safeParse(text: string): { object?: unknown; name?: unknown; performance?: { lowPower?: unknown } } | null {
  try {
    return JSON.parse(text) as { object?: unknown; name?: unknown; performance?: { lowPower?: unknown } };
  } catch {
    return null;
  }
}

/** Whether resolved opts carry a palette's dark variant (`palette.dark.<slot>.*`). */
function hasDarkPalette(o: Record<string, number> | null | undefined): boolean {
  if (!o) return false;
  for (const k in o) if (k.startsWith("palette.dark.")) return true;
  return false;
}

/** The Studio's per-family `VoiceOverrides` settings (docs/fx-view.md). */
export function defaultVoiceOptions(family: string | null, overrides: Record<string, number>): VoiceOverridesOptions {
  if (family === "orb") return { bandEaseRate: Infinity };
  if (family === "signal") return { audioStrength: 0, history: { count: Math.round(overrides.historyCount ?? 40), hz: 12 } };
  return {};
}

function isVoiceOverrides(v: unknown): v is VoiceOverrides {
  return v instanceof VoiceOverrides;
}

/**
 * One `requestAnimationFrame` per window for every mounted view (roadmap 10):
 * N views used to schedule N callbacks each display frame. A view's request
 * joins its window's queue, and the one rAF runs the queue in order; each view
 * still decides for itself whether to draw (its pacer) and asks again for the
 * next frame, so the behaviour is exactly the per-view one's.
 */
const frameQueues = new WeakMap<object, { cbs: Set<(now: number) => void>; id: number | null }>();

function requestFrame(win: Window, cb: (now: number) => void): void {
  let q = frameQueues.get(win);
  if (!q) frameQueues.set(win, (q = { cbs: new Set(), id: null }));
  q.cbs.add(cb);
  if (q.id != null) return;
  const queue = q;
  queue.id = win.requestAnimationFrame((now) => {
    queue.id = null;
    const run = [...queue.cbs];
    queue.cbs.clear();
    for (const f of run) {
      try {
        f(now);
      } catch (e) {
        // One view's error mustn't stop the others' frame; report it as the
        // browser would have for its own rAF callback.
        queueMicrotask(() => {
          throw e;
        });
      }
    }
  });
}

function cancelFrame(win: Window, cb: (now: number) => void): void {
  const q = frameQueues.get(win);
  if (!q) return;
  q.cbs.delete(cb);
  if (q.cbs.size === 0 && q.id != null) {
    win.cancelAnimationFrame?.(q.id);
    q.id = null;
  }
}

/** Below this short side (CSS px) a view defaults to 30 fps (roadmap 10). */
export const SMALL_VIEW_PX = 48;
/** The default cap for a small view, when neither the app nor the spec sets one. */
export const SMALL_VIEW_MAX_FPS = 30;

/**
 * Render an FX Spec (or a plain state) into `canvas`, animated, until
 * `destroy()`. Handles the clock (`t = elapsed * presetSpeed * speed`, pinned at
 * each speed change so the pose doesn't jump), the
 * backing store (CSS size x devicePixelRatio, capped at 2), theme, reduced
 * motion, pausing when off-screen or the tab is hidden, and voice.
 *
 * ```ts
 * import { mount } from "@sinua/web";
 * const fx = mount(canvas, { spec: mySpec, voice: source });
 * ```
 */
export function mount(canvas: HTMLCanvasElement, options: SinuaViewOptions): FxHandle {
  let opts: SinuaViewOptions = { ...options };
  let warned = false;
  const warnPlainState = (o: SinuaViewOptions) => {
    if (warned || o.spec != null || o.pattern != null || o.state == null) return;
    warned = true;
    console.warn("SinuaView: `state` without a `spec` is deprecated; use `pattern` (FX Spec 1.7 naming).");
  };
  warnPlainState(opts);
  let resolved: Resolved | null = null;
  let voice: VoiceOverrides | null = null;
  let boundSource: VoiceSource | null = null;
  // A raw source is bound through its `SharedVoiceSource`: this view gets its own
  // tracker (its family's easing), and other views / a voice button keep theirs.
  let releaseVoice: (() => void) | null = null;
  let offVoiceState: (() => void) | null = null;
  // Accessibility (docs/fx-view.md): the spec's block, the state the name last followed,
  // the announcer's memory, its recheck timer and the live region it speaks through.
  let a11yInfo: FxAccessibility = { name: null, states: {}, announce: null };
  let a11yState: string | null | undefined = undefined;
  let announcer: AnnouncerState | null = null;
  let announceTimer: ReturnType<typeof setTimeout> | null = null;
  let liveRegion: HTMLElement | null = null;
  // Rules (FX Spec 1.9): the state the spec's rules picked last (for hysteresis).
  let derived: string | null = null;
  // One-shot effect (docs/fx-view.md): the running one, on the performance clock.
  let effect: { code: number; duration: number; start: number } | null = null;
  let warnedEffect = false;
  // The tap hop: where the view was tapped (-1..1 from the drawn square's centre), and when.
  const HOP = effectInfo("hop");
  let tapAt: { x: number; y: number } | null = null;
  let lastHop = -Infinity;

  /** The effect's runtime keys while it runs (then it's cleared). */
  function withEffect<T extends Record<string, number>>(keys: T): T | (T & Record<string, number>) {
    if (!effect) return keys;
    const age = performance.now() / 1000 - effect.start;
    if (age >= effect.duration) {
      effect = null;
      return keys;
    }
    const tap = tapAt && effect.code === HOP?.code ? { tapX: tapAt.x, tapY: tapAt.y } : {};
    return { ...keys, effectCode: effect.code, effectAge: Math.max(0, age), effectReduced: isReduced() ? 1 : 0, ...tap };
  }
  // The expression's weights, eased from what was shown to the new target over 0.6 s.
  const EXPRESSION_S = 0.6;
  let exprName: string | null | undefined = undefined;
  let exprFrom: Record<string, number> = {};
  let exprTo: Record<string, number> | null = null;
  let exprStart = 0;
  // The palette's opts, resolved once per (pattern, palette).
  let paletteKey = "";
  let paletteKeys: Record<string, number> = {};
  function paletteNow(pattern: string): Record<string, number> {
    if (!opts.palette) return {};
    const key = `${pattern}\u0000${JSON.stringify(opts.palette)}`;
    if (key !== paletteKey) {
      paletteKey = key;
      // A file's own recipe: resolved again by its id (the registry keeps it by hash).
      let recipe: Record<string, unknown> | undefined;
      try {
        const text = specText();
        const doc = text ? (JSON.parse(text) as { recipe?: unknown }) : null;
        if (doc?.recipe && typeof doc.recipe === "object") recipe = doc.recipe as Record<string, unknown>;
      } catch {
        recipe = undefined;
      }
      const id = recipe && typeof recipe.id === "string" ? recipe.id : pattern;
      const r = paletteOverrides(id, opts.palette, recipe);
      paletteKeys = r.overrides;
      const bad = r.diagnostics.filter((d) => d.severity === "error");
      if (bad.length) {
        if (opts.onError) opts.onError(bad);
        else console.warn(`SinuaView palette: ${bad.map((d) => `${d.path}: ${d.message}`).join("; ")}`);
      }
    }
    return paletteKeys;
  }
  function expressionNow(): Record<string, number> | null {
    if (!exprTo) return null;
    const u = isReduced() ? 1 : Math.min(1, (performance.now() / 1000 - exprStart) / EXPRESSION_S);
    const e = u * u * (3 - 2 * u);
    const out: Record<string, number> = {};
    for (const [k, v] of Object.entries(exprTo)) out[k] = (exprFrom[k] ?? 0) + (v - (exprFrom[k] ?? 0)) * e;
    return out;
  }
  function applyExpression(): void {
    const name = opts.expression;
    if (name === exprName) return;
    exprFrom = expressionNow() ?? {};
    if (name === undefined) exprTo = null;
    else {
      const to = expressionOverrides(name ?? "none");
      if (!to) console.warn(`SinuaView: no expression named "${name}" (happy, surprised, thoughtful, sad, sleepy)`);
      exprTo = to ?? expressionOverrides("none");
    }
    exprStart = performance.now() / 1000;
    exprName = name;
  }
  /** Plays the hop (from a tap at `at`, or `trigger("hop")`): never over another effect, at most twice a second. */
  function hop(at: { x: number; y: number } | null): void {
    if (!HOP || destroyed) return;
    const now = performance.now() / 1000;
    if (effect && effect.code !== HOP.code && now - effect.start < effect.duration) return;
    if (now - lastHop < 0.5) return;
    lastHop = now;
    tapAt = at;
    effect = { code: HOP.code, duration: HOP.duration, start: now };
    refresh();
  }
  let player: FxSpecPlayer | null = null;
  let perf: FxPerformance = { maxFps: null, overrides: {} };
  let pacer = createFramePacer(null);

  let elapsed = 0;
  /**
   * The engine's clock. `elapsed * speed` alone jumps the pose whenever the speed
   * changes (a lifecycle state with its own speed, or an app changing `speed`),
   * because all the time already elapsed is rescaled at once. So the phase is
   * pinned at each speed change and runs from there:
   *   phase = phaseBase + (elapsed - elapsedBase) * speed
   * With a constant speed that is exactly `elapsed * speed`, bit for bit.
   */
  let phaseBase = 0;
  let elapsedBase = 0;
  let phaseSpeed: number | null = null;
  /** `elapsed` at the previous `phaseNow()`: a speed change counts from there. */
  let phaseLastRead = 0;
  /**
   * The engine speed per lifecycle state of a spec, cached. `FxSpecPlayer` resolves the
   * spec *with* the state and multiplies by that state's speed, so the view has to use the
   * same number: taking the file's base speed made a state with its own speed jump once.
   */
  // State transitions on the plain path (the spec path's live in FxSpecPlayer).
  const transition = new StateTransition();
  let lastLifecycle: string | undefined;
  /**
   * The built-in voice-state behaviour for plain input (`pattern` + `state`, no spec):
   * `voiceStateProfile` gives the overrides, a speed multiplier and which app input
   * drives `audioLevel`. Cached per pattern+state; `null` for a state outside the five
   * voice names, which leaves an app's own state names alone.
   */
  const profiles = new Map<string, VoiceStateProfile | null>();

  function profileFor(pattern: string, state: string | undefined): VoiceStateProfile | null {
    if (!state) return null;
    const key = `${pattern}\u0000${state}`;
    let profile = profiles.get(key);
    if (profile === undefined) {
      profile = voiceStateProfile(pattern, state);
      profiles.set(key, profile);
    }
    return profile;
  }

  /**
   * The plain path's design for a lifecycle state, as a transition side: the
   * voice-state profile under the app's overrides, at the effective speed.
   * Live keys (audio, pointer, `audioLevel`) are not part of it.
   */
  function plainSide(state: string | undefined): TransitionSide | null {
    if (!resolved) return null;
    const profile = profileFor(resolved.state, state);
    return {
      state: resolved.state,
      speed: resolved.presetSpeed * resolved.speed * (profile?.speed ?? 1),
      overrides: profile ? { ...profile.overrides, ...resolved.overrides } : { ...resolved.overrides },
    };
  }

  /** The lifecycle state now: the app's, else the bound voice source's. */
  function lifecycleNow(): string | undefined {
    const own = lifecycleStateOf(opts);
    // A conversation drives the state while a voice is bound (the app's `state` still wins);
    // otherwise a state the spec's rules derive from `inputs` wins over the app's own.
    if (voice) return own ?? voice.state;
    return derived ?? own;
  }

  function specText(): string | null {
    if (opts.spec == null) return null;
    const spec = asSpec(opts.spec);
    return typeof spec === "string" ? spec : JSON.stringify(spec);
  }

  /** Re-derive the rules' state from `inputs` (FX Spec 1.9), keeping the last one for hysteresis. */
  function applyRules(): void {
    const text = specText();
    derived = text != null && resolved && opts.rules !== false && !voice ? fxSpecDeriveState(text, opts.inputs ?? {}, derived) : null;
  }
  let lastTick: number | null = null;
  let lastReducedDraw = -Infinity;
  let scheduled = false;
  // A box-layout pattern (`patternLayout`: signal `playing`) fills
  // the whole box: the engine gets the box ratio as `aspect`, no centred square.
  let boxLayout = false;
  let destroyed = false;
  let manualPause = false;
  let onScreen = true;
  // Pointer scatter (`opts.pointer`): the position follows the pointer at once, in
  // engine space; the strength eases in and out so entering and leaving don't pop.
  const pointer = { x: 0, y: 0, active: false, strength: 0 };
  let pointerBound = false;
  let cssW = canvas.clientWidth || canvas.width;
  let cssH = canvas.clientHeight || canvas.height;

  const win: (Window & typeof globalThis) | undefined = typeof window === "undefined" ? undefined : window;
  const doc: Document | undefined = typeof document === "undefined" ? undefined : document;
  const darkMq = win?.matchMedia?.("(prefers-color-scheme: dark)");
  const motionMq = win?.matchMedia?.("(prefers-reduced-motion: reduce)");

  const isDark = () => (opts.theme === "dark" ? true : opts.theme === "light" ? false : !!darkMq?.matches);
  const isReduced = () => (opts.reducedMotion === "always" ? true : opts.reducedMotion === "never" ? false : !!motionMq?.matches);
  const isRunning = () => !destroyed && !manualPause && !opts.paused && onScreen && !doc?.hidden;
  /** The box ratio a box-layout pattern draws at (the engine's `aspect` range). */
  const boxAspect = () => Math.min(8, Math.max(0.125, canvas.width / Math.max(1, canvas.height)));
  /** A view whose short side is under SMALL_VIEW_PX (CSS px); 0 = not laid out yet. */
  const isSmall = () => {
    const short = Math.min(cssW, cssH);
    return short > 0 && short < SMALL_VIEW_PX;
  };
  let wasSmall = false;

  function applyInput(): void {
    const r = resolveInput(opts);
    if ("diagnostics" in r) {
      resolved = null;
      opts.onError?.(r.diagnostics);
    } else {
      resolved = r.value;
    }
    boxLayout = resolved ? patternLayout(resolved.state) === "box" : false;
    transition.cancel();
    lastLifecycle = lifecycleNow();
    player =
      resolved && opts.spec != null
        ? new FxSpecPlayer(asSpec(opts.spec), { crossFade: opts.crossFade, lowPower: !!opts.lowPower })
        : null;
    applyPerformance();
    applyVoice();
    const text = specText();
    a11yInfo = text != null && resolved ? fxSpecAccessibility(text) : { name: null, states: {}, announce: null };
    applyRules();
    applyA11y();
    refreshA11y();
  }

  function applyPerformance(): void {
    const lowPower = !!opts.lowPower;
    player?.setLowPower(lowPower);
    // FX Spec 1.2: the resolver reports the cap for this power state (`maxFps`,
    // the lowPower one when set) and sheds the spec's `lowPower.disable` itself.
    const specMaxFps = opts.spec != null && resolved ? resolveFxSpec(asSpec(opts.spec), { lowPower }).maxFps : null;
    // A small view (a list of avatars, a badge) defaults to 30 fps, unless the app
    // (`maxFps`, including 0 for display rate) or the spec (`performance.maxFps`) says.
    wasSmall = isSmall();
    const smallCap = opts.maxFps == null && !specMaxFps && wasSmall ? SMALL_VIEW_MAX_FPS : null;
    perf = performanceFor({
      lowPower,
      optionMaxFps: opts.maxFps ?? smallCap,
      specMaxFps,
      specHandlesLowPower: resolved?.specHandlesLowPower,
    });
    pacer = createFramePacer(perf.maxFps);
  }

  function unbindVoice(): void {
    releaseVoice?.();
    releaseVoice = null;
    offVoiceState?.();
    offVoiceState = null;
    boundSource = null;
  }

  function applyVoice(): void {
    const v = opts.voice ?? null;
    if (v == null) {
      unbindVoice();
      voice = null;
    } else if (isVoiceOverrides(v)) {
      unbindVoice();
      voice = v;
    } else if (v !== boundSource) {
      unbindVoice();
      boundSource = v;
      const tracked = SharedVoiceSource.of(v).track(
        opts.voiceOptions ?? defaultVoiceOptions(resolved?.family ?? null, resolved?.overrides ?? {}),
      );
      voice = tracked.overrides;
      releaseVoice = () => tracked.release();
      // The name and announcements follow the conversation even while the view is paused.
      offVoiceState = SharedVoiceSource.of(v).onStateChange(() => queueMicrotask(refreshA11y));
    }
  }

  /** The view's plain name: `label`, else the spec's `accessibility.name`, its `name`, the pattern. */
  function a11yBase(): string {
    return opts.label ?? a11yInfo.name ?? resolved?.name ?? resolved?.state ?? "";
  }

  function applyA11y(): void {
    const base = a11yBase();
    const label = base === "" ? "" : accessibleName(base, a11yState ?? null, a11yInfo.states, opts.labels ?? {});
    if (label === "") {
      canvas.removeAttribute("role");
      canvas.removeAttribute("aria-label");
      canvas.setAttribute("aria-hidden", "true");
    } else {
      canvas.setAttribute("role", "img");
      canvas.setAttribute("aria-label", label);
      canvas.removeAttribute("aria-hidden");
    }
  }

  /** The state the view shows changed (or may have): rename it, and let the announcer decide. */
  function refreshA11y(): void {
    if (destroyed) return;
    const st = lifecycleNow() ?? null;
    if (st === a11yState) return;
    a11yState = st;
    applyA11y();
    const base = a11yBase();
    const speak = base !== "" && (opts.announce ?? a11yInfo.announce ?? true);
    stepAnnouncer(speak ? a11yStateWords(base, st, a11yInfo.states, opts.labels ?? {}) : null);
  }

  function stepAnnouncer(words: string | null): void {
    if (announceTimer != null) clearTimeout(announceTimer);
    announceTimer = null;
    const now = performance.now() / 1000;
    const out = announceStep(announcer, words, now);
    announcer = out.state;
    if (out.announce != null) speakText(out.announce);
    if (out.recheckAt != null) {
      announceTimer = setTimeout(() => stepAnnouncer(announcer?.current ?? null), Math.max(0, (out.recheckAt - now) * 1000));
    }
  }

  /** Polite, visually hidden: a sibling of the canvas (inside the shadow root for `<sinua-view>`). */
  function speakText(text: string): void {
    const parent = canvas.parentNode;
    if (!parent || !doc) return;
    if (!liveRegion) {
      liveRegion = doc.createElement("span");
      liveRegion.setAttribute("aria-live", "polite");
      liveRegion.setAttribute("aria-atomic", "true");
      liveRegion.style.cssText = "position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap";
      parent.insertBefore(liveRegion, canvas.nextSibling);
    }
    liveRegion.textContent = text;
  }

  function resizeBacking(): void {
    const dpr = Math.min(win?.devicePixelRatio || 1, DPR_CAP);
    const w = Math.max(1, Math.round(cssW * dpr));
    const h = Math.max(1, Math.round(cssH * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
  }

  /** The engine speed of one lifecycle state of the spec (the product the player will use). */
  /**
   * The engine speed right now: the preset's tuned speed times the app's multiplier
   * (0 holds the pose). With a spec it is the *current lifecycle state's* speed, which
   * is what `FxSpecPlayer` renders with.
   */
  function speedNow(state?: string): number {
    // With a spec, the player's effective speed (mixed while a transition runs).
    if (player && opts.spec != null) return player.speed();
    if (!resolved) return 1;
    const side = plainSide(state);
    return side ? transition.speed(side, resolved.size as OrbSize) : 1;
  }

  /** The engine time to draw at, continuous across speed changes. */
  function phaseNow(stateForSpeed?: string): number {
    const speed = speedNow(stateForSpeed);
    if (phaseSpeed !== speed) {
      // Pin the phase as of the previous read, then run from there at the new speed:
      // the time since that read belongs to the new speed, so `speed` 0 freezes exactly.
      if (phaseSpeed !== null) {
        phaseBase += (phaseLastRead - elapsedBase) * phaseSpeed;
        elapsedBase = phaseLastRead;
      }
      phaseSpeed = speed;
    }
    phaseLastRead = elapsed;
    if (player && opts.spec != null) return phaseBase + (elapsed - elapsedBase) * speed;
    // The factors stay in the engine's own order (preset, then the app's multiplier, then
    // the voice state's), so without a profile this is bit-for-bit the old
    // `elapsed * presetSpeed * speed`.
    if (!transition.active) {
      const preset = resolved ? resolved.presetSpeed : 1;
      const own = resolved ? resolved.speed : 1;
      const profile = profileFor(resolved ? resolved.state : "", stateForSpeed)?.speed ?? 1;
      return phaseBase + (elapsed - elapsedBase) * preset * own * profile;
    }
    return phaseBase + (elapsed - elapsedBase) * speed;
  }

  /** The engine's pointer keys for this frame, easing the strength by `rawDt`; none when there's nothing to push. */
  function pointerKeys(rawDt: number, size: number, reduced: boolean): Record<string, number> {
    if (!opts.pointer) return {};
    const target = pointer.active ? 1 : 0;
    pointer.strength += (target - pointer.strength) * Math.min(1, POINTER_EASE_RATE * Math.min(rawDt, MAX_DT_S));
    if (reduced || pointer.strength < 0.001) return {};
    return { pointerX: pointer.x, pointerY: pointer.y, pointerRadius: size * 0.35, pointerStrength: pointer.strength * size * 0.12 };
  }

  function draw(rawDt: number): void {
    // A VoiceOverrides the app feeds itself has no state event: notice changes here.
    if (voice && !offVoiceState) refreshA11y();
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (!resolved) return;
    const reduced = isReduced();
    const voiceMap = voice ? voice.overrides(rawDt) : {};
    const pointerMap = pointerKeys(rawDt, resolved.size, reduced);
    const live = Object.keys(pointerMap).length ? { ...voiceMap, ...pointerMap } : voiceMap;
    // Low-power overrides (pre-1.2 / block-less default) sit between the spec's and the live keys.
    const withPerf = Object.keys(perf.overrides).length ? { ...perf.overrides, ...live } : live;
    const shown = withEffect(boxLayout ? { ...withPerf, aspect: boxAspect() } : withPerf);
    const expr = expressionNow();
    const pal = opts.palette && resolved ? paletteNow(resolved.state) : null;
    // A palette's dark variant (FX Spec 1.13, design note 23): the engine picks it
    // when told `dark`. Only sent when a variant exists, so other frames are untouched.
    // A character from a file always gets it (a state may name the theme; unused without a variant).
    const darkPal = resolved?.family === "character" || hasDarkPalette(pal) || hasDarkPalette(resolved?.overrides);
    const dark = isDark() && darkPal ? { dark: 1 } : null;
    const extra = { ...shown, ...(expr ?? {}), ...(pal ?? {}), ...(dark ?? {}) };
    const t0 = opts.onFrame ? performance.now() : 0;
    let frame: OrbFrame | null = null;
    let packed: PackedFrame | null = null;
    let previous: OrbFrame | null = null;
    let blend = 1;
    if (player) {
      // FX Spec: FxSpecPlayer owns the v1.1 state/inputs/transitions and the 1.2
      // low-power resolution; runtime keys go in as extra overrides (spread last).
      const lifecycle = lifecycleNow();
      player.setState(lifecycle);
      if (reduced) player.skipTransition();
      for (const [name, v] of Object.entries(opts.inputs ?? {})) player.setInput(name, v);
      if (opts.voiceLevelInput && voice) player.setInput(opts.voiceLevelInput, voice.metrics.level);
      // The player multiplies by the state's speed, so it takes an *elapsed*:
      // the phase divided by the current speed lands t back on the integrated phase.
      // Reduced motion uses the elapsed that lands t exactly on the static pose.
      // The player multiplies by *this state's* speed, so the division uses the same one.
      // `|| 1` only guards the division; the phase itself is frozen at speed 0.
      const stateSpeed = speedNow(lifecycle) || 1;
      const at = reduced ? REDUCED_MOTION_T / stateSpeed : phaseNow(lifecycle) / stateSpeed;
      const out = player.frame(at, Math.min(rawDt, MAX_DT_S), extra);
      frame = out.frame;
      previous = out.previous;
      blend = out.blend;
    } else {
      // Plain state: the packed transport, painted straight from the buffer.
      // With a lifecycle state (given, or the bound voice's), the built-in voice-state
      // profile goes *under* the app's own overrides, and the voice's live keys stay last.
      const lifecycle = lifecycleNow();
      if (lifecycle !== lastLifecycle) {
        // A state change animates on the transition clock for the voice-state profile's
        // time for the pair (design note 31), or `crossFade` seconds; reduced motion cuts.
        const pair = fxSpecTransition("{}", lastLifecycle, lifecycle);
        transition.start(reduced ? 0 : (opts.crossFade ?? pair.duration), pair.curve);
        lastLifecycle = lifecycle;
      }
      transition.advance(Math.min(rawDt, MAX_DT_S));
      if (reduced) transition.cancel();
      const profile = profileFor(resolved.state, lifecycle);
      const side = plainSide(lifecycle)!;
      const t = reduced ? REDUCED_MOTION_T : phaseNow(lifecycle);
      // `audioInput` names which app input drives `audioLevel` here (never an engine key).
      const level = profile?.audioInput ? opts.inputs?.[profile.audioInput] : undefined;
      const withLevel = level != null ? { ...extra, audioLevel: level } : extra;
      // Seconds since the state changed (a character blinks at the end of the user's turn).
      const age = transition.stateAge;
      const live = age != null ? { ...withLevel, stateAge: age } : withLevel;
      if (transition.active) {
        const out = transition.frames(side, resolved.size as OrbSize, t, live);
        frame = out.frame;
        previous = out.previous;
        blend = out.blend;
      } else {
        // The side's overrides, plus the rate sums once a rate changed mid-session.
        const own = transition.steadyOverrides(side, resolved.size as OrbSize, t);
        packed = frameWithOverridesPacked(resolved.state as OrbState, resolved.size as OrbSize, t, { ...own, ...live });
      }
    }
    if (!frame && !packed) return;
    const t1 = opts.onFrame ? performance.now() : 0;
    // Square engine space, centered in whatever box the canvas has -- or, for a
    // box-layout pattern, `size * aspect` by `size` over the whole box.
    const side = boxLayout ? canvas.height : Math.min(canvas.width, canvas.height);
    const scale = side / resolved.size;
    ctx.save();
    if (!boxLayout) ctx.translate((canvas.width - side) / 2, (canvas.height - side) / 2);
    if (packed) drawPacked(ctx, packed, isDark(), scale);
    else if (previous && frame) drawCrossDissolve(ctx, previous, frame, blend, isDark(), scale);
    else if (frame) drawFrame(ctx, frame, isDark(), scale);
    ctx.restore();
    if (opts.onFrame) {
      const t2 = performance.now();
      opts.onFrame({ dtMs: rawDt * 1000, computeMs: t1 - t0, paintMs: t2 - t1 });
    }
  }

  function tick(now: number): void {
    scheduled = false;
    if (!isRunning()) return;
    // Frame cap: a skipped display frame does nothing (lastTick stays, so the
    // next drawn frame's dt covers the whole gap and the clock keeps wall time).
    if (!pacer(now)) {
      schedule();
      return;
    }
    const rawDt = lastTick == null ? 0 : Math.max(0, (now - lastTick) / 1000);
    lastTick = now;
    if (isReduced()) {
      // Static pose; still redraw (throttled) while a voice is attached --
      // the voice cue is information, not decoration.
      // So does a one-shot effect (its reduced variant: a tint, no motion).
      if ((voice || effect) && now - lastReducedDraw >= REDUCED_MOTION_VOICE_INTERVAL_MS) {
        lastReducedDraw = now;
        draw(rawDt);
      }
      if (voice || effect) schedule();
      return;
    }
    // Stall clamp, widened so a low cap (e.g. 8 fps) isn't mistaken for a stall.
    elapsed += Math.min(rawDt, Math.max(MAX_DT_S, perf.maxFps ? 1.5 / perf.maxFps : 0));
    draw(rawDt);
    schedule();
  }

  function schedule(): void {
    if (!scheduled && isRunning() && win?.requestAnimationFrame) {
      scheduled = true;
      requestFrame(win, tick);
    }
  }

  function refresh(): void {
    // Re-evaluate running state after any change: draw a still frame now
    // (so a paused or reduced-motion view isn't blank), then (re)start the loop.
    if (destroyed) return;
    if (!isRunning() && scheduled) {
      if (win) cancelFrame(win, tick);
      scheduled = false;
    }
    if (!isRunning()) lastTick = null;
    // Only a view someone can see is drawn: an app that feeds a level into a view
    // scrolled away or in a background tab would otherwise paint every update.
    // Becoming visible again comes back through here (the IntersectionObserver,
    // visibilitychange), which draws the latest state then.
    if (onScreen && !doc?.hidden) draw(0);
    schedule();
  }

  const ro =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver((entries) => {
          const box = entries[0]?.contentRect;
          if (!box || box.width === 0 || box.height === 0) return; // hidden: keep the last real size
          cssW = box.width;
          cssH = box.height;
          resizeBacking();
          if (isSmall() !== wasSmall) applyPerformance();
          draw(0);
        });
  ro?.observe(canvas);
  const io =
    typeof IntersectionObserver === "undefined"
      ? null
      : new IntersectionObserver((entries) => {
          onScreen = entries[entries.length - 1]?.isIntersecting ?? true;
          refresh();
        });
  io?.observe(canvas);
  const onVisibility = () => refresh();
  doc?.addEventListener("visibilitychange", onVisibility);
  const onMq = () => refresh();
  darkMq?.addEventListener?.("change", onMq);
  motionMq?.addEventListener?.("change", onMq);

  // Engine space is the square `draw` centres in the canvas box, 0..size on each side
  // (for a box-layout pattern: the whole box, `size` tall).
  const onPointerMove = (e: PointerEvent) => {
    if (!resolved) return;
    const r = canvas.getBoundingClientRect();
    const side = boxLayout ? r.height : Math.min(r.width, r.height);
    if (!side) return;
    const ox = boxLayout ? 0 : (r.width - side) / 2;
    const oy = boxLayout ? 0 : (r.height - side) / 2;
    pointer.x = ((e.clientX - r.left - ox) / side) * resolved.size;
    pointer.y = ((e.clientY - r.top - oy) / side) * resolved.size;
    pointer.active = true;
  };
  const onPointerLeave = () => {
    pointer.active = false;
  };
  // A mouse button going up doesn't end a hover; a finger lifting does.
  const onPointerUp = (e: PointerEvent) => {
    if (e.pointerType !== "mouse") pointer.active = false;
  };
  // Tap to hop: the tap's place in the drawn square, -1..1 from its centre.
  const onTap = (e: PointerEvent) => {
    const r = canvas.getBoundingClientRect();
    const side = boxLayout ? r.height : Math.min(r.width, r.height);
    if (!side) return;
    const ox = boxLayout ? 0 : (r.width - side) / 2;
    const oy = boxLayout ? 0 : (r.height - side) / 2;
    const c = (v: number) => Math.max(-1, Math.min(1, v));
    hop({ x: c(((e.clientX - r.left - ox) / side) * 2 - 1), y: c(((e.clientY - r.top - oy) / side) * 2 - 1) });
  };
  let tapBound = false;
  function applyTap(): void {
    const want = !!opts.tap && !destroyed;
    if (want === tapBound) return;
    if (want) canvas.addEventListener("pointerdown", onTap);
    else canvas.removeEventListener("pointerdown", onTap);
    tapBound = want;
  }
  function applyPointer(): void {
    const want = !!opts.pointer && !destroyed;
    if (want === pointerBound) return;
    if (want) {
      canvas.addEventListener("pointermove", onPointerMove);
      canvas.addEventListener("pointerdown", onPointerMove);
      canvas.addEventListener("pointerup", onPointerUp);
      canvas.addEventListener("pointerleave", onPointerLeave);
      canvas.addEventListener("pointercancel", onPointerLeave);
    } else {
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerdown", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("pointerleave", onPointerLeave);
      canvas.removeEventListener("pointercancel", onPointerLeave);
      pointer.active = false;
      pointer.strength = 0;
    }
    pointerBound = want;
  }

  applyInput();
  applyPointer();
  applyTap();
  applyExpression();
  resizeBacking();
  refresh();

  return {
    update(next) {
      // The lifecycle state / inputs / voiceLevelInput are read every frame --
      // changing them must not rebuild the player (that would drop a transition).
      const before = patternOf(opts);
      opts = { ...opts, ...next };
      warnPlainState(opts);
      const inputChanged =
        "spec" in next || patternOf(opts) !== before || "size" in next || "overrides" in next || "speed" in next || "crossFade" in next;
      if (inputChanged) applyInput();
      else {
        if ("voice" in next || "voiceOptions" in next) {
          if ("voiceOptions" in next) unbindVoice();
          applyVoice();
        }
        if ("inputs" in next || "rules" in next || "voice" in next) applyRules();
        if ("label" in next || "labels" in next) applyA11y();
        if ("maxFps" in next || "lowPower" in next) applyPerformance();
      }
      if ("pointer" in next) applyPointer();
      if ("tap" in next) applyTap();
      if ("expression" in next) applyExpression();
      if ("labels" in next || "announce" in next) a11yState = undefined; // re-word the current state
      refreshA11y();
      refresh();
    },
    trigger(name: string) {
      if (destroyed) return;
      const info = effectInfo(name);
      if (!info) {
        if (!warnedEffect) console.warn(`SinuaView: no effect named "${name}" (success, error, celebrate, hop)`);
        warnedEffect = true;
        return;
      }
      if (info.code === HOP?.code) return hop(null);
      tapAt = null;
      effect = { code: info.code, duration: info.duration, start: performance.now() / 1000 };
      // An effect is an event: spoken now, outside the state rate limit.
      const base = a11yBase();
      if (base !== "" && (opts.announce ?? a11yInfo.announce ?? true)) speakText(opts.labels?.[`effect:${name}`] ?? info.words);
      refresh();
    },
    pause() {
      manualPause = true;
      refresh();
    },
    resume() {
      manualPause = false;
      refresh();
    },
    destroy() {
      destroyed = true;
      unbindVoice();
      if (announceTimer != null) clearTimeout(announceTimer);
      announceTimer = null;
      liveRegion?.remove();
      liveRegion = null;
      applyPointer(); // destroyed: unbinds
      applyTap();
      if (scheduled && win) cancelFrame(win, tick);
      scheduled = false;
      ro?.disconnect();
      io?.disconnect();
      doc?.removeEventListener("visibilitychange", onVisibility);
      darkMq?.removeEventListener?.("change", onMq);
      motionMq?.removeEventListener?.("change", onMq);
    },
    get voice() {
      return voice;
    },
    get elapsed() {
      return elapsed;
    },
  };
}
