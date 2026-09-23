import {
  FxSpecPlayer,
  VoiceOverrides,
  frameWithOverridesPacked,
  resolveFxSpec,
  resolvedOpts,
  voiceStateProfile,
  type FxDiagnostic,
  type FxSpec,
  type OrbFrame,
  type OrbSize,
  type PackedFrame,
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
   * changes cross-fade (250 ms, `FxSpecPlayer`).
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
   * you -- note a source holds one metrics/state callback, so the view
   * takes them (read `handle.voice` for a meter or a label). If your app
   * already listens to the source, pass a `VoiceOverrides` you feed
   * yourself instead. The view never connects or disconnects the source.
   */
  voice?: VoiceSource | VoiceOverrides | null;
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
  /** Spec state cross-fade length, seconds. Default 0.25; 0 = cut. */
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
}

/** spec/orbs-spec.json `paint`: devicePixelRatioCap and the reduced-motion pose. */
export const DPR_CAP = 2;
export const REDUCED_MOTION_T = 0.6;
const MAX_DT_S = 0.1;
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
  const specSpeeds = new Map<string, number>();
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

  /** The lifecycle state now: the app's, else the bound voice source's. */
  function lifecycleNow(): string | undefined {
    return lifecycleStateOf(opts) ?? (voice ? voice.state : undefined);
  }
  let lastTick: number | null = null;
  let lastReducedDraw = -Infinity;
  let raf: number | null = null;
  let destroyed = false;
  let manualPause = false;
  let onScreen = true;
  let cssW = canvas.clientWidth || canvas.width;
  let cssH = canvas.clientHeight || canvas.height;

  const win: (Window & typeof globalThis) | undefined = typeof window === "undefined" ? undefined : window;
  const doc: Document | undefined = typeof document === "undefined" ? undefined : document;
  const darkMq = win?.matchMedia?.("(prefers-color-scheme: dark)");
  const motionMq = win?.matchMedia?.("(prefers-reduced-motion: reduce)");

  const isDark = () => (opts.theme === "dark" ? true : opts.theme === "light" ? false : !!darkMq?.matches);
  const isReduced = () => (opts.reducedMotion === "always" ? true : opts.reducedMotion === "never" ? false : !!motionMq?.matches);
  const isRunning = () => !destroyed && !manualPause && !opts.paused && onScreen && !doc?.hidden;

  function applyInput(): void {
    const r = resolveInput(opts);
    if ("diagnostics" in r) {
      resolved = null;
      opts.onError?.(r.diagnostics);
    } else {
      resolved = r.value;
    }
    specSpeeds.clear();
    player =
      resolved && opts.spec != null ? new FxSpecPlayer(asSpec(opts.spec), { crossFade: opts.crossFade, lowPower: !!opts.lowPower }) : null;
    applyPerformance();
    applyVoice();
    applyA11y();
  }

  function applyPerformance(): void {
    const lowPower = !!opts.lowPower;
    player?.setLowPower(lowPower);
    specSpeeds.clear(); // low power can change a state's resolved speed
    // FX Spec 1.2: the resolver reports the cap for this power state (`maxFps`,
    // the lowPower one when set) and sheds the spec's `lowPower.disable` itself.
    const specMaxFps = opts.spec != null && resolved ? resolveFxSpec(asSpec(opts.spec), { lowPower }).maxFps : null;
    perf = performanceFor({ lowPower, optionMaxFps: opts.maxFps, specMaxFps, specHandlesLowPower: resolved?.specHandlesLowPower });
    pacer = createFramePacer(perf.maxFps);
  }

  function applyVoice(): void {
    const v = opts.voice ?? null;
    if (v == null) {
      voice = null;
      boundSource = null;
    } else if (isVoiceOverrides(v)) {
      voice = v;
      boundSource = null;
    } else if (v !== boundSource) {
      boundSource = v;
      voice = VoiceOverrides.bind(v, opts.voiceOptions ?? defaultVoiceOptions(resolved?.family ?? null, resolved?.overrides ?? {}));
    }
  }

  function applyA11y(): void {
    const label = opts.label ?? resolved?.name ?? resolved?.state ?? "";
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

  function resizeBacking(): void {
    const dpr = Math.min(win?.devicePixelRatio || 1, DPR_CAP);
    const w = Math.max(1, Math.round(cssW * dpr));
    const h = Math.max(1, Math.round(cssH * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
  }

  /** The engine speed of one lifecycle state of the spec (the product the player will use). */
  function specSpeed(state: string | undefined): number {
    const key = state ?? "";
    const cached = specSpeeds.get(key);
    if (cached !== undefined) return cached;
    const r = resolveFxSpec(asSpec(opts.spec!), { state, lowPower: !!opts.lowPower });
    const speed = r.ok ? (resolvedOpts(r.state as OrbState, r.size as OrbSize)?.speed ?? 1) * r.speed : 1;
    specSpeeds.set(key, speed);
    return speed;
  }

  /**
   * The engine speed right now: the preset's tuned speed times the app's multiplier
   * (0 holds the pose). With a spec it is the *current lifecycle state's* speed, which
   * is what `FxSpecPlayer` renders with.
   */
  function speedNow(state?: string): number {
    if (player && opts.spec != null) return specSpeed(state);
    if (!resolved) return 1;
    const profile = profileFor(resolved.state, state);
    return resolved.presetSpeed * resolved.speed * (profile?.speed ?? 1);
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
    const preset = resolved ? resolved.presetSpeed : 1;
    const own = resolved ? resolved.speed : 1;
    const profile = profileFor(resolved ? resolved.state : "", stateForSpeed)?.speed ?? 1;
    return phaseBase + (elapsed - elapsedBase) * preset * own * profile;
  }

  function draw(rawDt: number): void {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (!resolved) return;
    const voiceMap = voice ? voice.overrides(rawDt) : {};
    // Low-power overrides (pre-1.2 / block-less default) sit between the spec's and the voice's.
    const extra = Object.keys(perf.overrides).length ? { ...perf.overrides, ...voiceMap } : voiceMap;
    const reduced = isReduced();
    const t0 = opts.onFrame ? performance.now() : 0;
    let frame: OrbFrame | null = null;
    let packed: PackedFrame | null = null;
    let previous: OrbFrame | null = null;
    let blend = 1;
    if (player) {
      // FX Spec: FxSpecPlayer owns the v1.1 state/inputs/cross-fade and the 1.2
      // low-power resolution; runtime keys go in as extra overrides (spread last).
      const lifecycle = lifecycleStateOf(opts) ?? (voice ? voice.state : undefined);
      player.setState(lifecycle);
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
      const profile = profileFor(resolved.state, lifecycle);
      const t = reduced ? REDUCED_MOTION_T : phaseNow(lifecycle);
      const merged: Record<string, number> = profile
        ? { ...profile.overrides, ...resolved.overrides, ...extra }
        : { ...resolved.overrides, ...extra };
      // `audioInput` names which app input drives `audioLevel` here (never an engine key).
      const level = profile?.audioInput ? opts.inputs?.[profile.audioInput] : undefined;
      if (level != null) merged.audioLevel = level;
      packed = frameWithOverridesPacked(resolved.state as OrbState, resolved.size as OrbSize, t, merged);
    }
    if (!frame && !packed) return;
    const t1 = opts.onFrame ? performance.now() : 0;
    // Square engine space, centered in whatever box the canvas has.
    const side = Math.min(canvas.width, canvas.height);
    const scale = side / resolved.size;
    ctx.save();
    ctx.translate((canvas.width - side) / 2, (canvas.height - side) / 2);
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
    raf = null;
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
      if (voice && now - lastReducedDraw >= REDUCED_MOTION_VOICE_INTERVAL_MS) {
        lastReducedDraw = now;
        draw(rawDt);
      }
      if (voice) schedule();
      return;
    }
    // Stall clamp, widened so a low cap (e.g. 8 fps) isn't mistaken for a stall.
    elapsed += Math.min(rawDt, Math.max(MAX_DT_S, perf.maxFps ? 1.5 / perf.maxFps : 0));
    draw(rawDt);
    schedule();
  }

  function schedule(): void {
    if (raf == null && isRunning() && win?.requestAnimationFrame) raf = win.requestAnimationFrame(tick);
  }

  function refresh(): void {
    // Re-evaluate running state after any change: draw a still frame now
    // (so a paused or reduced-motion view isn't blank), then (re)start the loop.
    if (destroyed) return;
    if (!isRunning() && raf != null) {
      win?.cancelAnimationFrame?.(raf);
      raf = null;
    }
    if (!isRunning()) lastTick = null;
    draw(0);
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

  applyInput();
  resizeBacking();
  refresh();

  return {
    update(next) {
      // The lifecycle state / inputs / voiceLevelInput are read every frame --
      // changing them must not rebuild the player (that would drop a cross-fade).
      const before = patternOf(opts);
      opts = { ...opts, ...next };
      warnPlainState(opts);
      const inputChanged =
        "spec" in next || patternOf(opts) !== before || "size" in next || "overrides" in next || "speed" in next || "crossFade" in next;
      if (inputChanged) applyInput();
      else {
        if ("voice" in next || "voiceOptions" in next) {
          if ("voiceOptions" in next) boundSource = null;
          applyVoice();
        }
        if ("label" in next) applyA11y();
        if ("maxFps" in next || "lowPower" in next) applyPerformance();
      }
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
      if (raf != null) win?.cancelAnimationFrame?.(raf);
      raf = null;
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
