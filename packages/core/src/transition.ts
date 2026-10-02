// State transitions, caller side (docs/fx-spec.md, *Transitions*). The engine
// says what to draw at an instant (`transitionMix`); this keeps the clock and
// what was on screen, so a change mid-transition starts from there. Used by
// `FxSpecPlayer` and by the web view's plain path; the iOS and Android views
// follow the same steps.
//
//   const tr = new StateTransition();
//   onStateChange: tr.start(duration, curve)      // before the new side renders
//   loadout change: tr.wear()                    // before the new loadout renders
//   per frame:     tr.advance(dt);
//                  const out = tr.frames(toSide, size, t, extra) // { frame, previous, blend }

import { frameTransitionWithOverrides, frameWithOverrides, transitionMix } from "./engine.js";
import type { OrbFrame, OrbSize, OrbState, TransitionSide } from "./index.js";

/** What to paint: `previous` dissolved into `frame` at `blend` (no `previous` = just `frame`). */
export interface TransitionFrames {
  frame: OrbFrame | null;
  previous: OrbFrame | null;
  blend: number;
}

/** How long a loadout change takes (design note 25), seconds. */
export const WEAR_S = 0.35;

export class StateTransition {
  private from: TransitionSide | null = null;
  /** What was on screen when the loadout last changed, while its change runs. */
  private wearFrom: TransitionSide | null = null;
  private wearAge = Infinity;
  private shown: TransitionSide | null = null;
  private age = Infinity;
  /** Seconds since the last state change (any change, cut or not); infinite before the first. */
  private since = Infinity;
  private duration = 0;
  private curve = "easeInOut";

  /** A state change happened: animate from what is on screen now. `duration` 0 = a cut. */
  start(duration: number, curve: string): void {
    this.from = this.shown;
    // A change of something already on screen; the first state isn't a change.
    if (this.shown) this.since = 0;
    this.duration = Math.max(0, duration);
    this.curve = curve;
    this.age = this.from && this.duration > 0 ? 0 : Infinity;
  }

  /**
   * The loadout changed (design note 25): the character eases from what is on screen
   * now -- a hat pops in, colours blend, a new eye style swaps in a blink. It runs on its
   * own clock, so a state change in the middle of it doesn't cut it, and the reverse.
   */
  wear(): void {
    if (!this.shown) return;
    this.wearFrom = this.shown;
    this.wearAge = 0;
  }

  /** Stop any transition now (reduced motion, a new design). */
  cancel(): void {
    this.age = Infinity;
    this.from = null;
    this.wearAge = Infinity;
    this.wearFrom = null;
  }

  /** No transition running: remember `to` as what is on screen (a caller that draws `to` itself). */
  settle(to: TransitionSide): void {
    if (!this.active) this.shown = to;
  }

  advance(dt: number): void {
    this.age += Math.max(0, dt);
    this.since += Math.max(0, dt);
    this.wearAge += Math.max(0, dt);
    if (this.wearAge >= WEAR_S) this.wearFrom = null;
  }

  /**
   * Seconds since the lifecycle state last changed, or `null` before the first
   * change. The frames carry it as the `stateAge` runtime key (a character
   * blinks at the end of the user's turn); other patterns ignore it.
   */
  get stateAge(): number | null {
    return Number.isFinite(this.since) ? this.since : null;
  }

  /** A loadout change is easing in (`wear`). */
  get wearing(): boolean {
    return this.wearFrom !== null;
  }

  get active(): boolean {
    return this.from !== null && this.age < this.duration;
  }

  /** The speed multiplier to run the phase at now, for `to` (the mixed speed mid-transition). */
  speed(to: TransitionSide, size: OrbSize): number {
    if (!this.active || !this.from) return to.speed;
    return transitionMix(this.from, to, size, this.age / this.duration, this.curve)?.speed ?? to.speed;
  }

  /**
   * The frames for `to` at engine time `t`. `extra` is the live runtime keys
   * (audio, pointer), spread over both sides.
   */
  frames(to: TransitionSide, size: OrbSize, t: number, live: Record<string, number> = {}): TransitionFrames {
    const extra = Number.isFinite(this.since) ? { ...live, stateAge: this.since } : live;
    const wearFrom = this.wearFrom;
    const plain = (side: TransitionSide) => frameWithOverrides(side.state as OrbState, size, t, { ...side.overrides, ...extra });
    // A loadout change in progress draws the new side easing from the old one (the
    // engine answers only for two loadouts of one character; anything else draws plain).
    const draw = (side: TransitionSide) =>
      (wearFrom &&
        frameTransitionWithOverrides(
          { ...wearFrom, overrides: { ...wearFrom.overrides, ...extra } },
          { ...side, overrides: { ...side.overrides, ...extra } },
          size,
          t,
          this.wearAge / WEAR_S
        )) ||
      plain(side);
    const from = this.from;
    const mix = from && this.active ? transitionMix(from, to, size, this.age / this.duration, this.curve) : null;
    if (!from || !mix) {
      this.shown = to;
      return { frame: draw(to), previous: null, blend: 1 };
    }
    if (mix.technique === "params") {
      const base = { state: to.state, speed: mix.speed, overrides: mix.overrides };
      const swapped = { ...base, overrides: { ...mix.overrides, ...mix.structuralTo } };
      const structural = Object.keys(mix.structuralTo).length > 0;
      this.shown = structural && mix.swap >= 0.5 ? swapped : base;
      if (!structural || mix.swap <= 0) return { frame: draw(base), previous: null, blend: 1 };
      if (mix.swap >= 1) return { frame: draw(swapped), previous: null, blend: 1 };
      return { frame: draw(swapped), previous: draw(base), blend: mix.swap };
    }
    this.shown = to;
    if (mix.technique === "morph") {
      const withExtra = (s: TransitionSide) => ({ ...s, overrides: { ...s.overrides, ...extra } });
      const frame = frameTransitionWithOverrides(withExtra(from), withExtra(to), size, t, mix.weight);
      if (frame) return { frame, previous: null, blend: 1 };
    }
    return { frame: draw(to), previous: draw(from), blend: mix.weight };
  }
}
