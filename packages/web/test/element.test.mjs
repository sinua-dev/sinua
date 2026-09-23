// <sinua-view> in plain node (no DOM): importing is SSR-safe, define() is a
// no-op without customElements, and the attribute / FX Spec 1.7 name mapping
// is right. The real DOM lifecycle is verified in a browser (docs/fx-view.md).
import { test } from "node:test";
import assert from "node:assert/strict";

test("importing @sinua/web/element touches no DOM and defines nothing", async () => {
  assert.equal(typeof globalThis.window, "undefined");
  assert.equal(typeof globalThis.HTMLElement, "undefined");
  const m = await import("../dist/element-define.js");
  assert.equal(typeof m.defineSinuaViewElement, "function");
  assert.doesNotThrow(() => m.defineSinuaViewElement());
  assert.doesNotThrow(() => m.defineSinuaViewElement("sinua-orb-test"));
});

test("attributes read as their types", async () => {
  const { attributeValue } = await import("../dist/element.js");
  assert.equal(attributeValue("pattern", "breathing"), "breathing");
  assert.equal(attributeValue("size", "32"), 32);
  assert.equal(attributeValue("max-fps", "24"), 24);
  assert.equal(attributeValue("paused", ""), true);
  assert.equal(attributeValue("paused", null), false);
  assert.equal(attributeValue("low-power", "false"), true, "boolean attributes: presence means true, like HTML's");
  assert.equal(attributeValue("theme", null), undefined);
  assert.equal(attributeValue("unknown", "x"), undefined);
});

test("FX Spec 1.7 names map to mount's options", async () => {
  const { optionsFromProps } = await import("../dist/element.js");
  const o = optionsFromProps({ pattern: "breathing", state: "listening", crossFade: 0.5, maxFps: 30, lowPower: true, inputs: { micLevel: 0.6 } });
  assert.equal(o.pattern, "breathing");
  assert.equal(o.state, "listening");
  assert.equal(o.crossFade, 0.5);
  assert.equal(o.maxFps, 30);
  assert.equal(o.lowPower, true);
  assert.deepEqual(o.inputs, { micLevel: 0.6 });
  assert.equal(o.paused, false);
  assert.equal(optionsFromProps({ spec: "  " }).spec, undefined, "a blank spec attribute is no spec");
  const spec = { fxSpec: "1.8", object: "orb", pattern: "working", size: 64 };
  assert.equal(optionsFromProps({ spec }).spec, spec);
  const onFrame = () => {};
  assert.equal(optionsFromProps({}, { onFrame }).onFrame, onFrame);
});

// ---- The element itself ---------------------------------------------------
// Everything above this line tests the two pure exports. The custom element's
// own behaviour -- shadow root, lifecycle, attribute plumbing, the microtask
// batching, the fxframe listener count -- was untested, which made <sinua-view>
// the least-covered public entry point in the package despite being the one the
// docs point every non-React framework at. `mount()` is real here, not stubbed:
// these run the element against the actual mount path.
import { elementEnv } from "./dom.mjs";

/** A fresh element environment with `<sinua-view>` defined in it. */
async function defined(opts = {}) {
  const e = elementEnv(opts);
  const { defineSinuaViewElement } = await import("../dist/element.js");
  defineSinuaViewElement();
  return e;
}

test("element: upgrading builds a shadow root with one canvas", async () => {
  const e = await defined();
  const el = e.create();
  assert.ok(el.shadowRoot, "attachShadow ran");
  assert.ok(el.shadowRoot.querySelector("canvas"), "the template contains a canvas");
  assert.equal(el.handle, null, "nothing is mounted before connect");
  assert.equal(el.voiceOverrides, null);
});

test("element: connect mounts and draws, disconnect destroys", async () => {
  const e = await defined();
  const el = e.create();
  el.pattern = "breathing";
  el.connect();
  assert.ok(el.handle, "connectedCallback mounted");
  e.step(3);
  assert.ok(el.shadowRoot.querySelector("canvas") !== null);
  el.disconnect();
  assert.equal(el.handle, null, "disconnectedCallback destroyed the handle");
});

test("element: many property changes produce exactly one update per microtask", async () => {
  const e = await defined();
  const el = e.create();
  el.connect();
  let updates = 0;
  const real = el.handle.update;
  el.handle.update = (...a) => {
    updates++;
    return real.apply(el.handle, a);
  };
  el.pattern = "breathing";
  el.size = 32;
  el.speed = 2;
  el.paused = true;
  assert.equal(updates, 0, "nothing is applied synchronously");
  await e.flush();
  assert.equal(updates, 1, "four property sets, one update");
  el.disconnect();
});

test("element: attributes reach the options through the same path", async () => {
  const e = await defined();
  const el = e.create();
  el.connect();
  el.attributeChangedCallback("pattern", null, "breathing");
  el.attributeChangedCallback("size", null, "32");
  el.attributeChangedCallback("paused", null, "");
  await e.flush();
  assert.equal(el.pattern, "breathing");
  assert.equal(el.size, 32, "read as a number, not the string");
  assert.equal(el.paused, true, "a present boolean attribute is true");
  el.attributeChangedCallback("unknown-attr", null, "x");
  await e.flush();
  el.disconnect();
});

test("element: a property set before upgrade survives it", async () => {
  // What a framework does when it renders before the element's module has
  // loaded: the assignment lands as an own property that shadows the accessor,
  // and the constructor's re-apply loop has to move it back.
  const e = await defined();
  const el = e.upgrade({ pattern: "breathing", size: 32 });
  assert.equal(el.pattern, "breathing", "the constructor re-applied it through the accessor");
  assert.equal(el.size, 32);
  assert.ok(!Object.prototype.hasOwnProperty.call(el, "pattern"), "and it no longer shadows");
  el.connect();
  e.step(2);
  assert.ok(el.handle, "the upgraded element mounts with the pre-set props");
  el.disconnect();
});

test("element: fxframe listeners turn onFrame on and off", async () => {
  const e = await defined();
  const el = e.create();
  el.pattern = "breathing"; // without something to draw there are no frames to report
  el.connect();
  let frames = 0;
  const onFrame = () => frames++;
  el.addEventListener("fxframe", onFrame);
  await e.flush();
  e.step(3);
  assert.ok(frames > 0, "a listener switches frame events on");
  const seen = frames;
  el.removeEventListener("fxframe", onFrame);
  await e.flush();
  e.step(3);
  assert.equal(frames, seen, "removing the last listener switches them back off");
  assert.doesNotThrow(() => el.addEventListener("fxframe", null), "a null listener is ignored");
  assert.doesNotThrow(() => el.removeEventListener("fxframe", null));
  el.disconnect();
});

test("element: an unresolvable spec dispatches fxerror with the diagnostics", async () => {
  const e = await defined();
  const el = e.create();
  const errors = [];
  el.addEventListener("fxerror", (ev) => errors.push(ev.detail));
  el.spec = '{"fxSpec":"1.8","object":"nope","pattern":"working","size":64}';
  el.connect();
  await e.flush();
  // Measured, not assumed: two events, because two render attempts fail -- the
  // mount on connect, then the update from the queued property change. Each
  // attempt reporting is defensible; a consumer should treat fxerror as "this
  // render failed", not "the spec changed".
  assert.equal(errors.length, 2, "one per failed render attempt");
  for (const detail of errors) {
    assert.ok(Array.isArray(detail) && detail.length > 0, "each carries the diagnostics");
  }
  el.disconnect();
});

test("element: defineSinuaViewElement is idempotent and takes a custom tag", async () => {
  const e = await defined();
  const { defineSinuaViewElement } = await import("../dist/element.js");
  assert.doesNotThrow(() => defineSinuaViewElement(), "defining twice is a no-op, not a throw");
  defineSinuaViewElement("my-orb");
  assert.ok(e.registry.get("my-orb"), "a custom tag registers");
  assert.ok(e.create("my-orb").shadowRoot, "and builds the same element");
});
