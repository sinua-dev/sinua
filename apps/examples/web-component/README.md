# `<sinua-view>` framework examples

Minimal apps rendering the Web Component (`@sinua/web/element`, docs/fx-view.md "Web Component")
with a spec property, a bound `inputs` property and an `fxframe` listener, in **plain HTML, Vue 3,
Svelte 5, Solid and Angular** (standalone, zoneless, built with the Angular CLI's `ng build`), plus
React 19 typing files. No app configures wasm: `@sinua/core` inlines its engine.

Not part of any other install:

```sh
npm ci            # here
npm run build     # vite build (html, vue, svelte, solid) + ng build (angular)
npm run typecheck # vue-tsc, svelte-check, tsc (Solid, React 19)
npm run smoke     # headless Chrome: draws, fxframe, bound property, destroy on removal, no errors
```

`scripts/ci-local.sh --examples` runs the same.
