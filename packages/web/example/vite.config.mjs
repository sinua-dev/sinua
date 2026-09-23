import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
// @sinua/core ships wasm-pack's bundler output (a top-level-awaited .wasm import).
export default defineConfig({
  plugins: [wasm()],
  build: { target: "esnext" },
  esbuild: { jsx: "automatic" },
  server: { fs: { allow: ["../..", "../../../spec"] } },
});
