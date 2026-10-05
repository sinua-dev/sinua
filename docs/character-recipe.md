# Character recipe reference (format 1)

A character is data: a **recipe**, a JSON object the engine reads and draws. The seven
built-ins are recipes (`spec/characters/*.json`). Your own goes into an FX Spec 1.12 file's
`recipe`, and the file's `pattern` names its `id`. Cosmetics (1.13) are recipe parts on a slot. The [remix guide](character-remix.md)
walks through making one; this page lists everything a recipe may hold.

- **Schema:** [`spec/character-recipe-1.schema.json`](../spec/character-recipe-1.schema.json)
  (`$id` `https://sinua.dev/schema/character-recipe-1.json`). Put
  `"$schema": "https://sinua.dev/schema/character-recipe-1.json"` in a recipe file, or use the
  FX Spec schema (`https://sinua.dev/schema/fx-spec-1.json`), which refers to it, and an
  editor completes and checks it as you type.
- **Generated:** the schema and the part tables below come from the engine's own tables
  (`crates/core_engine/src/character/recipe_schema.rs`), with the sentences in
  `spec/character-recipe-descriptions.json`. A test fails when either goes stale or a field has
  no sentence.
- **The engine has the last word:** it reports the same mistakes, with a JSON Pointer into the
  recipe (`/recipe/parts/2/rise: expected a number`).

## The box and the spaces

Everything is drawn in a **200 × 200 box**, `x` to the right, `y` down; the character usually
stands on `y` ≈ 184. Each part names the **space** it moves in:

| Space | Moves with | Typical parts |
|---|---|---|
| `body` | the body and head: lean, squash, hop, the head turn | body, arms, crest, steam |
| `face` | the face: turned on its surface, looking round | eyes, mouth |
| `mount` | a pivot rig's `mount`: bobs between the stand and the body | Hum's yoke |
| `ground` | nothing: stays on the floor | shadow |
| `whole` | the character's box, without the turn | feet, stand |

A **surface** (`surfaces`) is a sphere or cylinder (`c`, `r`, `depth`, `cylinder`) that face
parts sit on, so they wrap round when the head turns. A shallower `depth` turns them less.

## Top level

| Key | | Meaning |
|---|---|---|
| `recipe` | required | The format version: `1`. |
| `id` | required | The character's name: 1–32 of a–z, 0–9 and -. A file's `pattern` names it. |
| `profile` | optional | The built-in whose voice-state behaviour it borrows (how it turns, looks and moves per state). A remix of Cuppa says `"cuppa"`; without it, the shared `character` profile. |
| `palette` | required | Colours by name, `[hue 0–360, saturation 0–1, lightness 0–1]`. Parts name them; they are also the slots the `palette` option repaints. |
| `hue` | required | `base`: the hue the palette is drawn at; `turns`: the names the `hue` option rotates. |
| `contrast` | optional | `[[ink, ground], …]`: a face colour and what it sits on, so a `palette` override keeps it readable. |
| `rig` | required | `pivot` (`pivot`, `lean`, optional `squash`, `sway`, `mount`), `upright` (`base`) or `float` (`center`, `lean`, `float { rate, amp }`). |
| `surfaces` | optional | Named surfaces for face parts. |
| `parts` | required | What is drawn, back to front. |
| `burst` | required | The celebrate burst: `at`, `r`, three `colors`, optional `space`. |
| `slots` | optional | Anchor points for cosmetics: `at`, `follows` (`head` / `body` / `face`), `scale`, `angle`. See *Slots and cosmetics*. |
| `cosmetics` | optional | Cosmetics the character always wears (FX Spec 1.13): the same objects as a file's `cosmetics`. See *Slots and cosmetics*. |
| `grain` | optional | `{ "strength": 0–1 }`: film grain inside every body (FX Spec 1.13). See *The richer look*. |
| `roles` | optional | `{ "primary": "<slot>", "secondary": "<slot>", "accent": "<slot>" }` (FX Spec 1.13): the slots a named palette (`"palette": "sunset"`) or a role colour (`{ "primary": "#E63946" }`) repaints. A role left out isn't repainted. |

Every part has `part` (its kind) and `space` (a body layer inside `inner` may leave it out), and
may have `when` (`notSmallOrAccessories`: left out at 20 px with `accessories` off), `show`
(`{ idle, listening, thinking, speaking }`, 0–1: how visible it is per voice state) and `role`
(what it is: `head`, `ears`, `arms`…; a cosmetic's `behind` / `above` draws against it, design
note 28).

## Limits

Over a limit is an error, never trimmed: a file from outside can't make the engine slow.

- The recipe: at most 64 KB; at most 96 parts, a body's layers included (48 before FX Spec 1.13); at most 4,096
  points in all its path shapes together, after flattening.
- Lists (feathers, bars, stripes, glints, stops, layers): at most 32 entries.
- `segments`: 3–128. `count`: 1–24. Rounding steps: at least 1.
- Every number within ±1000.
- An SVG `path`: at most 16 KB, 512 commands, 512 points, 8 holes; the commands
  `M L H V C S Q T Z` (no arcs: [`fitPath`](character-remix.md#a-body-from-a-drawing) converts
  them); later subpaths are holes, and a subpath outside the first is an error.

## Shapes

A `shape` is one of:

- `{ "ellipse": [cx, cy, rx, ry, rotation, segments] }`
- `{ "roundRect": [x, y, w, h, radius, step] }`
- `{ "path": "M… Z" }`

A body's `light` is `{ "radial": [cx, cy, r] }`, an elliptical `{ "radial": [cx, cy, rx, ry, angle] }`
(radians), or `{ "linear": [x0, y0, x1, y1] }`, with `stops` `[[offset 0–1, colour], …]`. A stop may
carry an alpha, `[offset, colour, alpha]`, and a light takes any number of stops.

## The richer look (FX Spec 1.13)

Airbrushed shading, rim light and film grain on today's 2D painter, with no blur (`spec/examples/rich-bean.fxspec.json` and `rich-buzzy.fxspec.json` show all of it):

- **`shade`** (a body layer): its own `shape` and `light`, clipped to the body. Give the light
  stops that fade to alpha 0 and it is a soft mass: a core shadow, light bounced from below, a
  highlight, a head's soft shadow on the body, or, with `"surface": "face"`, a blush that turns
  with the face. Draw the shape round the light's ellipse; outside it the light is transparent
  anyway, and a smaller shape paints less.
- **`rim`** (a body layer): the body minus itself moved by `offset`, the edge the light reaches
  first, in `color` at `alpha`; `fade` thins it out round the sides.
- **`"light": "none"`** on a body: no fill of its own, only its layers, grain and outline. Use
  it as an overlay to add soft layers over a part that has none (BUZZY's helmet), with outline
  width 0.
- **`grain`**: `{ "strength": 0.08 }` lays a still noise of light and dark specks inside every
  body, under its outline; the face stays crisp. The same tile on every platform.
- **Options:** `grain` (0–1) and `shading` (on/off) on the view or in an FX Spec's `params`.
  Grain and rims are left out at 20 px.
- **Low power:** an FX Spec resolved under low power turns off the grain and the soft layers
  of a character that has them, so it is "light" again. Plain characters are untouched.
- **Cost:** soft layers and grain add coverage; the showcases are "medium" at 64 px (BEAN
  1.10, BUZZY 1.53 of the medium line).

## Slots and cosmetics

A **slot** is where a cosmetic (FX Spec 1.13) sits. Four names are standard, and cosmetics look
for them:

| Slot | Where | Follows |
|---|---|---|
| `headTop` | the top of the head, at its middle | `head`: the pose, the hop |
| `face` | between the eyes | `face`: the pose and the head turn (wrapped on the `face` surface) |
| `neck` | where the head meets the body | `body` |
| `chest` | the middle of the chest | `body` |

`at` is the slot point in the 200-unit box; `scale` and `angle` (radians) size and turn every
cosmetic on it. A character without a slot simply doesn't wear cosmetics made for it. The
built-ins:

| id | `headTop` | `face` | `neck` | `chest` |
|---|---|---|---|---|
| `buzzy` | (100, 40) × 1.0 | (100, 92) | (100, 146) | (100, 168) |
| `hum` | (100, 22) × 0.9 | (100, 96) | (100, 150) | (100, 130) |
| `wisp` | (100, 38) × 0.9 | (100, 88) | (100, 128) | (100, 140) |
| `chirp` | (100, 52) × 1.0 | (100, 100) | (100, 150) | (100, 140) |
| `cuppa` | (90, 40) × 0.95 | (90, 100) | — | (90, 140) |
| `bean` | (100, 34) × 0.85 | (100, 114) | — | (100, 150) |
| `beep` | (100, 30) × 0.85 | (100, 72) | — | (100, 140) |

**A cosmetic's units:** its parts are drawn in the slot's local units. The slot point is
(0, 0) and up is −y. One unit is one box unit times the slot's `scale`. On `headTop` a hat is
drawn for a head about 80 units wide and has **40 units of height**; each character's
`headTop.scale` makes that fit its head. A cosmetic on `headTop` zooms the whole character
out about its feet, just enough that the hat and the tap hop fit in the box. The other slots
have scale 1 in this version.

A cosmetic is an object:

| Key | | Meaning |
|---|---|---|
| `id` | required | 1–32 of a–z, 0–9 and -, unique in the list. |
| `label` | optional | What it is, for people ("party hat"). |
| `slot` | required | The slot it sits on. |
| `palette` | optional | Its own colours; in the character's palette they are `<id>.<name>`, so `palette` repaints them. |
| `parts` | required | `body` (with its layers) and `eyes`, without `space` or `surface`; drawn on top of the character. |
| `fits` | optional | The characters it is made for. |
| `fit` | optional | Per character: `at` (local units), `scale`, `angle` on top of the slot. |

Its parts are left out at 20 px unless `accessories` is on, like `when: notSmallOrAccessories`.
The guide with a worked hat is [character-cosmetics.md](character-cosmetics.md).

## The built-in characters

| id | What it is | Parts worth borrowing |
|---|---|---|
| `buzzy` | A round-helmeted robot with a glowing face screen and ear pods. | `faceScreen`, `earPods`, `chestCore` |
| `hum` | A studio microphone: a capsule on a stand whose grille lights as it speaks. | `stand`, `yoke`, `grille`, `tally` |
| `wisp` | A floating spirit with a curling tail and sparkles. | `spirit`, `halo`, `sparkles` |
| `chirp` | A round bird with a crest and wings; its beak is its mouth. | `crest`, `wings`, `beak`, `notes` |
| `cuppa` | A coffee mug on its saucer: steam rises while it listens and thickens with its voice. | `steam`, `stand`, a `path` body with a hole (the handle) |
| `bean` | A coffee bean on little feet, with aroma sparkles. | `feet`, a `path` body, `patch` |
| `beep` | A small tin robot with arms: a hand to the ear to listen, to the chin to think. | `arms`, `tally`, `chestCore` |

## Parts

<!-- generated:parts (crates/core_engine/src/character/recipe_schema.rs) -->

### `shadow`

The soft shadow under the character.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where it sits: `[x, y]` in the 200-unit box. |
| `r` | [2 numbers] | required | Its radii `[rx, ry]`. |
| `floats` | true / false | optional | Widens with the float rig's drift (a floating character's shadow breathes). |

### `body`

A shape filled with its light, its inner layers clipped to it, and an outline. `turnLight` `[dx, k]` moves a radial light as the body turns. `"light": "none"` (and no stops) draws only the layers: an overlay that adds soft layers over a part that has none (BUZZY's helmet).

| Field | Type | | Meaning |
|---|---|---|---|
| `shape` | shape | required | A shape: `{ "ellipse": [cx, cy, rx, ry, rotation, segments] }`, `{ "roundRect": [x, y, w, h, radius, step] }` or `{ "path": "M… Z" }` (an SVG path with `M L H V C S Q T Z`; later subpaths are holes). |
| `light` | light | required | `{ "radial": [cx, cy, r] }`, or an ellipse `[cx, cy, rx, ry, angle]` (radians; the centre stays put while the body turns under it), `{ "linear": [x0, y0, x1, y1] }`, or `"none"` (an overlay: no fill). |
| `light.stops` | [[offset, colour, alpha?], …] | required | The light's gradient: `[[offset 0–1, colour], …]`, each with an optional alpha `[offset, colour, alpha]`; any number of stops. |
| `inner` | [layers] | optional | Layers clipped to the body: `patch`, `band`, `stripes`, `glints`, `grille`, `shade`, `rim` or `eyes`. |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `eyes` *(a part or a body layer)*

The shared eyes: they blink, look round, follow the voice state and take the expression. A part on its own (in `face` space) or a body layer.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | The point between the eyes. |
| `scale` | [2 numbers] | required | Its size: `[at 64 and 32 px, at 20 px]`. |
| `ink` | colour | required | The ink colour (a palette name); effects tint it. |
| `glow` | number or [2 numbers] | required | A glow round the eyes, 0–1: one number, or `[at 64 and 32 px, at 20 px]`. |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |
| `style` | shape / glossy / pixel / dot | optional | The eye style: `shape` (the default, a solid shape), `glossy` (a lens, an iris and pupil that follow the gaze, highlights and a lid line), `pixel` (a grid of glowing cells; on a `faceScreen` the mouth too) or `dot` (a soft point). The `eyeStyle` option overrides it. |
| `iris` | colour | optional | The glossy eye's iris colour (a palette name). Absent: teal. |
| `sclera` | true / false | optional | The glossy eye has a white sclera and a dark lid line instead of a dark lens. |

### `feet`

Two feet of three strokes each, standing on the ground.

| Field | Type | | Meaning |
|---|---|---|---|
| `x` | number | required | The centre line between the feet. |
| `gap` | number | required | Each foot's distance from `x`. |
| `top` | number | required | Where the legs start (y). |
| `ground` | number | required | Where they meet the floor (y). |
| `toe` | [2 numbers] | required | A toe's reach `[dx, y]` from the foot. |
| `width` | number | required | The stroke width. |
| `color` | colour | required | Its colour (a palette name). |

### `crest`

Curling feathers that lift while listening (higher with the user's voice), perk while speaking and droop while thinking.

| Field | Type | | Meaning |
|---|---|---|---|
| `base` | [2 numbers] | required | Where the feathers grow from `[x, y]`. |
| `spread` | [numbers] | required | One number per feather: its sideways direction and spread (negative to the left). |
| `mid` | number | required | Which feather (0-based) is the middle one; it rises highest. |
| `root` | number | required | How far apart the roots are (times `spread`). |
| `ctrl` | [4 numbers] | required | The curve's control point: `[x factor, y, lift factor, middle extra]`. |
| `tip` | [5 numbers] | required | The tip: `[x factor, per-feather offset, y, lift factor, middle extra]`. |
| `lift.ears` | [2 numbers] | required | How much listening lifts it: `[at rest, with the user's voice]`. |
| `lift.talk` | [2 numbers] | required | How much speaking lifts it: `[at rest, with the level]`. |
| `lift.dots` | number | required | How much thinking lowers it. |
| `turnShift` | number | required | How far it slides sideways as the head turns. |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `width` | number | required | The stroke width. |
| `color` | colour | required | Its colour (a palette name). |

### `wings`

Two wings that flutter with the voice; turned, they swing round the body (the near one grows, the far one thins).

| Field | Type | | Meaning |
|---|---|---|---|
| `x` | number | required | The centre line it is mirrored round (x). |
| `at` | [2 numbers] | required | A wing's centre `[dx from x, y]`. |
| `size` | [2 numbers] | required | A wing's radii `[rx, ry]`. |
| `angle` | number | required | Its tilt (radians, mirrored). |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `flutter.rate` | number | required | How fast it flutters. |
| `flutter.gain` | name | required | The option name that scales the flutter (e.g. `flutterGain`). |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |
| `turn.near` | number | required | How much the near wing grows when turned. |
| `turn.far` | number | required | How much the far wing thins when turned. |
| `color` | colour | required | Its colour (a palette name). |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `beak`

The beak is the mouth: its lower half opens with the voice.

| Field | Type | | Meaning |
|---|---|---|---|
| `lower` | [3 points] | required | The lower half's three points `[[x, y], …]`; the last one drops when it opens. |
| `drop` | number | required | How far the lower tip drops, fully open. |
| `upper.quad` | [3 points] | required | The upper half's curve: three points. |
| `upper.segments` | number | required | Points along that curve: 3–128. |
| `upper.tip` | [2 numbers] | required | The upper half's tip `[x, y]`. |
| `upper.rise` | number | required | How far the upper tip rises, fully open. |
| `open.effect` | number | required | How open it is during success / celebrate. |
| `open.rest` | number | required | How open it is at rest. |
| `colors` | [2 colours] | required | `[lower, upper]` (palette names). |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |

### `notes`

Music notes rising while it speaks, and thought dots lighting in turn while it thinks.

| Field | Type | | Meaning |
|---|---|---|---|
| `count` | number | required | How many: 1–24. |
| `rate` | number | required | How fast the notes rise. |
| `from` | [2 numbers] | required | Where a note starts `[x, y]`. |
| `travel` | [2 numbers] | required | How far it travels `[dx, dy]`. |
| `stagger` | number | required | Each next note's vertical offset. |
| `scale` | [2 numbers] | required | A note's size: `[at 64 px, at 32 and 20 px]`. |
| `floor` | number | required | The lowest note opacity while speaking. |
| `color` | colour | required | Its colour (a palette name). |
| `dots.at` | [2 numbers] | required | The first dot `[x, y]`. |
| `dots.step` | [2 numbers] | required | Each next dot's offset `[dx, dy]`. |
| `dots.r` | [2 numbers] | required | A dot's radius `[first, growth per dot]`. |
| `dots.segments` | number | required | Points per dot: 3–128. |
| `dots.rate` | number | required | How fast the lit dot moves along. |
| `dots.base` | number | required | An unlit dot's opacity. |
| `dots.gain` | number | required | How much brighter the lit dot is. |

### `stand`

A stand: a shadowed round foot and a stem.

| Field | Type | | Meaning |
|---|---|---|---|
| `under` | [4 numbers] | required | The shadow ellipse under the foot `[cx, cy, rx, ry]`. |
| `foot` | [4 numbers] | required | The foot ellipse `[cx, cy, rx, ry]`. |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `stem` | [6 numbers] | required | The stem, a rounded rectangle `[x, y, w, h, radius, step]`. |
| `dark` | colour | required | The shadow's colour. |
| `color` | colour | required | Its colour (a palette name). |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `yoke`

A U-shaped yoke holding the body at its sides: a dark stroke under a lighter one.

| Field | Type | | Meaning |
|---|---|---|---|
| `x` | [2 numbers] | required | Its two sides `[left, right]`. |
| `top` | number | required | Where the sides start (y). |
| `knee` | number | required | Where they start to curve (y). |
| `bottom` | number | required | The bottom of the U (y). |
| `mid` | number | required | The bottom's middle (x). |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `under.width` | number | required | The dark stroke's width. |
| `under.color` | colour | required | The dark stroke's colour. |
| `over.width` | number | required | The light stroke's width. |
| `over.color` | colour | required | The light stroke's colour. |

### `tally`

A tally light: red while listening, amber blinking while thinking, off otherwise.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where it sits: `[x, y]` in the 200-unit box. |
| `r` | number | required | Its radius. |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `off` | colour | required | Its colour when off. |
| `listening` | colour | required | Its colour while listening. |
| `thinking` | colour | required | Its colour while thinking. |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `halo`

A soft halo: a radial fade (not drawn at 20 px).

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where it sits: `[x, y]` in the 200-unit box. |
| `r` | number | required | Its radius. |
| `segments` | number | required | How many points a curve or ellipse is drawn with: 3–128. |
| `color` | colour | required | Its colour (a palette name). |
| `stops` | [[a, b], …] | required | The fade: `[[offset 0–1, opacity 0–1], …]`. |

### `spirit`

A spirit's body: a round head flowing into a tail that curls (with `curlGain`), sways and trails the head turn, with smoke puffs off the tail tip and a shine.

| Field | Type | | Meaning |
|---|---|---|---|
| `head` | [3 numbers] | required | The head `[x, y, radius]`. |
| `stops` | [[offset, colour, alpha?], …] | required | The body's gradient `[[offset 0–1, colour], …]`. |
| `line` | colour | required | The outline colour (a palette name). |
| `smoke` | colour | required | The smoke puffs' colour. |
| `shine` | colour | required | The head's shine colour. |
| `tail.lag` | number | required | How late the tail follows the head turn (seconds). |
| `tail.swing` | number | required | How far the tail swings with the turn. |

### `ovalMouth`

A mouth that is a small smile at rest, thinking dots, or an oval that opens with the voice; it takes the expression's resting mouth.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where it sits: `[x, y]` in the 200-unit box. |
| `scale` | [2 numbers] | required | Its size: `[at 64 and 32 px, at 20 px]`. |
| `halfWidth` | number | required | Half the mouth's width. |
| `ink` | colour | required | The ink colour (a palette name); effects tint it. |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |

### `sparkles`

Sparkles round the head: wandering at rest, gathering while listening, orbiting while thinking, streaming out with the voice.

| Field | Type | | Meaning |
|---|---|---|---|
| `count` | number | required | How many: 1–24. |
| `head` | [2 numbers] | required | The point they move round `[x, y]`. |
| `colors` | [2 colours] | required | `[odd, even]` sparkles' colours. |

### `torso`

A robot torso: a rounded shoulder shape lit top to bottom.

| Field | Type | | Meaning |
|---|---|---|---|
| `shades` | [3 colours] | required | `[light, mid, dark]` (palette names). |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `chestCore`

A voice core on the chest: a dark disc, a ring and bars that move with the voice; it follows the head turn at 40 %.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where it sits: `[x, y]` in the 200-unit box. |
| `r` | number | required | Its radius. |
| `bars` | [numbers] | required | Each bar's horizontal offset. |
| `colors` | [3 colours] | required | `[disc, ring, bars]` (palette names). |

### `earPods`

Ear pods with chevrons and listening arcs; turned, they swing round the head.

| Field | Type | | Meaning |
|---|---|---|---|
| `x` | number | required | The centre line it is mirrored round (x). |
| `reach` | number | required | Each pod's distance from `x`. |
| `front` | true / false | optional | This is the near pod's copy in front of the helmet (list it after the helmet). |
| `pod` | colour | required | The pods' colour. |
| `line` | colour | required | The outline colour (a palette name). |
| `chevron` | colour | required | The chevrons' colour. |
| `arcs` | colour | required | The listening arcs' colour. |

### `helmet`

A helmet: a rounded square lit from the top left; turned, its back edge shows.

| Field | Type | | Meaning |
|---|---|---|---|
| `shades` | [3 colours] | required | `[light, mid, dark]` (palette names). |
| `line` | colour | required | The outline colour (a palette name). |

### `fin`

A fin on top of the helmet (64 and 32 px, with accessories).

| Field | Type | | Meaning |
|---|---|---|---|
| `color` | colour | required | Its colour (a palette name). |
| `line` | colour | required | The outline colour (a palette name). |

### `faceScreen`

A face screen: the eyes and mouth on it (clipped to it), its rim, and a glass visor with a drifting reflection; it rides the face surface.

| Field | Type | | Meaning |
|---|---|---|---|
| `screen` | colour | required | The screen's colour. |
| `ink` | colour | required | The eyes' and mouth's colour. |
| `line` | colour | required | The outline colour (a palette name). |
| `glass` | colour | required | The visor's colour. |
| `glassEdge` | colour | required | The visor rim's colour. |
| `glint` | colour | required | The reflection's colour. |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |
| `style` | shape / glossy / pixel / dot | optional | The eye style: `shape` (the default, a solid shape), `glossy` (a lens, an iris and pupil that follow the gaze, highlights and a lid line), `pixel` (a grid of glowing cells; on a `faceScreen` the mouth too) or `dot` (a soft point). The `eyeStyle` option overrides it. |
| `iris` | colour | optional | The glossy eye's iris colour (a palette name). Absent: teal. |
| `sclera` | true / false | optional | The glossy eye has a white sclera and a dark lid line instead of a dark lens. |

### `steam`

Soft wisps rising and fading: higher while listening, swaying more while thinking, thicker with the voice.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [2 numbers] | required | Where the wisps rise from `[x, y]`. |
| `count` | number | required | How many: 1–24. |
| `spread` | number | required | How far apart the wisps start. |
| `rise` | number | required | How high they rise (kept inside the box). |
| `width` | number | required | A wisp's width. |
| `sway` | number | required | How far they sway. |
| `rate` | number | required | How fast they rise. |
| `blur` | number | required | How soft they are. |
| `color` | colour | required | Its colour (a palette name). |
| `alpha` | number | required | Its opacity, 0–1. |

### `arms`

Two arms (upper arm, forearm, hand): hanging at rest, a hand to the ear while listening and to the chin while thinking, beats while speaking. `arms: false` turns them off.

| Field | Type | | Meaning |
|---|---|---|---|
| `shoulder` | [2 numbers] | required | The right shoulder `[x, y]`; the left mirrors it round `mirror`. |
| `mirror` | number | required | The centre line the arms mirror round (x). |
| `length` | [2 numbers] | required | `[upper arm, forearm]`. |
| `width` | number | required | The stroke width. |
| `hand` | number | required | The hand's radius. |
| `ear` | [2 numbers] | required | Where the right hand goes while listening `[x, y]`. |
| `chin` | [2 numbers] | required | Where the right hand goes while thinking `[x, y]`. |
| `back` | true / false | optional | This is the copy behind the body: the far arm, shown as the head turns away (list it before the body). |
| `color` | colour | required | Its colour (a palette name). |
| `handColor` | colour | required | The hands' colour. |
| `outline.width` | number | required | The outline width (scaled with the character's line weight). |
| `outline.color` | colour | required | The outline colour (a palette name). |

### `patch` *(body layer: inside a body's `inner`)*

A body layer: a shape in one colour, clipped to the body (on a surface, it wraps when the head turns).

| Field | Type | | Meaning |
|---|---|---|---|
| `shape` | shape | required | A shape: `{ "ellipse": [cx, cy, rx, ry, rotation, segments] }`, `{ "roundRect": [x, y, w, h, radius, step] }` or `{ "path": "M… Z" }` (an SVG path with `M L H V C S Q T Z`; later subpaths are holes). |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |
| `color` | colour | required | Its colour (a palette name). |

### `band` *(body layer: inside a body's `inner`)*

A body layer: a shape in one colour clipped to the body, flat (a sleeve, a belt).

| Field | Type | | Meaning |
|---|---|---|---|
| `shape` | shape | required | A shape: `{ "ellipse": [cx, cy, rx, ry, rotation, segments] }`, `{ "roundRect": [x, y, w, h, radius, step] }` or `{ "path": "M… Z" }` (an SVG path with `M L H V C S Q T Z`; later subpaths are holes). |
| `color` | colour | required | Its colour (a palette name). |

### `stripes` *(body layer: inside a body's `inner`)*

A body layer: thin rounded stripes across the body.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [numbers] | required | Each stripe's y. |
| `rect` | [5 numbers] | required | A stripe: `[x, w, h, radius, step]`. |
| `color` | colour | required | Its colour (a palette name). |
| `alpha` | number | required | Its opacity, 0–1. |

### `glints` *(body layer: inside a body's `inner`)*

A body layer: rounded highlights.

| Field | Type | | Meaning |
|---|---|---|---|
| `at` | [[a, b], …] | required | Each glint `[y, height]`. |
| `rect` | [4 numbers] | required | A glint: `[x, w, radius, step]`. |
| `color` | colour | required | Its colour (a palette name). |
| `alpha` | number | required | Its opacity, 0–1. |

### `grille` *(body layer: inside a body's `inner`)*

A body layer and a mouth: slots that light with the voice while speaking and one light scanning while thinking.

| Field | Type | | Meaning |
|---|---|---|---|
| `count` | number | required | How many: 1–24. |
| `x` | number | required | The first slot's x. |
| `pitch` | number | required | The distance between slots. |
| `y` | number | required | The slots' y. |
| `slot` | [3 numbers] | required | A slot at 64 and 32 px: `[width, height, height when lit]`. |
| `smallSlot` | [3 numbers] | required | A slot at 20 px: `[width, height, height when lit]`. |
| `rise` | [2 numbers] | required | How it moves up `[at rest, when lit]`. |
| `step` | number | required | The slots' rounding step (at least 1). |
| `ink` | colour | required | The ink colour (a palette name); effects tint it. |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |

### `shade` *(body layer: inside a body's `inner`)*

A soft mass clipped to the body (FX Spec 1.13): its own `light`, usually an ellipse whose stops fade to alpha 0, so it needs no blur. An airbrushed shadow or highlight, light bounced from below, a head's soft shadow on the body, or (on the `face` surface) a blush. Left out when `shading` is off and under low power.

| Field | Type | | Meaning |
|---|---|---|---|
| `shape` | shape | required | A shape: `{ "ellipse": [cx, cy, rx, ry, rotation, segments] }`, `{ "roundRect": [x, y, w, h, radius, step] }` or `{ "path": "M… Z" }` (an SVG path with `M L H V C S Q T Z`; later subpaths are holes). |
| `surface` | surface | optional | The surface it is drawn on (a `surfaces` name), so it wraps when the head turns. Absent: flat. |
| `light` | light | required | Its light: `{ "radial": [cx, cy, r] }` or `[cx, cy, rx, ry, angle]`, or `{ "linear": [x0, y0, x1, y1] }`. Draw the `shape` round the light's ellipse: outside it the light is transparent anyway, and a smaller shape paints less. |
| `light.stops` | [[offset, colour, alpha?], …] | required | `[[offset, colour, alpha], …]`: fade the last stop to 0 for a soft edge. |

### `rim` *(body layer: inside a body's `inner`)*

A rim of light along the edge the light reaches first (FX Spec 1.13): the body minus itself moved by `offset`. Left out at 20 px, when `shading` is off and under low power.

| Field | Type | | Meaning |
|---|---|---|---|
| `offset` | [2 numbers] | required | `[dx, dy]`: the body moved away from the light; the rim is as wide as the move (`[6, 7]` lights the top-left edge). |
| `color` | colour | required | Its colour (a palette name). |
| `alpha` | number | required | Its opacity, 0–1. |
| `fade` | number | required | How far (box units) it fades out against the offset, so it thins round the sides; 0 = even. |

<!-- /generated:parts -->
