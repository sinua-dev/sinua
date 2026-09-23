# The `beacon` family

## Why this exists

`docs/gpt.md`'s taxonomy names `beacon` as "Event / notification / location
/ network / attention". Unlike `orbs`, `signal` and `ring` — all continuous,
ongoing indicators — a beacon draws attention to a **single moment**, often
one-shot, sometimes sparsely repeating, and then fades. It is the fourth
sibling family in `crates/core_engine/src/beacon/` (see
[`architecture.md`](architecture.md#the-family-pattern)); `lib.rs`'s
`resolve_any` tries `orbs`, `signal`, `ring`, then `beacon`.

Directly relevant uses for this project's own audience: a "new voice
message" / "incoming call" badge, a connection-quality / reconnecting
indicator (a genuine pain point in WebRTC voice apps), and a location mark.

## States and modes

| State | Mode | File | What it looks like |
|---|---|---|---|
| `notifying` | `ping` | `modes/ping.rs` | A solid badge dot with rings that expand outward and fade — the notification / radar-ping cue. `ringCount` staggered ripples; `once: 1` plays a single ripple after `t = 0` and then rests on the badge. |
| `reconnecting` | `pulse` | `modes/pulse.rs` | A pulsing dot inside a four-segment connection-quality ring. `quality: 0..1` (LiveKit's lost / poor / good / excellent) sets how many segments are lit and scales the pulse down: lost pulses fully, excellent holds a steady dot with all four lit. |
| `scanning` | `radar` | `modes/radar.rs` | A beam sweeping clockwise over a dot-field scope. The scope lights up just behind the beam and fades, and seeded "blips" flare as the beam passes, then persist and dim until the next pass. It's the "searching / nearby / discovering" cue. |
| `broadcasting` | `broadcast` | `modes/broadcast.rs` | The `((•))` glyph for "live", "transmitting" and "on air": a dot with radio-wave arcs on each side that light up in sequence (iterative or cumulative, optionally reversing), or statically to a `level`. |
| `locating` | `halo` | `modes/halo.rs` | A dot inside a translucent accuracy halo with a slow ring pulsing from the dot to the halo's edge — Google Maps' blue dot. `accuracy: 0..1` (1 = certain) shrinks the halo and the pulse's reach. |

Like `signal` and `ring`, no base-profile scaling: every dimension is a
fraction of `size`, resolved opts are empty, speed is 1.0 (periods are in
seconds: Tailwind's 1s ping and 2s pulse). The `reconnecting` dip curve is
the shared `primitives::pulse_wave`, the same one the generic
`apply_pulse` post-process uses, so `pulseStrength` on any other state
pulses exactly like this dot ([`engine.md`](engine.md#pulse-and-decay-shared-energystate-envelopes)).

## Prior art (fetched, not recalled, 2026-09-18)

- **Tailwind CSS `animate-ping`** (`tailwindcss.com/docs/animation`):
  `75%, 100% { transform: scale(2); opacity: 0 }`, `1s cubic-bezier(0, 0,
  0.2, 1) infinite`, documented as "useful for things like notification
  badges" (radar ping / ripple). `ping` transcribes this exactly: each ring
  grows from the dot's radius to `ringReach` and fades to nothing by 75%
  of the period, then rests. **`animate-pulse`**: `50% { opacity: .5 }`,
  `2s cubic-bezier(0.4, 0, 0.6, 1)` — `pulse`'s dot.
- **androidx Material 3 `Badge.kt`** (raw source): a content-less badge is
  a small circle (`BadgeTokens.Size`), a badge with content a pill. This
  engine has no text, so the badge is the dot.
- **LiveKit `ConnectionQualityIndicator.tsx`** (raw source): sets
  `data-lk-quality` to `excellent / good / poor / lost / unknown`; the
  `quality` opt maps those onto `1 / 0.66 / 0.33 / 0` and the lit-segment
  rule (`ceil(quality · 4)`) shows *something* for any live-but-poor link.
- **The "Reconnecting…" pattern** (server-sent-events.com's connection-status
  guide, via WebSearch): a pulsing amber dot next to the text; "color
  carries none of the meaning on its own — each state needs accompanying
  text". Hence grey is the default here, amber is one `hue` opt away, and
  **the text is the caller's job**.
- **Google Maps' blue dot** (WebSearch coverage): the dot "pulsates,
  growing into a wide, semi-transparent circle before shrinking quickly
  back"; the translucent circle around it is the position's uncertainty —
  "the smaller it is, the more certain". `halo` maps `accuracy` onto that
  halo's radius and the pulse's reach.

## Geometry

All three modes use the shared `primitives::arc_polyline` (a 360° sweep is
a closed circle) and `primitives::cubic_bezier` — the arc sampling and
easing solver first written for `ring` and moved to `primitives.rs` the
moment a second family needed them, so `beacon` doesn't import from `ring`
(the same "an earlier family must not become a false hub" reasoning behind
`primitives.rs` itself). The badge and the halo are `Dot`s: a filled circle
is the one fill this engine already has, and the halo gets `z = −1` so
`finalize_frame`'s z-sort draws it under the dot with no special casing.

- `ping`: `period` 1.0, `ringCount` 2 (staggered by `period / rings`), `once`
  0, `dotSize` 0.12, `ringReach` 0.41, `ringWidth` 0.03. Ring alpha
  `0.6·(1 − e)` with `e = cubic-bezier(0,0,0.2,1)(u / 0.75)` — invisible
  past 75% of the cycle, which `with_polylines` culls.
- `pulse`: `period` 2.0, `quality` 0, `dotSize` 0.12, `segmentRadius` 0.3,
  `segmentWidth` 0.04, `segmentGap` 14. Dip `d = ease(tri(u)) · (1 −
  quality)`: dot alpha `0.95·(1 − 0.5d)`, radius `× (1 − 0.1d)`.
- `halo`: `period` 2.0, `accuracy` 0.5 → halo radius `lerp(0.41, 0.15,
  accuracy)·size`, `haloOpacity` 0.14, `dotSize` 0.1, `ringWidth` 0.025.

## The one-shot contract

The engine is a pure function of `t`, so "an event just happened" is
expressed by the **caller restarting `t` from 0** at that moment — with
`once: 1` the ripple then plays exactly once and rests on the badge. This
is the engine-side equivalent of Vercel Geist's spinner rule ("mount the
Spinner only after the action starts"). `BeaconStudio`'s **Trigger** button
does exactly this to the panel's clock.

The interrupt/barge-in flash (`primitives::apply_interrupt`) is
conceptually a beacon behavior too, but it has to layer over *another
family's* frame, which a family can't do — so it stays a post-process. If a
compositing step (several families in one frame) ever exists, that's where
the two would meet.

## Tests

No golden vector. `ping`: the badge is always there; a ring is born at the
dot's edge at full alpha, expands and fades, and is gone for the last
quarter of the period; two rings are staggered by half a period; `once`
plays one period then rests while the default repeats. `pulse`: a lost
connection pulses from alpha 0.95 to 0.475 with a slight shrink; full
quality lights all four segments and holds the dot steady; `0.5` lights
two, `0.33` still two (LiveKit "poor" shows something), `0.25` one;
segments are open arcs with gaps. `halo`: the halo shrinks with accuracy
and z-sorts under the dot; the ring pulses from the dot toward (never
past) the halo's edge, fades, and is gone by the end of the period.

## Radar (`scanning`): design notes

**Prior art (fetched, see the families LOG):**
- **CodeFronts' "Weather Radar Sweep" CSS.** The beam is `conic-gradient(acc
  65% 0°, acc 22% 18°, transparent 46°)`, turning `3.2s linear infinite`
  over range rings drawn every 21%. Cells brighten as the beam reaches them
  with the keyframes `0%, 5% {opacity .95} 55% {.45} 100% {.12}` over one
  turn, phase-offset by angle.
- **The plan position indicator (Wikipedia).** "A radial trace on the PPI
  sweeps in unison with [the antenna]", with long-persistence phosphor
  keeping blips visible between rotations and range drawn as concentric
  circles.
- The CodePen/CSS-Zone radar loaders follow the same anatomy.

**No new primitive: the decision, written out per the house rule.** The
beam is a filled wedge that fades with angle, and the engine has no fill
primitive. Two options with the existing primitives:
1. **Round-capped `Polyline` arcs.** A radius-wide arc covers the disk
   radially, but its round caps stick out R/2 past both ends, so the glow
   shows up *ahead* of the beam. Stacking arcs has the same problem.
2. **A dot field.** The scope is `ringCount` range rings of `Dot`s. Each
   dot's alpha comes from its angle behind the beam, run through
   CodeFronts' stops. This is the same move `apply_gradient` makes: on a
   dot renderer, a gradient becomes per-dot color.

Option 2 is used. The leading edge is a crisp `Polyline` from center to rim.

**One deliberate deviation, found on the contact sheet.** CodeFronts' 46°
trail, sampled by dots, lights only two or three dots per ring, so the
beam read as a clock hand rather than a sweep. The default is therefore
**90°** (`trailLength`). The stops keep the same ratios (0.65 at the beam,
0.22 at 18/46 of the way back, 0 at the end), just stretched. Dot spacing
was also tuned by eye: a pitch of 4.5 dot radii, compared against 6 (too
sparse) and 3.5 (the idle scope turns into a grey disc).

**Keys:**

| Key | Default | Meaning |
|---|---|---|
| `period` | 3.2 | seconds per turn (CodeFronts) |
| `ringCount` | 4 | range rings, 1..8 |
| `dotSize` | 0.018 | radius of each scope dot |
| `trailLength` | 90 | degrees the trail extends behind the beam |
| `blipCount` | 3 | number of blips, 0..12; positions come from `hash_d`, so they're deterministic |
| `seed` | 0 | changes the blip layout |
| `hue` / `saturation` | 200 / 0 | grey by default; use `hue: 120` for CRT green |

**Rendering:**
- Scope dots sit at idle alpha 0.10, and the idle field stands in for the
  range rings. At the beam they rise to 0.95 and grow by 40%.
- Blips follow CodeFronts' cell keyframes, keyed on how far the beam has
  travelled past them.
- The beam is a `Polyline` drawn under the dots (paint order). The origin
  dot sits on top.
- Stateless: rendering is a pure function of `t`.

**Tests** (`radar.rs`):
- the beam starts at 12, reaches 3 o'clock at period/4, and repeats every period
- the trail lights dots behind the beam, never ahead, and is monotone
- dots sit exactly on `ringCount` radii, with the outer rings denser
- the blip curve is 0.95 → 0.45 → 0.12, monotone
- `blipCount` and `seed` behave deterministically
- grey by default, and `hue` reaches every element

## Broadcast (`broadcasting`): design notes

**Geometry** comes from Material Symbols' `sensors` icon. I parsed the raw
SVG path by hand (the fetch summarizer's own numbers were wrong). In the
960 box:
- a center dot with r 80
- two arcs per side, with centerline radii 200 and 360 and an 80 stroke
- each arc spans 90° and is centered on the horizontal axis

Our version:
- The outer arc's edge is scaled to sit on the 0.41·size silhouette.
- The first arc keeps Material's one-stroke gap from the dot.
- Waves are evenly spaced.
- With more waves the stroke thins so the pitch stays at two strokes, which
  keeps a one-stroke gap between waves. The contact sheet showed 4 waves
  merging into a blob before this fix, and a test now covers it.
- Material's arc ends are cut flat; our paint contract rounds them.

**Motion** follows SF Symbols' **Variable Color** on
`dot.radiowaves.left.and.right` (WWDC23 "Animate symbols in your app",
plus Donny Wals, Daniel Saidi and blakecrosley.com):
- Layers light up in sequence.
- `.iterative` lights one layer at a time; `.cumulative` fills and holds;
  `.reversing` runs the sequence back out.
- Inactive layers are either dimmed or hidden.
- As a static value, "0.5 colorizes half the beams".

How this maps to our keys:
- `cumulative` (0/1) and `reversing` (0/1) select the behavior.
- `inactiveOpacity` controls the unlit waves; 0 hides them.
- `level` (0..1) sets the static value. When present it replaces the
  animation, and the last lit wave can be partially lit.
- Each sequence starts with one step where only the dot is lit.
- Neighbouring steps cross-fade over 0.15 of a step, so it doesn't strobe.
- **`period` 1.2s is a first guess.** Apple publishes no timing for the
  effect.

**Keys:**

| Key | Default | Meaning |
|---|---|---|
| `waveCount` | 2 | arcs per side, 1..4 |
| `sides` | 2 | 2 = `((•))`, 1 = right only |
| `waveSweep` | 90 | degrees per arc |
| `period` | 1.2 | seconds per lighting sequence |
| `cumulative` | 0 | 0 = iterative, 1 = cumulative |
| `reversing` | 0 | 1 = sequence runs back out |
| `inactiveOpacity` | 0.18 | alpha of unlit waves; 0 hides them |
| `level` | absent | static 0..1; replaces the animation when set |
| `dotSize` | 0.07 | center dot size |
| `strokeWidth` | 0.07 | arc stroke; thinned automatically for many waves |
| `hue` / `saturation` | 200 / 0 | grey by default; e.g. `hue 0, saturation 0.85` for a red "live" dot |

**Tests** (`broadcast.rs`):
- the default layout: a dot plus 2×2 arcs at 3 and 9 o'clock, mirrored, inside the silhouette
- many waves thin the stroke instead of merging
- `sides 1` and the clamping of `waveCount`
- iterative: one wave at a time, after a dot-only beat
- cumulative never removes a wave on the way out
- `reversing` is symmetric, and the period repeats
- `level` is independent of `t`
- hiding culls unlit waves
- grey by default, and the hue applies everywhere

## Studio

The Web Studio's `families/beacon/{BeaconStudio.tsx, knobs.ts}` — minimal,
functional, mirroring `SignalStudio`'s skeleton so it inherits the kit
styling: state picker, size, per-state knobs (`period`/`ringCount`/`once`;
`quality`; `accuracy`), badge size, Color, a Resolved readout, and the
**Trigger** button. Registered in `App.tsx`'s `FAMILIES` as `beacon`.
`Scanning` has its own UI ids (`radarPeriod`, `radarRings`, `radarDotSize`)
mapped onto the engine's `period`/`ringCount`/`dotSize`, because its
defaults differ from ping's. It also adds Trail, Blips and Blip layout,
and hides the badge-size knob. `Broadcasting` adds Waves, Sides, Period,
Fill (iterative/cumulative), Reverse, Inactive, Level and Dot size (UI ids
`bcPeriod` and `bcDotSize` map to `period` and `dotSize`). Level has an
"animate" setting just below 0: in that position the key is left out
entirely. The state grid is 3 columns.

## Known, not done

- No numbered badge (no text primitive).
- Radar: **filled wedge done as an option (`trailFill` = 1, materials phase 1).**
  - The trail is one polygon from the beam back `trailLength` degrees. The default is **46**, CodeFronts' own value: the 90 only compensated for dots sampling the wedge. Capped at 120.
  - It is faded by a **linear gradient across the wedge**, from the beam edge to the trail's end along the 0.6 r chord, using CodeFronts' stops (0.65 at the beam, then 0.22 at 18/46 of the way, then 0).
  - For wedges up to 120° that reads as the conic fade it stands in for. There's no conic in the contract (SwiftUI iOS 18+, and none in SVG). One polygon instead of a fan of slices means no anti-aliasing seams.
  - The scope dots stay at their idle look.
  - The default stays the dot trail. There are
  also no caller-supplied target positions (`blip{i}Angle`/`Range`); blips
  are seeded. Add those if a real consumer needs them.
- Ink lightness is fixed at the family-wide `white` 0.15, so a saturated
  hue renders dark on a light theme (the same contract every family uses);
  an `ink` opt would be the extension if a brighter badge is wanted.
- Native Studio panels: none, same as `signal`/`ring`; frames render on
  every platform through the existing paint contract.
