// The ONLY place the component brand lives. Rename = edit here + `node scripts/codegen/generate.mjs`.
// Everything generated (types, tags, file names) derives from these two strings.
export const naming = {
  typePrefix: "Sinua", // SinuaOrb, SinuaRing, SinuaSize, SinuaGlow, ...
  tagPrefix: "sinua-", // <sinua-orb>, <sinua-ring>, ...
};

// Where each emitter writes, relative to the repo root.
export const outputs = {
  swift: "packages/ios/Sources/Sinua/Generated",
  kotlin: "packages/android/view/src/main/kotlin/dev/sinua/view/generated",
  reactNative: "packages/react-native/src/generated",
  // The React components and the custom elements share one directory in the web package.
  react: "packages/web/src/generated",
  webComponent: "packages/web/src/generated",
};

export const catalogPath = "spec/parameters.json";
