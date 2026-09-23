import vue from "@vitejs/plugin-vue";
// Vue: the one-line setup -- tell the template compiler the tag is a custom element.
export default { plugins: [vue({ template: { compilerOptions: { isCustomElement: (tag) => tag === "sinua-view" } } })], build: { target: "esnext" } };
