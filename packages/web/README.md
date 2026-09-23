# @sinua/web

The drop-in sinua view for the web: `mount()`, a React `<SinuaView/>`, and the
`<sinua-view>` custom element for Vue, Svelte, Solid, Angular and plain HTML.

```bash
npm i @sinua/web @sinua/core
```

```tsx
import { SinuaView } from "@sinua/web/react";

<SinuaView pattern="breathing" style={{ width: 160, height: 160 }} />;
```

It runs the frame loop, themes, pauses off screen, follows reduced motion, and reacts to
a voice source from [`@sinua/voice`](../voice). Apache-2.0.
