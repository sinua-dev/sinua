# Preparing a character SVG

How to draw a character so that it becomes a Sinua character that animates and wears
cosmetics (design note 28). The convention uses only **layer names and plain shapes**, so it
survives Figma, Illustrator, Inkscape and AI drawing tools. The Sinua Studio imports a file
drawn this way; by hand, the same names map to a recipe's `role`, `slots` and eyes
([`character-recipe.md`](character-recipe.md), [`character-remix.md`](character-remix.md)).

## Export settings

- **Figma:** Export → SVG → ⋯ → turn on **Include "id" attribute**. Without it, Figma drops the
  layer names (repeated names get `_2`, `_3`, which is fine).
- **Illustrator:** layer names are written as `data-name`, which is read the same way.
- **Inkscape:** layer and object names (`id` or `inkscape:label`) are kept.

## Name the groups by what they are

Group your shapes and name each group with a word from this list; the name may carry more
words around it, separated by `-`, `_` or a space (`ear-left`, `Ears`, `arm right paw`):

| Role | What it is |
|---|---|
| `head` | the head (and everything that turns with it, unless it has its own role) |
| `face` | the face area, muzzle, visor |
| `eyes` | drawn eyes: replaced by Sinua's eyes, so they blink and look round |
| `mouth` | a drawn mouth: replaced by Sinua's mouth, so it talks |
| `nose`, `cheeks` | on the face |
| `ears`, `hair`, `antenna` | on the head |
| `arms`, `legs`, `tail`, `neck`, `body` | on the body |
| `shadow` | the ground shadow (stays on the floor) |

A word only counts as a whole word: `earring` is not `ears`, `headphones` is not `head`. A
group with no role word is drawn as part of its parent. Keep the paint order you want: it is
kept exactly.

Faces work best when the eyes and mouth are separate groups (or absent, with eye guides,
below). Arms drawn as separate groups can gesture later; arms merged into the body can't.

## Guides (invisible shapes)

Guides tell Sinua where things go instead of guessing. Draw them as plain shapes, give them
any style (they are never drawn), and name them:

| Name | Shape | Means |
|---|---|---|
| `slot-headTop` | an ellipse across the top of the head | where hats sit; its **width** is the head's width there, its **rotation** the head's tilt |
| `slot-face` | an ellipse across the eyes | where glasses sit; also the face's turning surface |
| `slot-neck` | an ellipse where head meets body | bow ties, scarves. Leave it out if the character has no neck |
| `slot-chest` | an ellipse on the chest | badges |
| `eye-left`, `eye-right` | a circle per eye | where Sinua draws the eyes, for a character drawn without eyes |

A character without guides still imports (a hat goes on the topmost point), and the Studio's
fitting room places the rest by hand.

## Depth

With roles, a cosmetic can sit between parts: `"behind": ["ears"]` puts a beanie behind the
ears so they poke through ([`character-cosmetics.md`](character-cosmetics.md), *Depth*).

## Limits

A character stays within the recipe limits: 96 parts and 4,096 path points in all, 64 KB.
Strokes are turned into filled outlines; a full-size background rectangle is dropped.
