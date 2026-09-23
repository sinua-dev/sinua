// No wasm setup: @sinua/core inlines its engine.
export default { build: { target: "esnext" }, server: { fs: { allow: ["..", "../../../../packages"] } } };
