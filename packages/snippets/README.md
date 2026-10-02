# @sinua/snippets

The code the Studio's export drawer prints, as a library: the same FxView snippet text per
platform (Web, React, SwiftUI, Compose, React Native) and the `.fxspec.json` file variant,
so the docs site and the Studio can't drift.

```bash
npm i @sinua/snippets
```

Also `fitPath`, `pathBox` and `svgPaths`: an SVG path from a drawing tool (relative
commands, arcs) turned into a character recipe's shape and fitted into a box of its
200-unit space ([docs/character-remix.md](../../docs/character-remix.md)).

TypeScript only, with no engine dependency. Apache-2.0.
