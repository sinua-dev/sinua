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
      "trim": [48, 0.95, 0.62], "outline": [330, 0.5, 0.2]
    },
    "parts": [
      { "part": "body",
        "shape": { "path": "M-20 2 L0 -33 L20 2 Q0 8 -20 2 Z" },
        "light": { "linear": [-20, 0, 20, 0], "stops": [[0, "felt"], [1, "feltDark"]] },
        "inner": [{ "part": "band", "shape": { "roundRect": [-24, -6, 48, 6, 0, 4] }, "color": "trim" }],
        "outline": { "width": 2.5, "color": "outline" } },
      { "part": "body",
        "shape": { "ellipse": [0, -35, 5, 5, 0, 20] },
        "light": { "radial": [-1.5, -36.5, 7], "stops": [[0, "trim"], [1, "trim"]] },
        "outline": { "width": 2, "color": "outline" } }
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
- The recipe limits count the character with its cosmetics: 96 parts, 4,096 path points and 64 KB.
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

## Let end users pick: wardrobe and loadout

A **wardrobe** is what an end user may pick; the brand decides what is in it. A **loadout** is
what one user picked: a small value your app stores in its own account and passes back next
launch (`spec/examples/wardrobe-bean.fxspec.json`).

```json
"cosmetics": [ { "id": "party-hat", "category": "hat", ... } ],
"wardrobe": {
  "cosmetics": [ { "id": "round-glasses", "category": "glasses", "slot": "face", ... } ],
  "palettes": { "mint": { "primary": "#3FBF9F", "accent": "#FFD166" } },
  "irises": { "hazel": "#9C7A3C", "violet": "#7A4BD6" }
}
```

```json
{ "loadout": 1, "wear": ["round-glasses"], "palette": "mint", "iris": "hazel", "eyeStyle": "glossy" }
```

- `wear`: ids from the wardrobe or the file's `cosmetics`, one per slot (a later one replaces
  an earlier one); `[]` wears nothing. Without a loadout the file wears its own `cosmetics`.
- `palette`: a `wardrobe.palettes` name or a built-in palette (`sunset`, `ocean`, …), by name only.
- `iris` (design note 27): an eye colour, a `wardrobe.irises` name or a catalog one
  (`catalog:eyes-brown`, `-blue`, `-green`, `-hazel`, `-violet`), by name only. It joins the
  palette (`palette` still picks the rest) and colours the `iris` slot, which the glossy eye
  draws; the other eye styles have no iris. A free colour is the brand's, through the file's
  `palette: { "iris": … }`.
- `eyeStyle`: `auto`, `shape`, `glossy`, `pixel` or `dot`.
- **Never an error.** A loadout lives for months; when it names something the wardrobe no
  longer has, a newer format, or an unknown key, that part warns and is skipped, and the
  character still draws.
- `category` (optional, on any cosmetic) groups a picker: `hat`, `glasses`, `scarf`, `badge`,
  `frame`, `effect` or `other`.

Pass it to the view: `loadout` on `SinuaView` / `SinuaCharacter` (Web, React, the web
components, SwiftUI `SinuaLoadout`, Compose `SinuaLoadout`, React Native). A change **eases**:
an item that arrives pops in with a small overshoot, one that leaves shrinks away, the height a
hat needs eases, colours blend, and a new eye style swaps while the eyes blink (0.35 s, a cut
under reduced motion). It runs on its own clock beside a voice state change. Your app calls
nothing; the view does it when the value changes.

**For a picker screen:**
- `cosmeticsFor(spec, character)` (`SinuaCosmeticFit.list` natively) lists every item with
  `fits` and a `reason` key to translate: `fits`, `no-slot` (the character has no such slot)
  or `not-made-for` (its `fits` leaves the character out), plus the English `why`.
- **Thumbnails** on the user's own character: `frameStill(spec, size, { loadout, turnYaw })`
  draws a still pose (no blink, no glance; `turnYaw` about ±0.5 shows another angle).
  `characterThumbnail` (`@sinua/web`, a PNG `Blob`), `SinuaThumbnail.image` (iOS 16+) and
  `SinuaThumbnail.bitmap` (Android) paint it. Thumbnails never take the live character's
  place in the engine, so a grid of them is safe.
- To apply a loadout yourself: `applyLoadout(spec, loadout)` returns the file and its warnings.

The Studio's character panel has a **Wardrobe** group: every item as a thumbnail on the
current character, a click to wear it (with the same soft change), and *Copy loadout*.

## Catalog packs: ready cosmetics as data

Sinua ships a free catalog (`spec/catalog/catalog-1.json`): 14 cosmetics (hats, glasses,
headphones whose pads glow while the character speaks, a scarf, a bow tie, badges, two frames)
and 11 palettes (6 colour palettes and 5 eye colours, `catalog:eyes-brown`, `-blue`, `-green`, `-hazel`, `-violet`). It is data, never part of the engine, so only an app that loads it carries it.

```js
import { loadSinuaCatalog } from "@sinua/web/catalog";     // Web; React Native: SINUA_CATALOG + the `catalogs` prop
loadSinuaCatalog();                                          // iOS: SinuaCatalog.load(); Android: SinuaCatalog.load(context)
```

A file then names its items as `"<namespace>:<id>"`:

```json
"cosmetics": ["catalog:crown"],
"wardrobe": { "cosmetics": ["catalog:headphones", "catalog:halo-ring"] },
"palette": "catalog:berry"
```

- A catalog palette acts like a built-in one: it is written in roles (`primary`, `secondary`,
  `accent`, plus `dark`), so it fits every character, and a role a character lacks is skipped.
- A catalog item or palette that isn't loaded only warns; the character still draws.
- Catalog ids are permanent: an item to be removed is marked `deprecated` first and its id
  is never reused, so stored loadouts keep working.

**Your own pack.** A brand loads its own the same way, under its own namespace:

```json
{ "catalog": 1, "namespace": "acme", "cosmetics": [ ... ], "palettes": { ... } }
```

`loadCatalog(json)` takes the text, so a pack can come from your server: seasonal items arrive
without an app release. For example, fetch a winter pack and load it:

```js
const pack = await (await fetch("https://example.com/sinua/winter-2026.json")).json();
loadCatalog(pack);   // its "winter-hat" has "season": "winter"; a picker can show it by date
```

Loading a namespace again replaces it; `unloadCatalog("acme")` forgets it. At most 256 items
are kept across all packs; a pack over that, or with a bad namespace, loads nothing and says why.

## Fit by capability, not by name

A slot already says where an item can sit: a bow tie needs a `neck` slot, so every character
with one wears it, a brand's own included, and Cuppa, Bean and Beep (no neck) skip it with the
reason `no-slot`. For what a slot can't say, a recipe lists `tags` and a cosmetic lists
`requires`, from a closed list: `has-ears`, `has-arms`, `round`, `tall`, `screen-face`,
`floats`. A character without a required tag skips the item (reason `missing-tag`); an unknown
tag warns (the schema lists the known ones). `fits` (character names) stays for a brand's explicit list.

## Depth: behind the ears

A recipe's parts may name a `role` (`head`, `face`, `ears`, `arms`, `legs`, `antenna`, `hair`,
`tail`, `eyes`, `mouth`, `nose`, `cheeks`, `neck`, `shadow`, `body`; design note 28). A cosmetic's
`behind` / `above` lists roles:
- `"behind": ["ears"]` draws it before the character's first `ears` part, so the ears (and
  everything after them) cover it: a beanie with the fox's ears poking through;
- `"above": ["head"]` draws it after the last `head` part;
- `behind` wins when both match; a character without such a part draws it on top, as before.
  The built-ins name no roles, so nothing about them changes. An imported character
  (`docs/character-svg-guides.md`) gets its roles from its layer names.

## A nudge per item

A loadout's `wear` entry may be an object, for drag-to-dress screens:

```json
{ "loadout": 1, "wear": [{ "id": "party-hat", "offset": [2, -1], "scale": 1.05, "rotate": 5 }] }
```

The nudge is relative to the slot and bounded (`offset` ±10 units, `scale` 0.8–1.2, `rotate`
±15°); out of range warns and is clamped. The item still follows the rig: the tilt, the hop, the
head turn.

## Next in 1.13

Planned additions on top of this format (not in this release yet):
- brand cosmetics in the Studio editor and a fitting room (draggable slot handles, a depth
  control, the whole catalog previewed on a character).
