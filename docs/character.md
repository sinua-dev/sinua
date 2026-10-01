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
(`SinuaAvatar` is the ring-around-a-picture component from `ring`). FX Spec 1.11.

Design note and research (private repo): `sinua-studio/docs/agents/families/design-07-characters.md`,
`research-characters.md`.

## Patterns

| Pattern | Character | Voice signature |
|---|---|---|
| `buzzy` | A small space-hero assistant: a rounded-square indigo helmet with an amber crest and chevron ear pods, a glass visor over a face screen, a voice core in the chest | the mouth is a voice line (two even waves that flow; the level sets their height); the chest bars follow the level; while listening, sound arcs light at the ear pods with the user's level |
| `hum` | A vintage studio microphone that hosts the show: a red capsule on a brass yoke and stand, the face on its grille band | the grille is the mouth: its slots light with the level while speaking, one light scans them while thinking; a tally light is red while listening and blinks amber while thinking; the capsule tips toward you while listening and sways on the yoke while speaking (`swayGain`) |
| `wisp` | A helpful spirit: a round, glowing head flowing into a curling smoke tail, violet into teal, with a soft halo | its sparkles wander at rest, gather in while it listens (closer as the user speaks), orbit its crown while it thinks and stream out with its voice; the tail curls tighter while thinking (`curlGain`); the mouth is an oval that opens with the level |
| `chirp` | A songbird: a coral egg-shaped body with a cream breast, teal wings and a three-feather crest | the beak is the mouth: it opens with the level while speaking and little notes rise from it, the wings flutter (`flutterGain`); it tilts its head and lifts its crest while listening; while thinking the crest drops and three thought dots light in turn |

**Shared, not repeated.** What every character does alike lives once: the face (`face.rs`),
the rig (`rig.rs`), the drawing helpers (`geom.rs`) and the kit (`kit.rs`: the size tiers, the
ground shadow, the effect colours, the celebrate burst, the mute fade and the frame). The
catalog defines the rig and the options once, as `key@character`; a character's own file and
catalog entries hold only what makes it itself. Each character is named after a sound.

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

The head yaws and pitches like a solid, not a flat sticker (design note 8, after a
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
| `hue` | 232 | turns the shell (and its line and screen tints); the eyes and amber stay |
| `mouth` | 1 | 0 = eyes only |
| `accessories` | 1 | 0 drops the crest, chevrons and listening arcs |
| `look` | 1 | how much the eyes glance around on their own (0 = at the viewer) |
| `turn` | 0.71 | how far the head turns (see *The head turn*; 0 = flat) |
| `seed` | 0 | when it blinks and glances |

A frame-wide `color` or `gradient` doesn't apply to a character. The FX Spec reports it
as an error, and raw `colorMix` / `gradientStrength` overrides are ignored. The generic
effect drawing (ring, tick, burst) and the interrupt flash are skipped too
(`effects::draws_own`): the face is the effect.

## Tests

- Rust: `geom` (areas, stroke width, ring offsets, convex and half-plane clipping,
  transforms), `face` (blink, lid, smile, mirrored tilt, gaze, the voice line's swing,
  clipping, effect eyes), `rig` (the seeded blink rate and de-sync, the turn blink, the
  mouth rules, the startle, effects, the mute squint, glance bounds) and `buzzy` (in its
  box at every size and pose, fills only and fixed, detail by size, `hue`, ear arcs,
  mute, colour overrides ignored, determinism).
- sinua golden 1.8.0: each character at every size and time plus eight input-driven cases
  (among them `turned`, thinking's facing, and `turn-off`); wasm, iOS and Android read the
  same file.
- `turn`: every character in its box and "light" at every corner of the turn, `turn: 0`
  equal to the flat frame, 20 px never turning, no part popping across yaw 0, Wisp's tail
  lag; `cost_table` and `bench_turn_all` (ignored) measure it.
- FX Spec: the 1.11 gate, `color` rejected, `hue` and rig params accepted, `stateAge`
  rejected in `params`, and `spec/examples/buzzy-assistant.fxspec.json` on every platform.
- `packages/core/test/character.test.mjs`: through wasm, and `StateTransition.stateAge`.
