// The voice button's logic (docs/fx-view.md, *Voice button*): a pure state
// machine plus a small controller that runs its effects on a source. DOM-free,
// so the Web element, React and React Native share it; iOS and Android mirror
// it and all three read spec/voice-button-cases.json.
import { SharedVoiceSource } from "./shared.js";
import type { VoiceSource } from "./voice.js";

/** `toggle`: a press connects, then mutes and unmutes. `pushToTalk`: hold to talk. */
export type VoiceButtonMode = "toggle" | "pushToTalk";

export type VoiceButtonState = "ready" | "connecting" | "listening" | "muted" | "error";

export type VoiceButtonEvent =
  | { type: "press" }
  | { type: "release" }
  /** A long-press or the ✕: end the session. */
  | { type: "end" }
  | { type: "connectOk" }
  | { type: "connectFail"; reason: string }
  /** The session ended on the source's side (a remote hang-up, a drop it gave up on). */
  | { type: "dropped" }
  /** The mute changed elsewhere (another control, the app). */
  | { type: "muteChanged"; muted: boolean };

export type VoiceButtonEffect = "connect" | "disconnect" | "mute" | "unmute";

export interface VoiceButtonModel {
  state: VoiceButtonState;
  /** The last failure, for `error`. */
  reason: string | null;
  /** Push-to-talk: the button is held down right now. */
  held: boolean;
}

export interface VoiceButtonConfig {
  mode: VoiceButtonMode;
  /** The source can mute (`setMuted`); without it a press ends the session instead. */
  canMute: boolean;
}

export const VOICE_BUTTON_INITIAL: VoiceButtonModel = { state: "ready", reason: null, held: false };

/** One event in, the next model and the effects to run on the source, in order. */
export function voiceButtonStep(
  m: VoiceButtonModel,
  e: VoiceButtonEvent,
  c: VoiceButtonConfig,
): { model: VoiceButtonModel; effects: VoiceButtonEffect[] } {
  const to = (state: VoiceButtonState, effects: VoiceButtonEffect[] = [], patch: Partial<VoiceButtonModel> = {}) => ({
    model: { ...m, state, reason: state === "error" ? m.reason : null, ...patch },
    effects,
  });
  const same = { model: m, effects: [] as VoiceButtonEffect[] };
  const ptt = c.mode === "pushToTalk" && c.canMute;
  const live = m.state === "listening" || m.state === "muted";
  switch (e.type) {
    case "press":
      if (m.state === "ready" || m.state === "error") return to("connecting", ["connect"], { held: ptt });
      if (m.state === "connecting") return ptt ? to("connecting", [], { held: true }) : same;
      if (ptt) return m.state === "muted" ? to("listening", ["unmute"], { held: true }) : to("listening", [], { held: true });
      if (!c.canMute) return to("ready", ["disconnect"]);
      return m.state === "listening" ? to("muted", ["mute"]) : to("listening", ["unmute"]);
    case "release":
      if (!ptt || !m.held) return same;
      if (m.state === "listening") return to("muted", ["mute"], { held: false });
      return { model: { ...m, held: false }, effects: [] };
    case "end":
      if (m.state === "connecting" || live) return to("ready", ["disconnect"], { held: false });
      if (m.state === "error") return to("ready", [], { held: false });
      return same;
    case "connectOk":
      if (m.state !== "connecting") return same;
      // Push-to-talk released before the session was up: it starts muted.
      return ptt && !m.held ? to("muted", ["mute"]) : to("listening");
    case "connectFail":
      if (m.state !== "connecting") return same;
      return { model: { state: "error", reason: e.reason, held: false }, effects: [] };
    case "dropped":
      return live ? to("ready", [], { held: false }) : same;
    case "muteChanged":
      if (!live) return same;
      return to(e.muted ? "muted" : "listening");
  }
}

/** The accessible name per state; every platform's default wording. */
export interface VoiceButtonLabels {
  ready: string;
  connecting: string;
  listening: string;
  muted: string;
  error: string;
  /** The ✕ / long-press action. */
  end: string;
}

export const DEFAULT_VOICE_BUTTON_LABELS: Readonly<VoiceButtonLabels> = {
  ready: "Start voice",
  connecting: "Connecting",
  listening: "Microphone on",
  muted: "Microphone muted",
  error: "Voice unavailable",
  end: "End voice session",
};

/** What a screen reader should hear as the action (the hint), per state and mode. */
export function voiceButtonHint(state: VoiceButtonState, mode: VoiceButtonMode, canMute: boolean): string {
  switch (state) {
    case "ready":
      return "Connects the voice session";
    case "connecting":
      return "";
    case "error":
      return "Tries again";
    case "listening":
      if (mode === "pushToTalk" && canMute) return "Release to mute";
      return canMute ? "Mutes the microphone" : "Ends the voice session";
    case "muted":
      return mode === "pushToTalk" ? "Hold to talk" : "Unmutes the microphone";
  }
}

/**
 * Runs the state machine against a source: presses in, `connect` /
 * `disconnect` / `setMuted` out, and the source's own drops and mutes folded
 * back in. The views bound to the same source (directly or through
 * `SharedVoiceSource`) follow the mute.
 */
export class VoiceButtonController {
  readonly source: SharedVoiceSource;
  private model: VoiceButtonModel = VOICE_BUTTON_INITIAL;
  private readonly cbs = new Set<(m: VoiceButtonModel) => void>();
  private readonly offs: Array<() => void>;
  private attempt = 0;
  private mode: VoiceButtonMode;

  constructor(source: VoiceSource, mode: VoiceButtonMode = "toggle") {
    this.source = SharedVoiceSource.of(source);
    this.mode = mode;
    this.offs = [
      this.source.reportsConnection
        ? this.source.onConnectionChange((c) => {
            if (!c) this.dispatch({ type: "dropped" });
          })
        : this.source.onStateChange((s) => {
            if (s === "idle") this.dispatch({ type: "dropped" });
          }),
      this.source.onMuteChange((muted) => this.dispatch({ type: "muteChanged", muted })),
    ];
  }

  get state(): VoiceButtonState {
    return this.model.state;
  }

  /** The last failure's message while in `error`. */
  get reason(): string | null {
    return this.model.reason;
  }

  get canMute(): boolean {
    return this.source.canMute;
  }

  setMode(mode: VoiceButtonMode): void {
    this.mode = mode;
  }

  get currentMode(): VoiceButtonMode {
    return this.mode;
  }

  /** Called on every model change; returns its unsubscribe. */
  onChange(cb: (m: VoiceButtonModel) => void): () => void {
    this.cbs.add(cb);
    return () => {
      this.cbs.delete(cb);
    };
  }

  press(): void {
    this.dispatch({ type: "press" });
  }

  release(): void {
    this.dispatch({ type: "release" });
  }

  end(): void {
    this.dispatch({ type: "end" });
  }

  /** Unsubscribes from the source. Doesn't disconnect it: the app owns the session. */
  destroy(): void {
    for (const off of this.offs) off();
    this.cbs.clear();
  }

  private dispatch(e: VoiceButtonEvent): void {
    const { model, effects } = voiceButtonStep(this.model, e, { mode: this.mode, canMute: this.source.canMute });
    const changed = model.state !== this.model.state || model.reason !== this.model.reason || model.held !== this.model.held;
    this.model = model;
    if (changed) for (const cb of this.cbs) cb(model);
    for (const fx of effects) this.run(fx);
  }

  private run(fx: VoiceButtonEffect): void {
    switch (fx) {
      case "connect": {
        const attempt = ++this.attempt;
        this.source.connect().then(
          () => {
            if (attempt === this.attempt) this.dispatch({ type: "connectOk" });
          },
          (err: unknown) => {
            if (attempt === this.attempt) this.dispatch({ type: "connectFail", reason: err instanceof Error ? err.message : String(err) });
          },
        );
        return;
      }
      case "disconnect":
        this.attempt++; // a connect still in flight no longer counts
        this.source.disconnect();
        return;
      case "mute":
        this.source.setMuted(true);
        return;
      case "unmute":
        this.source.setMuted(false);
        return;
    }
  }
}
