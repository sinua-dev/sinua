import solid from "vite-plugin-solid";
// Solid: objects need `prop:`, events `on:` (typed by @sinua/web/types/solid).
export default { plugins: [solid()], build: { target: "esnext" } };
