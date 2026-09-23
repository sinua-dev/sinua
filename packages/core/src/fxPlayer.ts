// The recommended caller loop for an FX Spec v1.1 file, packaged: the
// engine is stateless by design, so everything that needs memory lives
// here, caller-side -- which lifecycle state is current, the cross-fade
// when it changes, and optional easing of the app's input values. Same
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
// The cross-fade is the Studio's: 250 ms, cubic ease-out (LiveKit's
// audio-visualizer `0.25s ease-out` state transition, SignalStudio.tsx).

import { frameWithOverrides, resolveFxSpec, resolvedOpts } from "./engine.js";
import type { FxSpec, FxSpecResolved, OrbFrame, OrbSize, OrbState } from "./index.js";

export interface FxSpecPlayerOptions {
  /** State cross-fade length in seconds. Default `0.25`; `0` = cut. */
  crossFade?: number;
  /**
   * Per-input easing rate (per second, `k = min(1, rate * dt)`, the
   * `VoiceOverrides`/`ReactiveBinding` approach). Inputs not listed are
   * used as-is. The first value set for an input is taken without a ramp.
   */
  inputEaseRate?: Record<string, number>;
  /** Start in low power (see `setLowPower`). */
  lowPower?: boolean;
}

export interface FxPlayerFrame {
  /** The current state's frame (`null` if the spec has errors). */
  frame: OrbFrame | null;
  /** While a state change is fading: the previous state's frame, else `null`. */
  previous: OrbFrame | null;
  /** Weight of `frame` over `previous`, `0..1` (1 when not fading). */
  blend: number;
  /** The resolution behind `frame` (diagnostics, inactive bindings, state key). */
  resolved: FxSpecResolved;
}

/** Same stall clamp as `VoiceOverrides`, `ReactiveBinding` and the Studios. */
const MAX_DT_S = 0.1;

export class FxSpecPlayer {
  private readonly spec: string;
  private readonly fadeS: number;
  private readonly rates: Record<string, number>;
  private readonly goals = new Map<string, number>();
  private readonly values = new Map<string, number>();
  private current: string | undefined;
  private previous: string | undefined;
  private fadeAge = Infinity;
  private lowPower: boolean;

  constructor(spec: FxSpec | string, opts: FxSpecPlayerOptions = {}) {
    this.spec = typeof spec === "string" ? spec : JSON.stringify(spec);
    this.fadeS = Math.max(0, opts.crossFade ?? 0.25);
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

  /** The lifecycle key now (a `states` key; `undefined` = the top-level design). */
  get state(): string | undefined {
    return this.current;
  }

  /** Switch lifecycle state; starts a cross-fade from the one showing now. */
  setState(key: string | undefined): void {
    if (key === this.current) return;
    this.previous = this.current;
    this.current = key;
    this.fadeAge = this.fadeS > 0 ? 0 : Infinity;
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
    this.fadeAge += step;
    const inputs = this.inputs;
    const now = this.render(this.current, elapsed, inputs, extraOverrides);
    let previous: OrbFrame | null = null;
    let blend = 1;
    if (this.fadeAge < this.fadeS) {
      previous = this.render(this.previous, elapsed, inputs, extraOverrides).frame;
      const u = this.fadeAge / this.fadeS;
      blend = 1 - Math.pow(1 - u, 3);
    }
    return { frame: now.frame, previous, blend, resolved: now.resolved };
  }

  private render(
    state: string | undefined,
    elapsed: number,
    inputs: Record<string, number>,
    extra: Record<string, number>
  ): { frame: OrbFrame | null; resolved: FxSpecResolved } {
    const resolved = resolveFxSpec(this.spec, { state, inputs, lowPower: this.lowPower });
    if (!resolved.ok) return { frame: null, resolved };
    const st = resolved.state as OrbState;
    const size = resolved.size as OrbSize;
    const preset = resolvedOpts(st, size);
    const t = elapsed * (preset?.speed ?? 1) * resolved.speed;
    return { frame: frameWithOverrides(st, size, t, { ...resolved.overrides, ...extra }), resolved };
  }
}
