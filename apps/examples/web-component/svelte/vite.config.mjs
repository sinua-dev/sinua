import { svelte } from "@sveltejs/vite-plugin-svelte";
// Svelte: no setup -- it sets properties the element has.
export default { plugins: [svelte()], build: { target: "esnext" } };
