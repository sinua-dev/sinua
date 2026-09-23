// React Native components (packages/react-native/src/generated): per object a component over SinuaView;
// the types and the pure props -> overrides mapping live in one dependency-free .ts file. The component
// template is shared with the web (react-component.mjs); this file is what makes it React Native's.
import { emitParams } from "./ts-params.mjs";
import { emitComponent, emitIndex } from "./react-component.mjs";

/** @type {import("./react-component.mjs").Renderer} */
export const RENDERER = {
  preamble: ['import * as React from "react";', 'import { View } from "react-native";'],
  viewModule: "../SinuaView",
  moduleSuffix: "",
  effect: "React.useEffect",
  blankView: "<View style={rest.style} />",
  indexTail: () => [],
};

export function emit(models, naming) {
  return [emitParams(models, naming), ...models.map((m) => emitComponent(m, naming, RENDERER)), emitIndex(models, naming, RENDERER)];
}
