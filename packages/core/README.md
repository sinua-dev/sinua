# @sinua/core

The sinua geometry engine for the web: a compiled Rust core (WebAssembly) plus a
TypeScript API. It computes the frames; a view draws them.

```bash
npm i @sinua/core
```

```ts
import { frame, resolveFxSpec } from "@sinua/core";

const f = frame("breathing", 64, 1.2);        // dots, lines and polylines at t = 1.2 s
const r = resolveFxSpec(specJson, { state: "listening" });
```

- **No bundler configuration.** The wasm is inlined, so it works in Vite, webpack,
  Angular CLI, a plain `<script type="module">` and node. Pages with a Content Security
  Policy need `'wasm-unsafe-eval'` in `script-src`.
- Drawing is in [`@sinua/web`](../web) (or the native views); this package is the
  engine and the FX Spec resolver.
- Apache-2.0. Third-party code: see `THIRD_PARTY_LICENSES`.
