// `<sinua-view>` -- the drop-in view as a standard custom element, for any
// framework or none (Vue, Svelte, Angular, Solid, plain HTML; React 19 too).
// It is `mount()` on a canvas in the element's shadow root: no painting here.
//
//   import "@sinua/web/element";              // defines <sinua-view>
//   <sinua-view pattern="breathing"></sinua-view>
//   el.spec = spec; el.inputs = { micLevel: 0.6 }; el.addEventListener("fxframe", …)
//
// Names follow FX Spec 1.7 (docs/fx-spec.md): `pattern` is the visual
// ("breathing"), `state` the app lifecycle key ("listening"). Importing this
// module on a server is safe: nothing touches `window`/`HTMLElement` until
// `defineSinuaViewElement()` runs in a browser (docs/fx-view.md, *Web Component*).
import type { VoiceOverrides } from "@sinua/core";
import { mount, type FxFrameStats, type FxHandle, type SinuaViewOptions } from "./mount.js";

/** The element's properties. The scalar ones are also attributes (kebab-case, see `FX_VIEW_ATTRIBUTES`). */
export interface SinuaViewElementProps {
  /** An FX Spec: object, JSON text, or a JSON import. Takes precedence over `pattern`. As an attribute: JSON text. */
  spec?: object | string | null;
  /** The visual without a spec, e.g. "breathing". */
  pattern?: string | null;
  /** The spec's lifecycle key to render, e.g. "listening" (default: the voice's state). */
  state?: string | null;
  size?: 20 | 32 | 64;
  speed?: number;
  /** Engine overrides for `pattern` (property only). */
  overrides?: Record<string, number> | null;
  /** The spec's binding inputs, e.g. `{ heartRate: 72 }` (property only). */
  inputs?: Record<string, number> | null;
  /** A `VoiceSource` or `VoiceOverrides` (property only; the view never connects it). */
  voice?: SinuaViewOptions["voice"];
  voiceLevelInput?: string | null;
  /** State cross-fade, seconds (default 0.25). */
  crossFade?: number;
  theme?: "auto" | "light" | "dark";
  paused?: boolean;
  reducedMotion?: "auto" | "always" | "never";
  maxFps?: number | null;
  lowPower?: boolean;
  label?: string | null;
}

export interface SinuaViewElementEventMap extends HTMLElementEventMap {
  /** Per drawn frame -- dispatched only while a listener is attached. */
  fxframe: CustomEvent<FxFrameStats>;
  /** The spec didn't resolve (nothing is drawn); `detail`: the diagnostics. */
  fxerror: CustomEvent<unknown[]>;
}

export interface SinuaViewElement extends HTMLElement, SinuaViewElementProps {
  /** The live `mount` handle while connected (pause/resume), else null. */
  readonly handle: FxHandle | null;
  /** The bound VoiceOverrides while connected (a meter, a lifecycle label), else null. */
  readonly voiceOverrides: VoiceOverrides | null;
  addEventListener<K extends keyof SinuaViewElementEventMap>(type: K, listener: (this: SinuaViewElement, ev: SinuaViewElementEventMap[K]) => void, options?: boolean | AddEventListenerOptions): void;
  addEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions): void;
  removeEventListener<K extends keyof SinuaViewElementEventMap>(type: K, listener: (this: SinuaViewElement, ev: SinuaViewElementEventMap[K]) => void, options?: boolean | EventListenerOptions): void;
  removeEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | EventListenerOptions): void;
}

/** Attribute → property and how the text is read. Objects (`overrides`, `inputs`, `voice`) are properties only. */
export const FX_VIEW_ATTRIBUTES: Readonly<Record<string, { prop: keyof SinuaViewElementProps; type: "string" | "number" | "boolean" }>> = {
  spec: { prop: "spec", type: "string" },
  pattern: { prop: "pattern", type: "string" },
  state: { prop: "state", type: "string" },
  size: { prop: "size", type: "number" },
  speed: { prop: "speed", type: "number" },
  "voice-level-input": { prop: "voiceLevelInput", type: "string" },
  "cross-fade": { prop: "crossFade", type: "number" },
  theme: { prop: "theme", type: "string" },
  paused: { prop: "paused", type: "boolean" },
  "reduced-motion": { prop: "reducedMotion", type: "string" },
  "max-fps": { prop: "maxFps", type: "number" },
  "low-power": { prop: "lowPower", type: "boolean" },
  label: { prop: "label", type: "string" },
};

const PROPS: readonly (keyof SinuaViewElementProps)[] = [
  "spec", "pattern", "state", "size", "speed", "overrides", "inputs", "voice", "voiceLevelInput",
  "crossFade", "theme", "paused", "reducedMotion", "maxFps", "lowPower", "label",
];

/** An attribute's text → its property value (`null` = attribute removed). */
export function attributeValue(name: string, text: string | null): unknown {
  const a = FX_VIEW_ATTRIBUTES[name];
  if (!a) return undefined;
  if (a.type === "boolean") return text !== null;
  if (text === null) return undefined;
  return a.type === "number" ? Number(text) : text;
}

/**
 * The element's properties → `mount` options (pure, so it's testable without a
 * DOM). The names are `mount`'s own (FX Spec 1.7): `pattern` is the plain
 * input, `state` the spec's lifecycle state.
 */
export function optionsFromProps(
  p: SinuaViewElementProps,
  hooks: Pick<SinuaViewOptions, "onError" | "onFrame"> = {}
): SinuaViewOptions {
  let spec: SinuaViewOptions["spec"];
  if (typeof p.spec === "string") spec = p.spec.trim() ? p.spec : undefined;
  else if (p.spec) spec = p.spec;
  return {
    spec,
    pattern: p.pattern ?? undefined,
    state: p.state ?? undefined,
    size: p.size,
    speed: p.speed,
    overrides: p.overrides ?? undefined,
    inputs: p.inputs ?? undefined,
    voice: p.voice ?? null,
    voiceLevelInput: p.voiceLevelInput ?? undefined,
    crossFade: p.crossFade,
    theme: p.theme,
    paused: p.paused ?? false,
    reducedMotion: p.reducedMotion,
    maxFps: p.maxFps ?? undefined,
    lowPower: p.lowPower ?? false,
    label: p.label ?? undefined,
    ...hooks,
  };
}

const PROPS_KEY = Symbol("sinua-view props");
const SET_KEY = Symbol("sinua-view set");

/** Built lazily so importing this module never touches `HTMLElement` (SSR). */
function createClass(): CustomElementConstructor {
  class SinuaViewElement extends HTMLElement {
    static observedAttributes = Object.keys(FX_VIEW_ATTRIBUTES);
    [PROPS_KEY]: SinuaViewElementProps = {};
    private fxHandle: FxHandle | null = null;
    private readonly canvas: HTMLCanvasElement;
    private frameListeners = 0;
    private queued = false;

    constructor() {
      super();
      const root = this.attachShadow({ mode: "open" });
      root.innerHTML =
        "<style>:host{display:block;aspect-ratio:1;contain:content}:host([hidden]){display:none}" +
        "canvas{display:block;width:100%;height:100%}</style><canvas part=\"canvas\"></canvas>";
      this.canvas = root.querySelector("canvas") as HTMLCanvasElement;
      // Properties a framework set before the element upgraded shadow the accessors: re-apply them.
      const self = this as unknown as Record<string, unknown>;
      for (const p of PROPS) {
        if (Object.prototype.hasOwnProperty.call(this, p)) {
          const v = self[p];
          delete self[p];
          self[p] = v;
        }
      }
    }

    get handle(): FxHandle | null {
      return this.fxHandle;
    }

    get voiceOverrides(): VoiceOverrides | null {
      return this.fxHandle?.voice ?? null;
    }

    connectedCallback() {
      if (!this.fxHandle) this.fxHandle = mount(this.canvas, this.options());
    }

    disconnectedCallback() {
      this.fxHandle?.destroy();
      this.fxHandle = null;
    }

    attributeChangedCallback(name: string, _old: string | null, text: string | null) {
      const a = FX_VIEW_ATTRIBUTES[name];
      if (a) this[SET_KEY](a.prop, attributeValue(name, text));
    }

    addEventListener(type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | AddEventListenerOptions) {
      if (!listener) return;
      super.addEventListener(type, listener, options);
      if (type === "fxframe") {
        this.frameListeners++;
        this.schedule();
      }
    }

    removeEventListener(type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | EventListenerOptions) {
      if (!listener) return;
      super.removeEventListener(type, listener, options);
      if (type === "fxframe" && this.frameListeners > 0) {
        this.frameListeners--;
        this.schedule();
      }
    }

    [SET_KEY](prop: keyof SinuaViewElementProps, value: unknown) {
      (this[PROPS_KEY] as Record<string, unknown>)[prop] = value;
      this.schedule();
    }

    /** One `update()` per microtask, however many properties changed. */
    private schedule() {
      if (this.queued) return;
      this.queued = true;
      queueMicrotask(() => {
        this.queued = false;
        this.fxHandle?.update(this.options());
      });
    }

    private options(): SinuaViewOptions {
      const fire = (type: string, detail: unknown) => this.dispatchEvent(new CustomEvent(type, { detail, bubbles: true, composed: true }));
      return optionsFromProps(this[PROPS_KEY], {
        onError: (diagnostics) => fire("fxerror", diagnostics),
        onFrame: this.frameListeners > 0 ? (stats) => fire("fxframe", stats) : undefined,
      });
    }
  }

  for (const p of PROPS) {
    Object.defineProperty(SinuaViewElement.prototype, p, {
      configurable: true,
      enumerable: true,
      get(this: SinuaViewElement) {
        return (this[PROPS_KEY] as Record<string, unknown>)[p];
      },
      set(this: SinuaViewElement, v: unknown) {
        this[SET_KEY](p, v);
      },
    });
  }
  return SinuaViewElement;
}

/**
 * Registers the element (default tag `sinua-view`). Idempotent, and a no-op
 * where there is no `customElements` (a server, a worker).
 */
export function defineSinuaViewElement(tag = "sinua-view"): void {
  if (typeof customElements === "undefined" || customElements.get(tag)) return;
  customElements.define(tag, createClass());
}

declare global {
  interface HTMLElementTagNameMap {
    "sinua-view": SinuaViewElement;
  }
}
