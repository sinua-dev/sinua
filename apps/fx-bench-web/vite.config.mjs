import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
// @sinua/core ships wasm-pack's bundler output (a top-level-awaited .wasm import).
export default defineConfig({
  plugins: [wasm()],
  build: { target: "esnext" },
  // The cases live in the repo's spec/bench/ (shared with the iOS and Android bench apps).
  server: { fs: { allow: ["../.."] } },
});
