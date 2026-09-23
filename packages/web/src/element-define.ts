// `import "@sinua/web/element"` -- defines <sinua-view> (a no-op on a
// server) and re-exports the element's types and helpers. See element.ts.
import { defineSinuaViewElement } from "./element.js";

defineSinuaViewElement();

export * from "./element.js";
