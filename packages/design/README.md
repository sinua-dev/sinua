# @sinua/design

The look the Sinua Studio and the docs site (sinua.dev) share, so a change to a color, a
control or the type scale is made once. Not published: both consumers depend on it by path.

- `tokens.css`: the semantic tokens (`--bg-panel`, `--text-2`, `--accent`, spacing, radii,
  motion), light and dark. Dark keys on `data-theme="dark"` or a `.dark` class on `<html>`.
- `colors.css`: the `@radix-ui/colors` scales the tokens point at.
- `fonts.css`: Inter Variable and JetBrains Mono Variable, self-hosted.
- `components.css` + the React components: `PgTabs` (pill tabs with radio semantics),
  `PgSlider` (a native range under a fill), `Disclosure`, `Snippet` (a code viewer with
  platform tabs), `Icon`, and the segmented control (`.seg` / `.seg-btn`) and thumbnail
  tiles (`.tiles` / `.tile`) as CSS.

Components read only the semantic tokens, never a raw Radix step. React 18 or newer.
