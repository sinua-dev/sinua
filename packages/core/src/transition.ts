// State transitions, caller side (docs/fx-spec.md, *Transitions* and *The transition
// contract*; design note 31). The engine says what to draw for a set of weighted sides
// (`voiceBlend`); this keeps the clock: a weight per state that moves continuously, so a
// change mid-transition heads somewhere else from where it is, with no kink. Used by
// `FxSpecPlayer` and by the web view's plain path; the iOS and Android views follow the
// same steps (and `spec/transition-timeline.json` holds them to it).
//
//   const tr = new StateTransition();
//   onStateChange: tr.start(duration, curve, authored) // before the new side renders
//   loadout change: tr.wear()                           // before the new loadout renders
//   per frame:     tr.advance(dt);
//                  phase += dt * tr.speed(toSide, size);
//                  const out = tr.frames(toSide, size, phase, extra) // { frame, previous, blend }
import { frameTransitionWithOverrides, frameWithOverrides, transitionMix, voiceBlend } from "./engine.js";
import type { OrbFrame, OrbSize, OrbState, TransitionMix, TransitionSide } from "./index.js";

/** What to paint: `previous` dissolved into `frame` at `blend` (no `previous` = just `frame`). */
export interface TransitionFrames {
  frame: OrbFrame | null;
  previous: OrbFrame | null;
  blend: number;
}

/** How long a loadout change takes (design note 25), seconds. */
export const WEAR_S = 0.35;

/**
 * The transition clock (design note 31): three first-order lags in a row per weight, so
 * a weight's position, velocity and acceleration stay continuous through any change,
 * and the motion looks the same at any frame rate. `ωt ≈ 6.3` is ~95 % of the way, so a
 * change's `duration` sets `ω = 6.3 / duration`.
 */
export const LAG_95 = 6.3;
/** A frame's step is capped here, as the views cap the phase and the inputs. */
export const MAX_DT_S = 0.1;
/**
 * A gap longer than this (back from the background, a restored tab): the weights land
 * on the target at once. The user didn't watch the middle of the change.
 */
export const SETTLE_GAP_S = 1;
/** Below this a state that is no longer the target is dropped. */
const GONE = 1e-4;
/** The lattice-sharing orb patterns (the engine morphs them point by point). */
const LATTICE = ["glowing", "calibrating", "progressing"];

interface Entry {
  /** Its side: the latest `to` for the target, a snapshot for the others. */
  side: TransitionSide | null;
  /** Weight and the two lags behind it. */
  x: number;
  l1: number;
  l2: number;
  /** Velocity (per second), for an authored curve's carry-over. */
  v: number;
  /** An authored curve's start (weight, velocity). */
  x0: number;
  v0: number;
}

/** The rate keys of one pattern: the last rate seen, and the accumulated cycles once one changed. */
interface Rates {
  last: Record<string, number>;
  acc: Record<string, number>;
}

export class StateTransition {
  private entries: Entry[] = [];
  private omega = LAG_95 / 0.6;
  /** An authored curve (the file wrote it): eased with velocity carried, over `duration`. */
  private authored: { curve: string; duration: number; age: number } | null = null;
  private size: OrbSize = 64;
  private rates = new Map<string, Rates>();
  private lastT: number | null = null;
  /** What was on screen when the loadout last changed, while its change runs. */
  private wearFrom: TransitionSide | null = null;
  private wearAge = Infinity;
  /** Seconds since the last state change (any change, cut or not); infinite before the first. */
  private since = Infinity;
  /** The side last drawn (`frames`' `to`, or `settle`'s). */
  private shown: TransitionSide | null = null;

  /**
   * A state change happened: the weights head for the new state from where they are.
   * `duration` 0 = a cut. `authored`: the file wrote `curve` for this change, so it is
   * kept (the motion's velocity carried into it); otherwise the lag clock, ~95 % of the
   * way in `duration`.
   */
  start(duration: number, curve: string, authored = false): void {
    if (this.entries.length) this.since = 0;
    if (!(duration > 0) || !this.entries.length) {
      this.entries = [entry(null, 1)];
      this.authored = null;
      return;
    }
    this.entries.push(entry(null, 0));
    this.omega = LAG_95 / duration;
    this.authored = authored ? { curve, duration, age: 0 } : null;
    for (const e of this.entries) {
      e.x0 = e.x;
      e.v0 = e.v;
    }
  }

  /**
   * The loadout changed (design note 25): the character eases from what is on screen
   * now -- a hat pops in, colours blend, a new eye style swaps in a blink. It runs on its
   * own clock, so a state change in the middle of it doesn't cut it, and the reverse.
   */
  wear(): void {
    const shown = this.shown;
    if (!shown) return;
    this.wearFrom = shown;
    this.wearAge = 0;
  }

  /** Stop any transition now (reduced motion, a new design). */
  cancel(): void {
    const t = this.target;
    this.entries = t ? [entry(t.side, 1)] : [];
    this.authored = null;
    this.wearAge = Infinity;
    this.wearFrom = null;
  }

  /** No transition running: remember `to` as what is on screen (a caller that draws `to` itself). */
  settle(to: TransitionSide): void {
    if (!this.active) this.entries = [entry(to, 1)];
    this.shown = to;
  }

  advance(dt: number): void {
    const raw = Math.max(0, dt);
    this.since += raw;
    this.wearAge += raw;
    if (this.wearAge >= WEAR_S) this.wearFrom = null;
    if (raw > SETTLE_GAP_S) {
      const t = this.target;
      if (t) this.entries = [entry(t.side, 1)];
      this.authored = null;
      this.rates.clear();
      return;
    }
    const step = Math.min(raw, MAX_DT_S);
    if (this.entries.length < 2 || step === 0) return;
    const target = this.target;
    const a = this.authored;
    if (a) {
      a.age += step;
      const s = Math.min(1, a.age / a.duration);
      const eased = this.ease(a.curve, s);
      // Hermite's velocity term: the old velocity, fading out over the change.
      const carry = s * s * s - 2 * s * s + s;
      for (const e of this.entries) {
        const goal = e === target ? 1 : 0;
        const x = s >= 1 ? goal : e.x0 + (goal - e.x0) * eased + e.v0 * a.duration * carry;
        e.v = (x - e.x) / step;
        e.x = e.l1 = e.l2 = x;
      }
      if (s >= 1) this.authored = null;
    } else {
      const k = 1 - Math.exp(-this.omega * step);
      for (const e of this.entries) {
        const goal = e === target ? 1 : 0;
        const before = e.x;
        e.l1 += (goal - e.l1) * k;
        e.l2 += (e.l1 - e.l2) * k;
        e.x += (e.l2 - e.x) * k;
        e.v = (e.x - before) / step;
      }
    }
    this.entries = this.entries.filter((e) => e === target || e.x >= GONE || e.l1 >= GONE);
    if (this.entries.length === 1 && target) {
      target.x = target.l1 = target.l2 = 1;
      target.v = 0;
    }
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

  /** More than one state is on screen. */
  get active(): boolean {
    return this.entries.length > 1;
  }

  /** The state weights, oldest first (the newest is the target); for tests and tools. */
  get weights(): number[] {
    return this.entries.map((e) => e.x);
  }

  /** The rate sums the view keeps for `pattern` (empty until one of its rates changed); for tests and tools. */
  rateSums(pattern: string): Record<string, number> {
    return { ...(this.rates.get(pattern)?.acc ?? {}) };
  }

  /** The speed multiplier to run the phase at now, for `to` (the weighted speed mid-transition). */
  speed(to: TransitionSide, _size?: OrbSize): number {
    if (!this.active) return to.speed;
    let sum = 0;
    let total = 0;
    for (const e of this.entries) {
      const side = e === this.target ? to : e.side;
      if (!side) continue;
      sum += e.x * side.speed;
      total += e.x;
    }
    return total > 0 ? sum / total : to.speed;
  }

  /**
   * The frames for `to` at engine time `t`. `live` is the live runtime keys
   * (audio, pointer), spread over every side.
   */
  frames(to: TransitionSide, size: OrbSize, t: number, live: Record<string, number> = {}): TransitionFrames {
    this.size = size;
    if (!this.entries.length) this.entries = [entry(to, 1)];
    const target = this.target!;
    target.side = to;
    this.shown = to;
    const extra = Number.isFinite(this.since) ? { ...live, stateAge: this.since } : live;
    const dp = this.tick(t);

    // One group per pattern; within it the engine blends the sides by weight.
    const groups = new Map<string, Entry[]>();
    for (const e of this.entries) {
      if (!e.side) continue;
      const g = groups.get(e.side.state);
      if (g) g.push(e);
      else groups.set(e.side.state, [e]);
    }
    const drawn = [...groups.entries()]
      .map(([pattern, es]) => ({ pattern, es, w: es.reduce((a, e) => a + e.x, 0) }))
      .sort((a, b) => b.w - a.w)
      .slice(0, 2);
    const blended = drawn.map((g) => this.blend(g.pattern, g.es, target, size, t, dp));
    const withExtra = (o: Record<string, number>) => ({ ...o, ...extra });
    const draw = (pattern: string, o: Record<string, number>) => this.draw(pattern, size, t, withExtra(o), extra);

    if (drawn.length === 1) {
      const { pattern } = drawn[0];
      const m = blended[0];
      const structural = Object.keys(m.mix.structuralTo).length > 0;
      if (!structural || m.mix.swap <= 0) return { frame: draw(pattern, m.overrides), previous: null, blend: 1 };
      const swapped = { ...m.overrides, ...m.mix.structuralTo };
      if (m.mix.swap >= 1) return { frame: draw(pattern, swapped), previous: null, blend: 1 };
      return { frame: draw(pattern, swapped), previous: draw(pattern, m.overrides), blend: m.mix.swap };
    }
    // Two patterns: the one holding the target fades in over the other.
    const [i, j] = drawn[0].es.includes(target) ? [1, 0] : [0, 1];
    const blend = drawn[j].w / (drawn[i].w + drawn[j].w);
    if (LATTICE.includes(drawn[i].pattern) && LATTICE.includes(drawn[j].pattern)) {
      const side = (k: number) => ({ state: drawn[k].pattern, speed: 1, overrides: withExtra(blended[k].overrides) });
      const frame = frameTransitionWithOverrides(side(i), side(j), size, t, blend);
      if (frame) return { frame, previous: null, blend: 1 };
    }
    return { frame: draw(drawn[j].pattern, blended[j].overrides), previous: draw(drawn[i].pattern, blended[i].overrides), blend };
  }

  /**
   * No transition running and the caller paints `to` itself (the web view's packed
   * path): the overrides to draw, `to`'s plus the rate sums once a rate has changed
   * (until then exactly `to.overrides`). Call it once per frame, as `frames`.
   */
  steadyOverrides(to: TransitionSide, size: OrbSize, t: number): Record<string, number> {
    this.settle(to);
    const dp = this.tick(t);
    const target = this.target!;
    const { overrides } = this.blend(to.state, [target], target, size, t, dp);
    return Object.keys(this.rates.get(to.state)?.acc ?? {}).length ? overrides : to.overrides;
  }

  /** Engine time moved to `t`: the step since the last frame (a jump back or a long gap restarts the sums). */
  private tick(t: number): number {
    const dp = this.lastT == null ? 0 : t - this.lastT;
    if (dp < 0 || dp > 1) this.rates.clear();
    this.lastT = t;
    return dp < 0 || dp > 1 ? 0 : dp;
  }

  private get target(): Entry | undefined {
    return this.entries[this.entries.length - 1];
  }

  /** A pattern's sides blended by weight, with its rate keys accumulated. */
  private blend(pattern: string, es: Entry[], target: Entry, size: OrbSize, t: number, dp: number): { mix: TransitionMix; overrides: Record<string, number> } {
    const sides = es.map((e) => e.side!);
    const ti = Math.max(0, es.indexOf(target));
    const heaviest = es.reduce((b, e, k) => (e.x > es[b].x ? k : b), 0);
    const mix =
      voiceBlend(sides, es.map((e) => e.x), es.includes(target) ? ti : heaviest, size) ??
      ({ technique: "params", weight: 1, speed: sides[0].speed, overrides: sides[0].overrides, structuralTo: {}, swap: 0, rates: {} } as TransitionMix);
    let r = this.rates.get(pattern);
    if (!r) this.rates.set(pattern, (r = { last: {}, acc: {} }));
    for (const [k, rate] of Object.entries(mix.rates)) {
      const before = r.last[k];
      if (k in r.acc) r.acc[k] += dp * rate;
      // The first change of a rate: from here the view keeps the sum (until then the
      // engine's own `t × rate` is exact, so a steady view draws what it always drew).
      else if (before !== undefined && before !== rate) r.acc[k] = (t - dp) * before + dp * rate;
      r.last[k] = rate;
    }
    return { mix, overrides: { ...mix.overrides, ...r.acc } };
  }

  /** One frame, eased from a loadout change in progress (the engine answers only for two loadouts of one character). */
  private draw(pattern: string, size: OrbSize, t: number, o: Record<string, number>, extra: Record<string, number>): OrbFrame | null {
    const wearFrom = this.wearFrom;
    return (
      (wearFrom &&
        frameTransitionWithOverrides(
          { ...wearFrom, overrides: { ...wearFrom.overrides, ...extra } },
          { state: pattern, speed: 1, overrides: o },
          size,
          t,
          this.wearAge / WEAR_S
        )) ||
      frameWithOverrides(pattern as OrbState, size, t, o)
    );
  }

  /** A CSS keyword curve at `s`, as the engine eases it. */
  private ease(curve: string, s: number): number {
    const side = this.target?.side;
    if (!side) return s;
    return transitionMix(side, side, this.size, s, curve)?.weight ?? s;
  }
}

function entry(side: TransitionSide | null, x: number): Entry {
  return { side, x, l1: x, l2: x, v: 0, x0: x, v0: 0 };
}
