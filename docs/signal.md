# The `signal` family

## Why this exists, and why it's separate from `orbs`

`orbs` (see [`engine.md`](engine.md)) is a spherical "thinking orb" — a
floating, ambient presence indicator. Voice-AI product surfaces need a
second, deliberately different form factor: a linear, compact, **dockable**
audio-reactive strip (a chat input bar, a call-screen bottom bar) — not
another orb variation. This was a real correction made mid-session: `signal`
was first (wrongly) added as a mode file inside `orbs::modes`, purely for
convenience (reusing `Dot`/`finalize_frame`). That blurred the actual product
split `docs/gpt.md`'s component taxonomy calls for (Orb/Core/Ring/Signal/
Beacon as peer primitives, not sub-features of each other) — internally,
`signal`'s state would have shown up as an 18th entry in `orbs::presets::STATES`,
indistinguishable from a real orb state. `crates/core_engine/src/signal/` is
now a genuine sibling module to `orbs`, with its own state list
(`signal::presets::STATES`) and its own preset resolution
(`signal::presets::resolve_preset`) — `signal`'s states never appear in
`orbs::presets::STATES`, and vice versa. See
[`architecture.md`'s *family pattern*](architecture.md#the-family-pattern)
for the full story, including why `signal` still imports `Dot`/`Line`/
`finalize_frame` from the family-agnostic `crates/core_engine/src/primitives.rs`
rather than having its own copies — a linear bar layout is still just points
and lines, arranged differently, unlike a genuinely different rendering
technology (the deferred Glass/Liquid material work would need that).

`lib.rs`'s `resolve_any` tries `orbs::presets::resolve_preset` first, then
`signal::presets::resolve_preset` — one public `frame`/`frame_with_overrides`/
`resolved_opts` surface covers both families, so no platform binding needs a
second function name for a second family.

## States and modes

| State | Mode | File | What it looks like |
|---|---|---|---|
| `signaling` | `bar` | `modes/bar.rs` | A row of solid vertical bars with rounded caps — a discrete audio-EQ look, not a continuous waveform. |
| `waveform` | `waveform` | `modes/waveform.rs` | Layered continuous signed curves crossing a centerline — an oscilloscope/live-waveform look, not bars. |
| `metering` | `matrix` | `modes/matrix.rs` | A dot-grid LED EQ, in the vintage spectrum-analyzer style: columns of round LEDs, each lighting `floor(level · ledCount)` from the bottom (or out from the middle row), over a dim grid of unlit LEDs. It takes the same bands and sequencer as `bar`. |
| `scrolling` | `scroll` | `modes/scroll.rs` | A scrolling history of amplitude samples — thin mirrored pills, newest at the right, sliding left and fading out at the left edge; the WhatsApp/iMessage voice-message and SoundCloud look. Shows the last few seconds, where the other two show *now*. |

More styles land here as they're designed and built; this table is kept in sync with `signal::presets::STATES`, not
with in-flight work.

Unlike `orbs::presets::resolve_preset`, `signal::presets::resolve_preset`
has no base-profile/scaling machinery (`orbs::profiles`'s count-pair/
count-key/radius-key scaling rules) — every `signal` mode function computes
its layout proportionally from `size` directly (margins, bar width, bar
height as fractions of `size`), so "the resolved opts" is just an empty
map: every opt's own `get(.., default)` fallback in the mode function IS
the default. Revisit only if/when a style needs per-size tuning beyond
what's already proportional.

## The `Polyline` primitive (why `signal` needed one)

Both `signal` styles render entirely as `Polyline`s
(`crates/core_engine/src/primitives.rs`): one continuous stroked path
through an ordered `points` list, with `white`/`a`/`w` plus the same
`saturation`/`hue` color channel `Dot` has. Its paint contract, ported
identically to every renderer (`drawFrame.ts`, `OrbCanvasView.swift`,
`OrbCanvas.kt`, and each platform's SVG exporter): polylines draw **first**
(then lines, then dots), each as **one** stroked path with **round caps and
round joins**, no fill. `OrbFrame` gained a `polylines` field for it; every
`orbs` mode leaves that empty, so the golden suite is untouched — the same
additive pattern as `Dot.saturation`/`hue`.

**Why a new primitive instead of more `Line`s, for the record:** the first
`waveform` drew its curve as ~48 consecutive but *independent* `Line`
segments, and every renderer strokes each `Line` as its own butt-capped
path. At every bend that left a wedge-shaped gap on the outside and a
double-painted (darker, at alpha < 1) notch on the inside — a visibly
segmented "caterpillar" at any point count, with chopped-flat ends. This was
confirmed by rendering real frames to SVG/PNG and zooming in, not guessed.
The fix is one stroke with round joins, which is a *renderer-level*
property no arrangement of `Line`s can express — hence a primitive, not a
denser polyline. Two consequences worth knowing: one alpha per path (a fade
*along* a curve is done by emitting several polylines, which is what
`waveform`'s layers do), and a zero-length two-point path renders as a
round dot of diameter `w` under round caps on every target renderer (`bar`
relies on this for its shortest pills).

## `bar`: design notes

Each bar is one two-point vertical `Polyline`: a stroke whose round caps
make it a pill, **centered on the strip's midline** and growing both ways,
the way LiveKit's and Siri's bar visualizers do — not rising from a
baseline like a bar chart. The stroked segment is shortened by one bar
width so the *visible* pill (shaft plus both caps) is exactly the computed
height; `minHeight` means visible height, not shaft length. The bar
width floor is proportional to `size` (an absolute 1.5px floor once made
size-20 bars nearly touch).

**Two earlier versions looked bad, and here's why, for the record:** bars
were first a vertical stack of separate small `Dot`s (mirroring
`orbs::modes::spectrum`'s technique for its dense circular EQ ring) — that
works there because the ring is dense, but with `signal`'s few, wide linear
bars the gaps between stacked dots read as scattered dots, not a bar. The
second version was one thick butt-capped `Line` shaft plus one `Dot` cap on
top: solid, but round at the top only, flat at the bottom, and anchored to a
baseline, so it read as a bar chart. `Polyline`'s round caps made both the
cap trick and the baseline unnecessary. Lesson for any future `Dot`-stack
idea: it only reads as "solid" when the primitives are dense enough
relative to their spacing; at low counts, prefer a stroke.

**Two independent signals drive the same bars** — confirmed in both
LiveKit's and ElevenLabs' shipped `BarVisualizer` components (their actual
source was read, not guessed — see the voice-reactive-pipeline research
this was built from):
- **Height**, while `speaking`, comes from real per-band audio volume
  (`audioBand0..N` opts, via the shared `primitives::audio_band` — same
  indexed-key encoding `orbs::modes::spectrum` uses). Bands are laid out
  **center-out and mirrored** (lowest band on the middle bar, highest at
  both ends), not left-to-right the way LiveKit's own component does it: a
  real voice spectrum's energy falls off with frequency, so left-to-right
  always produced a lopsided staircase towering on the left.
- A state-driven **highlight sequencer** (`voiceStateCode`, `0..4` —
  mirrors the Web Studio's `audio/types.ts` `AgentState`, encoded as a
  number since a flat `HashMap<String, f64>` opts map can't carry a
  string) cycles which bar(s) are lit, at a cadence tied to the voice
  session's lifecycle phase, for every *non*-`speaking` state — this is
  what keeps bars visibly alive when there's no audio yet (`thinking`,
  `connecting`) instead of freezing. Cadences are ported straight from
  LiveKit's `sequencerIntervals`: `listening`'s center bar blinks every
  500ms, `thinking` chases back and forth every 150ms/step,
  `connecting`/`idle` sweep inward from both ends over a
  2000ms-per-step-per-bar (idle: 4000ms, no real upstream distinction
  either) cadence.
- **A real bug found and fixed during this build**: the sequencer
  originally only changed a bar's *color* when lit, not its *height* — so
  `listening`'s "blinking center bar" was invisible (a color-only pulse on
  an unchanging bar looks like nothing happened). Fixed by having `lit`
  also drive height for non-`speaking` states (a lit bar rises to a fixed
  0.65 fraction; unlit stays at the floor). Caught by a test
  (`listening_blinks_only_the_center_bar_on_a_fixed_cadence`) that initially
  failed for exactly this reason — see `bar.rs`'s test module.
- Never fully flat: `heightFrac = minHeight + (1 - minHeight) * raw`
  — the same floor trick `orbs::modes::spectrum` uses, so silence/idle
  still reads as "present," not "broken."

No golden vector — same tradeoff as every additive `orbs` mode; correctness
here rests on `bar.rs`'s own unit tests (every state renders every bar as
one centered two-point pill; bar width scales with `size`; `listening`'s
blink cadence is deterministic given `t`; real audio bands actually change
bar height vs. the floor, mirrored about the center; lit bars carry the
requested color and unlit ones stay grey).

## `waveform`: design notes

Built after `bar` and for a specific reason: bars — even solid pill-shaped
ones — are a bar-chart/EQ silhouette, not what most people picture when they
hear "live waveform" (a continuous trace, like the reference icons and
Siri's wave). Each trace is one `Polyline` of `pointCount` samples (default
96), no `Dot`s or `Line`s at all. See *The `Polyline` primitive* above for
why the first version's `Line`-segment approach had to go.

**Everything is a sum of smooth sines, on purpose.** The curve is analytic,
so smoothness needs no spline or Bezier — just enough samples, one `sin`
each (96 is ~2.7px per segment on the 4× Studio stage; at the 24× PNG
export the segments are longer but the curvature is gentle enough that
facets don't show). The only thing that can put a visible corner in the
trace is a kink in the math itself — see the `speaking` note below for the
one that shipped briefly.

What makes it read as a designed waveform rather than a sine plot, each a
deliberate call:

- **Layers** (`layerCount`, default 3): traces are emitted back-to-front —
  faint, phase-shifted, lower-amplitude companions first, the main trace
  last and strongest (companions at 0.72× amplitude, 0.38× alpha, 0.8×
  width per step). Overlapping translucent traces are the Siri-style
  richness; one trace alone reads as a generic plot.
- **Edge envelope** is Siri's attenuation `(K / (K + u^4))^K` (K = 4,
  `u = 1.7 · x_norm`), not the semicircle `sqrt(1 − x²)` the first version
  used: the semicircle has a vertical tangent at the ends, so the trace
  dropped abruptly at the edges. This bell is ~1 across the middle third
  and ~0 at the margins, in **every** state — including `speaking`, which
  previously had no envelope and started at full amplitude on the left edge.
- **Stroke width is proportional to `size`** (`lineWidth`, default
  0.035); the first version's absolute 1.6px was a hairline at 64 and 8% of
  the canvas at 20. `amplitude` (default 0.26) scales the swing the
  same way.
- **Color**: `hue`/`saturation` opts, same semantics as `bar`, with a slight
  per-layer hue drift (only visible when `saturation > 0`).

Same two-signal split as `bar`, applied to a curve instead of discrete bar
heights:

- **`speaking`**: real per-band audio sets each layer's **amplitude**, not
  the trace's shape. The bands are split into `layerCount` contiguous groups,
  low to high; layer *k*'s drive is 60% the overall level (all layers
  breathe with the voice as a whole) plus 40% its own group's mean (bass
  and sibilance visibly pull different layers). The shape under that
  amplitude is a two-term traveling sine per layer (~1.5 and ~1 crests
  across the visible bell, moving in opposite directions, so the trace
  rolls and folds instead of scrolling). A small share of the synthetic
  shimmer below runs underneath, so a pause between words reads as
  "present," not dead-flat — `bar`'s floor trick, applied to motion instead
  of height.

  **Why not "spectrum as shape," for the record:** two earlier `speaking`
  versions used band magnitude as the trace's *spatial* envelope — first
  left-to-right (always lopsided with a real voice spectrum), then mirrored
  about the center via `|x_norm|`. The mirrored one looked terrible in the
  live Studio and was caught there: `|x|` has a cusp at zero, so every frame
  had a sharp V at the center, and a spectrum-shaped envelope reads as one
  spiky bump rather than a wave. No renderer smoothing (Bezier, splines)
  can hide a corner that's really in the math; the curve had to be smooth
  by construction. `waveform.rs` now has a regression test for exactly this
  (`speaking_trace_has_no_corner_at_the_center`, a turn-angle check around
  the center sample).
- **Every other state**: a synthetic idle motion — two offset sine terms
  plus `perlin3` jitter under the envelope above. Amplitude/speed/frequency
  scale up through `Idle < Connecting < Listening < Thinking`, so the trace
  visibly gets "busier" the closer the session is to actually producing
  audio — the same "summed sines, center-weighted" recipe that shows up
  independently in both LiveKit's and ElevenLabs' shipped idle/processing
  states (see [`effects-research.md`](effects-research.md)), just applied
  to a continuous curve instead of discrete bar heights.

No golden vector, same tradeoff as every other additive mode; correctness
rests on `waveform.rs`'s own unit tests (every state produces one
left-to-right trace per layer with the requested point count; the main
trace draws last and strongest; width scales with `size`; idle visibly
moves off the flat centerline; edges are nearly flat relative to the
center; `thinking` is measurably more energetic than `idle` across several
`t` samples; louder real audio bands move the trace more than silence while
silence keeps a shimmer floor; low bands drive the main trace harder than
high bands and vice versa for a companion; no corner at the center sample
while `speaking`).

## `scroll`: design notes

The third style, and the one the original three-style plan was actually
missing (`waveform` turned out to be the *third* planned style, a static
Siri-curve; this scrolling history is the second). Prior art, all read
from shipped source (fetched, not recalled,
2026-09-17 20:50):

- **wavesurfer.js `plugins/record.ts`** — the live "scrolling waveform":
  one peak (`max |sample|`) per frame appended to a fixed window
  (`scrollingWaveformWindow (5s) × FPS (100) = 500` samples) that is
  shifted left by one each frame, rendered `normalize = true` "to prevent
  visual dancing". That fixed, shifting window *is* the ring-buffer
  technique.
- **wavesurfer.js `renderer.ts`** — bars are mirrored about `halfHeight`
  when `barAlign` is unset (the default look), drawn with `roundRect`,
  with a `barMinHeight`.
- **Telegram Android `SeekBarWaveform.java`** — 5 bits per sample, one
  bar per 3dp with a 2dp stroke (~2/3 fill), rounded rects, and new bars
  *grow in* via `appearProgress`. Telegram anchors bars to a baseline; we
  mirror instead (wavesurfer's default, and consistent with our own `bar`).
- **react-native-waveform-recorder** — `scroll` mode: metering at 30/s but
  visual samples at `samplesPerSecond = 12`, `barWidth 3 / barGap 2 /
  barRadius = barWidth/2`, `newSampleEntry: 'grow'`.

**The ring buffer lives in the caller, not the engine.** The engine is a
pure function of `t` with no memory (the same constraint `sonar` had to
adapt to), so the history arrives through opts with the indexed-key
encoding `audioBandN` already uses: `historyCount` = N,
`history0..history{N-1}` = amplitudes `0..1` oldest first, and
`historyPhase` (`0..1`) = how far the caller is between its last push and
the next. `historyPhase` slides the whole row left by that fraction of a
slot — so the motion is continuous at 60fps even though pushes arrive at
~12/s — and grows the newest bar in from a dot (Telegram's/RN's "appear"
entry, with alpha from 30% so it doesn't pop in at full ink). With no
`history0` in the opts (Studio without a voice source; tests) a
deterministic synthetic pattern scrolls instead (`hash_d` over an absolute
push index, three neighbors blended so consecutive samples correlate like a
real speech envelope), its floor/spread set by `voiceStateCode` on the same
`Idle < … < Speaking` busyness ramp the other styles use. `primitives::
indexed_opt` was added for the `historyN` lookup — `format!`-based and
unbounded, unlike `audio_band`'s allocation-free 16-entry `match`; a few
dozen tiny allocations per frame is nothing, a 16-bar cap would be a real
limit.

Geometry: each sample is one two-point vertical `Polyline` (round caps →
pill), mirrored about the centerline like `bar`, `barWidth` 0.6 of the
slot (Telegram's 2/3), `minHeight` 0.08 so silence reads as a thin
dotted line rather than an empty strip (a zero-length polyline renders as a
dot of width `w`). Left-edge fade: alpha ramps from ~3% at the margin line
to full over the leftmost `fadeWidth` (0.3) of the strip, so the oldest bar
dissolves as it slides out instead of being clipped. The floor stays above
`with_polylines`' cull threshold on purpose: the output is always exactly
`historyCount` polylines, oldest first, so `polylines[i]` is sample `i` at
every phase — an invariant the tests (and any consumer) rely on.

No golden vector; correctness rests on `scroll.rs`'s tests (one centered
pill per sample, oldest→newest left→right; supplied history drives height
and silence keeps a visible floor; `historyPhase` moves a bar exactly one
slot per full phase and is linear in between; the newest bar grows in;
alpha ramps up away from the left edge and is flat past the fade zone; the
synthetic fallback is deterministic in `t`, visibly varied, scrolls rather
than reshuffles, and `speaking` is busier than `idle`).

**State changes ease caller-side (done 2026-09-18, studio-ui-ux).** The
engine is a pure function of `t` and `voiceStateCode` is an integer, so the
Web Studio cross-fades the previous state's frame into the new one over
250 ms ease-out (LiveKit's audio-visualizer state transition), via
`OrbCanvas`'s `fadeFrom` + `drawCrossDissolve`; style changes still cut. An
app gets the same by rendering both codes for a moment. The native Studios
do the same (iOS/Android, 2026-09-18), through their Transitions
cross-fade paint.

## `matrix`: design notes

Transcribed from **audioMotion-analyzer**'s LED mode. I read its README and
the raw `src/audioMotion-analyzer.js` (see the families LOG). Details taken
from it:
- The mode is enabled with `ledBars`, and LEDs are laid out with
  `setLedParams(maxLeds, spaceV, spaceH)`. The built-in tables range from
  `[128, 3, .45]` down to `[24, 16, .125]`.
- A bar lights `value * ledCount | 0` LEDs, so the count is floored.
- **Unlit LEDs are drawn in `LEDS_UNLIT_COLOR = '#7f7f7f22'`**, a mid grey
  at alpha ≈ 0.13.

**Why dots are fine here, when they weren't for `bar`.** An early `bar`
built from a stack of small `Dot`s read as scattered dots and was rejected.
Matrix is made of dots on purpose. What turns it into a *panel* is
audioMotion's dim unlit grid: every LED is always drawn, and only the lit
part is inked. The contact sheet confirmed it reads as an LED EQ, not
scatter.

**Shared with `bar`, not copied.** `bar.rs` now exposes `VoiceState`,
`voice_state`, `highlighted` (the LiveKit sequencer) and a new
`band_level`, which is bar's own center-out band mapping extracted
verbatim. Bar's tests are unchanged, so the extraction is an identity
change. Matrix calls the same helpers:
- While `Speaking`, a column's level is `minLevel + (1 − minLevel) ·
  band_level`.
- Otherwise the sequencer lifts active columns to 0.65, and the rest stay
  at `minLevel`.

Audio only drives amplitude (column height), never layout.

**Keys:**

| Key | Default | Meaning |
|---|---|---|
| `columnCount` | 12 | 1..32 |
| `ledCount` | 8 | 2..24; audioMotion's coarsest table has 24 |
| `minLevel` | 0.125 | one of 8 LEDs, the same role as `minHeight` |
| `ledSize` | 0.7 | LED diameter as a fraction of its cell |
| `mirror` | 0 | 1 grows out from the middle row, the LiveKit-centered look of `bar` |
| `peak0..N` | — | caller-owned peak per column, drawn as one lit LED |
| `hue` / `saturation` | 200 / 0 | |
| `voiceStateCode` / `audioBand*` | — | as for `bar` |

The grid uses the same box as `bar`: 10% side margins and a strip 60% of
the size tall.

**Ink:**
- Lit LEDs in an active column: 0.08 / 0.95, colored.
- Lit LEDs in an inactive column: 0.72 / 0.32 grey, bar's inactive look.
- Unlit LEDs: 0.5 grey at alpha 0x22/255.

Every mark is a `Dot`.

**Not done:**
- audioMotion's peak hold / gravity / fade. These are stateful, so here
  they're the caller's job (`peak{i}`), the same contract as `scroll`'s
  history.
- The `bar-level` color mode (green → yellow → red by height).

**Tests (`matrix.rs`):**
- The full grid stays inside the strip.
- Lit count is floored: 0.99 × 8 = 7.
- Louder bands give taller columns, using the same mapping as
  `band_level`.
- Non-speaking columns follow `bar`'s sequencer.
- `mirror` is symmetric about the middle row.
- A peak adds one LED above the level.
- Unlit LEDs are always grey at 0x22 alpha.

## `muted`: the mute / error cue (an opts flag, not a state)

Every real voice product has to show "mic muted" / "connection lost", and
`signal` — the literal audio-in/out indicator — is where it belongs. Prior
art (fetched):
Amazon's Voice Interoperability design guide says "It is very important
for a product to convey a device's Microphone On/Off state" and that
agents must not reuse a color with a conflicting meaning ("the same color
should not be used as Listening for one agent and Mic Off for another");
an Echo's light ring goes solid red with the mic off (Amazon's own help
page returned 503 on two fetches; SlashGear/Digital Trends confirm, and
note a *flashing* red means a connectivity problem); Google Meet turns the
mic icon red with a slash. LiveKit's `BarVisualizer` — the component `bar`
is modeled on — has **no** muted/disconnected visual at all (its bars just
collapse to `minHeight` on silence), so this fills a gap in the reference,
not just in this repo.

It's `primitives::apply_muted`, a mode-agnostic post-process run last in
`lib.rs`'s `render()` (after pointer scatter and the audio pulse), so all
three `signal` styles get it with zero mode code — and so does every
`orbs` state, for free. Opts: `muted` (`0..1`, `1` = fully muted; `0|1`
works as-is, a continuous value just lets a caller ease it), `mutedTint`
(`0..1`, default `0` = grey; above `0` tints toward `mutedHue`, Alexa-style
red being a product choice rather than the default), `mutedHue` (degrees,
default `8`). Effect on every `Dot`/`Line`/`Polyline`: alpha `× (1 −
0.55·m)`, ink lightened (`white → lerp(white, 0.62, m)`), saturation
`→ lerp(sat, 0.85·tint, m)` with hue snapped to `mutedHue` when tinting,
radius/stroke `× (1 − 0.15·m)`. Geometry untouched, which is what keeps it
mode-agnostic. No preset sets it → `frame()` and the golden suite are
unaffected.

**Freezing/slowing the animation is the caller's job, on purpose.** The
engine is a pure function of `t`; scaling `t` inside it would snap the
pattern to a different pose the instant `muted` toggles (at `t = 30s`, a
0.1× scale jumps to the `t = 3s` pose). So — exactly like `t * speed`
(see [`engine.md`](engine.md#the-t--speed-callout)) — the caller advances
its own clock at ~10% while muted. `SignalStudio` is the reference
implementation: its clock is an accumulator (`elapsedRef += dt ×
(muted ? 0.1 : 1)`), not wall time, so the rate can change with no jump,
and pause/resume falls out of the same accumulator.

## Studio

The Web Studio's `families/signal/SignalStudio.tsx` is `signal`'s own Studio
panel — a separate file and a separate top-level tab from `OrbStudio.tsx`
(App.tsx's `FAMILIES` array), not a mode bolted onto the Orb panel. A
`Style` tab (`Bar` / `Waveform`, mapping directly onto `"signaling"`/
`"waveform"`) picks between the two states from one panel rather than
duplicating the whole Studio shell per style; each style's own knob group
(`BarKnobs`: bar count/min height/bar width; `WaveKnobs`: points/line
width/layers/amplitude; `ScrollKnobs`: samples/bar width/min height/edge
fade; `MatrixKnobs` for the `Matrix` tab / `"metering"`: columns/LEDs/min
level/LED size/grow-from) only renders while that style is selected, and a shared Color group
(hue/saturation) applies to whichever is active, since all three render as
`Polyline`s with the same color channel. For `scrolling`, the panel owns
the ring buffer: while a voice source is connected it tracks the peak
`level` between pushes and pushes at `HISTORY_HZ = 12` (the RN recorder's
`samplesPerSecond`, and the engine's synthetic rate, so both move alike),
carrying the remainder so a slow frame doesn't drift the cadence, and
passes `history0..N` + `historyPhase` each frame; the buffer is pre-filled
with silence on connect so the strip is full-width from the first frame,
and dropped on disconnect so the engine's synthetic pattern takes over. A
"🔇 Mute" toggle in the stage row sets `muted: 1` (plus `mutedTint` from
the Color group's "Muted tint" slider and `mutedHue`) and slows the
panel's clock to 10% — see *`muted`* above for why the slowdown lives here
and not in the engine. An "⚡ Interrupt" button fakes a barge-in by
stamping the wall clock; `interruptAge` is derived from it per frame for
`primitives::apply_interrupt` (see [`engine.md`](engine.md#interrupt--barge-in-flash)),
the same way a real adapter would on OpenAI's `speech_started` / Gemini's
`interrupted`. It has its own
manual "Preview state" picker (0-4, matching `voiceStateCode`) so every
lifecycle phase can be previewed without a live voice connection, shared by
both styles. The 🎙️ Live Mic / ▶️ Test Tone buttons reuse the exact same
`VoiceSource` pipeline `OrbStudio` uses (see
[`audio-pipeline.md`](audio-pipeline.md)) — when connected, the *real*
voice lifecycle state drives `voiceStateCode` directly, overriding the
manual picker. The per-band values are eased at render rate before they
reach the engine (`smoothedBandsRef`, ~40ms) — the same second smoothing
pass `OrbStudio` applies to `audioLevel`, for the same reason: the analysis
layer updates at ~30fps, and feeding that straight into a 60fps render loop
held each value for two frames then jumped, which read as stutter on bar
heights and on the trace's amplitude. This is a deliberate difference from `OrbStudio`'s own
decision not to let a connected voice
source auto-switch which *orb state* is shown: for Orb, auto-switching
would mean jumping between different shipped products; for `signal`,
driving its one built-in state machine from real voice state **is the
point** of the family.
