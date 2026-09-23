// React components for the web (packages/web/src/generated): per object a
// component over `<SinuaView/>`, with the typed params and the pure
// props -> overrides mapping shared with the React Native emitter. The
// component template itself is shared too (react-component.mjs); this file
// is what makes it the web's.
import { emitParams } from "./ts-params.mjs";
import { emitComponent, emitIndex } from "./react-component.mjs";

/** @type {import("./react-component.mjs").Renderer} */
export const RENDERER = {
  preamble: ['import { useEffect } from "react";'],
  viewModule: "../react.js",
  moduleSuffix: ".js",
  effect: "useEffect",
  blankView: "<div className={rest.className} style={rest.style} />",
  // The custom elements live in the same generated/ directory on the web.
  indexTail: (P) => [`export { define${P}Elements, ${P}_ELEMENT_TAGS } from "./${P}Elements.js";`],
};

export function emit(models, naming) {
  return [emitParams(models, naming), ...models.map((m) => emitComponent(m, naming, RENDERER)), emitIndex(models, naming, RENDERER)];
}
