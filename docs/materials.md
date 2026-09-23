# Materials: glow, noise, gradient, liquid, particles, holographic

> **Materials vs effects** (the naming split, 2026-09-19):
> - A **material** is a frame → frame transform the *engine* computes. It produces or modifies primitives. The materials are colour, gradient, glow (both stacked copies and blur halos), noise, pulse, **liquid**, **particles** and **holographic**. They are configured in the FX Spec `materials` section and cost engine time.
> - An **effect** is a *paint-contract instruction* the renderer executes: `blur` and `blend`, on `Fill`s and `EffectRun`s (see [`engine.md`](engine.md#paint-contract-fills-and-effects-materials-phase-1-2026-09-18)). Effects are not a spec section; they are reached through a material's options (`glow.mode: "blur"`, `liquid.blur`). They cost paint time, which is what `blurLoad` estimates.
>
> Studio labels and docs follow this split.

Three **family-agnostic post-processes** in
`crates/core_engine/src/primitives.rs` — `apply_glow`, `apply_noise`,
`apply_gradient` — that any object (Orb, Signal, Ring, Beacon, Core) can
wear, in any state, by setting opts. They are not modes, not states and
not a family. The product's parameter plan
(sections 10–13) frames Glow/Noise/Gradient as *materials/behaviors*
layered onto objects rather than as objects of their own, precisely so the
product doesn't read as "an effect library" — and the engine already had
the mechanism for that: the opts-activated post-process chain
`render()` runs on every family's finished frame (`apply_pointer`,
`apply_audio_reactive`, `apply_interrupt`, `apply_muted`, see
[`engine.md`](engine.md#pointertouch-scatter)). Materials are three more
links in that chain. No mode file knows they exist; no preset sets them,
so plain `frame()` and every golden vector are unaffected.

## The chain

```text
mode geometry
  → apply_pointer        (pointer scatter)
  → apply_audio_reactive (volume breathing)
  → apply_pulse          (periodic pulse / breathe — engine.md)
  → apply_particles      (material: stateless particle layer — wall-clock time)
  → apply_noise          (material: organic jitter — moves geometry)
  → apply_liquid         (material: metaball contours)
  → apply_color          (material: one colour for the frame + the ink|fixed paint mode)
  → apply_gradient       (material: color ramp by final position, 2 or 3 stops)
  → apply_holo           (material: holographic hue sweep — wall-clock time)
  → apply_glow           (material: halos, inheriting the ramped color)
  → apply_interrupt      (barge-in flash)
  → apply_decay          (one-shot fade-out — engine.md)
  → apply_muted          (mute dim — always has the last word)
```

Pulse swells geometry, so it runs before the materials copy or color it
(a glow halo breathes with its source). Noise runs before anything that
copies geometry (glow) so halos track the jittered dot. Gradient runs before glow so a halo inherits its source's
ramped color. Interrupt and mute stay last so the flash's tint timeline
and the mute's dimming override the materials too.

## Material time (2026-09-19)

Callers pass the mode's clock, `t = elapsed × presetSpeed × specSpeed`. `render` computes `wall = t / presetSpeed`, which is `elapsed × specSpeed`, and hands it to every post-process that has its own sense of time. So these are **real seconds on every state**:
- `pulsePeriod`
- `noiseSpeed`
- `particleLife`
- `holoSpeed`

A 2 s pulse is 2 s on a 1× ring and on a 3.24× `breathing`. A spec's own `speed` (the user's "whole object slower/faster" knob) still scales all of them together with the geometry.

Deliberately **not** on either clock:
- **Interrupt and decay** don't read `t`. `interruptAge` and `decayAge` are seconds on the *caller's* clock since a one-shot event, and a caller measures them against its own timestamp.
- **Liquid** has no time input; it follows the geometry it melts.
- **Colour, gradient and glow** are static.

Measured in motion (default knobs, `pulsePeriod 2`):

| state | preset speed | pulse period (real) | noise field drift |
|---|---|---|---|
| tracking | 1× | 2.00 s | — (no dots) |
| speaking | 1× | — | 1.20 px/s |
| working | 1.885× | — | 1.09 px/s |
| breathing | 3.24× | 2.00 s | 1.18 px/s |
| drifting | 3.315× | — | 1.20 px/s |

On the mode clock, `breathing`'s pulse was 0.62 s and its noise 3.24× faster.

## Opts

| Material | Key | Default | Meaning |
|---|---|---|---|
| glow | `glowStrength` | — (off) | `0..1` master; absent or `0` = no-op |
| | `glowRadius` | `2.5` | halo outer edge as a multiple of dot radius / stroke width |
| | `glowLayers` | `4` | concentric copies, `1..8` |
| | `glowTint` | `0` | `0..1`: `0` keeps the source's color, `1` re-tints to `glowHue` |
| | `glowHue` | `200` | degrees |
| noise | `noiseStrength` | — (off) | `0..1` master |
| | `noiseAmplitude` | `0.04` | max displacement as a fraction of `size` |
| | `noiseScale` | `2` | lattice cells across the frame (low = one slow swell, high = fine shimmer) |
| | `noiseSpeed` | `0.4` | field drift per **real** second (the preset speed is divided out) |
| | `noiseSeed` | `0` | a different field per object sharing a screen |
| gradient | `gradientStrength` | — (off) | `0..1` master (also the blend amount) |
| | `gradientAngle` | `90` | degrees, CSS convention: `0` up, clockwise, `90` = left→right |
| | `gradientHue` | `200` | start hue |
| | `gradientHue2` | `320` | end hue; `h1 + 360` = full rainbow, `h2 < h1` = the other way round |
| | `gradientSaturation` | `0.8` | saturation the ramp blends toward |

All three read like the existing post-processes: `frameWithOverrides(state,
size, t, { glowStrength: 1, gradientStrength: 1 })` from `@sinua/core`,
or the same keys in the `overrides` map on iOS/Android/React Native. No
new records — `Dot`/`Line`/`Polyline`/`OrbFrame` are unchanged, so the
native bindings didn't need regenerating; only the wasm package did.

## `glow`: layered halos (design notes)

There is no blur, no gradient fill and no bloom pass on any of this
project's renderers, so the glow is the one the CSS world fakes every day:
**stacked translucent copies of increasing size and decreasing alpha**.
CSS-Tricks' neon text stacks eight `text-shadow`s (7px → 151px) "so that
they can be stacked over one another to add more depth to the glow" — a
single shadow "would not have the depth required"; Tobias Ahlin's layered
`box-shadow`s double the blur per layer and note that as layers are added
"you'll have to decrease the alpha value for each layer"; Josh Comeau's
shadow guide is the same recipe. Android's classic "glowing dot"
(kodintent) draws one radial gradient with stops `[0, 0.5, 1]` =
[color, color, transparent] at "nearly twice" the plain dot's radius —
where `glowRadius`'s `2.5` default comes from.

The falloff is Gaussian because that is what a canvas shadow *is*: the
canvas spec derives the Gaussian's `σ` from `shadowBlur / 2`, and Skia's
`ConvertRadiusToSigma` is `0.577 · radius + 0.5` (its comment: a box
convolved with itself three times). Layer `k` of `L` is scaled to
`s_k = 1 + (glowRadius − 1)(k + 1)/L` and given alpha
`a · strength · exp(−½ u_k²) / L`, with `u_k` the layer's extra radius in
sigmas and `σ = (glowRadius − 1) · r / 2` — so the outermost layer sits two
sigmas out. With four layers the stack peaks at about `0.49 · strength ·
a` beside the source, so a full-strength glow never out-inks the thing it
surrounds (a unit test pins this).

**Draw order matters and is deliberate:** halos are spliced in
*immediately before their own element* in the frame's existing z-sorted
order, so a front dot's halo paints over a back dot the way a real halo
would — not "all halos under all dots". Copies whose alpha would fall
under the renderers' cull threshold (`0.02`) are skipped, so a faint
frame doesn't triple its draw count for nothing. On light themes the halo
is dark ink at low alpha (a soft bleed); a dark theme mirrors it into a
light glow, same as everything else.

Known limits (both visible in the verification sheet): a very large,
very translucent dot — `beacon`'s accuracy halo — shows its four layers as
concentric rings rather than a smooth glow, the same stepped-alpha limit
`shimmer` works around with nine layers; and copies of a polyline all
share its geometry, so a glow on a wide bar reads as a soft outline, not
a bloom. The stepped look is a *paint-time* limitation, not an engine one:
a real blur is a renderer concern (Canvas 2D `ctx.filter = "blur(Npx)"`,
CSS `filter: blur()`, SVG `<feGaussianBlur>`, Core Graphics / Compose
blur effects), so a consumer can smooth these halos without the engine
knowing — flagged to `studio-ui-ux` as an optional paint-layer option;
not a new Rust primitive. Shading *within* one stroke still needs a
per-vertex-color primitive (not scheduled).

### Real-blur mode (materials phase 1, 2026-09-18)

`glowMode` = 1 (FX Spec `materials.glow.mode: "blur"`) replaces the stacked copies with **one halo per element**:
- each halo is enlarged to the middle of the stacked profile (`1 + (glowRadius − 1)/2`), at alpha `0.6 × strength × a`;
- the halos come first, as one block per list, and each block gets an `EffectRun`;
- the run's σ is the list's median size (dot radius or stroke width) × `(glowRadius − 1)/2`, so it scales with the object;
- `glowBlend` = 1 (`blend: "additive"`) composites the halos additively, which is a dark-theme look (see the paint contract in [`engine.md`](engine.md#paint-contract-fills-and-effects-materials-phase-1-2026-09-18));
- fills are not glowed.

Compared with stacked mode:
- About half the elements (`working`: 1032 vs 2028), but with blur work instead. `estimateCost` reports it as `blurLoad`.
- Halos sit under every original. In stacked mode each element's copies are interleaved with it.

`blurScale` (default 1; FX Spec low power with `disable: ["blur"]` sets 0) scales every σ. At 0 blur mode falls back to the stacked copies, which every renderer can draw. Stacked mode (`glowMode` 0, the default) is unchanged byte for byte.

A bug was fixed along the way: `apply_glow` used to rebuild the frame with `colorMode: ink`, so a `fixed` colour plus glow lost its fixed look on dark themes. It now keeps the frame's mode.

## `noise`: organic jitter (design notes)

Every dot, line endpoint and polyline vertex is displaced by a smooth,
time-varying field from the engine's real gradient noise (`perlin3`, the
Perlin 2002 structure added for `aurora`) — so a rigid lattice (`globe`, a
`bar` row, an `arc`) breathes like something alive with no mode knowing.
The recipe is Nature of Code's noise walker: sample the field from
**different regions of noise space per axis** (the book starts `x` at `0`
and `y` at `10,000` "so that x and y appear to act independently of each
other") and advance slowly through the third dimension for time — the
`x`-shift samples `(x·f, y·f, z)`, the `y`-shift `(x·f + 31.7, y·f + 17.3,
z + 10000)`, with `f = noiseScale / size` and `z = wall · noiseSpeed +
noiseSeed` (`wall` = the caller's `t` ÷ the preset speed, see *Material
time* below).

Displacement is computed from each point's *pre-jitter* position, so a
dot and a line endpoint or polyline vertex at the same coordinate move
together — `web`/`crystallize` edges stay attached to their dots (unit
test). Only `x`/`y` move; `z`, radius and alpha are untouched, so the
frame's z-sort still holds. The default amplitude (`0.04` of `size`) is
deliberately subtle — a living tremor, not a scramble; raise `noiseAmplitude`
for a visible warp, raise `noiseScale` for fine shimmer instead of one
slow swell.

## `gradient`: a color ramp by position (design notes)

**Two or three stops (2026-09-18).**
- `gradientHue3` adds a third stop. `gradientMid` (0.05..0.95, default
  0.5) places stop 2 along the ramp: `h1→h2` over `[0, mid]`, then
  `h2→h3` over `[mid, 1]`.
- Without `gradientHue3` the ramp is the original two-stop one, bit for
  bit (tested). `gradientMid` alone changes nothing.
- The no-wrap rule applies to each segment. `300 → 40` passes through
  green, so pass 400 to go the short way round through red. A picker
  should do that conversion.
- **Lines** now take the ramp too, sampled at their midpoint.

"A real gradient" for a dot/stroke renderer is not a fill — it is
**per-element color interpolation across the frame**: a grey `globe`
becomes blue-to-magenta left to right, a `ring` arc shades along its
sweep. The coordinate contract is CSS Images Level 3's `linear-gradient()`
verbatim, so a designer's mental model transfers: "0deg points upward, and
positive angles represent clockwise rotation, so 90deg point toward the
right"; the gradient line passes through the center of the box, and its
length is `abs(W · sin A) + abs(H · cos A)`, which lands `0` and `1`
exactly on the two corners the angle points away from and toward (unit
test checks 90°, 0° and 45°). The box is the `size × size` frame, not the
geometry's bounding box — a bbox changes every frame as dots move, which
would make colors flicker.

The hue ramp is a plain `h1 + (h2 − h1) · u`, no wrap logic: `h2 = h1 +
360` gives a full rainbow, `h2 < h1` runs the other way around the wheel.
Per element, `saturation → lerp(sat, gradientSaturation, strength)`; hue
snaps to the ramp when the element was achromatic (its old hue was
meaningless) and eases toward it via `lerp_hue` otherwise. `white` is
kept — it is the lightness in every renderer's HSL branch, so a mode's
depth shading survives the recolor. **A `Polyline` is coloured per
vertex (2026-09-19):** each vertex samples the ramp at its own position
into `Polyline.hues`, so a `waveform` layer shades along its length and
`tracking`'s rings ramp around their sweep. `hue` stays the mean-vertex
sample, which is the fallback for renderers without per-vertex paint (paint
rule: [`engine.md`](engine.md#per-vertex-stroke-colour-2026-09-19)). A stroke
whose vertices all land on one hue carries no `hues`. Before this, a
polyline had one colour per path, so concentric rings all took the centre's
hue and the gradient only tinted them. `Line` has carried
`saturation`/`hue` since the Color system pass (2026-09-18) and samples
the ramp at its midpoint.

## `color`: one colour for the whole frame (design notes, 2026-09-18)

`primitives::apply_color` is the engine half of the Studio's colour picker.
It runs after noise and before gradient. Colour is the base tint, the
gradient is more specific and wins where it's set, glow halos inherit
whichever applied, and interrupt and mute keep the last word.

It recolours **dots, lines and polylines**. `Line` gained
`saturation`/`hue` for this; before, `connecting`'s 86 edges couldn't be
coloured at all.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `colorMix` | 0..1 | absent → no recolour | How far each element moves toward the target (CSS `color-mix()` model). Bindable (Reactive Inputs). |
| `colorHue` | 0..360 | 200 | Target hue. Grey elements snap to it; coloured ones ease along the **shorter** arc (`lerp_hue`, CSS's default `shorter hue`). |
| `colorSaturation` | 0..1 | 0.8 | `lerp(sat, S, mix)`. |
| `colorLightness` | −1..1 | 0 | An order-preserving **bias**, never an overwrite. `w + b(1−w)` for b ≥ 0 and `w(1+b)` for b < 0, scaled by mix. It is monotone, so a sphere's depth ramp keeps its order and spread. This was checked by test on a real `working` frame and by eye on `working`/`breathing`. In `ink` mode, + means "toward the background" and − means "toward full ink", in both themes. |
| `colorMode` | 0 / 1 | 0 = ink | 1 = **fixed**. Sets `OrbFrame.colorMode` whenever present, even without `colorMix`, so a natively coloured state such as `glowing` can be pinned too. See [`engine.md`](engine.md#color-system-ink-vs-fixed-and-line-colour). |

What `fixed` means in practice: every element keeps its light-theme
lightness on dark themes too. A mode's far, pale "ghost" dots therefore
stand out on a dark background, where `ink` would have darkened them.
That is the definition, not a bug: "the same everywhere". For a brand
colour, pair `fixed` with a positive `colorLightness` (+0.3 to +0.5).

## `liquid`: metaball contours (materials phase 2, 2026-09-19)

`liquidStrength` > 0 turns the frame's **dots** into a metaball field and re-draws its iso-contours with existing primitives. It is the gooey, merging mood of voice orbs, done in this engine's own ink language: no shader and no raster threshold. It is stateless: the field is rebuilt from the current frame's dots on every call, so the liquid moves exactly as the underlying state moves. It sits in the chain after `noise` and before `color` / `gradient` / `glow`, so those colour, ramp and halo the liquid.

**Keys:**
- `liquidStrength` (0..1; 0 = off; also scales the liquid ink's alpha)
- `liquidReach` (default 2.5)
- `liquidThreshold` (0.5)
- `liquidCells` (grid resolution across the frame, 40)
- `liquidStyle` (1 outline (default) / 2 dots / 0 fill)
- `liquidSpacing` (dots: × median dot diameter, 2.25)
- `liquidWidth` (outline: × median dot radius, 0.8)
- `liquidKeep` (0/1: keep the source dots on top)
- `liquidBlur` (fill: σ, a soft edge through the paint contract's blur)

**How it works:**
- **Field.** Every dot adds `a × (1 − (d/R)²)³` inside `R`: a **compact kernel**. Wikipedia's *Metaballs* notes that a finite-support function lets points beyond its radius "be ignored", so each dot only touches nearby cells. Blinn's `1/r` never reaches zero.
  - `R = liquidReach × max(dot radius, ½ × the frame's median nearest-neighbour distance)`. Reach has to follow the spacing between dots, not only their size: an orb's dots are small and sparse, and on the contact sheet `3 × r` alone never let neighbours merge.
  - The weight is the dot's **alpha**, so bright dots pool and faint ones barely count. Radar's lit trail pools, while its idle scope stays below the threshold.
- **Contours.** Marching squares on a grid padded so every contour closes (Jamie Wong, *Metaballs and Marching Squares*; Wikipedia, *Marching squares*):
  - 16 cases from four corner bits;
  - the crossing on each edge is placed by linear interpolation;
  - the saddle cases 5 and 10 are resolved by the cell-centre average;
  - segments are stitched into closed loops through shared edge points.
  - Loops are grouped by containment, not winding: a loop inside an odd number of others is a **hole** of the smallest outer that contains it.
- **Ink.** Each blob takes the alpha-weighted mean ink of the dots inside it, times `liquidStrength` on alpha.
- **Styles:**
  - **Outline** (the default): one closed, round-joined `Polyline` per loop. On an orb it reads most clearly as liquid and stays the most "ink".
  - **Dots**: every loop resampled at an even spacing. A loop too small for three spaced dots becomes one dot at its centre (no triangles).
  - **Fill**: one `Fill` per outer loop, **with its holes**, painted with the even-odd rule. This was the one contract change: `Fill.holes`, packed v3.
- **Low power:** `performance.lowPower.disable: ["liquid"]` (FX Spec 1.4) sets `liquidStrength` to 0, so the state renders as plain dots.
- **Cost:** the output counts through the normal `elements` / `coverage` / `blurLoad` fields. The contour work is engine-side and bounded (cells × dots in support); it isn't classed.

**Per-state tuned defaults** (follow-up, 2026-09-19): orchestrator renders through `@sinua/web` showed working/searching fragmenting, composing combing and metering collapsing. The engine now fills in these defaults whenever `liquidStrength` > 0 **and the caller hasn't set the key**. Studio sliders and FX Spec `materials.liquid` always win.

| state (mode) | defaults | before → after (contact sheet) |
|---|---|---|
| working (`orbits`) | reach 4, threshold 0.4 | tiny fragments → pooled islands (and a cloud as a blurred fill) |
| searching (`globe`) | reach 4 | scattered specks → a liquid lattice of cells |
| composing (`ribbon`) | reach 5, threshold 0.6 | comb → a band; a small fringe remains where the lanes are densest |
| metering (`matrix`) | keep 1 | outline only → outline + the LED grid. With audio bands the outline is an **EQ silhouette** (the earlier "two capsules" render had no bands) |

Reach above ~6 collapses sparse orbs into one blob, which is why these stay moderate.

**Suitability** (`liquidSuitability(state)` → `{ level: recommended | ok | notRecommended, reason, defaults }`, for the Studio badge; the table is in `liquid.rs::suitability`). It was judged on a 34-state sheet of outline and blurred fill, with the tuned defaults:
- **recommended:** searching, solving, listening, connecting, breathing, shaping, glowing, drifting, speaking, metering, typing (three dots merge into one liquid pill)
- **ok:** working, weaving, composing, confirming, progressing, scanning
- **notRecommended:**
  - sparse, dim or line-drawn: initializing, calibrating, concluding, muted;
  - drawn with strokes or fills, with few or no dots: signaling, waveform, scrolling, the ring states, the single-dot beacons, generating. Liquid melts dots, so these have nothing to pool.

**Not copied:** the CSS "gooey" trick (blur, then an alpha-contrast `feColorMatrix`, from CSS-Tricks). It thresholds *rasterized pixels*, needs a container filter and bleed room, and has Safari limitations. The vector path rules it out, and real contours give the same merging topology as geometry.

**Where it works best** (contact sheet, 2026-09-19):
- sparse orbs and `drifting` (outline and fill read as real liquid, and fill blur softens it);
- `metering` (LED columns become liquid capsules);
- `speaking` (the ring becomes a band with a hole).
- Dense or dim clouds (`working`) pool only where they're bright. That's intended: liquid follows the light.

## `particles`: a stateless particle layer (materials phase 3, 2026-09-19; reworked the same day)

`particleStrength` > 0 emits `particleCount` small ink dots from the state's own geometry. Each particle drifts, flows in, orbits or rises, then fades. **They are plain `Dot`s with the emitter's ink and z**, so no renderer, exporter or packed layout changes. Glow, blur, additive, colour, gradient, holographic, liquid, decay and mute all reach them through the chain.

**Stateless.** GPU particle systems work the same way:
- Lutz Latta, *Building a Million Particle System* (GDC 2004): each particle is "computed from its birth to its death by a closed form function … defined by a set of start values and the current time", with "no extra storage for intermediate particle state".
- For particle `i`, a hashed life length `Lᵢ` (`particleLife` × 0.7–1.3) and phase give life `k = floor((t + φ)/L)` and age `u = frac(...)` (GameDev.net, *Stateless particles: time alive*).
- The start values of each life are hashed from `(i, k)`, so the particle is re-born somewhere new each life, deterministically.
- `particleSeed` changes the whole field.

**Wall-clock time.** Callers pass the mode's clock, `t = elapsed × presetSpeed` (× a spec's `speed`). `render` divides the preset speed back out for particles, so **`particleLife` is real seconds on every state**.
- The first version ran on the mode clock. `particleLife 2` lived ~0.6 s on `breathing` (3.24×) and `drifting` (3.315×), and the user saw it live as "too fast".
- A spec's own `speed` still scales particles: it's the "whole object slower/faster" knob.

**Emitters, picked by position:**
- The emitters are the frame's **dots when there are any, otherwise its strokes and fills** (polyline vertices and fill points), never a mix.
- Each life hashes a fixed target and takes the **nearest emitter to it**:
  - drift/orbit/rise: a point on the rim circle, so particles are born at the silhouette;
  - attract: anywhere in the disc.
- The first version picked emitters by list index. Orbs z-sort their dots every frame, so a particle jumped to a different dot each frame.
- Mixing dots and paths let a ping's fading ring or a broadcast's cycling arcs pull the nearest emitter away mid-life, which is why the set is one or the other.
- Particles leave from the mark's **edge** (`e.r`), not its centre.

**Motion** (`particleStyle`):
- **drift** (0): out from the centroid, with angle jitter;
- **attract** (1): spawn out at `spread` and flow in to the emitter;
- **orbit** (2): circle the centroid just outside the shape, slowly (a quarter to three quarters of a half-turn per life). The radius comes from the life's rim target, capped inside the edge-fade band;
- **rise** (3): up, drifting sideways.

Travel is `particleSpread × size`, with a gentle ease-out (`u·(1.6 − 0.6u)`) and a small hashed wobble. Alpha is `strength × sin(πu) × max(e.a, 0.7)`.

**Two more keys (FX Spec 1.6):**
- **`particleSync`** 0..1 pulls every birth together: at 1, every particle is born at once, a burst each life.
- **`particleAudio`** 0..1 couples brightness to the host's `audioLevel`, and only when one is present. Alpha × `lerp(1, 0.25 + 0.75·level, audio)`. It's stateless, so it reads the *current* level; positions never depend on it, so a jumpy level can't make particles jump.

**Inside the canvas, never clipped:** a particle fades out (smoothstep) over the last 8 % of the frame before its edge touches the canvas edge.

**Size:** the radius follows the state's dot size, clamped to **0.8–1.8 % of the frame** at size 0.6, and the default size is 0.8. At 2.5 %, a big-dot state's particles read as dark blobs in motion.

### Defaults, and per-state defaults

**Base defaults:** count 28, size 0.8, spread 0.18, life 4.5 s, style drift, sync 0, audio 0.

Each state's own defaults apply **only to keys the caller leaves unset**; explicit values always win. It's the same mechanism as liquid's (`particles::mode_defaults`, `with_mode_defaults`).

| States (mode) | Particles mean… | Defaults |
|---|---|---|
| breathing, glowing (`ring`, `aurora`) | ambient: a slow, sparse orbit | orbit, 20, life 6, spread 0.12 |
| drifting (`webflow`) | shedding: born by the silhouette, drifting out and dissolving (2026-09-19, after the user watched it live) | drift + **anchor**, 22, life 6, spread 0.22 |
| speaking + signal (`spectrum`, `bar`, `waveform`, `scroll`, `matrix`) | voice: drift that follows `audioLevel` | audio 1, 32, life 3 |
| ring family (`arc`, `spinner`, `nested`, `segmented`, `gauge`) | progress/done: a few lifting off | rise, 12, life 5, spread 0.2 |
| scanning (`radar`) | gathering a signal in | attract, 24, spread 0.3 |
| notifying (`ping`) | a burst with each ping | sync 1, 16, life 2.4, spread 0.28 |
| broadcasting (`broadcast`) | radiating out along the waves | 18, spread 0.24 |
| reconnecting, locating (`pulse`, `halo`) | sparse | 14 |
| generating, typing (`shimmer`, `dots`) | minimal | 8, spread 0.1, life 5 |
| every other orb | the base defaults | — |

`particleDefaults(state)` (TS; UniFFI `particle_defaults`) returns all seven keys for a state, base plus its own (drifting also returns `particleAnchor`). The Studio shows them as the knob defaults.

**Anchored drift** (`particleAnchor` 1, internal, drift only; `drifting`'s default). A plain drift particle re-derives its position every frame from the emitter nearest its hashed rim target. On a fast state (drifting, 3.3×) that emitter keeps changing as the nodes rotate, so the particle **rides the nodes**. That's Unity's *Local* simulation space ("particles … moving with the parent object"); what the user asked for is *World* space ("once emitted, particles won't follow the moving GameObject"; InheritVelocity *Initial* vs *Current*).
- **Staying stateless:** each life is born at a fixed world point, the frame centre + the life's hashed direction × 0.85 × the rim measured from the centre.
- It moves straight out from there with the usual ease-out.
- The emitter lends only its ink, with the emitters' mean alpha, so a node swap can't blink.
- **Dissolve:** fade in over the first 12% of life, then `(1 − u)^1.5`, and shrink to 60%.

**Measured** on drifting, 30 fps × 6 s real:

| | before (orbit) | anchored drift |
|---|---|---|
| distance from the centre, median / max | 25.6 / 28.7 px | 28.0 / 34.9 px |
| particle-frames outside the rim (25.5 px) | 51% | 72% |
| frame-to-frame move, median / p95 | 0.33 / 0.95 px | 0.07 / 0.12 px |

Every other state's frames were byte-compared (34 states × 3 times × 2 option sets) and are unchanged.

**Measured in motion (2026-09-19):** 30 s at 60 fps per state, lifetimes read from the 0.02-alpha visibility windows of single particles over six seeds; speed is the mean frame-to-frame displacement.

| state | preset speed | real lifetime (median / max) | speed (px/s at 64) | clipped |
|---|---|---|---|---|
| breathing | 3.24× | 4.58 / 6.57 s | 6.6 | 0 |
| drifting | 3.315× | 4.58 / 6.57 s | 11.8 | 0 |
| working | 1.885× | 3.43 / 4.93 s | 5.3 | 0 |
| glowing | 1.6× | 4.58 / 6.57 s | 6.6 | 0 |
| speaking | 1× | 2.55 / 3.30 s | 5.4 | 0 |
| waveform | 1× | 2.57 / 3.30 s | 4.8 | 0 |
| tracking / completing | 1× | 4.27 / 5.48 s | 2.2 | 0 |
| scanning | 1× | 3.87 / 7.40 s | 3.7 | 0 |
| notifying | 1× | 2.40 / 2.40 s (synced) | 7.1 | 0 |
| broadcasting | 1× | 3.83 / 4.93 s | 3.2 | 0 |

Before the fix, the median lifetime was 0.10 s on `breathing` and 0.23 s on `drifting`, with 34–39 px/s of per-frame jumping.

**Chain position:** after `pulse`, before `noise` → `liquid` → colour → gradient → holographic → glow. Noise jitters particles, **liquid melts them into droplets leaving the blobs**, and glow haloes them. Decay and mute fade them.

- **Low power:** `disable: ["particles"]` (FX Spec 1.5). The engine off switch is `particleStrength: 0`, and hosts' recommended low-power default includes it.
- **Cost:** particles are dots, so they count through `elements`. The baseline turns them off, so the "materials add N" hint works.
- **Suitability:** there is **no suitability badge**. Every state has something to emit from, and the per-state defaults carry the meaning.

**Not copied:** tsParticles / particles.js integrate velocities frame by frame and add collisions, links and bounce "outModes". That's stateful simulation this engine doesn't carry. Only their vocabulary informed the knobs.

## `holographic`: a hue sweep over kept lightness (materials phase 4, 2026-09-19)

`holoStrength` > 0 recolours every element with a **foil / iridescent hue sweep** and leaves its lightness (`white`), alpha, geometry and `color_mode` untouched. There's no shader and no new primitive; it only rewrites `saturation`/`hue`.

```text
hue = holoHue + holoSpan · (holoDepth · d + holoFacing · f) + 360 · holoSpeed · t
```

**Where it comes from:**
- Thin-film iridescence shifts colour with the view angle, `thickness = min + range · (1 − cos θ)` (Papadopoulos, *Implementing a foil sticker effect*).
- A phase walks the hue wheel (Quilez's cosine palette, `a + b·cos(2π(c·t + d))`, rainbow at `d = (0, .33, .67)`).
- In this HSL ink model, both reduce to a **hue offset**.

**The terms:**
- **Depth** `d = (z + 1)/2`, clamped, for dots (the orbs sphere is `z ∈ [-1, 1]`). Lines, polylines and fills have no z and take 0.5.
- **Facing** `f = 1 − √(1 − ρ²)`, Fresnel-like, where `ρ` = the element's distance from a **view point** over `0.75 · size`.
  - The view point circles the frame centre at `size/4`, angle `2π · holoSpeed · t`, like a card tilting in the hand. Without it, a centred ring (every element equidistant from the centre) would have no sweep.
  - Position: a dot's centre, a line's midpoint, **each polyline vertex** (with the vertex mean for the fallback `hue`), and a fill's centroid (for its ink and stops).
- **Time:** `holoSpeed` turns per second, on **wall-clock** time (the preset speed is divided out, like particles).

**Per element**, exactly `apply_gradient`'s rule:
- saturation → `lerp(s, holoSaturation, strength)`;
- the hue snaps when the element was achromatic, and eases along the shorter arc (`lerp_hue`) otherwise.

| Key | Default | Meaning |
|---|---|---|
| `holoStrength` | — (off) | `0..1` master (Energy) |
| `holoHue` | `180` | base hue |
| `holoSpan` | `300` | degrees of the wheel the sweep covers (`0..720`) |
| `holoSaturation` | `0.7` | saturation the sweep blends toward |
| `holoDepth` | `0.5` | weight of the depth term |
| `holoFacing` | `0.5` | weight of the facing term |
| `holoSpeed` | `0.05` | turns/s: hue drift and the view point's orbit (Motion, `-4..4`) |

**Chain position:** after colour and gradient (it's the top tint; at strength < 1 it blends from them), before glow, so halos inherit the foil hue. Interrupt and mute keep the last word.

**How it reads per family** (34-state sheet and time strips):
- **Orbs and dot clouds** get a real spatial rainbow: by depth across the sphere and by facing across the silhouette.
- **Strokes** are coloured **per vertex** (2026-09-19, `Polyline.hues`). Each vertex takes the facing term at its own position, so a ring's track sweeps along its length and the sweep turns with the view point. `hue` stays the vertex-mean sample, as the fallback.
- **Fills and lines** keep one colour per path (the centroid / midpoint).
- The user chose per-vertex after an A/B, and chose to keep HSL over OKLCH (column B, 2026-09-19). OKLCH stays a possible later opt-in.
- It follows each element's own lightness, so dark ink stays dark (a deep hue) and light ink shows the colour most.

- **Cost:** none. It adds no elements and no blur, so it isn't in the cost baseline and **isn't sheddable** in low power. `disable: ["holographic"]` is an error that says why.
- **FX Spec 1.6:** `materials.holographic { strength, hue, span, saturation, depth, facing, speed }`.

## Studio

The Web Studio's per-family panels don't have sliders for these keys yet;
`studio-ui-ux` is adding one shared "Effects" group (mute, interrupt,
pointer, audio and the three materials) to every panel rather than
per-family copies. Until then the
keys work through the `overrides` map from any caller.

## Sources

- WHATWG HTML canvas shadows (`σ = shadowBlur / 2`, Gaussian), as cited in
  Mozilla bug 578995: <https://bugzilla.mozilla.org/show_bug.cgi?id=578995>
- Skia `SkBlurMask::ConvertRadiusToSigma`: <https://raw.githubusercontent.com/google/skia/main/src/core/SkBlurMask.cpp>
- MDN `shadowBlur`, `createRadialGradient`, `createLinearGradient`:
  <https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/shadowBlur>
- CSS-Tricks, "How to Create Neon Text With CSS": <https://css-tricks.com/how-to-create-neon-text-with-css/>
- Tobias Ahlin, "Smoother & sharper shadows with layered box-shadows": <https://tobiasahlin.com/blog/layered-smooth-box-shadows/>
- Josh Comeau, "Designing Beautiful Shadows in CSS": <https://www.joshwcomeau.com/css/designing-shadows/>
- kodintent, "Android: using radial gradients in canvas – glowing dot example": <https://kodintent.wordpress.com/2015/06/29/android-using-radial-gradients-in-canvas-glowing-dot-example/>
- Stemkoski, Three.js shader glow (`pow(c − dot(n, v), p)`, additive — GPU-only, not adopted): <https://stemkoski.github.io/Three.js/Shader-Glow.html>
- Nature of Code, "Randomness" (Perlin noise walker): <https://natureofcode.com/random/>
- CSS Images Module Level 3, linear gradients: <https://www.w3.org/TR/css-images-3/#linear-gradients>
- Nikos Papadopoulos, "Implementing a foil sticker effect" (thin-film iridescence by view angle): <https://www.4rknova.com/blog/2025/08/30/foil-sticker>
- Inigo Quilez, "Procedural Color Palette": <https://iquilezles.org/articles/palettes/>
- OpenReplay, "Creating Holographic Effects in CSS": <https://blog.openreplay.com/creating-holographic-effects-css/>
