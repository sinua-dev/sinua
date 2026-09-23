// Generic "any app value -> one curated Reactive-Input key" helper: the
// same convenience `VoiceOverrides` (./voice.ts) gives the audio pipeline,
// generalized to the bind targets `docs/parameters.md` promises to keep
// supporting (its *Reactive Inputs: the public binding contract*). A step
// count driving a ring's `progress`, a heart rate driving the breathing
// swell, a download driving a glow -- without knowing each key's legal
// range or its hidden companions (`audioLevel` does nothing without
// `audioStrength`).
//
// Deliberately small: one mapping (range -> range, optional easing curve),
// not a declarative multi-input binding system, and no engine change.
// Smoothing is caller-side and opt-in (`ReactiveBinding`), because the
// engine is stateless by design. The raw `frameWithOverrides` opts map is
// still the escape hatch for any key not listed here.
//
// Mapping semantics follow Motion's `interpolate()` (motiondivision/motion,
// `packages/motion-dom/src/utils/interpolate.ts`): multi-stop input and
// output of equal length, input ascending or descending, one easing per
// segment or one for all. Unlike Motion (clamp on by default) and d3's
// `scaleLinear` (off by default), clamping is not optional here: every
// target is bounded, so extrapolating past it has no legal meaning.

/** The keys a developer can bind to -- the curated contract, nothing else. */
export type ReactiveTarget =
  | "progress"
  | "progress0"
  | "progress1"
  | "progress2"
  | "progress3"
  | "quality"
  | "accuracy"
  | "audioLevel"
  | "glowStrength"
  | "noiseStrength"
  | "gradientStrength"
  | "pulseStrength"
  | "colorMix"
  | "muted";

/**
 * FX Spec 1.7 / parameter-catalog names of the same targets (the one name
 * used by specs, typed component props and the docs). Accepted everywhere a
 * `ReactiveTarget` is; the overrides produced still use the engine keys.
 */
export type ReactiveTargetPath =
  | "progress[0]"
  | "progress[1]"
  | "progress[2]"
  | "progress[3]"
  | "glow.strength"
  | "noise.strength"
  | "gradient.strength"
  | "pulse.strength"
  | "color.mix";

export const REACTIVE_TARGET_PATHS: Readonly<Record<ReactiveTargetPath, ReactiveTarget>> = {
  "progress[0]": "progress0",
  "progress[1]": "progress1",
  "progress[2]": "progress2",
  "progress[3]": "progress3",
  "glow.strength": "glowStrength",
  "noise.strength": "noiseStrength",
  "gradient.strength": "gradientStrength",
  "pulse.strength": "pulseStrength",
  "color.mix": "colorMix",
};

/** A target by either name -> the engine key (`glow.strength` -> `glowStrength`). */
export function reactiveTargetKey(target: ReactiveTarget | ReactiveTargetPath): ReactiveTarget {
  return (REACTIVE_TARGET_PATHS as Record<string, ReactiveTarget>)[target] ?? (target as ReactiveTarget);
}

export interface ReactiveTargetInfo {
  /** The engine's legal range for this key; every output is clamped to it. */
  range: readonly [number, number];
  /**
   * What an omitted `output` maps onto, when it differs from `range`:
   * `progress0..3` allow extra laps (range 0..3) but default to one lap =
   * the goal; pass a multi-stop `output` to show laps.
   */
  defaultOutput?: readonly [number, number];
  /**
   * Keys the target needs alongside it to have any effect, with their
   * tuned defaults. Emitted *before* the target key, so a caller's own
   * later spread (`{ ...binding, audioStrength: 0.3 }`) wins.
   */
  companions?: Readonly<Record<string, number>>;
  /** What reads it -- for docs and tooling (e.g. the Studio's bindings list). */
  readBy: string;
}

/**
 * The same value as `DEFAULT_AUDIO_STRENGTH` in ./voice.ts (the Studio's
 * tuned "glimmer, not a jarring pulse" breathing amount). Kept as a
 * separate constant so this file doesn't reach into that module's
 * internals; keep the two in sync.
 */
const AUDIO_STRENGTH_DEFAULT = 0.18;

export const REACTIVE_TARGETS: Readonly<Record<ReactiveTarget, ReactiveTargetInfo>> = {
  progress: { range: [0, 1], readBy: "ring `completing` arc sweep / `stepping` / `measuring`, orb `progressing`" },
  // Per-ring values of ring `tracking` (outermost = 0). Past 1 draws another
  // lap, up to the mode's `maxLaps` (default 3) -- hence the [0, 3] range;
  // the default output is still one lap (the goal), laps are opt-in.
  progress0: { range: [0, 3], defaultOutput: [0, 1], readBy: "ring `tracking` ring 0 (outermost); >1 = extra laps" },
  progress1: { range: [0, 3], defaultOutput: [0, 1], readBy: "ring `tracking` ring 1; >1 = extra laps" },
  progress2: { range: [0, 3], defaultOutput: [0, 1], readBy: "ring `tracking` ring 2; >1 = extra laps" },
  progress3: { range: [0, 3], defaultOutput: [0, 1], readBy: "ring `tracking` ring 3 (innermost of 4); >1 = extra laps" },
  quality: { range: [0, 1], readBy: "beacon `reconnecting` pulse + lit segments" },
  accuracy: { range: [0, 1], readBy: "beacon `locating` halo radius" },
  audioLevel: {
    range: [0, 1],
    companions: { audioStrength: AUDIO_STRENGTH_DEFAULT },
    readBy: "apply_audio_reactive -- the breathing swell on any state (input-agnostic)",
  },
  glowStrength: { range: [0, 1], readBy: "apply_glow (material)" },
  noiseStrength: { range: [0, 1], readBy: "apply_noise (material)" },
  gradientStrength: { range: [0, 1], readBy: "apply_gradient (material)" },
  pulseStrength: { range: [0, 1], readBy: "apply_pulse (periodic pulse / breathe)" },
  colorMix: { range: [0, 1], readBy: "apply_color -- how far every element moves to colorHue/colorSaturation" },
  muted: { range: [0, 1], readBy: "apply_muted (mute / connection-lost cue)" },
};

/** An easing for one segment: a CSS keyword curve, or any `u -> u'` on `[0, 1]`. */
export type ReactiveCurve =
  | "linear"
  | "ease"
  | "easeIn"
  | "easeOut"
  | "easeInOut"
  | ((u: number) => number);

export interface ReactiveBindingSpec {
  /** Engine key (`glowStrength`) or its 1.7 path (`glow.strength`); see `REACTIVE_TARGET_PATHS`. */
  target: ReactiveTarget | ReactiveTargetPath;
  /** Input stops, ascending or descending, at least two. Default `[0, 1]`. */
  input?: readonly number[];
  /** Output stops, same length as `input`. Default: the target's `defaultOutput` (else its range). */
  output?: readonly number[];
  /** One curve for every segment, or one per segment. Default `"linear"`. */
  curve?: ReactiveCurve | readonly ReactiveCurve[];
}

/** The overrides object to spread into `frameWithOverrides`. */
export type ReactiveOverrides = Record<string, number>;

/**
 * Pure and stateless: one value -> `{ ...companions, [target]: mapped }`.
 *
 * ```ts
 * // 10k steps fills the ring, eased out so the last few thousand feel slower.
 * frameWithOverrides("completing", 64, t,
 *   bindReactiveInput({ value: steps, target: "progress", input: [0, 10_000], curve: "easeOut" }));
 * ```
 */
export function bindReactiveInput(spec: ReactiveBindingSpec & { value: number }): ReactiveOverrides {
  return reactiveMapper(spec)(spec.value);
}

/**
 * The reusable form (GSAP `mapRange`-style): validates the spec once and
 * returns `value -> overrides`. Throws on an invalid spec.
 */
export function reactiveMapper(spec: ReactiveBindingSpec): (value: number) => ReactiveOverrides {
  const target = reactiveTargetKey(spec.target);
  const map = compile({ ...spec, target });
  const info = REACTIVE_TARGETS[target];
  return (value) => withCompanions(target, info, map(value));
}

export interface ReactiveBindingOptions {
  /**
   * Render-rate easing of the *mapped* value, per second (`VoiceOverrides`'
   * exponential approach: `k = min(1, rate * dt)`). Default `0` = no
   * easing, the mapped value is used as-is. A slow sensor (a heart rate at
   * 1 Hz) wants something like `4`; a value that already changes smoothly
   * doesn't need any.
   */
  easeRate?: number;
}

/** Same stall clamp as `VoiceOverrides` and both Studios. */
const MAX_DT_S = 0.1;

/**
 * Stateful, for values that arrive at their own pace: `push` readings as
 * they come, call `overrides(dt)` once per rendered frame. The first push
 * is taken as-is (no ramp up from zero on the first frame); later ones are
 * eased at `easeRate`.
 */
export class ReactiveBinding {
  private readonly map: (value: number) => number;
  private readonly info: ReactiveTargetInfo;
  private readonly target: ReactiveTarget;
  private readonly easeRate: number;
  private goal: number | null = null;
  private current: number | null = null;

  constructor(spec: ReactiveBindingSpec, opts: ReactiveBindingOptions = {}) {
    this.map = compile(spec);
    this.target = reactiveTargetKey(spec.target);
    this.info = REACTIVE_TARGETS[this.target];
    this.easeRate = Math.max(0, opts.easeRate ?? 0);
  }

  push(value: number): void {
    this.goal = this.map(value);
    if (this.current === null) this.current = this.goal;
  }

  /** The eased, mapped value right now (the target range's min before any push). */
  get value(): number {
    return this.current ?? this.info.range[0];
  }

  overrides(dtSeconds: number): ReactiveOverrides {
    if (this.goal !== null && this.current !== null) {
      if (this.easeRate <= 0) {
        this.current = this.goal;
      } else {
        const dt = Math.max(0, Math.min(dtSeconds, MAX_DT_S));
        this.current += (this.goal - this.current) * Math.min(1, this.easeRate * dt);
      }
    }
    return withCompanions(this.target, this.info, this.value);
  }
}

function withCompanions(target: ReactiveTarget, info: ReactiveTargetInfo, v: number): ReactiveOverrides {
  return { ...info.companions, [target]: v };
}

/** Validates `spec` and returns the bare `value -> clamped mapped value`. */
function compile(spec: ReactiveBindingSpec): (value: number) => number {
  const info = REACTIVE_TARGETS[reactiveTargetKey(spec.target)];
  if (!info) {
    throw new Error(
      `bindReactiveInput: unknown target "${String(spec.target)}" -- one of ${Object.keys(REACTIVE_TARGETS).join(", ")}`
    );
  }
  const [lo, hi] = info.range;
  const input = spec.input ?? [0, 1];
  const output = spec.output ?? info.defaultOutput ?? [lo, hi];
  if (input.length < 2) throw new Error("bindReactiveInput: `input` needs at least two stops");
  if (input.length !== output.length) {
    throw new Error("bindReactiveInput: `input` and `output` must be the same length");
  }
  if (![...input, ...output].every(Number.isFinite)) {
    throw new Error("bindReactiveInput: `input`/`output` stops must be finite numbers");
  }
  const dir = Math.sign(input[input.length - 1] - input[0]);
  for (let i = 0; i < input.length - 1; i++) {
    if (dir === 0 || Math.sign(input[i + 1] - input[i]) !== dir) {
      throw new Error("bindReactiveInput: `input` must be strictly ascending or strictly descending");
    }
  }
  const segments = input.length - 1;
  const rawCurves = spec.curve ?? "linear";
  const curves: ReactiveCurve[] = Array.isArray(rawCurves)
    ? [...(rawCurves as readonly ReactiveCurve[])]
    : new Array<ReactiveCurve>(segments).fill(rawCurves as ReactiveCurve);
  if (curves.length !== segments) {
    throw new Error(`bindReactiveInput: ${segments} segment(s) need ${segments} curve(s), got ${curves.length}`);
  }
  const eases = curves.map(resolveCurve);

  const first = input[0];
  const last = input[segments];
  return (value) => {
    if (Number.isNaN(value)) return lo;
    // Clamp the input into the stops (either direction), find its segment,
    // ease within it, interpolate the output, clamp to the target's range.
    const v = dir > 0 ? Math.min(Math.max(value, first), last) : Math.min(Math.max(value, last), first);
    let i = 0;
    while (i < segments - 1 && (dir > 0 ? v > input[i + 1] : v < input[i + 1])) i++;
    const u = (v - input[i]) / (input[i + 1] - input[i]);
    const e = eases[i](Math.min(Math.max(u, 0), 1));
    const out = output[i] + (output[i + 1] - output[i]) * e;
    return Math.min(Math.max(out, lo), hi);
  };
}

function resolveCurve(c: ReactiveCurve): (u: number) => number {
  if (typeof c === "function") return c;
  switch (c) {
    case "linear":
      return (u) => u;
    // CSS keyword curves, exact values from MDN `animation-timing-function`.
    case "ease":
      return cubicBezier(0.25, 0.1, 0.25, 1);
    case "easeIn":
      return cubicBezier(0.42, 0, 1, 1);
    case "easeOut":
      return cubicBezier(0, 0, 0.58, 1);
    case "easeInOut":
      return cubicBezier(0.42, 0, 0.58, 1);
    default:
      throw new Error(`bindReactiveInput: unknown curve "${String(c)}"`);
  }
}

/**
 * CSS `cubic-bezier(x1, y1, x2, y2)`: solve `x(s) = u` for `s` (Newton,
 * bisection fallback), return `y(s)`. The same approach as the engine's
 * `primitives::cubic_bezier`, which isn't exported over wasm.
 */
function cubicBezier(x1: number, y1: number, x2: number, y2: number): (u: number) => number {
  const bez = (p1: number, p2: number, s: number) => {
    const inv = 1 - s;
    return 3 * inv * inv * s * p1 + 3 * inv * s * s * p2 + s * s * s;
  };
  const dx = (s: number) => {
    const inv = 1 - s;
    return 3 * inv * inv * x1 + 6 * inv * s * (x2 - x1) + 3 * s * s * (1 - x2);
  };
  return (u) => {
    if (u <= 0) return 0;
    if (u >= 1) return 1;
    let s = u;
    for (let k = 0; k < 8; k++) {
      const x = bez(x1, x2, s) - u;
      if (Math.abs(x) < 1e-7) return bez(y1, y2, s);
      const d = dx(s);
      if (Math.abs(d) < 1e-6) break;
      s -= x / d;
    }
    let lo = 0;
    let hi = 1;
    s = u;
    for (let k = 0; k < 40; k++) {
      const x = bez(x1, x2, s);
      if (Math.abs(x - u) < 1e-7) break;
      if (x < u) lo = s;
      else hi = s;
      s = (lo + hi) / 2;
    }
    return bez(y1, y2, s);
  };
}
