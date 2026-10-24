# The `signal` family

## What it is

`orbs` (see [`engine.md`](engine.md)) is a floating presence. `signal` is the family of shapes that show a
value: a voice's level, a meter, a clip's progress. `crates/core_engine/src/signal/` is a sibling module to
`orbs`, with its own state list (`signal::presets::STATES`) and its own preset resolution
(`signal::presets::resolve_preset`); its states never appear in `orbs::presets::STATES`, and vice versa. See
[`architecture.md`'s *family pattern*](architecture.md#the-family-pattern).

`lib.rs`'s `resolve_any` tries each family's `resolve_preset` in turn, so one public
`frame`/`frame_with_overrides`/`resolved_opts` surface covers every family.

Since FX Spec 1.15 (0.1.0-beta.12) the family is the three light volumes below. The flat patterns it had before
(`signaling`, `waveform`, `scrolling`, `metering`, `playing`) were removed; a file that still names one gets an
error that names its replacement.

## Light volumes: `veil`, `crystal`, `dune`

Each is hundreds of points of light in 3D (`crates/core_engine/src/volume/`), seen through a perspective camera.
A point is lit by its depth and drawn as a soft glow; on a dark background the glows add up ([`engine.md`](engine.md),
*Light*). Motion comes from a noise field, so it doesn't repeat.

| Pattern | Mode | File | What it is | With a voice | Without audio |
|---|---|---|---|---|---|
| `veil` | `veil` | `modes/veil.rs` | A sheet of light: layers of points curl one behind the other | swells with the level | `churn` keeps it moving while something is worked on; `level` is any 0..1 value |
| `crystal` | `crystal` | `modes/crystal.rs` | A crystal filled from below: each column lights up to its value, unlit cells stay as dust | the bands fill the columns | `level` fills it as one value (a battery, a quota, a goal) |
| `dune` | `dune` | `modes/dune.rs` | A dune: a series is its profile, the done part glows, a plane of light marks the position (box layout) | a clip's `envelope` + `progress` | a long task's `progress` |

- **The inputs are generic, not tied to the voice.** A shape reads them in this order:
  1. its own value (`level` or `progress`), from the app or an FX Spec binding;
  2. else the bound voice's `audioLevel` and bands, which count only while it listens or speaks;
  3. else its own noise field, so it lives with no input.
- **Tags, not locks.** The catalog tags each one `voice` and `status`; any input fits any shape.
- **Voice states** come from the profile:
  - `churn` 1 while thinking;
  - the generic swell, glow, pulse and colour mix are off, since a light volume draws its own light.
- **Point budget:** about 150 points at 48 px, 420 at 128 px and 1,000 from 256 px. A small view drops depth
  first, then resolution.
- **Reduced motion** holds the camera and the noise still and keeps each state's pose.
- **Low power** (`blurScale` 0) draws plain additive dots instead of soft glows.

## `dune` in a strip: the voice message

`SinuaVoiceMessage` (Web, iOS, Android, React Native) draws `dune` in a wide box: the clip's loudness as
`envelope0..63`, the position as `progress`, the box ratio as `aspect`.

- **Up to an aspect of 2.2** dune is the turned volume of the table above.
- **From 2.2 to 3** it eases into a strip: the camera turns to a side view, the dune fills the box with a 6 %
  margin each side, the sand stands taller so the envelope reads at 40 px, and the unplayed sand is brighter.
- **Seeking:** `playbackSeekProgress(aspect, x)` maps a touch to a position by inverting dune's own drawn base
  line (the same projection), so a finger on a spot of the dune lands on that spot's position; outside the dune
  it clamps to 0 or 1. The name is kept from the old bar row, so apps don't change.
- With `progress` given, the playhead holds still; without it, it drifts slowly across.

## `muted`: the mute / error cue (an opts flag, not a state)

Every voice product has to show "mic muted" / "connection lost". Prior art (fetched): Amazon's Voice
Interoperability design guide says a product must convey the microphone's on/off state and must not reuse a colour
with a conflicting meaning; an Echo's light ring goes solid red with the mic off; Google Meet turns the mic icon
red with a slash.

It's `primitives::apply_muted`, a mode-agnostic post-process run last in `lib.rs`'s `render()`, so every pattern
gets it with no mode code. Opts: `muted` (`0..1`, `1` = fully muted; a continuous value lets a caller ease it),
`mutedTint` (`0..1`, default `0` = grey; above `0` tints toward `mutedHue`), `mutedHue` (degrees, default `8`).
Effect on every mark: alpha `× (1 − 0.55·m)`, ink lightened, saturation `→ lerp(sat, 0.85·tint, m)` with the hue
snapped to `mutedHue` when tinting, radius/stroke `× (1 − 0.15·m)`. Geometry is untouched, which is what keeps it
mode-agnostic.

**Slowing the animation is the caller's job, on purpose.** The engine is a pure function of `t`; scaling `t`
inside it would snap the pattern to a different pose the instant `muted` toggles. So, exactly like `t * speed`
(see [`engine.md`](engine.md#the-t--speed-callout)), the caller advances its own clock more slowly while muted:
an accumulator (`elapsed += dt × (muted ? 0.1 : 1)`), not wall time, so the rate can change with no jump.
