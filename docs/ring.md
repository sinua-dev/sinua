# The `ring` family

## Why this exists, and what it is not

`docs/gpt.md`'s component taxonomy names five peer primitives — Orb, Core,
Ring, Signal, Beacon. `ring` is "Progress / activity / energy / status": a
flat, precise, dashboard-feeling circular indicator, deliberately the
opposite texture from Orb's organic ambient sphere. It is the third sibling
family in `crates/core_engine/src/` (`ring/`, next to `orbs/` and
`signal/` — see [`architecture.md`](architecture.md#the-family-pattern));
`lib.rs`'s `resolve_any` tries `orbs`, then `signal`, then `ring`, so the
one public `frame`/`frame_with_overrides`/`resolved_opts` surface covers it
with no new function names.

It is deliberately **not voice-specific**. Its only inputs are a generic
`progress` (determinate) or nothing at all (indeterminate) — a download, an
upload, a long tool call, a daily goal, any app with a task in flight —
which is exactly the product plan's "the FX library doesn't know business logic,
just generic inputs". It is also not a copy of Apple's Activity rings (see
*Prior art*).

## States and modes

| State | Mode | File | What it looks like |
|---|---|---|---|
| `completing` | `arc` | `modes/arc.rs` | A determinate progress ring: a round-capped arc sweeping clockwise from 12 o'clock over a faint track, driven by a `progress: 0..1` opt — the literal ring geometry that `orbs`' `progressing`/`eclipse` expresses as a day/night terminator. |
| `loading` | `spinner` | `modes/spinner.rs` | The indeterminate activity indicator: a chasing arc that grows to 270°, shrinks back to a dot, and keeps rotating — Material's spinner motion, transcribed from source. |
| `stepping` | `segmented` | `modes/segmented.rs` | A ring split into `segmentCount` equal, round-capped segments that fill in order from 12 o'clock — onboarding steps, "3 of 5 done", battery bars; or per-segment values (`segment0..N`) for WhatsApp-style seen/unseen statuses. |
| `measuring` | `gauge` | `modes/gauge.rs` | An open-bottom value arc (default 270°): a speedometer, battery or volume dial, a Watch complication. The value is shown as a bar (`fill`), a marker dot (`marker`), or both. |
| `tracking` | `nested` | `modes/nested.rs` | Several values as concentric rings, one per ring (`progress0..3`, outermost first) — steps / active minutes / sleep, a quota per resource. A value past 1 draws another lap. Generic, **not** Apple's Activity rings (see *Nested* below). |

Like `signal`, there is no base-profile/scaling machinery
(`ring::presets::resolve_preset` returns an empty opts map, speed 1.0):
every dimension is a fraction of `size` inside the mode function, so each
opt's own `get(.., default)` is the default.

## Prior art (fetched, not recalled, 2026-09-18)

- **androidx Material 3 `ProgressIndicator.kt`** (raw source): start angle
  `270f` (12 o'clock in canvas terms), determinate sweep `progress * 360`
  clockwise, `StrokeCap.Round` for both variants, and M3 expressive's
  `TrackActiveSpace` — the track stops short of the indicator's ends by a
  small gap. Its indeterminate variant is parameterized as a 6000ms cycle
  (1080° global rotation, extra 360° "jumps" every 1500ms, arc 0.10–0.87 of
  a turn under the standard easing).
- **Material Components Web `mdc-circular-progress`** (`_circular-progress.scss`,
  `_circular-progress-theme.scss`): `$arc-time 1333ms`, `$arc-size 270deg`,
  `$arc-start-rotation-interval 216deg`, `cubic-bezier(0.4, 0, 0.2, 1)`,
  48px with a 4px stroke, and the left/right-spin keyframes that grow and
  shrink the arc. This is the parameterization `spinner` transcribes — the
  simpler of the two to reproduce exactly.
- **Vercel Geist `Spinner`**: "Use a Spinner for indeterminate, single-action
  waits of roughly one to three seconds"; "Mount the Spinner only after the
  action starts. Pre-rendering and toggling visibility leaves a partial
  rotation visible at idle and reads as jank"; set `aria-busy` on the
  in-flight element. The mount rule is a **caller-side** rule this engine
  can't enforce — a caller should start feeding `t` from the moment the
  action starts, not keep a spinner rendering at `t = const`.
- **Apple HIG, Activity rings**: reserved for Move/Exercise/Stand only,
  "never change the look of the rings", not for ornamentation or branding.
  Looked at and **deliberately not imitated** — this family ships a generic
  single ring and a spinner, not a stacked triple. (Apple's HIG pages for
  progress indicators and Material's own spec pages are JavaScript-rendered
  and came back empty over plain fetch; the same specs were read from the
  source code above instead.)

## Geometry (`primitives::{ring_point, arc_polyline, cubic_bezier}` + `modes/arc.rs`'s `layout`)

(The arc sampling and easing solver were written for this family as
`ring/geom.rs` and moved to `primitives.rs` the moment `beacon` needed them
too — see [`architecture.md`](architecture.md#the-family-pattern).)

Angles are measured **clockwise from 12 o'clock**: `x = cx + r·sin θ`,
`y = cy − r·cos θ`. The stroke is `strokeWidth` (default 0.085, Material's
4/48) of `size`; the centerline radius is `0.41·size − stroke/2`, so the
outer edge sits on the same 0.82 silhouette every family uses. An arc is
one `Polyline` sampled every ~4° with both ends exact — the paint contract's
round caps are what make an arc's ends look like Material's. A zero-length
arc is two coincident points, which renders as a round dot of one stroke
width: that *is* Material's "0% progress" look, for free. `standard_ease`
is a real `cubic-bezier(0.4, 0, 0.2, 1)` solver (Newton with a bisection
fallback, the way browsers evaluate CSS easings), not a smoothstep stand-in.

**`arc` (`completing`)**: `progress` → `sweep = progress·360`. The track is
a second, fainter polyline (`trackOpacity` 0.18) covering the complement of
the indicator, shortened by `gap` stroke widths (default 1, M3's
`TrackActiveSpace`) at both ends — so the track disappears at `progress =
1` and is nearly a full circle at `progress = 0`. Indicator draws last.
`hue`/`saturation` color the indicator; the track stays grey.

**`spinner` (`loading`)**: one arc cycle is `CYCLE_S = 1.333`. Within a
cycle (`u = frac(t / 1.333)`), the head sweeps `270·ease(min(1, 2u))` and
the tail follows with `270·ease(max(0, 2u − 1))`, so the arc grows through
the first half and shrinks through the second, never below a 2° dot. The
base rotation is `(270 + 225)·cycles + 225·u`: the 270 matches the tail's
own travel so the arc's start is continuous across the cycle boundary (a
test checks this), and the extra 225°/cycle plays the role of MDC's 216°
start-rotation interval — successive cycles start at different clock
positions instead of repeating the same one. No track by default
(Material's indeterminate variant has none); `trackOpacity > 0` adds one.

Every mode-agnostic post-process applies unchanged: pointer scatter (no
dots, so a no-op), the audio pulse (no dots, no-op), the mute cue, and the
interrupt flash — whose centroid is taken over polyline vertices too, so a
`loading` ring flashes on a barge-in like everything else.

No golden vector — same tradeoff as every non-ported family. Correctness
rests on the unit tests: `geom` (12 o'clock is up, 90° is 3 o'clock; an
arc's ends are exact and a zero sweep is a dot; the easing is monotone
through the corners and ease-in-out), `arc` (0% is a dot at 12 over a
near-full track; 25% ends at 3 o'clock, clockwise; 100% closes the ring
and drops the track; the track keeps `gap_deg` from both indicator ends;
stroke scales with `size`), `spinner` (sweep stays within `[2°, 270°]`;
grows then shrinks within a cycle, fully extended at the midpoint; start
is continuous across the boundary and advances 495° per cycle; one
round-capped polyline, deterministic in `t`).

## Segmented (`stepping`): design notes

Transcribed from androidx **Wear Material 3
`SegmentedCircularProgressIndicator`** (raw source, fetched; see the
families LOG): segments start at 12 o'clock (`StartAngle = 270`); the
continuous variant fills `segmentCount · progress` segments with the last
one partially filled; the per-segment variant takes `segmentValue(i) →
Boolean`; and the gap between segments is `2·asin((stroke + gapSize) /
(size − stroke))`. The `+ stroke` term is what makes the visible gap
between two round caps exactly `gapSize`. The default gap is Wear M3's
`calculateRecommendedGapSize(strokeWidth) = strokeWidth / 3`. The
per-portion variant also matches 3llomi's `CircularStatusView` (the
WhatsApp Status clone): `portions_count`, `portion_spacing`, and
`setPortionColorForIndex` for seen/unseen.

**Keys:**

| Key | Default | Meaning |
|---|---|---|
| `segmentCount` | 5 | 1..24 |
| `progress` | 0 | 0..1; same key and meaning as `arc` |
| `segment0..N` | — | 0..1 fill per segment. When present, it overrides `progress` for that segment. M3's boolean generalized to a fraction |
| `gap` | 1/3 | the visible gap, in stroke widths |
| `strokeWidth` | 0.085 | stroke and radius come from `arc`'s shared `layout`, so a 1-segment ring equals `arc` exactly (tested) |
| `trackOpacity` | 0.18 | |
| `hue` / `saturation` | 200 / 0 | |

**Layout:** boundaries sit at `k·360/n`, and every gap is centered on its
boundary, so the first gap straddles 12 o'clock. `segmentCount 1` has no
gap. When there are too many segments for the stroke (the sweep would go
≤ 0), each segment collapses to a dot at its middle. The dot is shrunk to
`chord − gap`, so neighbours keep the gap instead of merging into a blob.
Both the collapse and the shrink were found and tested on the contact sheet.

**Drawing:**
- Each segment's full extent is drawn as track.
- The filled sub-arc starts at the segment's own start.
- An empty segment draws **no** indicator. `arc`'s 0% dot means
  "started", and an empty step shouldn't look started.

**Not transcribed:** Wear M3's `allowProgressOverflow` (wrap past 1 with an
overflow track color). `tracking` already expresses "more than 100%" as
laps, so a second overflow scheme would be redundant.

**Tests** (`segmented.rs`):
- empty = tracks only; full = every segment lit and covering its track
- 50% of 4 = two segments; 50% of 5 = two plus a half, ending at the exact angle
- the centerline chord across a gap = stroke + gap, and the gaps are symmetric about 12
- `segment{i}` overrides `progress`
- one segment = a full ring
- crowded segments = finite, non-overlapping dots
- a 1-segment ring equals `arc`

## Gauge (`measuring`): design notes

**Prior art (fetched; see the families LOG):**
- androidx Wear M3's progress samples open the ring at the bottom with `startAngle = 120f, endAngle = 60f`, which is a 300° sweep. They round a tiny value up to the stroke width, the same as our 0% round-cap dot.
- SwiftUI's `accessoryCircular` gauge style is "an open ring with a marker … at the gauge's current value", with no fill. It's used for Watch complications and Lock Screen widgets.
- Home Assistant's `ha-gauge.ts` is a 180° arc with an arc mode, a needle mode, and butt-capped severity levels.
- AG Charts' radial gauge measures angles clockwise from the top and shows the value as "a bar, a needle or both".

**Keys:**

| Key | Default | Meaning |
|---|---|---|
| `progress` | 0 | 0..1; same key as `arc`/`stepping` |
| `sweep` | 270 | degrees, clamped 90..330. 270 comes from the widening brief, between HA's 180 and Wear M3's 300 |
| `fill` | 1 | the value bar, Wear M3 style |
| `marker` | 0 | a dot at the value, SwiftUI style |
| `strokeWidth` | 0.085 | shared `layout`, same as `arc` |
| `trackOpacity` | 0.18 | the whole sweep, drawn under the fill |
| `hue` / `saturation` | 200 / 0 | |

**Angles:** measured clockwise from 12. The start is at `180 + (360 −
sweep)/2`, so a 270° sweep starts at 225° (7:30) and ends at 135° (4:30),
and the opening is centered at 6 o'clock. The value sits at `start +
progress·sweep`, and 50% is exactly 12 o'clock.

**Marker:** the marker reuses `nested`'s paper-colored separation halo. It
is a paper-colored dot 1.9× the stroke with an ink dot 1.3× the stroke on
top. This cuts the marker out of the track and fill in both themes; the
contact sheet shows it correctly in light and dark.

**Tests** (`gauge.rs`):
- the default runs 225° → 135° through 12, mirrored about the vertical axis
- 50% lands at 12, 100% ends where the track ends, and 0% is a dot
- `sweep` is clamped to 90..330
- with only the marker on, you get track + halo + dot and no bar
- the marker sits on the fill's head and is drawn last
- stroke and radius equal `arc`'s

## Nested (`tracking`): design notes

> **For any App Store app — Apple HIG constraint.** Apple's
> Human Interface Guidelines on Activity rings: "Use Activity rings only to
> show Move, Exercise, and Stand information", "Don't attempt to replicate
> or modify Activity rings for other purposes", "Never use Activity rings
> to display other types of data" (also: not for ornamentation, branding,
> labels or backgrounds). `tracking` is therefore a **generic** concentric
> ring and ships none of Apple's signature look as a default: no
> red/green/cyan triad, no color gradient along the ring, no arrow glyphs
> at the tips, no black disc behind it. Grey by default, one `hue` when
> saturated, `hueStep` for per-ring hues. Choosing colors, a background and
> iconography that recreate Apple's rings for non-Activity data is exactly
> what that guideline forbids — check final product styling against it.

Prior art that *isn't* Apple's (fetched, see the families LOG, 13:20
entry): Google Fit's two concentric rings (Heart Points + Steps, a double
arrow once the goal is surpassed), MKRingProgressView (values past 100% as
extra laps, a shadow under the line end, a backdrop ring per ring, a
grouped three-ring example), and swdevnotes' SwiftUI rings (past-360°
sweep with an end cap).

**Keys:** `ringCount` (1..4, default 3), `progress0..3` (per ring, `0..
maxLaps`; the single `progress` key stays `arc`'s), `strokeWidth` (0.07,
thinner than `arc`'s 0.085 so three rings leave a hole), `spacing` (gap
between rings in stroke widths, 0.25), `trackOpacity` (0.18), `maxLaps`
(3), `hue`/`saturation`/`hueStep` (200 / 0 / 0).

**Geometry:** the outer ring sits on `arc`'s 0.41·size silhouette; ring
`i`'s centerline is `r₀ − i·stroke·(1 + spacing)`. If that would bring the
innermost centerline inside 0.12·size, the stroke shrinks instead
(`max_stroke = size·0.29 / (0.5 + (n−1)(1 + spacing))`), so four rings
still leave a hole. Each ring gets a **full backdrop circle** as its track
(MKRingProgressView's "backdrop ring") rather than `arc`'s M3 gapped track,
which can't survive a second lap. Inner rings are slightly lighter ink
(`white = 0.15 + 0.07·i`) so they stay distinguishable in grey.

**Past 100%:** a full lap, then a **paper-colored separation halo** at the
head (a zero-length polyline, 1.35× the stroke, alpha 0.85), then the
remaining `fract(p)·360` arc on top; a whole number of laps draws a full
top ring. The halo stands in for MKRingProgressView's end-cap shadow, but
as a cut-out: this engine's ink is mirrored on dark themes, so a dark
shadow would render as a light glow there, while paper mirrors to paper
and reads as a separation in both themes (checked in the Studio in light
and dark).

**Stateless:** `t` is unused. A filling animation is the caller driving
`progress{i}`; `ReactiveBinding` (`packages/core`) can map and ease each
one. Tests (`nested.rs`): 0% dots at 12 on concentric tracks inside the
silhouette; each ring ends at its own value's angle; 1.25 draws lap, halo
and a top arc ending at 3 o'clock; exactly 2 laps = full top ring; values
clamp at `maxLaps`; the stroke shrinks for four thick rings and the default
is untouched; `trackOpacity 0` drops tracks; `ringCount` clamps to 1..4;
grey by default and `hueStep` spreads hues.

## Studio

The Web Studio's `families/ring/RingStudio.tsx` + `knobs.ts` — a minimal,
functional panel (the `studio-ui-ux` session applies the design system on
top): state tabs (`Completing`/`Loading`), size, a `progress` slider for
`completing` (starts at a third so the arc is visible before any slider is
touched; the engine's own default is 0), `strokeWidth`/`trackOpacity`/`gap`
knobs, a Color group, play/pause, FPS and a resolved readout. Registered in
`App.tsx`'s `FAMILIES` as `ring`. `Tracking` adds `ringCount`,
per-ring value sliders (0–300%, only as many as `ringCount`, in the
Reactive tier), `spacing` and `hueStep`, and hides `gap`; its default
stroke is 0.07 (switching state moves an untouched stroke to the new
state's default). `Stepping` shows `progress` and a `Segments` slider, and
relabels `gap` as "Segment gap". Its default gap is 0.33, not 1/3, so
exported snippets stay readable. The per-segment `segment{i}` keys are
API-only; there is no slider per segment. `Measuring` shows `progress`
(0.7), `Sweep`, and on/off `Value bar`/`Marker` sliders (the kit has no
toggle), and hides `gap`. The state grid is 3 columns and wraps five tiles
onto two rows.

## Not done, on purpose

- ~~No stacked multi-ring~~ — shipped as the generic `tracking`/`nested`
  (2026-09-18, widening), under the HIG constraint above.
- ~~No segmented/stepped ring~~: shipped as its own mode,
  `stepping`/`segmented` (2026-09-18, widening), rather than as a
  `segments` opt on `arc`. The two differ in empty-state and gap semantics.
- Gauge **needle** (HA's needle mode, AG's needle) and **severity levels /
  thresholds** (HA's colored level segments, AG's discrete fills). A
  needle would need a line from the center, which is a different visual
  language from this family's round-capped arcs. Levels are possible with
  per-range hues on the track if a consumer needs them. There are also no
  min/max labels, since the engine has no text primitive.
- Native Studio panels: like `signal`, none yet; the frames render on
  every platform through the existing `Polyline` paint contract.
