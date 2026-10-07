# Materials: glow, noise, gradient

> **Materials vs effects** (the naming split, 2026-09-19):
> - A **material** is a frame → frame transform the *engine* computes. It produces or modifies primitives. The materials are colour, gradient, glow (both stacked copies and blur halos), noise and pulse. (Liquid, particles and holographic were removed in 0.1.0-beta.9; a file that uses one is rejected.) They are configured in the FX Spec `materials` section and cost engine time.
> - An **effect** is a *paint-contract instruction* the renderer executes: `blur` and `blend`, on `Fill`s and `EffectRun`s (see [`engine.md`](engine.md#paint-contract-fills-and-effects-materials-phase-1-2026-09-18)). Effects are not a spec section; they are reached through a material's options (`glow.mode: "blur"`). They cost paint time, which is what `blurLoad` estimates.
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
  → apply_noise          (material: organic jitter — moves geometry)
  → apply_color          (material: one colour for the frame + the ink|fixed paint mode)
  → apply_gradient       (material: color ramp by final position, 2 or 3 stops)
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

A 2 s pulse is 2 s on a 1× ring and on a 3.24× `breathing`. A spec's own `speed` (the user's "whole object slower/faster" knob) still scales all of them together with the geometry.

Deliberately **not** on either clock:
- **Interrupt and decay** don't read `t`. `interruptAge` and `decayAge` are seconds on the *caller's* clock since a one-shot event, and a caller measures them against its own timestamp.
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

## `glow`: layered halos (design choices)

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

## `noise`: organic jitter (design choices)

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

## `gradient`: a color ramp by position (design choices)

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

## `color`: one colour for the whole frame (design choices, 2026-09-18)

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

## Studio

The Web Studio's per-family panels don't have sliders for these keys yet;
`studio-ui-ux` is adding one shared "Effects" group (mute, interrupt,
pointer, audio and the materials) to every panel rather than
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
- Inigo Quilez, "Procedural Color Palette": <https://iquilezles.org/articles/palettes/>
