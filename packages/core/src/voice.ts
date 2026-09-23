// The integrator-facing half of the voice contract: turns a live voice
// reading (`{level, bands}` + an `AgentState`) into the exact overrides
// object `frameWithOverrides` expects for either family. DOM-free and
// clock-free on purpose (dt-driven), so it runs anywhere the wasm/native
// engine does and can be tested under plain node.
//
// Why this exists: before it, every consumer hand-built the opts map and
// had to know three non-obvious things the Studio learned the hard way --
// the key encodings (`audioBand0..15` + `audioBandCount`, `voiceStateCode`
// 0..4, `history0..N` + `historyPhase` for the scrolling style), that a
// ~30 fps metrics feed must be eased at render rate or the visual visibly
// steps (a real, user-reported stutter -- docs/audio-pipeline.md, "A
// second, slower smoothing pass"), and the `muted` cue flag. The numbers
// below are the Studio's tuned ones, not new guesses.

/**
 * LiveKit Agents' `AgentState` vocabulary, verbatim (their
 * `livekit-agents/livekit/agents/voice/events.py`), adopted rather than
 * invented so a LiveKit-based app maps 1:1 -- see docs/audio-pipeline.md.
 */
export type AgentState = "initializing" | "idle" | "listening" | "thinking" | "speaking";

/** A smoothed, normalized reading from whichever `VoiceSource` is active. */
export interface VoiceMetrics {
  /** Overall smoothed volume, 0..1. */
  level: number;
  /** Per-band smoothed volume, 0..1 each, low frequency first. */
  bands: number[];
}

/**
 * One adapter per transport shape (WebRTC track vs. raw PCM over a
 * socket), not per vendor. The Studio ships `LocalMicVoiceSource`,
 * `TestToneVoiceSource`, `WebRTCVoiceSource` (OpenAI Realtime) and
 * `PCMStreamVoiceSource` (Gemini Live) against this interface
 * (the Web Studio's audio pipeline); a native app implements the same four
 * methods over its own capture stack and feeds `VoiceOverrides` below.
 */
export interface VoiceSource {
  connect(): Promise<void>;
  disconnect(): void;
  onMetrics(cb: (m: VoiceMetrics) => void): void;
  onStateChange(cb: (s: AgentState) => void): void;
  /**
   * Optional: the barge-in moment -- the user started talking while the
   * agent was audibly outputting and the vendor cut the response. Fired
   * once per interruption, before the matching state change. Optional so
   * sources without such a signal (a plain mic, a test tone, a native app
   * that hasn't wired it) stay valid; `VoiceOverrides` turns it into
   * `interruptAge` for `primitives::apply_interrupt`.
   */
  onInterrupt?(cb: () => void): void;
}

/**
 * `voiceStateCode` encoding -- mirrors `VoiceState` in
 * crates/core_engine/src/signal/modes/bar.rs (a flat `HashMap<String,
 * f64>` opts map can't carry a string). Read by every `signal` style;
 * ignored by every `orbs` state, which never auto-switch on voice state.
 */
export const VOICE_STATE_CODE: Readonly<Record<AgentState, number>> = {
  idle: 0,
  initializing: 1,
  listening: 2,
  thinking: 3,
  speaking: 4,
};

/** `primitives::audio_band`'s cap: keys `audioBand0`..`audioBand15` exist, nothing beyond. */
export const MAX_AUDIO_BANDS = 16;

export interface VoiceOverridesOptions {
  /**
   * Orb breathing-pulse intensity (`audioStrength` for
   * `primitives::apply_audio_reactive`). Default 0.18, the Studio's tuned
   * "glimmer, not a jarring pulse" value; `0` disables the pulse. Harmless
   * for `signal` states (their frames have no dots, the post-process
   * returns early).
   */
  audioStrength?: number;
  /**
   * Render-rate easing for `audioLevel`, per second. Default 7 (~150 ms to
   * settle) -- the Orb Studio's `AUDIO_EASE_RATE`, gentle because it
   * smooths a whole-orb breathing motion. `Infinity` passes the raw
   * reading through (no easing).
   */
  levelEaseRate?: number;
  /**
   * Render-rate easing for `audioBand*`, per second. Default 24 (~40 ms) --
   * the Signal Studio's `BAND_EASE_RATE`: bands drive discrete bars, so
   * this only needs to bridge the ~33 ms gap between analysis updates,
   * not add lag. `Infinity` passes the raw bands through -- what the Orb
   * Studio does, since its `spectrum` bars were tuned on raw bands.
   */
  bandEaseRate?: number;
  /**
   * Enables the scrolling-style history ring buffer (`historyCount`,
   * `history0..N-1`, `historyPhase` -- the engine is stateless, so the
   * caller owns this; see docs/signal.md, *`scroll`*). `count` samples,
   * pushed `hz` times per second (default 12 -- react-native-waveform-
   * recorder's `samplesPerSecond`, and `scroll.rs`'s synthetic rate, so
   * live and idle scroll at the same pace). Each push is the *peak* level
   * seen since the previous push, so a short burst between pushes isn't
   * lost. Off unless set.
   */
  history?: { count: number; hz?: number };
  /**
   * The barge-in flash (`primitives::apply_interrupt`). On by default:
   * after `interrupt()` (or a bound source's `onInterrupt`), `interruptAge`
   * is emitted for `window` seconds (default 1 -- the Signal Studio's
   * manual-flash window; the engine's own `interruptDuration` default is
   * 0.45 s, so the flash has ended well before the key stops). The other
   * fields are the engine's companions, emitted only when set. `false`
   * never emits `interruptAge`.
   */
  interrupt?: InterruptOptions | false;
  /** `muted: 1` for `primitives::apply_muted` -- the "mic muted / connection lost" cue. */
  muted?: boolean;
  /** `mutedTint` (0 = grey, 1 = fully tinted), only emitted when set. */
  mutedTint?: number;
  /** `mutedHue` in degrees, only emitted when set (engine default: 8, Echo red). */
  mutedHue?: number;
}

export interface InterruptOptions {
  /** Seconds `interruptAge` keeps being emitted after the moment. Default 1. */
  window?: number;
  /** `interruptDuration` (engine default 0.45 s). */
  duration?: number;
  /** `interruptStrength`, 0..1 (engine default 1). */
  strength?: number;
  /** `interruptTint`, 0..1 (engine default 0 = no tint). */
  tint?: number;
  /** `interruptHue`, degrees (engine default 45, warm amber). */
  hue?: number;
}

/** The opts object to spread into `frameWithOverrides`. */
export type VoiceOverridesMap = Record<string, number>;

const DEFAULT_AUDIO_STRENGTH = 0.18;
const DEFAULT_LEVEL_EASE_RATE = 7;
const DEFAULT_BAND_EASE_RATE = 24;
const DEFAULT_HISTORY_HZ = 12;
const DEFAULT_INTERRUPT_WINDOW_S = 1;
/** Clamp a stalled frame (backgrounded tab) the same way both Studios do. */
const MAX_DT_S = 0.1;

/**
 * Pure and stateless: one reading + one state -> the overrides object. No
 * easing, no history -- use this when you already run your own smoothing,
 * or for a one-off frame. Otherwise use `VoiceOverrides`.
 */
export function voiceOverrides(
  metrics: VoiceMetrics,
  state: AgentState,
  opts: VoiceOverridesOptions = {}
): VoiceOverridesMap {
  return buildOverrides(metrics.level, metrics.bands, state, opts, null);
}

/**
 * Stateful: feed it a `VoiceSource` (or raw readings), call `overrides(dt)`
 * once per rendered frame, spread the result into `frameWithOverrides`.
 * Eases level and bands at render rate, keeps the optional scrolling
 * history, and knows every key the engine reads.
 *
 * ```ts
 * const voice = VoiceOverrides.bind(source, { history: { count: 40 } });
 * await source.connect();
 * // per frame:
 * const frame = frameWithOverrides("signaling", 64, t, { ...knobs, ...voice.overrides(dt) });
 * ```
 */
export class VoiceOverrides {
  private readonly audioStrength: number;
  private readonly levelEaseRate: number;
  private readonly bandEaseRate: number;
  private readonly historyHz: number;
  private history: number[] | null;
  private historyAcc = 0;
  private historyPeak = 0;
  private historyPhase = 0;
  private readonly interruptOpts: InterruptOptions | null;
  /** Seconds since the last interrupt, or null when none is live. */
  private interruptAge: number | null = null;
  /** Set by `interrupt()`: the next frame reports age 0 without adding its dt (that dt predates the moment). */
  private interruptFresh = false;

  private raw: VoiceMetrics = { level: 0, bands: [] };
  private current: AgentState = "idle";
  private level = 0;
  private bands: number[] = [];

  /** The `muted` cue; flip it live, the next `overrides()` reflects it. */
  muted: boolean;
  mutedTint: number | undefined;
  mutedHue: number | undefined;

  constructor(opts: VoiceOverridesOptions = {}) {
    this.audioStrength = opts.audioStrength ?? DEFAULT_AUDIO_STRENGTH;
    this.levelEaseRate = opts.levelEaseRate ?? DEFAULT_LEVEL_EASE_RATE;
    this.bandEaseRate = opts.bandEaseRate ?? DEFAULT_BAND_EASE_RATE;
    this.historyHz = opts.history?.hz ?? DEFAULT_HISTORY_HZ;
    // Pre-filled with silence so a scrolling strip is full-width from the
    // first frame and simply starts moving, instead of filling in from the
    // right over the first few seconds (the Studio does the same on connect).
    this.history = opts.history ? new Array<number>(Math.max(1, Math.floor(opts.history.count))).fill(0) : null;
    this.interruptOpts = opts.interrupt === false ? null : opts.interrupt ?? {};
    this.muted = opts.muted ?? false;
    this.mutedTint = opts.mutedTint;
    this.mutedHue = opts.mutedHue;
  }

  /**
   * Convenience: subscribes to `source` and returns the tracker. Note that
   * a `VoiceSource` holds ONE metrics callback and ONE state callback --
   * `bind` takes both. Read `metrics`/`state` off the tracker for meters
   * and lifecycle pills instead of subscribing again, or feed the tracker
   * yourself with `push`/`setState` from your own callbacks.
   */
  static bind(source: VoiceSource, opts?: VoiceOverridesOptions): VoiceOverrides {
    const v = new VoiceOverrides(opts);
    source.onMetrics((m) => v.push(m));
    source.onInterrupt?.(() => v.interrupt());
    source.onStateChange((s) => {
      v.setState(s);
      // A source reports `idle` from its own disconnect (or a remote drop):
      // clear easing + history so a reconnect doesn't start mid-decay.
      if (s === "idle") v.reset();
    });
    return v;
  }

  /** Latest raw reading (un-eased) -- for a level meter. */
  get metrics(): VoiceMetrics {
    return this.raw;
  }

  /** Latest lifecycle state -- for a pill/label. */
  get state(): AgentState {
    return this.current;
  }

  push(metrics: VoiceMetrics): void {
    this.raw = metrics;
  }

  setState(state: AgentState): void {
    this.current = state;
  }

  /**
   * Mark the barge-in moment now: the next `overrides()` emits
   * `interruptAge` 0, then the age grows with each frame's `dt`. Called for
   * you by `bind` when the source has `onInterrupt`; call it yourself for a
   * manual trigger or from your own native callback.
   */
  interrupt(): void {
    if (this.interruptOpts) {
      this.interruptAge = 0;
      this.interruptFresh = true;
    }
  }

  /** Resize the scrolling history (e.g. from a knob); newest samples are kept. */
  setHistoryCount(count: number): void {
    const n = Math.max(1, Math.floor(count));
    if (!this.history) {
      this.history = new Array<number>(n).fill(0);
      return;
    }
    while (this.history.length > n) this.history.shift();
    while (this.history.length < n) this.history.unshift(0);
  }

  /** Back to silence: clears the eased values and refills the history with zeros. */
  reset(): void {
    this.raw = { level: 0, bands: [] };
    this.level = 0;
    this.bands = [];
    if (this.history) this.history.fill(0);
    this.historyAcc = 0;
    this.historyPeak = 0;
    this.historyPhase = 0;
    this.interruptAge = null;
    this.interruptFresh = false;
  }

  /**
   * Call once per rendered frame with the seconds since the previous
   * frame. Returns the complete overrides object for that frame.
   */
  overrides(dtSeconds: number): VoiceOverridesMap {
    const dt = Math.max(0, Math.min(dtSeconds, MAX_DT_S));

    // The interrupt age uses the *unclamped* dt: it's an age on the
    // caller's wall clock (the engine's contract), and a stalled tab
    // shouldn't resume mid-flash. The first frame after `interrupt()`
    // reports age 0 -- the peak -- and doesn't add its own dt, which
    // measures the interval *before* the moment.
    let interrupt: number | null = null;
    if (this.interruptAge != null && this.interruptOpts) {
      if (this.interruptFresh) this.interruptFresh = false;
      else this.interruptAge += Math.max(0, Number.isFinite(dtSeconds) ? dtSeconds : 0);
      interrupt = this.interruptAge;
      if (interrupt >= (this.interruptOpts.window ?? DEFAULT_INTERRUPT_WINDOW_S)) {
        interrupt = null;
        this.interruptAge = null;
      }
    }

    const kLevel = easeK(this.levelEaseRate, dt);
    this.level += (this.raw.level - this.level) * kLevel;

    const rawBands = this.raw.bands;
    if (this.bands.length !== rawBands.length) {
      this.bands = rawBands.slice();
    } else {
      const kBand = easeK(this.bandEaseRate, dt);
      for (let i = 0; i < rawBands.length; i++) {
        this.bands[i] += (rawBands[i] - this.bands[i]) * kBand;
      }
    }

    let history: { buf: number[]; phase: number } | null = null;
    if (this.history) {
      const interval = 1 / this.historyHz;
      this.historyAcc += dt;
      this.historyPeak = Math.max(this.historyPeak, this.raw.level);
      // Remainder-carrying cadence: a slow frame pushes once and keeps the
      // leftover, so the scroll rate never drifts with frame timing.
      while (this.historyAcc >= interval) {
        this.history.push(this.historyPeak);
        this.history.shift();
        this.historyPeak = 0;
        this.historyAcc -= interval;
      }
      this.historyPhase = this.historyAcc / interval;
      history = { buf: this.history, phase: this.historyPhase };
    }

    return buildOverrides(
      this.level,
      this.bands,
      this.current,
      {
        audioStrength: this.audioStrength,
        muted: this.muted,
        mutedTint: this.mutedTint,
        mutedHue: this.mutedHue,
      },
      history,
      interrupt == null ? null : { age: interrupt, opts: this.interruptOpts! }
    );
  }
}

function buildOverrides(
  level: number,
  bands: ReadonlyArray<number>,
  state: AgentState,
  opts: VoiceOverridesOptions,
  history: { buf: ReadonlyArray<number>; phase: number } | null,
  interrupt: { age: number; opts: InterruptOptions } | null = null
): VoiceOverridesMap {
  const out: VoiceOverridesMap = {
    audioLevel: clamp01(level),
    audioStrength: opts.audioStrength ?? DEFAULT_AUDIO_STRENGTH,
    voiceStateCode: VOICE_STATE_CODE[state],
  };
  // `audioBandCount` is the engine's "real audio is present" signal
  // (`spectrum.rs`, `bar.rs`): 0 means "use the synthetic idle pattern".
  const n = Math.min(bands.length, MAX_AUDIO_BANDS);
  out.audioBandCount = n;
  for (let i = 0; i < n; i++) out[`audioBand${i}`] = clamp01(bands[i]);
  if (history) {
    out.historyCount = history.buf.length;
    for (let i = 0; i < history.buf.length; i++) out[`history${i}`] = clamp01(history.buf[i]);
    out.historyPhase = history.phase;
  }
  if (interrupt) {
    out.interruptAge = interrupt.age;
    const o = interrupt.opts;
    if (o.duration != null) out.interruptDuration = o.duration;
    if (o.strength != null) out.interruptStrength = o.strength;
    if (o.tint != null) out.interruptTint = o.tint;
    if (o.hue != null) out.interruptHue = o.hue;
  }
  if (opts.muted) {
    out.muted = 1;
    if (opts.mutedTint != null) out.mutedTint = opts.mutedTint;
    if (opts.mutedHue != null) out.mutedHue = opts.mutedHue;
  }
  return out;
}

/** Per-frame easing factor; `rate: Infinity` = no easing (without `Infinity * 0 = NaN` on a zero-dt frame). */
function easeK(rate: number, dt: number): number {
  return dt > 0 ? Math.min(1, rate * dt) : 0;
}

function clamp01(v: number): number {
  return v > 1 ? 1 : v < 0 || Number.isNaN(v) ? 0 : v;
}
