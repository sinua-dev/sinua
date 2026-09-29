// One source, many listeners (docs/audio-pipeline.md, *Sharing a source*). A
// `VoiceSource` holds ONE callback of each kind, so a second subscriber -- a view
// next to a voice button, two views, an app's own state pill -- silently replaces
// the first. `SharedVoiceSource` subscribes once and fans out; it is itself a
// `VoiceSource`, so it goes anywhere a source goes. It also owns the session's
// mute, so every view bound through it shows the muted cue.
import { VoiceOverrides, type AgentState, type VoiceMetrics, type VoiceOverridesOptions, type VoiceSource } from "./voice.js";

type Listener<T> = (value: T) => void;

function add<T>(set: Set<Listener<T>>, cb: Listener<T>): () => void {
  set.add(cb);
  return () => {
    set.delete(cb);
  };
}

const shared = new WeakMap<VoiceSource, SharedVoiceSource>();

export class SharedVoiceSource implements VoiceSource {
  /** The wrapped source. Don't subscribe to it directly: that would take its one callback away from this fan-out. */
  readonly source: VoiceSource;
  private readonly metricsCbs = new Set<Listener<VoiceMetrics>>();
  private readonly stateCbs = new Set<Listener<AgentState>>();
  private readonly interruptCbs = new Set<Listener<void>>();
  private readonly muteCbs = new Set<Listener<boolean>>();
  private readonly connectionCbs = new Set<Listener<boolean>>();
  private currentState: AgentState = "idle";
  private isMuted = false;
  private isConnected = false;

  /**
   * The fan-out for `source`: the same instance every time for the same source, so a
   * view and a voice button given the same raw source share one subscription. A
   * `SharedVoiceSource` is returned as is.
   */
  static of(source: VoiceSource): SharedVoiceSource {
    if (source instanceof SharedVoiceSource) return source;
    let s = shared.get(source);
    if (!s) {
      s = new SharedVoiceSource(source);
      shared.set(source, s);
    }
    return s;
  }

  private constructor(source: VoiceSource) {
    this.source = source;
    source.onMetrics((m) => {
      for (const cb of this.metricsCbs) cb(m);
    });
    source.onStateChange((s) => {
      this.currentState = s;
      for (const cb of this.stateCbs) cb(s);
    });
    source.onInterrupt?.(() => {
      for (const cb of this.interruptCbs) cb();
    });
    source.onConnectionChange?.((c) => {
      this.isConnected = c;
      for (const cb of this.connectionCbs) cb(c);
    });
  }

  /** Adds a listener; returns its unsubscribe. */
  onMetrics(cb: (m: VoiceMetrics) => void): () => void {
    return add(this.metricsCbs, cb);
  }

  /** Adds a listener; returns its unsubscribe. */
  onStateChange(cb: (s: AgentState) => void): () => void {
    return add(this.stateCbs, cb);
  }

  /** Adds a listener; returns its unsubscribe. */
  onInterrupt(cb: () => void): () => void {
    return add(this.interruptCbs, cb);
  }

  /** Adds a listener; returns its unsubscribe. Only fires for a source that reports it (`reportsConnection`). */
  onConnectionChange(cb: (connected: boolean) => void): () => void {
    return add(this.connectionCbs, cb);
  }

  /** Whether the wrapped source reports `onConnectionChange`; without it, `idle` is the only hint that a session ended. */
  get reportsConnection(): boolean {
    return typeof this.source.onConnectionChange === "function";
  }

  /** The last `onConnectionChange` value (false before any). */
  get connected(): boolean {
    return this.isConnected;
  }

  /** Called with the new value whenever `setMuted` changes it; returns its unsubscribe. */
  onMuteChange(cb: (muted: boolean) => void): () => void {
    return add(this.muteCbs, cb);
  }

  /** The source's last reported state (`idle` before any). */
  get state(): AgentState {
    return this.currentState;
  }

  get muted(): boolean {
    return this.isMuted;
  }

  /** Whether the wrapped source can mute its microphone (`setMuted`). */
  get canMute(): boolean {
    return typeof this.source.setMuted === "function";
  }

  /** Connects unmuted: a new session never starts silent from an old mute. */
  connect(): Promise<void> {
    this.setMuted(false);
    return this.source.connect();
  }

  disconnect(): void {
    this.source.disconnect();
  }

  /**
   * Mutes or unmutes the microphone: silence goes out and the session stays up.
   * Views bound through this fan-out show the muted cue. A no-op (always unmuted)
   * when the source can't mute.
   */
  setMuted(muted: boolean): void {
    if (!this.canMute) muted = false;
    this.source.setMuted?.(muted);
    if (muted === this.isMuted) return;
    this.isMuted = muted;
    for (const cb of this.muteCbs) cb(muted);
  }

  /**
   * A view's own tracker, fed from this fan-out: its family's easing and history
   * (`opts`), the muted cue following `muted`. `release()` unsubscribes it.
   */
  track(opts?: VoiceOverridesOptions): { overrides: VoiceOverrides; release(): void } {
    const v = new VoiceOverrides(opts);
    v.setState(this.currentState);
    v.muted = this.isMuted;
    const offs = [
      this.onMetrics((m) => v.push(m)),
      this.onInterrupt(() => v.interrupt()),
      this.onStateChange((s) => {
        v.setState(s);
        // As `VoiceOverrides.bind`: a reconnect doesn't start mid-decay.
        if (s === "idle") v.reset();
      }),
      this.onMuteChange((m) => {
        v.muted = m;
      }),
    ];
    return {
      overrides: v,
      release: () => {
        for (const off of offs) off();
      },
    };
  }
}
