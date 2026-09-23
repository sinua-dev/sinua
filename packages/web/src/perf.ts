// Frame pacing and low-power policy for SinuaView (docs/fx-view.md,
// *Performance and power*). Pure where possible, so the Studio's own loops
// can pace exactly like SinuaView (studio-ui-ux uses `createFramePacer`).

/**
 * Frame-rate cap by skipping display frames: returns `true` when the frame at
 * `nowMs` should be drawn. `interval = 1000 / maxFps`; draw when there is no
 * previous frame or `now - last >= interval - 1 ms` (the 1 ms absorbs vsync
 * jitter), then advance `last` by the whole number of intervals the gap
 * covers (`last += interval * max(1, floor((now - last + 1) / interval))`),
 * so the schedule stays on a fixed grid and the average rate is exact -- 30
 * on a 60 Hz or 120 Hz display, 24 on 60 Hz -- with no drift and no burst
 * after a stall. (Carrying the remainder with `now - ((now - last) %
 * interval)` looks equivalent but isn't: a frame landing just inside the
 * 1 ms tolerance has a remainder of the whole gap, `last` doesn't move, and
 * the next frame draws too -- ~40 fps for a 30 cap on 60 Hz. Caught by
 * test/perf.test.mjs.) The animation clock stays wall time: a cap lowers
 * smoothness, never speed. `maxFps` <= 0 or undefined draws every frame.
 */
export function createFramePacer(maxFps: number | null | undefined): (nowMs: number) => boolean {
  if (!maxFps || maxFps <= 0 || !Number.isFinite(maxFps)) return () => true;
  const interval = 1000 / maxFps;
  let last: number | null = null;
  return (now) => {
    if (last === null) {
      last = now;
      return true;
    }
    const since = now - last;
    if (since < interval - 1) return false;
    last += interval * Math.max(1, Math.floor((since + 1) / interval));
    return true;
  };
}

/**
 * The default when low power is on and the spec has no `performance.lowPower`
 * (FX Spec 1.2): 30 fps, glow and particles off -- the recommended host default
 * in docs/fx-spec.md. Both are their material's documented off switch (a strict
 * no-op at 0). Liquid is left to the spec/app: it replaces dots rather than adding.
 */
export const DEFAULT_LOW_POWER = { maxFps: 30, overrides: { glowStrength: 0, particleStrength: 0 } as Record<string, number> };

export interface FxPerformance {
  /** The effective cap, or null for display rate. */
  maxFps: number | null;
  /** Engine opts to apply on top of the spec's (before voice keys). */
  overrides: Record<string, number>;
}

/**
 * The single place FX Spec 1.2's `performance` block is honoured.
 *
 * - `specMaxFps`: the resolver's effective cap (`FxSpecResolved.maxFps`,
 *   already the low-power one when the spec was resolved with `lowPower`).
 * - `specHandlesLowPower`: the spec has a `performance.lowPower` block, so
 *   the resolver has already shed its materials and set its cap. Otherwise,
 *   under low power, the host default applies on top: 30 fps, glow off.
 * - The view's own `maxFps` option caps further (the lowest wins).
 */
export function performanceFor(opts: {
  lowPower: boolean;
  optionMaxFps?: number | null;
  specMaxFps?: number | null;
  specHandlesLowPower?: boolean;
}): FxPerformance {
  const caps: number[] = [];
  let overrides: Record<string, number> = {};
  if (opts.specMaxFps && opts.specMaxFps > 0) caps.push(opts.specMaxFps);
  if (opts.lowPower && !opts.specHandlesLowPower) {
    caps.push(DEFAULT_LOW_POWER.maxFps);
    overrides = { ...DEFAULT_LOW_POWER.overrides };
  }
  if (opts.optionMaxFps && opts.optionMaxFps > 0) caps.push(opts.optionMaxFps);
  return { maxFps: caps.length ? Math.min(...caps) : null, overrides };
}

type BatteryManagerLike = EventTarget & { level: number; charging: boolean };

/**
 * The Battery Status API is not in TypeScript's DOM lib -- it is Chromium-only
 * (Firefox removed it as a fingerprinting vector, Safari never shipped it), so
 * `lib.dom.d.ts` does not declare `navigator.getBattery`. This is the narrow
 * local declaration for it, optional because on every other engine it is
 * genuinely absent and `watchLowBattery` returns null.
 *
 * Deliberately a declared shape plus one cast at the single call site, rather
 * than a `global` augmentation (which would claim the property exists for every
 * consumer of this package) or `any` (which would hide the two field reads this
 * file does on the result).
 */
interface NavigatorWithBattery {
  getBattery?: () => Promise<BatteryManagerLike>;
}

/**
 * Opt-in low-battery heuristic for the Web, which has no OS power-saving
 * signal: the Battery Status API exists only in Chromium browsers (Firefox
 * removed it as a fingerprinting vector, Safari never shipped it) and
 * reports level/charging, not a power-saving mode. Calls `cb(true)` when the
 * level is <= `threshold` and not charging. Resolves to a stop function, or
 * `null` where the API is unsupported (then nothing is ever reported).
 *
 * ```ts
 * const fx = mount(canvas, { spec });
 * watchLowBattery((low) => fx.update({ lowPower: low }));
 * ```
 */
export async function watchLowBattery(cb: (low: boolean) => void, threshold = 0.2): Promise<(() => void) | null> {
  const nav: NavigatorWithBattery | undefined =
    typeof navigator === "undefined" ? undefined : (navigator as Navigator & NavigatorWithBattery);
  if (!nav?.getBattery) return null;
  let battery: BatteryManagerLike;
  try {
    battery = await nav.getBattery();
  } catch {
    return null;
  }
  const report = () => cb(!battery.charging && battery.level <= threshold);
  battery.addEventListener("levelchange", report);
  battery.addEventListener("chargingchange", report);
  report();
  return () => {
    battery.removeEventListener("levelchange", report);
    battery.removeEventListener("chargingchange", report);
  };
}
