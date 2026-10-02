# Remixing a character

Start from a built-in, change what you need, and ship it as one file. No code, no Sinua
release. This guide makes **Latte** from **Cuppa**; the finished file is
[`spec/examples/remix-latte.fxspec.json`](../spec/examples/remix-latte.fxspec.json). Every key is
in the [recipe reference](character-recipe.md).

**The short route:** the Web Studio's Character tab has **Customize**: a form that does each
step below (colours, body from an SVG, parts, face) with a live preview, and exports the file.
This page is the same thing by hand, and what the file holds.

## 1. Copy the recipe

```js
import { characterRecipe } from "@sinua/core";
const recipe = characterRecipe("cuppa"); // the recipe as the engine carries it
```

Or copy `spec/characters/cuppa.json`. Put it in an FX Spec 1.12 file:

```json
{
  "$schema": "https://sinua.dev/schema/fx-spec-1.json",
  "fxSpec": "1.12",
  "object": "character",
  "pattern": "latte",
  "recipe": { "recipe": 1, "id": "latte", "…": "the rest of Cuppa's recipe" }
}
```

With `$schema`, an editor completes and checks every key as you type.

## 2. Name it, and keep how it behaves

- `id`: 1–32 of a–z, 0–9 and -. The file's `pattern` must be the same.
- `"profile": "cuppa"`: keeps Cuppa's voice-state behaviour: how far it turns while thinking,
  where it looks, how it leans. A renamed copy without it takes the shared `character`
  profile, which turns further; on a narrower body the eyes can reach the edge. Keep the
  original's profile unless you want the other one.

## 3. Colours

`palette` holds every colour by name, `[hue, saturation, lightness]`. Latte's mug is cream:

```json
"mug": [34, 0.45, 0.86], "mugLight": [38, 0.6, 0.95], "mugDark": [30, 0.32, 0.72],
"sleeve": [24, 0.45, 0.42], "sleeveDark": [22, 0.45, 0.32], "heart": [30, 0.35, 0.95]
```

- Move a colour's `Light` / `Dark` tones with it, or the body's gradient looks wrong.
- `hue.base` is the hue the palette is drawn at: set it to the new main colour's hue
  (Latte: `34`), so the `hue` option starts from there. `hue.turns` lists the colours that
  option rotates.
- These names are also what apps repaint with `palette` (see *Palette* in
  [character.md](character.md)). `contrast` keeps the eyes readable when they do.

To change only colours, you don't need a recipe at all: `palette` on a built-in does it.

## 4. A body from a drawing

A body part's `shape` can be an SVG path in the 200-unit box. Latte's is a tall glass:

```json
"shape": { "path": "M52 48 C52 40 128 40 128 48 L122 166 C121 174 114 178 106 178 H74 C66 178 59 174 58 166 Z" }
```

The engine takes `M L H V C S Q T Z` (no arcs), and later subpaths are holes (Cuppa's handle
is one). A path from a drawing tool usually has arcs and its own coordinates; `fitPath`
converts it:

```js
import { fitPath, pathBox, svgPaths } from "@sinua/snippets";
const { d } = svgPaths(svgFileText);           // every <path> in the file, joined
const fitted = fitPath(d, [52, 40, 128, 178]); // arcs -> curves, fitted into that box
// fitted.d goes into "shape": { "path": … }; fitted.warnings says what to fix
```

The box is `[x0, y0, x1, y1]` in the 200-unit space. The old body's box
(`pathBox(oldPath)`) keeps the face, the steam and the rig where they were; Latte uses the mug
without its handle. Inner layers (Cuppa's sleeve and heart) are clipped to the new body.

## 5. Parts

`parts` is drawn back to front. Leave out what you don't want: Latte drops the saucer
(`stand`). Some parts come in pairs: a `back` arm before the body and its `front` copy after
it, or ear pods with `front`. Keep or drop both.

- `show` fades a part by voice state: `"show": { "thinking": 1, "speaking": 1 }` draws it only
  then.
- `when: "notSmallOrAccessories"` leaves it out at 20 px with `accessories` off.
- The face: `surfaces.face` (`c`, `r`, `depth`) is where the eyes and mouth sit and how they
  wrap when the head turns.

## 6. Check it

- The engine reports a mistake with its place: `/recipe/parts/2/rise: expected a number`.
  `resolveFxSpec` returns these, and `SinuaView` reports them through `onError`.
- **Limits** (an error, never trimmed): 64 KB, 48 parts, lists of 32, numbers within ±1000;
  a path at most 16 KB and 512 commands.
- Look at all four voice states and at 20 px: the Studio's preview, or the `cost` estimate
  (`fxSpecCost`).

## 7. Ship it

The file is the character. Load it like any FX Spec:

```tsx
<SinuaView spec={latte} />
```

`SinuaCharacter` (and the typed components) take `pattern` and the options; for a recipe,
pass the whole file as `spec`. Apps can still set `palette`, `expression`, `hue`, `tap` and
the other options on top.
