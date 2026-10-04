// A simulated conversation as a `VoiceSource` (docs/audio-pipeline.md,
// *Simulated conversations*): a script of turns plays like a real agent --
// state changes on a timeline, a speech-like level and bands while someone
// talks, the barge-in flash -- with no microphone, no audio graph and no
// network. The curves come from the engine (`conversationAt`), so iOS and
// Android play the same conversation.
//
//   const voice = new SimulatedVoiceSource("barge-in");   // or a script
//   mount(el, { pattern: "glowing", voice });
//   await voice.connect();                                // plays and loops

import { conversationAt, conversationSample } from "./conversation.js";
import type { ConversationFrame, ConversationScript } from "./index.js";
import type { AgentState, VoiceMetrics, VoiceSource } from "./voice.js";

// This package is DOM-free (no `lib: dom`); every JS runtime has these two.
const timers = globalThis as unknown as {
  setInterval(fn: () => void, ms: number): unknown;
  clearInterval(id: unknown): void;
};

export interface SimulatedVoiceSourceOptions {
  /** Bands per metrics reading. Default 16 (the analysers' count). */
  bands?: number;
  /** Metrics per second. Default 30, like every other source. */
  rate?: number;
  /** Override the script's own `loop`. */
  loop?: boolean;
  /**
   * `false` = no timer: the caller drives time with `advance(dt)` (tests, a
   * render loop that already ticks). Default `true`.
   */
  autoTick?: boolean;
}

/** One turn on the timeline, for a timeline UI. */
export interface SimulatedTurn {
  state: string;
  start: number;
  seconds: number;
  line: string;
  voice?: "user" | "agent";
  bargeIn: boolean;
}

export class SimulatedVoiceSource implements VoiceSource {
  private readonly script: string;
  private readonly bands: number;
  private readonly rate: number;
  private readonly autoTick: boolean;
  /** Whether time wraps at the end. */
  loop: boolean;
  private t = 0;
  private playing = false;
  private connected = false;
  private timer: unknown = null;
  private last = 0;
  private lastState: string | null = null;
  private lastTurn = -1;
  private metricsCb: ((m: VoiceMetrics) => void) | null = null;
  private stateCb: ((s: AgentState) => void) | null = null;
  private interruptCb: (() => void) | null = null;
  private connectionCb: ((connected: boolean) => void) | null = null;
  private muted = false;
  private frameCb: ((f: ConversationFrame) => void) | null = null;
  /** The turns, with their start times. */
  readonly turns: SimulatedTurn[];
  /** The script's length in seconds. */
  readonly duration: number;

  /** `script`: a built-in sample's name, a script object, or script JSON. */
  constructor(script: string | ConversationScript, opts: SimulatedVoiceSourceOptions = {}) {
    const text = typeof script === "string" ? (conversationSample(script) ?? script) : JSON.stringify(script);
    const probe = conversationAt(text, 0, 1);
    if (!probe.ok) {
      const d = probe.diagnostics.find((x) => x.severity === "error");
      throw new Error(`SimulatedVoiceSource: ${d ? `${d.path || "/"}: ${d.message}` : "not a script"}`);
    }
    this.script = text;
    this.bands = Math.max(1, Math.round(opts.bands ?? 16));
    this.rate = Math.max(1, opts.rate ?? 30);
    this.autoTick = opts.autoTick ?? true;
    const parsed = JSON.parse(text) as ConversationScript;
    this.loop = opts.loop ?? parsed.loop ?? false;
    let start = 0;
    this.turns = parsed.turns.map((tu) => {
      const turn: SimulatedTurn = {
        state: tu.state,
        start,
        seconds: tu.seconds,
        line: tu.line ?? "",
        voice: tu.voice,
        bargeIn: !!tu.bargeIn,
      };
      start += tu.seconds;
      return turn;
    });
    this.duration = probe.total;
  }

  onMetrics(cb: (m: VoiceMetrics) => void): void {
    this.metricsCb = cb;
  }

  onStateChange(cb: (s: AgentState) => void): void {
    this.stateCb = cb;
  }

  onInterrupt(cb: () => void): void {
    this.interruptCb = cb;
  }

  /** Every tick's full reading (turn, progress, the line said so far), for captions and a timeline. */
  onFrame(cb: (f: ConversationFrame) => void): void {
    this.frameCb = cb;
  }

  /** Starts playing from the current time. Never asks for a microphone. */
  connect(): Promise<void> {
    const was = this.connected;
    this.connected = true;
    this.lastState = null;
    this.lastTurn = -1;
    if (!was) this.connectionCb?.(true);
    this.play();
    this.emit();
    return Promise.resolve();
  }

  onConnectionChange(cb: (connected: boolean) => void): void {
    this.connectionCb = cb;
  }

  /** Muted, the user's turns go silent (as a muted mic would); the agent's keep playing. */
  setMuted(muted: boolean): void {
    this.muted = muted;
    this.emit();
  }

  disconnect(): void {
    const was = this.connected;
    this.connected = false;
    if (was) this.connectionCb?.(false);
    this.pause();
    this.metricsCb?.({ level: 0, bands: new Array<number>(this.bands).fill(0) });
    if (this.lastState !== "idle") this.stateCb?.("idle");
    this.lastState = "idle";
  }

  play(): void {
    if (!this.connected || this.playing) return;
    this.playing = true;
    if (this.autoTick) {
      this.last = Date.now();
      this.timer = timers.setInterval(() => {
        const now = Date.now();
        this.advance((now - this.last) / 1000);
        this.last = now;
      }, 1000 / this.rate);
    }
  }

  pause(): void {
    this.playing = false;
    if (this.timer != null) timers.clearInterval(this.timer);
    this.timer = null;
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  /** The script time now, seconds. */
  get time(): number {
    return this.t;
  }

  /** Jump to `t` seconds (no barge-in flash for a turn you jump into). */
  seek(t: number): void {
    this.t = Math.max(0, Math.min(t, this.duration));
    this.lastTurn = this.turnAt(this.t);
    this.emit();
  }

  /** Move time on by `dt` seconds (the timer calls this; so can tests). */
  advance(dt: number): void {
    if (!this.playing) return;
    this.t += Math.max(0, dt);
    if (this.t >= this.duration) this.t = this.loop ? this.t % this.duration : this.duration;
    this.emit();
  }

  private turnAt(t: number): number {
    return conversationAt(this.script, Math.min(t, this.duration), 1).turn;
  }

  private emit(): void {
    if (!this.connected) return;
    // `loop` is ours, not the script's: past the end without it, hold the last instant.
    const at = this.loop ? this.t : Math.min(this.t, this.duration - 1e-9);
    const f = conversationAt(this.script, at, this.bands);
    if (f.turn !== this.lastTurn) {
      const entering = this.lastTurn !== -1 && f.bargeIn;
      this.lastTurn = f.turn;
      if (entering) this.interruptCb?.();
    }
    if (f.state !== this.lastState) {
      this.lastState = f.state;
      this.stateCb?.(f.state as AgentState);
    }
    if (this.muted && this.turns[f.turn]?.voice === "user") this.metricsCb?.({ level: 0, bands: f.bands.map(() => 0) });
    else this.metricsCb?.({ level: f.level, bands: f.bands });
    this.frameCb?.(f);
  }
}
