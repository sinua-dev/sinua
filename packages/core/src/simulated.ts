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
import type { AgentState, TranscriptUpdate, VoiceMetrics, VoiceSource } from "./voice.js";

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
  private transcriptCb: ((u: TranscriptUpdate) => void) | null = null;
  /** The turn being transcribed (`turn` is the script's index), and the counters that name turns. */
  private said: { turn: number; id: string; role: "user" | "assistant"; text: string } | null = null;
  private userTurns = 0;
  private assistantTurns = 0;
  /** The script's lines are spoken as they play: the transcript is already in step with the audio. */
  readonly supportsTranscript = true;
  readonly transcriptTiming = "synced" as const;
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

  /**
   * Transcript updates: each turn's line as it is said (`synced`); a turn cut by a
   * barge-in ends `truncated`. Muted user turns say nothing.
   */
  onTranscript(cb: (u: TranscriptUpdate) => void): void {
    this.transcriptCb = cb;
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
    this.close(false);
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
      // The turn that was being said ends: cut short by a barge-in, or said in full.
      if (this.said && this.said.turn !== f.turn) this.close(entering);
    }
    this.transcribe(f);
    if (f.state !== this.lastState) {
      this.lastState = f.state;
      this.stateCb?.(f.state as AgentState);
    }
    if (this.muted && this.turns[f.turn]?.voice === "user") this.metricsCb?.({ level: 0, bands: f.bands.map(() => 0) });
    else this.metricsCb?.({ level: f.level, bands: f.bands });
    this.frameCb?.(f);
  }

  /** Speaker of a turn: its `voice`, else what its state implies (listening = the user). */
  private roleOf(turn: number, state: string): "user" | "assistant" | null {
    const voice = this.turns[turn]?.voice;
    if (voice === "user") return "user";
    if (voice === "agent") return "assistant";
    if (state === "speaking") return "assistant";
    if (state === "listening") return "user";
    return null;
  }

  private transcribe(f: ConversationFrame): void {
    if (!this.transcriptCb) return;
    const role = this.roleOf(f.turn, f.state);
    if (!role || !f.line || (role === "user" && this.muted)) return;
    const text = Array.from(f.line.normalize("NFC")).slice(0, f.shown).join("");
    if (!this.said) {
      if (!text) return;
      const id = role === "user" ? `u${++this.userTurns}` : `a${++this.assistantTurns}`;
      this.said = { turn: f.turn, id, role, text: "" };
    }
    if (text === this.said.text) return;
    this.said.text = text;
    this.transcriptCb({ role, text, final: false, turnId: this.said.id });
  }

  /** Ends the turn being said: `cut` by a barge-in (what was said so far), else in full. */
  private close(cut: boolean): void {
    const s = this.said;
    this.said = null;
    if (!s || !this.transcriptCb) return;
    const full = (this.turns[s.turn]?.line ?? "").normalize("NFC");
    const text = (cut && s.role === "assistant" ? s.text : full).trim();
    this.transcriptCb(cut && s.role === "assistant" ? { role: s.role, text, final: true, turnId: s.id, truncated: true } : { role: s.role, text, final: true, turnId: s.id });
  }
}
