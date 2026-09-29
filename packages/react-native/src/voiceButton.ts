// The voice button's logic on React Native (docs/fx-view.md, *Voice button*): the same
// state machine as `@sinua/core`'s voiceButton.ts (this package doesn't depend on
// @sinua/core), held to the same spec/voice-button-cases.json table, plus a controller
// that drives a native voice handle. `<SinuaVoiceButton>` (./SinuaVoiceButton.tsx) is the UI.
import type { VoiceSourceHandle } from "./voice";

export type VoiceButtonMode = "toggle" | "pushToTalk";
export type VoiceButtonState = "ready" | "connecting" | "listening" | "muted" | "error";
export type VoiceButtonEvent =
  | { type: "press" }
  | { type: "release" }
  | { type: "end" }
  | { type: "connectOk" }
  | { type: "connectFail"; reason: string }
  | { type: "dropped" }
  | { type: "muteChanged"; muted: boolean };
export type VoiceButtonEffect = "connect" | "disconnect" | "mute" | "unmute";

export interface VoiceButtonModel {
  state: VoiceButtonState;
  reason: string | null;
  held: boolean;
}

export const VOICE_BUTTON_INITIAL: VoiceButtonModel = { state: "ready", reason: null, held: false };

/** One event in, the next model and the effects to run on the source, in order. */
export function voiceButtonStep(
  m: VoiceButtonModel,
  e: VoiceButtonEvent,
  c: { mode: VoiceButtonMode; canMute: boolean },
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

export interface VoiceButtonLabels {
  ready: string;
  connecting: string;
  listening: string;
  muted: string;
  error: string;
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
 * Drives a native voice handle. The native sources all report their connection and can
 * mute, so the session counts as up on `onConnectionChange(true)` (Android's `connect()`
 * resolves while the socket is still opening) and an `onError` while connecting is the
 * failure.
 */
export class VoiceButtonController {
  private model: VoiceButtonModel = VOICE_BUTTON_INITIAL;
  private readonly cbs = new Set<(m: VoiceButtonModel) => void>();
  private readonly offs: Array<() => void>;
  mode: VoiceButtonMode;
  readonly source: VoiceSourceHandle;

  constructor(source: VoiceSourceHandle, mode: VoiceButtonMode = "toggle") {
    this.source = source;
    this.mode = mode;
    this.offs = [
      source.onConnectionChange((up) => this.dispatch(up ? { type: "connectOk" } : { type: "dropped" })),
      source.onError((message) => this.dispatch({ type: "connectFail", reason: message })),
      source.onMuteChange((muted) => this.dispatch({ type: "muteChanged", muted })),
    ];
  }

  get state(): VoiceButtonState {
    return this.model.state;
  }

  get reason(): string | null {
    return this.model.reason;
  }

  readonly canMute = true;

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

  /** A screen reader's activation in push-to-talk: it can't hold, so it toggles (connecting starts live). */
  assistiveActivate(): void {
    if (this.mode === "toggle" || this.state === "ready" || this.state === "error") return this.press();
    const m = this.mode;
    this.mode = "toggle";
    this.press();
    this.mode = m;
  }

  destroy(): void {
    for (const off of this.offs) off();
    this.cbs.clear();
  }

  private dispatch(e: VoiceButtonEvent): void {
    const { model, effects } = voiceButtonStep(this.model, e, { mode: this.mode, canMute: this.canMute });
    const changed = model.state !== this.model.state || model.reason !== this.model.reason || model.held !== this.model.held;
    this.model = model;
    if (changed) for (const cb of this.cbs) cb(model);
    for (const fx of effects) this.run(fx);
  }

  private run(fx: VoiceButtonEffect): void {
    switch (fx) {
      case "connect":
        // Unmuted first: a new session never starts silent from an old mute.
        this.source.setMuted(false);
        this.source.connect().catch((err: unknown) => {
          this.dispatch({ type: "connectFail", reason: err instanceof Error ? err.message : String(err) });
        });
        return;
      case "disconnect":
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
