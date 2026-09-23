// The browser stubs the @sinua/web tests run against. Extracted from
// mount.test.mjs so element.test.mjs can use the same canvas rather than carry a
// second copy of it.
//
//   env()        window + document + a manual requestAnimationFrame clock
//   canvas()     a 2D context that records what was drawn, and its element
//   elementEnv() the above plus HTMLElement / customElements / CustomEvent,
//                the minimum <sinua-view> needs to upgrade and run
//
// Deliberately hand-written rather than jsdom: these assert exact drawing calls
// and an exact update count, so a stub whose behaviour is visible in this file
// is worth more than a full DOM whose behaviour is not.

export function env({ dpr = 1, dark = false, reduce = false, hidden = false } = {}) {
  const rafs = [];
  globalThis.window = {
    devicePixelRatio: dpr,
    matchMedia: (q) => ({ matches: q.includes("dark") ? dark : q.includes("reduce") ? reduce : false, addEventListener() {}, removeEventListener() {} }),
    requestAnimationFrame: (cb) => (rafs.push(cb), rafs.length),
    cancelAnimationFrame: (id) => (rafs[id - 1] = null),
  };
  globalThis.document = { hidden, addEventListener() {}, removeEventListener() {} };
  let now = 1000;
  // Run queued RAF callbacks, dtMs apart.
  const step = (n = 1, dtMs = 1000 / 60) => {
    for (let i = 0; i < n; i++) {
      now += dtMs;
      const due = rafs.splice(0);
      for (const cb of due) cb?.(now);
    }
  };
  return { rafs, step };
}

export function canvas(css = 100) {
  const calls = [];
  const ctx = {
    save() {}, restore() {}, beginPath() {}, moveTo() {}, lineTo() {}, stroke() {}, fill() {},
    scale(s) { calls.push(["scale", s]); },
    translate() {},
    clearRect() { calls.length = 0; }, // each frame starts with a clear: keep the latest frame only
    arc(x, y, r) { calls.push(["arc", x, y, r]); },
    set fillStyle(v) { calls.push(["fill", v]); },
    set strokeStyle(v) {}, set lineWidth(v) {}, set lineCap(v) {}, set lineJoin(v) {}, set globalAlpha(v) {},
  };
  const attrs = {};
  const el = {
    clientWidth: css, clientHeight: css, width: 300, height: 150,
    getContext: () => ctx,
    setAttribute: (k, v) => (attrs[k] = v),
    removeAttribute: (k) => delete attrs[k],
  };
  ctx.canvas = el;
  return { el, calls, attrs };
}

/**
 * `env()` plus the custom-element side of the DOM. Returns the same `step`
 * clock, plus `define`/`create` helpers so a test reads as the browser would
 * do it: define the tag, make one, connect it.
 */
export function elementEnv(opts = {}) {
  const base = env(opts);
  const registry = new Map();

  class ShadowRoot {
    constructor(host) {
      this.host = host;
      this.children = [];
      this._html = "";
    }
    set innerHTML(html) {
      this._html = html;
      // Only what the element's own template needs: one <canvas>.
      this.children = html.includes("<canvas") ? [canvas().el] : [];
      for (const c of this.children) c.tagName = "CANVAS";
    }
    get innerHTML() {
      return this._html;
    }
    querySelector(sel) {
      return this.children.find((c) => c.tagName?.toLowerCase() === sel.toLowerCase()) ?? null;
    }
  }

  class FakeHTMLElement {
    constructor() {
      // Custom-element *upgrade*: the element already exists in the document and
      // the browser runs the constructor with that object as `this`. Returning it
      // from the base constructor is how JS lets a subclass's `super()` adopt an
      // existing object, which is what `upgrade()` below uses.
      if (FakeHTMLElement.upgrading) {
        const existing = FakeHTMLElement.upgrading;
        FakeHTMLElement.upgrading = null;
        return existing;
      }
      this.shadowRoot = null;
      this._listeners = new Map();
      this.isConnected = false;
    }
    attachShadow() {
      this.shadowRoot = new ShadowRoot(this);
      return this.shadowRoot;
    }
    addEventListener(type, listener) {
      if (!listener) return;
      const list = this._listeners.get(type) ?? [];
      list.push(listener);
      this._listeners.set(type, list);
    }
    removeEventListener(type, listener) {
      if (!listener) return;
      const list = (this._listeners.get(type) ?? []).filter((l) => l !== listener);
      this._listeners.set(type, list);
    }
    dispatchEvent(ev) {
      for (const l of this._listeners.get(ev.type) ?? []) l.call(this, ev);
      return true;
    }
  }

  class FakeCustomEvent {
    constructor(type, init = {}) {
      this.type = type;
      this.detail = init.detail;
      this.bubbles = !!init.bubbles;
      this.composed = !!init.composed;
    }
  }

  globalThis.HTMLElement = FakeHTMLElement;
  globalThis.CustomEvent = FakeCustomEvent;
  globalThis.customElements = {
    define: (tag, ctor) => {
      if (registry.has(tag)) throw new Error(`${tag} already defined`);
      registry.set(tag, ctor);
    },
    get: (tag) => registry.get(tag),
  };

  /** `connect()` / `disconnect()`, i.e. what the DOM does on insert and removal. */
  const withLifecycle = (el) => {
    el.connect = () => {
      el.isConnected = true;
      el.connectedCallback();
    };
    el.disconnect = () => {
      el.isConnected = false;
      el.disconnectedCallback();
    };
    return el;
  };

  /** Construct an element of a defined tag, as `document.createElement` would. */
  const create = (tag = "sinua-view") => {
    const Ctor = registry.get(tag);
    if (!Ctor) throw new Error(`${tag} is not defined`);
    return withLifecycle(new Ctor());
  };

  /** Let the element's queued microtask run. */
  const flush = () => new Promise((r) => queueMicrotask(r));

  /**
   * An element that existed *before* the tag was defined, then upgraded -- what
   * a framework produces when it renders before the element's module loads.
   * `seed` is applied while the accessors are absent, so it lands as an own
   * property that shadows them, exactly as in a browser.
   */
  const upgrade = (seed = {}, tag = "sinua-view") => {
    const Ctor = registry.get(tag);
    if (!Ctor) throw new Error(`${tag} is not defined`);
    const existing = new FakeHTMLElement();
    Object.assign(existing, seed);
    Object.setPrototypeOf(existing, Ctor.prototype);
    FakeHTMLElement.upgrading = existing;
    new Ctor();
    return withLifecycle(existing);
  };

  return { ...base, registry, create, flush, upgrade };
}
