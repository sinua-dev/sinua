# Character cosmetics (FX Spec 1.13)

A character can **wear things**: a hat, glasses, a scarf, a badge. A cosmetic is data in the
FX Spec file, drawn by the engine, so it looks the same on Web, iOS, Android and React
Native, moves with the character, and needs no Sinua release. The engine ships no cosmetic of
its own; a file carries each one it uses.

This page builds a party hat (`spec/examples/party-hat.fxspec.json`). Every field is in the
[recipe reference](character-recipe.md), *Slots and cosmetics*; the file key is in
[`fx-spec.md`](fx-spec.md), *v1.13*.

## A hat in one file

```json
{
  "fxSpec": "1.13",
  "object": "character",
  "pattern": "bean",
  "cosmetics": [{
    "id": "party-hat",
    "label": "party hat",
    "slot": "headTop",
    "palette": {
      "felt": [330, 0.72, 0.62], "feltDark": [326, 0.62, 0.48],
      "trim": [48, 0.95, 0.62], "line": [330, 0.5, 0.2]
    },
    "parts": [
      { "part": "body",
        "shape": { "path": "M-20 2 L0 -33 L20 2 Q0 8 -20 2 Z" },
        "light": { "linear": [-20, 0, 20, 0], "stops": [[0, "felt"], [1, "feltDark"]] },
        "inner": [{ "part": "band", "shape": { "roundRect": [-24, -6, 48, 6, 0, 4] }, "color": "trim" }],
        "outline": { "width": 2.5, "color": "line" } },
      { "part": "body",
        "shape": { "ellipse": [0, -35, 5, 5, 0, 20] },
        "light": { "radial": [-1.5, -36.5, 7], "stops": [[0, "trim"], [1, "trim"]] },
        "outline": { "width": 2, "color": "line" } }
    ]
  }]
}
```

Change `pattern` to any of the seven characters and the same hat sits on that head.

## 1. Pick a slot

Every character has up to four **slots**:
- `headTop`: the top of the head;
- `face`: between the eyes;
- `neck`;
- `chest`.

A cosmetic names one. It moves with that slot:
- on `headTop` with the pose and the tap hop;
- on `face` also with the head turn (glasses slide round the head);
- on `neck` and `chest` with the body.

Cuppa, Bean and Beep have no `neck`. A cosmetic for a slot a character doesn't have isn't
drawn on it: you get a warning, and the plain character draws.

## 2. Draw it in the slot's units

The parts use the same library as a recipe, limited to **`body`** (any shape: `ellipse`,
`roundRect` or an SVG `path`, with its layers `patch`, `band`, `stripes`, `glints`) and
**`eyes`**. They have no `space` and no `surface`, because a cosmetic always draws in its slot.

- The **slot point is (0, 0)**, and up is −y.
- On `headTop`, draw for a head about **80 units wide**, with **40 units of height** for the
  hat. Each character's slot `scale` makes that fit its own head.
- The parts draw **on top of** the character, in list order.

To bring an SVG from a drawing tool in, draw it in a 200 × 200 box with the slot point at
(100, 100). `fitPath` in `@sinua/snippets` turns arcs into curves. Then subtract 100 from
every coordinate, or draw it around (0, 0) to begin with.

## 3. Colours

- The cosmetic's own `palette` uses the recipe's colour format, `[hue, saturation, lightness]`.
- Its parts name these colours by their short name (`felt`). A cosmetic colour hides the
  character's colour of the same name.
- In the character's palette they are **`<id>.<name>`**, so an app or brand can repaint the
  hat without touching it: `"palette": { "party-hat.felt": "#2E8B57" }`.
- A part may also name the **character's own** colours (`mug` on Cuppa). Such a cosmetic then
  follows `hue` and `palette`, but it only fits characters that have that colour.

## 4. Fit

The character decides how big and how turned a cosmetic is, through its slot's `scale` and
`angle`. A cosmetic can still adjust:
- `"fits": ["cuppa", "bean"]`: only these characters wear it. The others warn and draw
  plain.
- `"fit": { "hum": { "at": [0, 4], "scale": 0.9, "angle": 0.1 } }`: a nudge on one
  character. `at` is in local units; `scale` multiplies and `angle` (radians) adds.

## 5. Room for a hat

The characters fill their 200-unit box, and some have little room above the head (HUM about
20 units). A cosmetic on `headTop` therefore **zooms the whole character out** about its feet,
just enough that the hat and the tap hop fit in the box:

| buzzy | hum | wisp | chirp | cuppa | bean | beep |
|---|---|---|---|---|---|---|
| 7 % | 15 % | 6 % | 0 | 6 % | 7 % | 9 % |

Taking a hat on or off changes the character's size. In 1.13 a cosmetic is file-wide; a soft
change between loadouts comes with the loadout API.

## Size and small screens

- Cosmetics are left out at **20 px** unless `accessories` is on; a part's own `when` can say
  otherwise.
- The recipe limits count the character with its cosmetics: 48 parts and 64 KB.
- The party hat leaves every character "light" in the cost estimate. The zoom makes the
  character's own fills smaller, so the cost often goes down.

## Mistakes

The engine reports them with a pointer into `cosmetics`:
- `/cosmetics/0/parts/1/part`: a part that isn't `body` or `eyes`;
- `/cosmetics/0/parts/0/space`: a `space` (and likewise a `surface`);
- `/cosmetics/0/parts/0/outline/color`: a colour neither the cosmetic nor the character has;
- `/cosmetics/1/id`: an id that is listed twice, or isn't 1–32 of a–z, 0–9 and -;
- `/cosmetics/0/slot`, `/cosmetics/0/fits`: **warnings**, when a character can't wear it.

## In a recipe

A recipe may carry `cosmetics` too: a brand's own character can come with its cap on. A
file's `cosmetics` are added after the recipe's own.

## Next in 1.13

Planned additions on top of this format (not in this release yet):
- named palettes;
- face styles;
- a serializable **loadout** with a "what fits this character" list, still-pose thumbnails
  and a soft change;
- a separate cosmetic **catalog** pack;
- brand cosmetics in the Studio editor.
