// The base the generated per-object elements register through (`<sinua-orb>`…).
// It owns no painting and no render loop: it puts a `<sinua-view>` in its
// shadow root and sets properties on it, so a typed element behaves exactly
// like the view it wraps -- including pausing off screen, the DPR cap and the
// `fxframe` / `fxerror` events, which are composed and so reach listeners on
// the typed element too.
import { defineSinuaViewElement, type SinuaViewElement } from "./element.js";

/** What a generated element knows about its object (from the parameter catalog). */
export interface TypedElementDefinition {
  /** The engine object, e.g. "orb" -- a spec for anything else is reported. */
  object: string;
  /** Catalog parameter names that are also attributes, kebab-cased: `ring-count` -> `ringCount`. */
  params: readonly string[];
  /** Material group names (`glow`, `color`, …): properties only, since they hold objects. */
  groups: readonly string[];
  /** The generated pure mapping: typed params -> engine overrides. */
  toOverrides: (pattern: string, params: Record<string, unknown>) => Record<string, number>;
  /** The generated spec guard: a message when the spec describes another object. */
  specError: (spec: string | object, expected: string) => string | null;
  /** The view element to wrap; defaults to the one `defineSinuaViewElement()` registers. */
  viewTag?: string;
}

/** Options the wrapped view takes as they are (scalars are attributes, kebab-cased). */
const VIEW_SCALARS = ["state", "size", "speed", "theme", "paused", "reducedMotion", "maxFps", "lowPower", "label", "voiceLevelInput", "crossFade"] as const;
/** Objects and handles: properties only. */
const VIEW_OBJECTS = ["spec", "inputs", "voice"] as const;

const kebab = (name: string) => name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
const num = (text: string) => (text.trim() === "" || Number.isNaN(Number(text)) ? text : Number(text));

export function defineTypedElement(tag: string, def: TypedElementDefinition): void {
  if (typeof customElements === "undefined" || customElements.get(tag)) return;
  const viewTag = def.viewTag ?? "sinua-view";
  defineSinuaViewElement(viewTag);

  const attrToProp = new Map<string, string>();
  for (const name of [...def.params, ...VIEW_SCALARS, "pattern"]) attrToProp.set(kebab(name), name);

  class TypedElement extends HTMLElement {
    static observedAttributes = [...attrToProp.keys()];
    private readonly view: SinuaViewElement;
    private props: Record<string, unknown> = {};
    private queued = false;

    constructor() {
      super();
      const root = this.attachShadow({ mode: "open" });
      root.innerHTML = `<style>:host{display:block;aspect-ratio:1;contain:content}:host([hidden]){display:none}${viewTag}{display:block;width:100%;height:100%}</style>`;
      this.view = document.createElement(viewTag) as SinuaViewElement;
      root.append(this.view);
      // Properties set before the element upgraded shadow the accessors: re-apply them.
      const self = this as unknown as Record<string, unknown>;
      for (const p of [...def.params, ...def.groups, ...VIEW_SCALARS, ...VIEW_OBJECTS, "pattern"]) {
        if (Object.prototype.hasOwnProperty.call(this, p)) {
          const v = self[p];
          delete self[p];
          self[p] = v;
        }
      }
    }

    /** The wrapped `<sinua-view>`, for its `handle` (pause/resume) and `voiceOverrides`. */
    get view$(): SinuaViewElement {
      return this.view;
    }

    attributeChangedCallback(name: string, _old: string | null, text: string | null) {
      const prop = attrToProp.get(name);
      if (!prop) return;
      this.set(prop, text === null ? undefined : num(text));
    }

    set(prop: string, value: unknown) {
      this.props[prop] = value;
      if (this.queued) return;
      this.queued = true;
      queueMicrotask(() => {
        this.queued = false;
        this.apply();
      });
    }

    private apply() {
      const { pattern, spec, ...rest } = this.props as Record<string, unknown> & { pattern?: string; spec?: object | string };
      const view = this.view as unknown as Record<string, unknown>;
      if (spec != null) {
        const message = def.specError(spec as string | object, def.object);
        if (message) {
          this.dispatchEvent(new CustomEvent("fxerror", { detail: [{ message: `<${tag}>: ${message}` }], bubbles: true, composed: true }));
          view.spec = undefined;
          return;
        }
        view.spec = spec;
      }
      const params: Record<string, unknown> = {};
      for (const p of [...def.params, ...def.groups]) if (rest[p] !== undefined) params[p] = rest[p];
      if (pattern !== undefined) {
        view.pattern = pattern;
        view.overrides = def.toOverrides(pattern, params);
      }
      for (const p of [...VIEW_SCALARS, ...VIEW_OBJECTS]) if (rest[p] !== undefined) view[p] = rest[p];
    }
  }

  for (const p of [...def.params, ...def.groups, ...VIEW_SCALARS, ...VIEW_OBJECTS, "pattern"]) {
    Object.defineProperty(TypedElement.prototype, p, {
      configurable: true,
      enumerable: true,
      get(this: TypedElement) {
        return (this as unknown as { props: Record<string, unknown> }).props[p];
      },
      set(this: TypedElement, v: unknown) {
        this.set(p, v);
      },
    });
  }
  customElements.define(tag, TypedElement);
}
