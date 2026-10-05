# The `character` family

## Why this exists

Every other family is an abstract shape. Some products want a *someone*: a coach, a
host, a helper with a face. The market's answer is "a coloured blob with two eyes"
(libraries.dev's bot-avatars, Microsoft's Mico, the agent-avatar web components), and
most of it ignores the voice. This family is the voice-first version: each character has
a face whose gaze knows whose turn it is, and a mouth that follows only the voice's
amplitude.

It is the seventh sibling family (`crates/core_engine/src/character/`), tried after the
others by `lib.rs`'s `resolve_any`. Object `character`, component `SinuaCharacter`
(`SinuaAvatar` is the ring-around-a-picture component from `ring`). FX Spec 1.11; in 1.12 every
character became a recipe (data, not code), and a file can carry its own.

## Patterns

| Pattern | Character | Voice signature |
|---|---|---|
| `buzzy` | A small space-hero assistant: a rounded-square indigo helmet with an amber crest and chevron ear pods, a glass visor over a face screen, a voice core in the chest | the mouth is a voice line (two even waves that flow; the level sets their height); the chest bars follow the level; while listening, sound arcs light at the ear pods with the user's level |
| `hum` | A vintage studio microphone that hosts the show: a red capsule on a brass yoke and stand, the face on its grille band | the grille is the mouth: its slots light with the level while speaking, one light scans them while thinking; a tally light is red while listening and blinks amber while thinking; the capsule tips toward you while listening and sways on the yoke while speaking (`swayGain`) |
| `wisp` | A helpful spirit: a round, glowing head flowing into a curling smoke tail, violet into teal, with a soft halo | its sparkles wander at rest, gather in while it listens (closer as the user speaks), orbit its crown while it thinks and stream out with its voice; the tail curls tighter while thinking (`curlGain`); the mouth is an oval that opens with the level |
| `chirp` | A songbird: a coral egg-shaped body with a cream breast, teal wings and a three-feather crest | the beak is the mouth: it opens with the level while speaking and little notes rise from it, the wings flutter (`flutterGain`); it tilts its head and lifts its crest while listening; while thinking the crest drops and three thought dots light in turn |
| `cuppa` | A coffee mug on its saucer (FX Spec 1.12): drawn from an SVG path, the handle is a hole; coffee at the rim, a sleeve with a heart | the mouth is an oval that opens with the level; its steam (the `steam` part) rises higher while it listens, curls while it thinks and thickens with the voice; it squashes a little as it talks |
| `bean` | A coffee bean on little feet (FX Spec 1.12): an SVG-path body with its S-shaped groove and lit edge as path patches, rosy cheeks | the mouth is an oval that opens with the level; aroma sparkles gather while it listens, orbit while it thinks and stream out with its voice |
| `beep` | A small tin robot (FX Spec 1.12): a path head and body, a face screen, an antenna light, a chest core, legs, and **arms** | a hand goes to the ear to listen and to the chin to think, and the arms beat with the voice while speaking; the antenna light is red while listening and amber while thinking; `arms: false` takes the arms off |

**Expressions**: the app picks one with `expression` (a view prop, or the
FX Spec key in the base and in `states`). It stays until changed, and a change eases over 0.6 s.

| Expression | Eyes | Resting mouth |
|---|---|---|
| `happy` | the happy arc (smile 0.8) | smile |
| `surprised` | wider and taller, lids open | a small "O" |
| `thoughtful` | a lid, the left one shorter, a slight tilt | smile |
| `sad` | a lid, outer corners dropped, a little shorter | a downturned line |
| `sleepy` | heavy lids, a slow deep breath | smile |

- **Layering:** the expression owns the eyes' shape. The voice state keeps the gaze, the head turn,
  the blinks and the talking or thinking mouth: a sad Bean still turns to you while listening, and
  its mouth still follows the voice while speaking.
- **Weights:** each expression is an engine opt (`expressionHappy` … `expressionSleepy`, 0..1), so
  transitions blend them like any number. The views ease their own prop.
- **Mouths:** Chirp's beak and Hum's grille have no resting mouth to change; there the expression is
  in the eyes only.
- **Effects:** effects still play on top.

**Tap to hop**: a tap on a `SinuaCharacter` plays the `hop` effect.
- The body crouches, hops 9 units and squashes on landing; the ground (the shadow, feet and
  saucer) stays put.
- The eyes smile and glance toward the tap, then come back. The voice state keeps the mouth,
  ears and lids.
- `tap: false` turns it off; reduced motion keeps only the smile. See
  [`fx-view.md`](fx-view.md), *One-shot effects*.

All seven are recipes (`spec/characters/*.json`): the first four were moved over in 1.12 with
their drawing unchanged bit for bit. Cuppa and Bean were the first new ones made **only from
recipes** (1.12 item 5): they needed no new drawing code, only one new library part, the steam.
Beep added the `arms` part.

**Shared, not repeated.** What every character does alike lives once: the face (`face.rs`),
the rig (`rig.rs`), the drawing helpers (`geom.rs`) and the kit (`kit.rs`: the size tiers, the
ground shadow, the effect colours, the celebrate burst, the mute fade and the frame). The
catalog defines the rig and the options once, as `key@character`; a character's own recipe and
catalog entries hold only what makes it itself. Each character is named after a sound.

## Recipes and parts (`character/recipe.rs`, `character/parts/`)

A character is **data** (FX Spec 1.12). Its recipe,
`spec/characters/<id>.json`, names:
- its **palette** (and which colours `hue` turns);
- its **rig**: `pivot` (Chirp, Hum: tilt about a point, with optional squash, voice sway and a
  `mount` for Hum's yoke), `upright` (Buzzy) or `float` (Wisp);
- the **surfaces** its face turns on;
- an ordered list of **parts** from the library, each with its numbers, in a **space**
  (`ground`, `whole`, `mount`, `body`, `face`);
- the celebrate **burst**, and its **slots**.

The parts are the behaviour; the recipe says where, how big, what colour and how much. A new
look (shape, colour, size, which parts, how they react) is a new recipe. A new behaviour (a
new kind of sparkle, a new mouth mechanism) is a new part in Rust.

| Library | Parts |
|---|---|
| `common` | `shadow`, `body` (radial or linear light that stays put while the body turns; inner layers `patch`, `band`, `stripes`, `glints`, `grille`, `eyes`; outline), `eyes` |
| `bird` (Chirp) | `feet`, `crest`, `wings`, `beak`, `notes` (with the thought dots) |
| `mic` (Hum) | `stand`, `yoke`, `grille` (a body layer), `tally` |
| `spirit` (Wisp) | `halo`, `spirit` (smoke, the curling tail that trails the turn, shine), `ovalMouth`, `sparkles` |
| `ranger` (Buzzy) | `torso`, `chestCore`, `earPods` (and the near pod's front copy), `helmet`, `fin`, `faceScreen` |
| `arms` (Beep) | `arms`: two arms (upper arm, forearm, round hand) from mirrored shoulders. The hands blend with the voice state: hanging at rest, the right hand to the recipe's `ear` while listening and its `chin` while thinking, beats with the level while speaking; up for celebrate, a shrug for error, open for the hop. Elbows come from a two-segment IK, bent outward. A `back` copy before the body draws the far arm behind it when turned. Not at 20 px, nor with `arms` off |
| `steam` (Cuppa) | `steam`: soft blurred wisps that rise from a point, curl and fade at the top. Listening lifts them, thinking curls them, the voice thickens them. They never rise past the top of the box, and they are not drawn at 20 px or with `accessories` off |

- **One reader.** Every part kind has a schema (its fields, in order, with types). A single
  reader checks a recipe against them: a missing field, a wrong type, an unknown field, an
  unknown colour or surface, each with its JSON pointer
  (`/parts/2/lift/ears: expected 2 values`). The per-part readers it replaced cost ~11 KB
  gzip more.
- **`show`:** any part may say how visible it is per voice state,
  `{ "idle", "listening", "thinking", "speaking" }`. The weights come from the pose
  (`earGain`, the dots and talk mouths), so a state change fades it in or out. Absent, the
  part always draws at its own alpha.
- **Shapes:** `ellipse`, `roundRect` or **`path`** (FX Spec 1.12), wherever
  a part takes a shape (`body`, `patch`, `band`). A path is an SVG `d` in the 200-unit box:
  - **Commands:** `M L H V C S Q T Z`, absolute and relative. No arcs (`A`): the error says to
    convert them to curves (Figma: Flatten).
  - **Read once:** `character/path.rs` reads the path when the recipe is read, and flattens
    each curve by its length (`geom::cubic` / `quad`, 2–32 pieces). A frame costs nothing
    extra, and every platform gets the same points.
  - **Holes:** later subpaths are holes, painted even-odd (a mug's handle). A subpath outside
    the outline is an error; a separate shape goes in its own part.
  - **Clipping and outline:** a path body may be concave, so its inner layers clip with
    Greiner–Hormann (`character/region.rs`), not `clip_convex`. A forehead band across a
    cat's ears becomes one piece per ear. The outline is mitred at sharp corners (at most 2×)
    and drawn round each hole. Ellipse and roundRect keep the old convex path exactly, so
    the built-in characters are byte-identical.
  - **Limits:** `d` ≤ 16 KB and ≤ 512 commands; ≤ 512 points after flattening; ≤ 8 holes;
    ≥ 3 points per subpath; numbers within ±1000. An error gives the pointer and the byte:
    `/parts/1/shape/path: at 7: …`.
  - **Not checked:** a self-intersecting path is not an error; it paints even-odd.
  - **Size:** path cost +9.3 KB gzip in the wasm (parser 3.1, clipping 3.0, miter 1.0, shape
    wiring 2.2).
  - **Test recipe:** `recipe_tests/cat.json`.
- **Slots** (`headTop`, `face`, `neck`, `chest`): where 1.13's cosmetics will attach. Each one
  follows its chain (`head`/`body` take the rig's pose; `face` also wraps onto the face
  surface), resolved per frame by `character::recipe::slots` as a position, scale and angle.
  They are not drawn and not public yet.
- **Same drawing.** The four launch characters were hand-written Rust. Before that code
  went, a test compared the recipes with it on 900 frames (3 sizes × 5 times × 15 poses ×
  4), and they matched exactly. The golden cases are byte-identical too. A bit-exact
  snapshot test (`recipe_snapshot`, ignored) guards later refactors of the reader or the parts.
- **Size:** the reader and the recipes cost about 17 KB gzip in the wasm (1.12 items 1a
  and 1b saved 16 KB just before). FX Spec-carried recipes (1.12, item 3) reuse the same
  reader; the registry, the limits and the FX Spec wiring add about 7 KB gzip.
- **Limits.** Every recipe, built-in or from a file, stays within: 64 KB, 96 parts (inner
  layers included), 4,096 path points in all, lists of 32, `segments` 3–128, `count` 1–24, edge steps ≥ 1, numbers
  within ±1000. Over a limit is an error with its pointer, never trimmed.
- **In an FX Spec (1.12).** A file's `recipe` is registered (`character/registry.rs`) under
  `recipe:<id>:<fnv-1a 64>` and its `pattern` resolves to that key, which `render` draws like
  any character ([`fx-spec.md`](fx-spec.md), *v1.12*). The registry keeps the last 32 used.
  A recipe's voice states are the shared `patterns.character` profile, or a built-in's with
  `"profile": "chirp"`; its gains (`Recipe::gains`) are allowed in `params`.

## The face (`character/face.rs`)

Shape eyes in the line of Anki's Cozmo and Vector: no pupils and no brows, and the eye's
*shape* is the expression. That gives a lot of expression from little detail, and it
still reads at 20 px. An eye is a rounded rectangle cut by a sloped top lid (`lid`,
`eyeTilt`), with an optional happy arc from below (`eyeSmile`) and a shorter left eye for
a quizzical look (`eyeAsym`). Its glow is a blurred copy of the same shape. The effects
are eye shapes too: success `^ ^` with a grin, error `x x` with a short shake, celebrate
stars with a burst.

The face is independent of any body: a character gives it an anchor, a scale, a colour
and an optional clip (BUZZY's screen), and gets fills back.

### Eye styles (FX Spec 1.13)

The shape eye is the default. Three more styles draw inside or over the same eye shape, so
blinks, the gaze, the turn blink, the startle and every expression work unchanged; the
effect eyes (stars, the X) stay as they are.

| Style | Looks like | At 20 px |
|---|---|---|
| `glossy` | a dark lens (or a white `sclera`), a radial-gradient iris and a pupil that slide inside the eye with the gaze, one big and two small highlights that stay with the light, a lid line | the lens, the iris and one highlight |
| `pixel` | the eye lit as a grid of rounded cells, glowing on a screen; a shut eye is a row of cells; on a `faceScreen` the mouth goes pixel too | the same, fewer cells |
| `dot` | a soft glowing point on a screen, a crisp dot on a body; blinks squash it | the dot without its halo |

A recipe sets its own on its `eyes` part (or `faceScreen`): `"style"`, `"iris"` (a palette
name; teal when absent) and `"sclera"`. The `eyeStyle` option (a prop on every platform, or
`params.eyeStyle` by name or number) overrides it on any character: `auto` (0) keeps the
recipe's. The built-ins keep the shape eye; `spec/examples/glossy-bean.fxspec.json`,
`pixel-beep.fxspec.json` and `dot-hum.fxspec.json` show the others. Every style stays a
`light` frame.

## The rig (`character/rig.rs`)

The pose is plain numbers (`eyeW`, `eyeH`, `lid`, `gazeX`, `gazeY`, `lean`, `tilt`,
`mouthTalk`, `mouthGain`, `squashGain`, …). The voice-state profile sets them per state
(`spec/voice-state-profile.json`, `patterns.buzzy`), so a state change *interpolates*
them (design-01's same-pattern transitions): the eyes slide up when thinking starts,
they don't jump. On top of that, the rig adds the living motion. It is a pure function
of `t`, `seed` and the runtime inputs:

- **Blinks** on a seeded schedule (one per 4.5 s slot) and **glances** (`look`).
  `seed` keeps two characters from blinking in sync.
- **The turn blink.** A state that sets `turnBlink` (thinking does) blinks once in its
  first 0.18 s, read from `stateAge`, the view's seconds since the state changed.
- **Turn-aware gaze**, after Andrist & Mutlu (HRI 2014): listening looks at the viewer;
  thinking looks up and away, which people read as thinking and as holding the turn.
- **Barge-in** (`interruptAge`): the eyes open wide and the body hops back. It replaces
  the generic flash, which would darken the whole palette.
- **Mute** (`muted`): a heavy-lidded squint and a 30 % fade. The colours stay; the
  generic grey-out is skipped (the user's call, 2026-09-30).
- **Speaking:** the mouth's swing is `audioLevel × mouthGain`, with a small squash and
  lift. Amplitude only, no visemes. The voice line is two even waves along the whole mouth
  that flow with time (only the last tenth at each end settles onto the line); the level
  sets their height, never their shape, so a jumpy level doesn't make the line jitter.
  Under reduced motion the waves stop flowing.

**State changes are smooth.** Every rig value interpolates over the transition (the
spec's 1.9 `transitions`, default 0.6 s easeInOut). The mouth's shapes are weights too
(`mouthTalk`, `mouthDots`, and the smile for what's left): mid-change the dots fade out
while the voice line fades in, instead of switching at the halfway point. One value doesn't
interpolate: **`turnBlink` is an arrival value** (`"transition": "arrive"` in the catalog;
`transition.rs` takes the new state's value at once). If it did interpolate, entering
thinking would barely blink, since the blink window is the first 0.18 s and the value
would still be near 0, and leaving thinking would blink on the way out.

## The head turn (`character/turn.rs`)

The head yaws and pitches like a solid, not a flat sticker (after a
comparison with libraries.dev's bot-avatars; nothing copied). It stays stateless: the
angles are a function of `t`, `seed` and the state's numbers, so a state change slides the
turn like any other rig value. Two cheap layers, no extruded slice stack:

- **The face rides a surface.** Eyes and mouth are mapped onto a flattened sphere (Buzzy's
  screen, Wisp's head, Chirp's body) or an upright cylinder (Hum's capsule) and turned with
  it, so they slide and narrow towards the far edge.
- **The body shows it.** The light stays put while the head turns under it; Buzzy's helmet
  shows a side band (only the crescent, not a second copy); Buzzy's pods and Chirp's wings
  swing round (the near one comes forward, the far one goes behind or thins at the rim,
  never popping at the switch); Wisp's tail trails the head by 0.25 s
  (`angles(t − 0.25)`, still stateless).

| Key | Default | Meaning |
|---|---|---|
| `turn` | 0.71 | how far the head may turn: 1 = ±35° (pitch ±15°), the default ±25°, 0 = flat |
| `turnYaw`, `turnPitch` | 0 | where the state faces (−1..1 of the turn; + = the viewer's right, + = up) |
| `turnWander` | 0 | looking corner to corner and holding each look (3.2 s slots) |
| `turnNod` | 0 | little nods with the voice level (amplitude only) |

The profile gives every character the same language: idle wanders, listening turns to you
and a little up, thinking looks up and to the left (with the gaze), speaking nods and
wanders a little. With no facing numbers the head faces straight ahead, and `turn: 0`
draws exactly the flat character. Off at 20 px; under reduced motion the wander and the
nods stop and only the state's facing stays. Every state stays in the "light" cost class
at 32 and 64 px (worst: Buzzy at 64 px, coverage 0.94 of the medium line).

## Drawing: fills only (`character/geom.rs`)

The paint contract draws by type: fills, then polylines, lines, dots. A stroked outline
would always land on top of every fill, so a torso's outline would cross the head in front
of it. A character is therefore built only from `Fill`s, which keep their own order:
- outlines are ring fills (an outer offset with the inner offset as an even-odd hole);
- thick strokes (the crest, chevrons, the voice line) are turned into polygons with round
  caps;
- shading that must stay inside a body is clipped to it in the engine (Sutherland–Hodgman
  against the convex body).

So the paint contract didn't change: no painter, exporter or packed transport was
touched, and every platform draws a character with the code it already has. The frame is
`colorMode: fixed`: the screen stays dark and the eyes cyan in both themes.

Sizes: 64 draws everything; 32 drops the visor; 20 keeps the helmet, screen, pods, eyes
and mouth, with heavier lines.

## Options

| Key | Default | Meaning |
|---|---|---|
| `hue` | 232 | turns the body (and its outline and screen tints); the eyes and accent stay |
| `mouth` | 1 | 0 = eyes only |
| `accessories` | 1 | 0 drops the crest, chevrons and listening arcs |
| `look` | 1 | how much the eyes glance around on their own (0 = at the viewer) |
| `turn` | 0.71 | how far the head turns (see *The head turn*; 0 = flat) |
| `seed` | 0 | when it blinks and glances |
| `arms` | 1 | 0 takes the arms off (a character with an `arms` part: Beep) |
| `eyeStyle` | 0 | the eye style: 0 = the recipe's, 1 shape, 2 glossy, 3 pixel, 4 dot (see *Eye styles*) |

## Palette (FX Spec 1.12)

`palette` repaints some of a character's colours and leaves the rest as drawn:
`"palette": { "body": "#E63946", "accent": "#FFFFFF" }` in an FX Spec (base and `states`), or
the `palette` prop on a view.
- **Slots** are the recipe's palette names (`characterRecipe(id).palette` on the Web), named by
  the part they paint, never by colour: `body` is every character's main
  colour, `outline`, `cheeks`, `shine` and `iris` mean the same everywhere, and the eyes are
  `eyes` on a body face and `glow` on a screen face. An unknown slot is an error with a "did you mean".
- **Labels:** `spec/character-slot-labels.json` gives every slot a label and a one-line
  description for a colour picker (`{ "buzzy": { "body": { "label": "Body", "description": … } } }`),
  in English, with the localization key `sinua.slot.<character>.<slot>`.
- **Colours** are hex or DTCG, as everywhere in the FX Spec.
- **Tones follow:** given `body`, the recipe's `bodyLight` and `bodyDark` move with it and keep
  their own offset in hue, saturation and lightness. A tone given outright wins.
- **After `hue`:** `hue` still turns the slots it turns; a colour given in `palette` is never
  turned.
- **Contrast:** a recipe names the ink each face part sits on (`contrast`, e.g.
  `[["eyes", "body"]]`). When a new ground comes within 0.35 lightness of an ink left as drawn,
  the ink moves to the far side (0.9 on dark, 0.12 on light). An ink given outright is kept,
  with a warning at `/palette/<ink>`. The outline stays as drawn.
- **Blending:** the override is engine keys (`palette.<slot>.h/.s/.l/.w`, set only through
  `palette`; `params` rejects them). A state change keeps the colour and blends its weight, so
  the hue never sweeps from 0. A view's prop applies at once.

| Character | Slots (a `+Light/Dark` slot's tones follow it) |
|---|---|
| buzzy | body (+Light/Dark), outline, screen, accent, glow, visor, visorEdge, shine, iris |
| hum | body (+Light/Dark), stand (+Dark), grille, glow, outline, tallyListening, tallyThinking, tallyOff, shine, iris |
| wisp | body, bodyMid, tail, outline, eyes, sparkles, twinkles, shine, iris |
| chirp | body (+Light/Dark), belly, feathers, beak (+Dark), outline, eyes, iris |
| cuppa | mug (+Light/Dark), coffee, crema, sleeve (+Dark), heart, shine, cheeks, saucer (+Dark), outline, eyes, steam, iris |
| bean | body (+Light/Dark), groove, grooveEdge, shine, cheeks, aroma, foam, outline, eyes, iris |
| beep | body (+Light/Dark), arms, screen, glow, accent, tallyOff, tallyListening, tallyThinking, outline, rivets, cheeks, iris |

`iris` colours the glossy eye (`eyeStyle: glossy`); the other eye styles have no iris and
ignore it. Beep's `arms` paints only its arms.

### Named palettes and roles (FX Spec 1.13)

A palette can also be written over **roles**, so one palette fits every character: each
recipe's `roles` maps `primary`, `secondary`, `accent` and `iris` to its own slots.
- `"palette": "sunset"` (or `{ "theme": "sunset" }`) applies a built-in named palette:
  `sunset`, `ocean`, `forest`, `candy`, `mono`, `night`.
- `{ "primary": "#E63946" }` paints by role, without knowing the slot names.
- Precedence: the theme's roles < roles written out < slots written out.
- A role a character doesn't map is skipped from a theme (Buzzy and Bean have no
  `secondary`); written outright, it is read as a slot name.
- **Dark theme:** every built-in palette has a dark variant, and
  `"dark": { "primary": "#B5202D" }` gives a file's own. The views pass `dark` in a dark theme
  and the engine picks the variant, so no re-resolve is needed. A character without a variant
  draws the same in both themes, as before.
- On a view, the `palette` prop takes the same keys (`{ theme: "ocean", accent: "#FF6B6B" }`).
- **A slot named like a role:** Buzzy's and Beep's accent role is their `accent` slot, and every
  character's `iris` role is its `iris` slot, so `{ "accent": "#FF6B6B" }` paints the same slot
  either way. The precedence only matters when a role points elsewhere: on Hum,
  `{ "accent": … }` paints `tallyListening`.
- **Eye colour (E4):** `{ "iris": "#7A4BD6" }` gives any character purple glossy eyes; a theme
  or a catalog palette may carry `iris` too.

| Character | primary | secondary | accent | iris |
|---|---|---|---|---|
| buzzy | body | — | accent | iris |
| hum | body | — | tallyListening | iris |
| wisp | body | bodyMid | tail | iris |
| chirp | body | belly | beak | iris |
| cuppa | mug | sleeve | heart | iris |
| bean | body | — | aroma | iris |
| beep | body | arms | accent | iris |

| Palette | primary · secondary · accent (light) | dark |
|---|---|---|
| sunset | #F1774B · #FFD6AD · #D5346A | #DD562C · #D89264 · #EE6391 |
| ocean | #2E87C2 · #D4EDF7 · #17C4B3 | #2E6A9E · #72ADCA · #3CDDC7 |
| forest | #3E8E5B · #E3F1DA · #F1AF3B | #387551 · #90B280 · #F5BB47 |
| candy | #F490B1 · #FFF0F7 · #7E56C2 | #E56C98 · #DF9FC3 · #A07CDE |
| mono | #8B9098 · #E8EAED · #2F3237 | #737882 · #ACB1B9 · #D8DADF |
| night | #3B3F7D · #1F2242 · #F5C451 | #404696 · #363A63 · #F8D062 |

The palettes live in `spec/palettes.json` (embedded by `build.rs` as constants).

A frame-wide `color` or `gradient` doesn't apply to a character. The FX Spec reports it
as an error, and raw `colorMix` / `gradientStrength` overrides are ignored. The generic
effect drawing (ring, tick, burst) and the interrupt flash are skipped too
(`effects::draws_own`): the face is the effect.

## Cosmetics (FX Spec 1.13)

End users pick from a file's `wardrobe` with a `loadout`: see [`character-cosmetics.md`](character-cosmetics.md), *Let end users pick*.

A character can wear a hat, glasses or a badge: `cosmetics` in an FX Spec file, `body` and
`eyes` parts drawn on one of its slots (`headTop`, `face`, `neck`, `chest`) in the slot's
units. They move with the slot (pose, hop, head turn), take `palette` as `<id>.<name>`, and
are left out at 20 px unless `accessories` is on. A cosmetic on `headTop` zooms the character
out about its feet so the hat fits. The guide is [`character-cosmetics.md`](character-cosmetics.md);
the slots of each character are in [`character-recipe.md`](character-recipe.md), *Slots and
cosmetics*.

## The richer look (FX Spec 1.13)

Airbrushed shading, rim light and film grain, with no blur: a recipe's `shade` layers are soft
masses (elliptical lights whose stops fade to alpha 0), `rim` lights the edge the light reaches
first, and `grain` lays a still noise inside every body; the face stays crisp. `"light": "none"`
makes a body an overlay that only carries layers (BUZZY's helmet). The `grain` and `shading`
options (props on every platform, or `params`) tune them; an FX Spec resolved under low power
turns them off. The built-ins are unchanged; `spec/examples/rich-bean.fxspec.json` and
`rich-buzzy.fxspec.json` show the look. Details: [`character-recipe.md`](character-recipe.md),
*The richer look*.

## Remixing a built-in character

A built-in's recipe is data: `characterRecipe("cuppa")` (Web) returns it as JSON. Copy it, give
it a new `id` and `"profile": "<the original>"`, change it, and ship it as an FX Spec 1.12
file's `recipe`. The [remix guide](character-remix.md) walks through it (the result is
`spec/examples/remix-latte.fxspec.json`); the [recipe reference](character-recipe.md) lists every
key, and `spec/character-recipe-1.schema.json` checks it in an editor. The Studio's editor
 does the same with a form; `fitPath` in `@sinua/snippets` turns an SVG from a
drawing tool into a body.

## Tests

- Rust: `geom` (areas, stroke width, ring offsets, convex and half-plane clipping,
  transforms), `face` (blink, lid, smile, mirrored tilt, gaze, the voice line's swing,
  clipping, effect eyes), `rig` (the seeded blink rate and de-sync, the turn blink, the
  mouth rules, the startle, effects, the mute squint, glance bounds), the recipes (every
  built-in recipe parses; bad recipes say where; every recipe in every pose is
  deterministic, fills only, `Fixed` and in its box; slots follow tilt and turn; `show`
  fades a part by voice state; `path` is reserved) and each character's own claims in
  `character/recipe_tests/` (in its box, detail by size, `hue`, ear arcs, the grille, the
  tally, the sparkles, the beak, the crest, mute, colour overrides ignored, determinism).
- sinua golden 1.8.0: each character at every size and time plus eight input-driven cases
  (among them `turned`, thinking's facing, and `turn-off`); wasm, iOS and Android read the
  same file.
- `turn`: every character in its box and "light" at every corner of the turn, `turn: 0`
  equal to the flat frame, 20 px never turning, no part popping across yaw 0, Wisp's tail
  lag; `cost_table` and `bench_turn_all` (ignored) measure it.
- Eye styles (`character/recipe_tests/eyes.rs`): every style inside the eye's bounds, a blink
  shuts each one, the glossy iris follows the gaze while the highlights stay, the 20 px
  collapse, expressions reshape each style, effect eyes ignore it, `eyeStyle` over a recipe's
  `style` / `iris` / `sclera`, unknown names, the three showcases light and frozen.
- FX Spec: the 1.11 gate, `color` rejected, `hue` and rig params accepted, `stateAge`
  rejected in `params`, and `spec/examples/buzzy-assistant.fxspec.json` on every platform.
- `packages/core/test/character.test.mjs`: through wasm, and `StateTransition.stateAge`.
