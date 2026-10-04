// The recommended caller loop for an FX Spec v1.1 file, packaged: the
// engine is stateless by design, so everything that needs memory lives
// here, caller-side -- which lifecycle state is current, the transition
// when it changes (./transition.ts), and optional easing of the app's
// input values. Same
// ideas as `VoiceOverrides` (./voice.ts) and `ReactiveBinding`
// (./reactive.ts); the spec itself (parse, merge, bindings) stays in Rust.
//
//   const player = new FxSpecPlayer(specJson, { inputEaseRate: { heartRate: 4 } });
//   agent.onStateChange((s) => player.setState(s));       // "listening", "speaking", ...
//   mic.onLevel((v) => player.setInput("micLevel", v));
//   // per rendered frame:
//   const { frame, previous, blend } = player.frame(elapsed, dt);
//   previous ? drawCrossDissolve(ctx, previous, frame, blend) : draw(ctx, frame);
//
// State changes animate through the engine's transition system: the same
// pattern interpolates its parameters, the lattice trio morphs, anything else
// cross-fades. Duration and curve come from the spec's 1.9 `transitions`
// (default 0.6 s, easeInOut).

import { applyLoadout, fxSpecTransition, resolveFxSpec, resolvedOpts, type Loadout } from "./engine.js";
import type { FxSpec, FxSpecResolved, OrbFrame, OrbSize, OrbState, TransitionSide } from "./index.js";
import { StateTransition } from "./transition.js";

export interface FxSpecPlayerOptions {
  /**
   * Overrides every state change's duration, in seconds (`0` = a cut). Unset:
   * the spec's `transitions` (default 0.6 s).
   */
  crossFade?: number;
  /**
   * Per-input easing rate (per second, `k = min(1, rate * dt)`, the
   * `VoiceOverrides`/`ReactiveBinding` approach). Inputs not listed are
   * used as-is. The first value set for an input is taken without a ramp.
   */
  inputEaseRate?: Record<string, number>;
  /** Start in low power (see `setLowPower`). */
  lowPower?: boolean;
  /** An end user's loadout (FX Spec 1.13, design note 25; see `setLoadout`). */
  loadout?: Loadout | null;
}

export interface FxPlayerFrame {
  /** The current state's frame (`null` if the spec has errors). */
  frame: OrbFrame | null;
  /** While a transition needs two frames: the one to dissolve from, else `null`. */
  previous: OrbFrame | null;
  /** Weight of `frame` over `previous`, `0..1` (1 when there's no `previous`). */
  blend: number;
  /** The resolution behind `frame` (diagnostics, inactive bindings, state key). */
  resolved: FxSpecResolved;
}

/** Same stall clamp as `VoiceOverrides`, `ReactiveBinding` and the Studios. */
const MAX_DT_S = 0.1;

export class FxSpecPlayer {
  /** The file as given. */
  private readonly file: string;
  /** The file with the loadout applied (what is resolved). */
  private spec: string;
  private loadoutWarnings: FxSpecResolved["diagnostics"] = [];
  private readonly fadeS: number | undefined;
  /**
   * The engine time: pinned at each speed change and run from there, so a state with
   * its own speed (or a mixed speed mid-transition) doesn't jump the pose. With one
   * speed it is exactly `elapsed × speed`.
   */
  private phaseBase = 0;
  private elapsedBase = 0;
  private phaseSpeed: number | null = null;
  private lastElapsed = 0;
  private readonly rates: Record<string, number>;
  private readonly goals = new Map<string, number>();
  private readonly values = new Map<string, number>();
  private current: string | undefined;
  private readonly transition = new StateTransition();
  private lowPower: boolean;

  constructor(spec: FxSpec | string, opts: FxSpecPlayerOptions = {}) {
    this.file = typeof spec === "string" ? spec : JSON.stringify(spec);
    this.spec = this.file;
    if (opts.loadout) this.applyLoadout(opts.loadout);
    this.fadeS = opts.crossFade == null ? undefined : Math.max(0, opts.crossFade);
    this.rates = { ...opts.inputEaseRate };
    this.lowPower = opts.lowPower ?? false;
  }

  /**
   * The host's power state (iOS Low Power Mode, Android Battery Saver, an
   * app policy): applies the spec's 1.2 `performance.lowPower` -- shed
   * materials and the lower cap, reported as `resolved.maxFps`. Pace your
   * render loop to `resolved.maxFps` (null = your default).
   */
  setLowPower(on: boolean): void {
    this.lowPower = on;
  }

  /**
   * An end user's loadout (FX Spec 1.13, design note 25): what they wear from the spec's
   * `wardrobe`, a palette and an eye style. A change eases (a hat pops in, colours blend)
   * on its own clock; `null` goes back to the file as it is. What the spec no longer
   * offers is skipped (see `loadoutDiagnostics`), the rest applies.
   */
  setLoadout(loadout: Loadout | null): void {
    const before = this.spec;
    this.applyLoadout(loadout);
    if (this.spec !== before) this.transition.wear();
  }

  /** The warnings from applying the loadout (stale items, unknown palettes). */
  get loadoutDiagnostics(): FxSpecResolved["diagnostics"] {
    return this.loadoutWarnings;
  }

  private applyLoadout(loadout: Loadout | null): void {
    if (!loadout) {
      this.spec = this.file;
      this.loadoutWarnings = [];
      return;
    }
    const r = applyLoadout(this.file, loadout);
    this.spec = r.spec;
    this.loadoutWarnings = r.diagnostics;
  }

  /** The lifecycle key now (a `states` key; `undefined` = the top-level design). */
  get state(): string | undefined {
    return this.current;
  }

  /** Switch lifecycle state; starts a transition from what is showing now. */
  setState(key: string | undefined): void {
    if (key === this.current) return;
    const t = fxSpecTransition(this.spec, this.current, key);
    this.transition.start(this.fadeS ?? t.duration, t.curve, this.fadeS === undefined && t.authored);
    this.current = key;
  }

  private phase(elapsed: number, speed: number): number {
    if (elapsed < this.lastElapsed) this.phaseSpeed = null;
    if (this.phaseSpeed === null) {
      this.phaseBase = 0;
      this.elapsedBase = 0;
    } else if (this.phaseSpeed !== speed) {
      this.phaseBase += (this.lastElapsed - this.elapsedBase) * this.phaseSpeed;
      this.elapsedBase = this.lastElapsed;
    }
    this.phaseSpeed = speed;
    this.lastElapsed = elapsed;
    return this.phaseBase + (elapsed - this.elapsedBase) * speed;
  }

  /** End a running transition now (reduced motion): the next frame is the current state. */
  skipTransition(): void {
    this.transition.cancel();
  }

  /**
   * The effective speed multiplier (preset x spec, mixed mid-transition) the
   * next frame renders at: a view integrating its own phase runs at this.
   */
  speed(): number {
    const side = this.side(this.inputs);
    return side ? this.transition.speed(side.side, side.size) : 1;
  }

  /** Report an app input value (`micLevel`, `steps`, `heartRate`, ...). */
  setInput(name: string, value: number): void {
    this.goals.set(name, value);
    if (!this.values.has(name)) this.values.set(name, value);
  }

  /** Forget an input: its bindings go inactive (their static/engine value). */
  clearInput(name: string): void {
    this.goals.delete(name);
    this.values.delete(name);
  }

  /** The eased input values as passed to the spec right now. */
  get inputs(): Record<string, number> {
    return Object.fromEntries(this.values);
  }

  /**
   * Render at `elapsed` wall-clock seconds; `dt` (seconds since the last
   * call) advances input easing and the cross-fade. `extraOverrides` are
   * runtime keys the spec doesn't own -- e.g. `VoiceOverrides`' spectrum
   * bands and `voiceStateCode` -- spread last.
   */
  frame(elapsed: number, dt: number, extraOverrides: Record<string, number> = {}): FxPlayerFrame {
    const step = Math.max(0, Math.min(dt, MAX_DT_S));
    for (const [name, goal] of this.goals) {
      const rate = this.rates[name] ?? 0;
      const cur = this.values.get(name) ?? goal;
      this.values.set(name, rate > 0 ? cur + (goal - cur) * Math.min(1, rate * step) : goal);
    }
    this.transition.advance(step);
    const now = this.side(this.inputs);
    if (!now) {
      this.transition.cancel();
      return { frame: null, previous: null, blend: 1, resolved: this.resolve(this.inputs) };
    }
    const t = this.phase(elapsed, this.transition.speed(now.side, now.size));
    const out = this.transition.frames(now.side, now.size, t, extraOverrides);
    return { ...out, resolved: now.resolved };
  }

  private resolve(inputs: Record<string, number>): FxSpecResolved {
    return resolveFxSpec(this.spec, { state: this.current, inputs, lowPower: this.lowPower });
  }

  /** The current state as a transition side (effective speed), or null if the spec has errors. */
  private side(inputs: Record<string, number>): { side: TransitionSide; size: OrbSize; resolved: FxSpecResolved } | null {
    const resolved = this.resolve(inputs);
    if (!resolved.ok) return null;
    const size = resolved.size as OrbSize;
    const preset = resolvedOpts(resolved.state as OrbState, size);
    return {
      side: { state: resolved.state, speed: (preset?.speed ?? 1) * resolved.speed, overrides: resolved.overrides },
      size,
      resolved,
    };
  }
}
