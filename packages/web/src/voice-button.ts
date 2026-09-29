// `<sinua-voice-button>` -- a mic button bound to a voice source (docs/fx-view.md,
// *Voice button*). The logic is `VoiceButtonController` from @sinua/core (the same
// state table iOS and Android test); this file is the DOM: a real <button>, an
// engine-drawn ring behind the icon, and a small ✕ that ends the session.
//
//   import { defineSinuaVoiceButtonElement } from "@sinua/web/voice-button";
//   defineSinuaVoiceButtonElement();
//   <sinua-voice-button mode="toggle"></sinua-voice-button>
//   el.source = source;   // the same source you give <sinua-view>
//
// Importing this module on a server is safe: nothing touches `HTMLElement` until
// `defineSinuaVoiceButtonElement()` runs in a browser.
import {
  DEFAULT_VOICE_BUTTON_LABELS,
  VoiceButtonController,
  voiceButtonHint,
  type VoiceButtonLabels,
  type VoiceButtonMode,
  type VoiceButtonState,
  type VoiceSource,
} from "@sinua/core";
import { mount, type FxHandle, type SinuaViewOptions } from "./mount.js";

/** Hold this long on the button to end the session (also the ✕ and Escape). */
export const VOICE_BUTTON_LONG_PRESS_MS = 600;

export interface VoiceButtonChangeDetail {
  state: VoiceButtonState;
  /** The failure's message in `error`. */
  reason: string | null;
}

/** A closed, thin ring: at button size the default stroke reads as a heavy outline. */
const RING = { progress: 1, strokeWidth: 0.045 };
/**
 * The ring sits on the button's edge (its canvas is 128% of the button) and swells
 * outward with the level: 0.75 reaches the canvas edge at full level. The views'
 * default (0.18) barely moves a ring this small.
 */
const RING_VOICE = { audioStrength: 0.75 };

/** The inner ring per state: an engine view, or none (icon only). */
export function voiceButtonRing(state: VoiceButtonState): Pick<SinuaViewOptions, "pattern" | "state" | "overrides" | "speed"> | null {
  switch (state) {
    case "connecting":
      return { pattern: "loading", state: "initializing", overrides: { strokeWidth: 0.045, trackOpacity: 0.15 } };
    case "listening":
      return { pattern: "completing", state: "listening", overrides: RING };
    case "muted":
      // Same ring, the muted cue (from the shared source), barely moving.
      return { pattern: "completing", state: "listening", overrides: RING, speed: 0.15 };
    default:
      return null;
  }
}

const ICONS: Record<"mic" | "micOff" | "alert", string> = {
  mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21"/>',
  micOff: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M4 4l16 16"/>',
  alert: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v5.5M12 16.2v.3"/>',
};

const STYLE = `
:host{--sinua-vb-bg:#f1f1f4;--sinua-vb-fg:#1c1c21;--sinua-vb-muted:#8a8a93;--sinua-vb-error:#c93b3b;
  display:inline-block;position:relative;width:56px;height:56px;vertical-align:middle;contain:layout}
:host([hidden]){display:none}
@media (prefers-color-scheme:dark){:host(:not([theme="light"])){--sinua-vb-bg:#26262c;--sinua-vb-fg:#ececf0;--sinua-vb-muted:#8d8d96;--sinua-vb-error:#ef6a6a}}
:host([theme="dark"]){--sinua-vb-bg:#26262c;--sinua-vb-fg:#ececf0;--sinua-vb-muted:#8d8d96;--sinua-vb-error:#ef6a6a}
.main{all:unset;box-sizing:border-box;position:relative;display:grid;place-items:center;width:100%;height:100%;
  border-radius:50%;background:var(--sinua-vb-bg);color:var(--sinua-vb-fg);cursor:pointer;touch-action:none;
  -webkit-tap-highlight-color:transparent;user-select:none;-webkit-user-select:none}
.main:focus-visible{outline:2px solid var(--sinua-vb-fg);outline-offset:3px}
.main[data-state="muted"]{color:var(--sinua-vb-muted)}
.main[data-state="error"]{color:var(--sinua-vb-error)}
.main[data-state="connecting"]{cursor:progress}
canvas{position:absolute;inset:-14%;width:128%;height:128%;pointer-events:none}
canvas[hidden]{display:none}
svg{position:relative;width:44%;height:44%;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round}
.end{all:unset;box-sizing:border-box;position:absolute;top:-6px;right:-6px;width:22px;height:22px;border-radius:50%;
  display:grid;place-items:center;background:var(--sinua-vb-bg);color:var(--sinua-vb-fg);cursor:pointer;
  box-shadow:0 0 0 2px color-mix(in srgb,var(--sinua-vb-fg) 18%,transparent)}
.end:focus-visible{outline:2px solid var(--sinua-vb-fg);outline-offset:2px}
.end[hidden]{display:none}
.end svg{width:12px;height:12px;stroke-width:2.4}
.hint{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
`;

/** The element's properties. `mode` and `theme` are also attributes. */
export interface SinuaVoiceButtonElementProps {
  /** The voice source; give your `<sinua-view>` the same one (the button shares it, never steals it). */
  source?: VoiceSource | null;
  /** `toggle` (default) or `push-to-talk` (attribute) / `pushToTalk` (property). */
  mode?: VoiceButtonMode;
  /** Accessible names per state; missing ones fall back to the defaults. */
  labels?: Partial<VoiceButtonLabels>;
  theme?: "auto" | "light" | "dark";
}

export type SinuaVoiceButtonElement = HTMLElement &
  SinuaVoiceButtonElementProps & {
    /** The current state. */
    readonly state: VoiceButtonState;
    /** The controller, for tests and custom UI around it. */
    readonly controller: VoiceButtonController | null;
  };

function modeOf(v: string | null | undefined): VoiceButtonMode {
  return v === "push-to-talk" || v === "pushToTalk" ? "pushToTalk" : "toggle";
}

/** Built lazily so importing this module never touches `HTMLElement` (SSR). */
function createClass(): CustomElementConstructor {
  class VoiceButton extends HTMLElement {
    static observedAttributes = ["mode", "theme"];
    private src: VoiceSource | null = null;
    private ctl: VoiceButtonController | null = null;
    private offChange: (() => void) | null = null;
    private ring: FxHandle | null = null;
    private ringKey = "";
    private labelSet: Partial<VoiceButtonLabels> = {};
    private buttonMode: VoiceButtonMode = "toggle";
    private themeValue: "auto" | "light" | "dark" = "auto";
    private longPress: ReturnType<typeof setTimeout> | null = null;
    private swallowClick = false;
    private keyHeld = false;
    private pointerDown = false;
    private readonly main: HTMLButtonElement;
    private readonly end: HTMLButtonElement;
    private readonly canvas: HTMLCanvasElement;
    private readonly icon: SVGSVGElement;
    private readonly hint: HTMLElement;

    constructor() {
      super();
      const root = this.attachShadow({ mode: "open", delegatesFocus: true });
      root.innerHTML =
        `<style>${STYLE}</style>` +
        '<button class="main" part="button" type="button" aria-describedby="hint">' +
        '<canvas part="ring" aria-hidden="true" hidden></canvas>' +
        '<svg viewBox="0 0 24 24" aria-hidden="true" part="icon"></svg></button>' +
        '<button class="end" part="end" type="button" hidden>' +
        '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg></button>' +
        '<span class="hint" id="hint"></span>';
      this.main = root.querySelector(".main") as HTMLButtonElement;
      this.end = root.querySelector(".end") as HTMLButtonElement;
      this.canvas = root.querySelector("canvas") as HTMLCanvasElement;
      this.icon = root.querySelector(".main svg") as SVGSVGElement;
      this.hint = root.querySelector(".hint") as HTMLElement;
      this.main.addEventListener("pointerdown", this.onPointerDown);
      this.main.addEventListener("pointerup", this.onPointerUp);
      this.main.addEventListener("pointercancel", this.onPointerUp);
      this.main.addEventListener("pointerleave", this.onPointerUp);
      this.main.addEventListener("click", this.onClick);
      this.main.addEventListener("keydown", this.onKeyDown);
      this.main.addEventListener("keyup", this.onKeyUp);
      this.main.addEventListener("contextmenu", (e) => e.preventDefault());
      this.end.addEventListener("click", () => this.ctl?.end());
      // Properties a framework set before the element upgraded shadow the accessors: re-apply them.
      const self = this as unknown as Record<string, unknown>;
      for (const p of ["source", "mode", "labels", "theme"]) {
        if (Object.prototype.hasOwnProperty.call(this, p)) {
          const v = self[p];
          delete self[p];
          self[p] = v;
        }
      }
      this.render();
    }

    get source(): VoiceSource | null {
      return this.src;
    }

    set source(v: VoiceSource | null) {
      if (v === this.src) return;
      this.src = v ?? null;
      this.bind();
    }

    get mode(): VoiceButtonMode {
      return this.buttonMode;
    }

    set mode(v: VoiceButtonMode) {
      this.buttonMode = modeOf(v);
      this.ctl?.setMode(this.buttonMode);
      this.render();
    }

    get labels(): Partial<VoiceButtonLabels> {
      return this.labelSet;
    }

    set labels(v: Partial<VoiceButtonLabels>) {
      this.labelSet = v ?? {};
      this.render();
    }

    get theme(): "auto" | "light" | "dark" {
      return this.themeValue;
    }

    set theme(v: "auto" | "light" | "dark") {
      this.themeValue = v === "light" || v === "dark" ? v : "auto";
      this.ring?.update({ theme: this.themeValue });
    }

    get state(): VoiceButtonState {
      return this.ctl?.state ?? "ready";
    }

    get controller(): VoiceButtonController | null {
      return this.ctl;
    }

    connectedCallback() {
      this.bind();
    }

    disconnectedCallback() {
      this.unbind();
    }

    attributeChangedCallback(name: string, _old: string | null, text: string | null) {
      if (name === "mode") this.mode = modeOf(text);
      if (name === "theme") this.theme = (text ?? "auto") as "auto" | "light" | "dark";
    }

    private bind() {
      this.unbind();
      if (!this.src || !this.isConnected) {
        this.render();
        return;
      }
      this.ctl = new VoiceButtonController(this.src, this.buttonMode);
      this.offChange = this.ctl.onChange((m) => {
        this.render();
        this.dispatchEvent(
          new CustomEvent<VoiceButtonChangeDetail>("voicebuttonchange", {
            detail: { state: m.state, reason: m.reason },
            bubbles: true,
            composed: true,
          }),
        );
      });
      this.render();
    }

    private unbind() {
      this.clearLongPress();
      this.offChange?.();
      this.offChange = null;
      this.ctl?.destroy();
      this.ctl = null;
      this.ring?.destroy();
      this.ring = null;
      this.ringKey = "";
    }

    private render() {
      const state = this.state;
      const labels = { ...DEFAULT_VOICE_BUTTON_LABELS, ...this.labelSet };
      const reason = this.ctl?.reason;
      this.main.dataset.state = state;
      this.main.disabled = !this.src;
      this.main.setAttribute("aria-label", state === "error" && reason ? `${labels.error}: ${reason}` : labels[state]);
      if (state === "listening" || state === "muted") this.main.setAttribute("aria-pressed", String(state === "muted"));
      else this.main.removeAttribute("aria-pressed");
      if (state === "connecting") this.main.setAttribute("aria-busy", "true");
      else this.main.removeAttribute("aria-busy");
      this.hint.textContent = voiceButtonHint(state, this.buttonMode, this.ctl?.canMute ?? false);
      this.icon.innerHTML = ICONS[state === "error" ? "alert" : state === "muted" ? "micOff" : "mic"];
      const live = state === "connecting" || state === "listening" || state === "muted";
      this.end.hidden = !live;
      this.end.setAttribute("aria-label", labels.end);
      this.renderRing(state);
    }

    private renderRing(state: VoiceButtonState) {
      const ring = voiceButtonRing(state);
      const key = ring ? JSON.stringify(ring) : "";
      if (key === this.ringKey) return;
      this.ringKey = key;
      this.canvas.hidden = !ring;
      if (!ring) {
        this.ring?.pause();
        return;
      }
      const opts: SinuaViewOptions = {
        ...ring,
        voice: this.src,
        voiceOptions: RING_VOICE,
        theme: this.themeValue,
        label: "",
        crossFade: 0.3,
      };
      if (this.ring) {
        this.ring.update(opts);
        this.ring.resume();
      } else this.ring = mount(this.canvas, opts);
    }

    private readonly onPointerDown = (e: PointerEvent) => {
      if (e.button !== 0 || !this.ctl) return;
      this.pointerDown = true;
      this.main.setPointerCapture?.(e.pointerId);
      this.swallowClick = false;
      if (this.buttonMode === "pushToTalk") this.ctl.press();
      this.clearLongPress();
      this.longPress = setTimeout(() => {
        this.longPress = null;
        if (this.state === "ready" || this.state === "error") return;
        this.swallowClick = true;
        this.ctl?.end();
      }, VOICE_BUTTON_LONG_PRESS_MS);
    };

    private readonly onPointerUp = () => {
      if (!this.pointerDown) return;
      this.pointerDown = false;
      this.clearLongPress();
      if (this.buttonMode === "pushToTalk") this.ctl?.release();
    };

    private readonly onClick = (e: MouseEvent) => {
      if (this.swallowClick) {
        this.swallowClick = false;
        return;
      }
      if (!this.ctl) return;
      if (this.buttonMode === "toggle") {
        this.ctl.press();
        return;
      }
      // Push-to-talk handles the pointer and keys itself. A click with neither --
      // a screen reader's activation -- can't hold, so it toggles instead.
      if (e.detail === 0 && !this.keyHeld) this.assistiveToggle();
    };

    private readonly onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (this.state !== "ready") this.ctl?.end();
        return;
      }
      if (this.buttonMode !== "pushToTalk" || (e.key !== " " && e.key !== "Enter")) return;
      e.preventDefault(); // no synthetic click
      if (e.repeat || this.keyHeld) return;
      this.keyHeld = true;
      this.ctl?.press();
    };

    private readonly onKeyUp = (e: KeyboardEvent) => {
      if (this.buttonMode !== "pushToTalk" || (e.key !== " " && e.key !== "Enter")) return;
      e.preventDefault();
      if (!this.keyHeld) return;
      this.keyHeld = false;
      this.ctl?.release();
    };

    private assistiveToggle() {
      const c = this.ctl;
      if (!c) return;
      // Connecting: a press that is never released, so the session starts live.
      if (c.state === "ready" || c.state === "error") return c.press();
      c.setMode("toggle");
      c.press();
      c.setMode(this.buttonMode);
    }

    private clearLongPress() {
      if (this.longPress != null) clearTimeout(this.longPress);
      this.longPress = null;
    }
  }
  return VoiceButton;
}

/**
 * Registers the element (default tag `sinua-voice-button`). Idempotent, and a
 * no-op where there is no `customElements` (a server, a worker).
 */
export function defineSinuaVoiceButtonElement(tag = "sinua-voice-button"): void {
  if (typeof customElements === "undefined" || customElements.get(tag)) return;
  customElements.define(tag, createClass());
}

declare global {
  interface HTMLElementTagNameMap {
    "sinua-voice-button": SinuaVoiceButtonElement;
  }
  interface HTMLElementEventMap {
    voicebuttonchange: CustomEvent<VoiceButtonChangeDetail>;
  }
}
