# FxView — the drop-in renderer (Web, iOS, Android)

`packages/` used to ship geometry only: every painter lived inside a Studio
app, so an integrator had to copy paint code and write a render loop. `FxView`
is the one-line version. You give it an FX Spec (or a plain state) and,
optionally, a `VoiceSource`. It runs the clock, the paint contract, theming,
reduced motion, pausing and accessibility for you.

```ts
// Web, no framework
import { mount } from "@sinua/web";
const fx = mount(canvas, { spec, voice });          // fx.update({...}), fx.destroy()

// React
import { FxView } from "@sinua/web/react";
<FxView spec={spec} voice={voice} style={{ width: 160, height: 160 }} />
```

```swift
// SwiftUI -- packages/ios, product Sinua
import Sinua
FxView(spec: specJSON, voice: micSource).frame(width: 160, height: 160)
FxView(pattern: "speaking")
```

```kotlin
// Compose -- packages/android, module :sinua-view (dev.sinua.view)
FxView(spec = specJson, voice = micSource, modifier = Modifier.size(160.dp))
FxView(pattern = "speaking")
```

The headline case is `FxView` plus a `VoiceSource`: an orb that reacts to a
conversation. On Web, the sources are the `@sinua/voice` package
(`/openai`, `/gemini`, `/elevenlabs`, `/livekit`, `/mic`, `/tone`); natively
they are SinuaVoice and its vendor products. See
[`audio-pipeline.md`](audio-pipeline.md).

## API (the same concepts on every platform)

| Option | Meaning |
|---|---|
| `spec` | An FX Spec, as a JSON text or object on Web and as JSON text on native ([`fx-spec.md`](fx-spec.md)). It is resolved once per change, and an invalid spec draws nothing and reports its diagnostics (Web: `onError`; native: a logged warning). |
| `pattern`, `size`, `overrides`, `speed` | Plain input instead of a spec: `pattern` is the look ("speaking", "tracking"). `overrides` are engine opts, e.g. a Studio export. `speed` multiplies the preset's tuned speed. A speed change is **phase-continuous**: the clock keeps the pose it had and runs on at the new rate, instead of rescaling all the elapsed time at once, so a lifecycle state with its own speed doesn't jump. `speed` 0 holds the pose where it is. |
| `voice` | A `VoiceSource` or a `VoiceOverrides`. A source holds one metrics/state callback, so the view binds it; read the view's `voice` (Web handle) for a meter. If your app already listens to the source, pass a `VoiceOverrides` you feed yourself. The view never connects or disconnects the source. The bound defaults are the Studio's per family, derived from `spec.object`: orb gets raw bands (`bandEaseRate` ∞); signal gets `audioStrength` 0 and a scrolling history of 40 @ 12 Hz (or the spec's `historyCount`). Override with `voiceOptions` (Web). |
| `state`, `inputs` | With a spec: `state` is the app lifecycle key and picks the spec's `states` entry, and `inputs` drive `bindings`. **With a voice, `state` defaults to the voice's `AgentState`** (`listening`, `speaking`, …, which is the `states` convention in `fx-spec.md`), so a v1.1 spec follows the conversation. A key the spec lacks renders the top-level design. |
| `voiceLevelInput` | Also feeds the voice level into this spec input, e.g. `"agentVolume"`, so the spec's own binding drives the look. |
| `crossFade` | The state-change cross-fade, in seconds: 0.25 s cubic ease-out, the Studio's and `FxSpecPlayer`'s. 0 cuts. One name on every platform (Web React and Android called it `crossFadeSeconds` before 2026-09-21). |
| `theme` | `auto` (system), `light` or `dark`. Ink mirrors on dark unless the frame's `colorMode` is `fixed`. The background is transparent, so the host's paper shows through. |
| `paused` | Freezes the clock; resuming continues from the same pose. |
| `reducedMotion` | `auto` follows the system setting: `prefers-reduced-motion` on Web, `accessibilityReduceMotion` on iOS, and "Remove animations" on Android (`ANIMATOR_DURATION_SCALE` 0). It renders a static frame at t = 0.6. While a voice is attached, the view still redraws at up to 30 Hz, because the voice cue is information, not decoration. |
| label (`label` / `accessibilityLabel` / `contentDescription`) | The accessible name, with an image role. It defaults to the spec's `name`, else the state. `""` marks the view decorative. |

### Names (FX Spec 1.7): `pattern` and `state`

A **pattern** is the look ("speaking", "tracking"). A **state** is the app's
lifecycle key ("listening"), which picks a spec's `states` entry. FxView used
the older labels until 1.7. They still work, deprecated:

| Before | Now | Old label |
|---|---|---|
| `FxView(state: "speaking")` (plain input) | `FxView(pattern: "speaking")` | Swift `@available(deprecated)`, Kotlin `@Deprecated`. Web/RN: `state` without a `spec` is read as the pattern, with one `console.warn`. |
| `specState: "listening"` (spec path) | `state: "listening"` | Deprecated on every platform. |
| SinuaViewLayout `fxState` / `fxSpecState`, `.state` / `.specState` | `fxPattern` / `fxState`, `.pattern` / `.state` | Renamed, no alias (it was never published). |

On Web, changing `state` on a spec view only moves the lifecycle state, which cross-fades.
Only a changed `pattern` rebuilds the input.

## Paint rules (`spec/orbs-spec.json` `paint`)

- **Clock:** `t = elapsed · presetSpeed · speed`. `elapsed` accumulates real
  frame time, clamped to 0.1 s per frame so a stall doesn't jump. This is
  exactly `frame_from_fx_spec`'s math; the Web tests assert the drawn frame
  equals `frameFromFxSpec(spec, elapsed)`.
- **Pausing:**
  - Web: off-screen (`IntersectionObserver`) and hidden tab (`visibilitychange`).
  - iOS: `onDisappear` and a scene phase other than active.
  - Android: lifecycle below RESUMED.
  - Every platform: `paused`.
- **DPR cap 2:** the Web backing store is CSS size × min(devicePixelRatio, 2).
  SwiftUI `Canvas` and Compose `Canvas` draw vectors at display scale, so the
  cap only concerns raster backings.
- **Layout:** engine space is square and centered in whatever box the view
  gets.
- **The paint contract is unchanged:** ordering, ink, round-capped
  polylines and `colorMode`. It moved, verbatim, out of the Studios:

| Platform | Package painter | Studio now |
|---|---|---|
| Web | `packages/web/src/paint.ts` (`drawFrame`, `drawCrossDissolve`, `ink`) | the Web Studio's `drawFrame.ts` re-exports it |
| iOS | `packages/ios/Sources/Sinua/FxPaint.swift` | `OrbCanvasView.swift`: `typealias OrbPaint = FxPaint` |
| Android | `packages/android/view/.../FxPaint.kt` (`fxInk`, `paintFxFrame`) | `OrbCanvas.kt`: `ink` / `paintFrame` delegate |

## Parity with the Studio (the proof the move changed nothing)

- **Web:** 0 differing pixels in real Chrome. The old Studio `drawFrame`
  (snapshot) and the package's were checked with a `getImageData` diff over
  109 frames: 35 states × 3 times plus the example specs, each in light and
  dark, as plain frames and cross-dissolves. That's 436 renders, all
  non-blank.
- **iOS:** `SinuaTests` renders the old `OrbPaint` (a snapshot kept in
  the test target) and `FxPaint` through `ImageRenderer`. All 68 bitmaps are
  byte-identical (every state, light and dark).
- **Android:** an instrumented test rasterizes the old `paintFrame` (snapshot)
  and `paintFxFrame` with `CanvasDrawScope`. All 136 rasters are identical
  (every state, dark and light, alpha 1 and 0.37).

## Other checks

- **Web** (`packages/web`, `npm test`, 9 node tests with stubbed browser APIs
  and a recording canvas):
  - the spec frame equals `frameFromFxSpec`, and the state frame uses
    `t = elapsed·presetSpeed·speed`;
  - reduced motion draws t 0.6 with no loop;
  - DPR cap; pausing on a hidden tab, `pause()` and `paused`;
  - voice keys merge;
  - the voice's `AgentState` selects the v1.1 state;
  - an invalid spec reaches `onError` and draws nothing;
  - aria attributes.

  The example page (`packages/web/example`, `npm run example`) renders all
  three one-liners in both themes, animating, with no console errors.
- **iOS:**
  - The spec path equals `frameFromFxSpec` for every example spec.
  - The state cross-fade math matches.
  - `FxView` itself renders through `ImageRenderer` (a spec, a state, dark).
- **Android** (instrumented, emulator):
  - The spec path equals `frameFromFxSpec`.
  - The cross-fade matches.
  - `FxView` renders in a Compose test, drawing and animating: 19k inked
    pixels, 25k changed between captures.

## Fills and effects (Materials phase 1)

The paint contract gained **fills** (closed polygons, solid or with a
linear/radial gradient) and **effects** (Gaussian blur σ, additive blend).
The contract itself lives in `docs/engine.md` ("Paint contract: fills and
effects", families). This section covers how each renderer draws it.

| | Web (`@sinua/web`) | SwiftUI (`FxPaint`) | Compose (`paintFxFrame`) |
|---|---|---|---|
| Fill path | `closePath`, nonzero | `Path.closeSubpath`, nonzero | `Path.close`, nonzero |
| Fill with `holes` (phase 2, Liquid) | outer ring + each hole as a subpath of one path, `fill("evenodd")` | same, `FillStyle(eoFill: true)` | same, `PathFillType.EvenOdd` (carried into the framework path for blur) |
| Gradient | `createLinearGradient` / `createRadialGradient(x0,y0,0, x0,y0,r)` | `.linearGradient` / `.radialGradient(startRadius: 0, endRadius: r)` | `Brush.linearGradient` / `radialGradient`, `TileMode.Clamp` |
| Stop colour | `ink()` per stop, alpha = stop.a × fill.a, mirrored on dark unless `colorMode` fixed | same (`FxPaint.ink`) | same (`fxInk`) |
| Additive | `globalCompositeOperation = "lighter"` | `blendMode = .plusLighter` | `BlendMode.Plus` |
| Blur σ | **canvas shadow**: `shadowBlur = 2σ·scale` (the HTML spec's σ = shadowBlur/2, device px), element drawn 10 000 units away and shifted back via `shadowOffsetX`, `shadowColor` = opaque ink. Safari has no `ctx.filter` (WebKit bug 198416) | `addFilter(.blur(radius: σ · FxPaint.blurRadiusPerSigma))`, constant **1.0** (measured, see below) | `BlurMaskFilter(radius)` on a framework `Paint`, `radius = (σ·scale − 0.5) / 0.57735` (Skia's conversion) |
| Fallback | a **blurred gradient fill** uses `ctx.filter` where present; Safari draws it sharp | none needed | **API < 29: no blur** (BlurMaskFilter isn't reliable under HW acceleration before Android 10); additive still works |

- **Draw order:** fills → polylines → lines → dots, as before. Effect runs
  (`EffectRun`) apply blur/blend to a range of one list and never reorder.
  A cross-dissolve's alpha goes on top.
- **Low power:** FX Spec 1.3 `performance.lowPower.disable: ["blur"]` makes
  the resolver emit no runs and σ 0, so renderers do nothing special. (The
  Web test asserts the low-power frame has no runs.)
- **Holographic-lite** (phase 4) only rewrites hue/saturation, and
  **Particles** (phase 3) are plain `Dot`s: neither needs a painter
  change. Low power sheds particles (`particleStrength` 0 in the host
  default); holographic has no cost to shed.
- **Plain frames are untouched.** A frame without fills or effects takes
  the exact old code path on every platform:
  - Web: 0 differing pixels against the old Studio painter over 440 renders,
    and `drawPacked` v1 is 0 bytes different over 204.
  - iOS: bitmap parity still 68/68.
  - Android: raster parity still 136/136.
  - Packed v2 frames paint through `unpackFrame` → `drawFrame`.

**How the Web blur was validated.** In Chrome, which has both techniques,
the shadow blur was compared with a `ctx.filter` `blur(σ·scale px)` render
of the same dots, for σ 0.5–4 in light and dark. The largest differences
were mean 0.85/255 and p99 15/255.

**Cross-platform tolerance.** Two renders are composited over the theme's
paper (light 255 / dark 0), then compared per RGB channel; raw RGBA is
meaningless at near-zero alpha. The thresholds are mean ≤ 2/255 and
p99 ≤ 24/255. These are measured on families' golden cases (t 0.6,
size 64, drawn at 256 px):

| Case | web↔iOS mean / p99 | web↔Android | iOS↔Android |
|---|---|---|---|
| highlight fill (solid + linear gradient) | ≤ 0.06 / 1.1 | ≤ 0.10 / 1.1 | ≤ 0.14 / 1.3 |
| trail fill (gradient wedge + dots + beam) | ≤ 0.17 / 2 | ≤ 0.17 / 3 | ≤ 0.22 / 3 |
| glow blur (520 dots) | ≤ 0.49 / 10.7 | ≤ 0.50 / 9 | ≤ 0.41 / 6 |
| glow blur + additive (12 polylines) | ≤ 0.67 / 5.1 | ≤ 0.69 / 5 | ≤ 0.54 / 3.1 |
| Liquid fill (1 fill + 1 hole, even-odd) | ≤ 0.22 / 4.1 | ≤ 0.32 / 5.8 | ≤ 0.14 / 2 |
| Liquid outline (2 closed polylines + the kept 96-dot LED grid, re-baselined by families 2026-09-19) | ≤ 0.37 / 12.8 | ≤ 0.43 / 13.9 | ≤ 0.43 / 14.7 |
| Liquid dots (contour-resampled dots) | ≤ 0.48 / 16 | ≤ 0.42 / 14.1 | ≤ 0.30 / 9.6 |
| Particles drift (re-baselined, golden 1.5.0) | ≤ 0.50 / 16.3 | ≤ 0.43 / 14.2 | ≤ 0.30 / 9.8 |
| Particles attract | ≤ 0.22 / 7 | ≤ 0.21 / 7.1 | ≤ 0.15 / 5 |
| Particles orbit (12-particle ring + 2 polylines) | ≤ 0.10 / 4 | ≤ 0.29 / 4 | ≤ 0.26 / 3 |
| Particles → liquid (24 thin polylines after families' Q2 re-baseline) **AA exception** | ≤ 1.01 / **12.1 half-res** (28.1 full-res, info) | ≤ 0.97 / 11.3 half-res (24.1 full-res) | ≤ 0.74 / 7.7 half-res (15.2 full-res) |
| Holographic (glowing) | ≤ 0.47 / 16.3 | ≤ 0.41 / 14.1 | ≤ 0.27 / 9 |
| Holographic + liquid fill (holes) | ≤ 0.40 / 3.8 | ≤ 0.42 / 5 | ≤ 0.15 / 2 |
| Holographic + glow | ≤ 0.18 / 2.9 | ≤ 0.29 / 3 | ≤ 0.31 / 2.9 |
| Holographic + gradient | ≤ 0.34 / 7 | ≤ 0.40 / 9.8 | ≤ 0.37 / 8.1 |

All values are /255, and each cell is the worse of light and dark. The
contact sheet was checked by eye too: the three renderers are
indistinguishable.

**This table is enforced, not remembered** (since 2026-09-20).
`scripts/materials-check.sh` renders all 21 cases on all three painters and
fails the build outside the tolerance above; the `materials` job in
`.github/workflows/ci.yml` runs it on every push. Both native render tests
already ran in CI — `MaterialsRenderTests` inside `-only-testing:SinuaTests`,
`renderMaterialsGoldenCases` inside `:sinua-view:connectedDebugAndroidTest` —
so the check collects work that was previously produced and discarded, and
costs no device time. A re-run reproduces the worst cases in this table
exactly: web↔iOS 1.01 / 16.3, web↔Android 0.973 / 14.2, iOS↔Android 14.7.

A render that never arrives is a **failure**, named. Until that date it was
not: `compare.cjs` recorded an absent platform as the string `"missing"` and
its verdict loop only inspected object values, so an empty directory printed
42 rows of `"missing"` and then `ALL WITHIN TOLERANCE`, exit 0. For Liquid fill, the hole is empty on every platform:
the centre pixel of each render has alpha 0, so the paper shows. Adding
holes didn't move the phase-1 numbers at all.

### Colour conversion differs between the Web and the natives, by decision

`ink()` is not one implementation but three, and they do not agree exactly:

| | how it converts | precision |
|---|---|---|
| Web (`packages/web/src/paint.ts:15`) | emits the CSS string `hsla(h, round(s*100)%, round(l*100)%, a)` — **the browser** converts | **saturation and lightness rounded to whole percent first** |
| Swift (`FxPaint.swift`) | a hand-written `hslToRgb`, because SwiftUI's `Color(hue:saturation:brightness:)` is HSB, not HSL | full |
| Kotlin (`FxPaint.kt`) | Compose's `Color.hsl` | full (Float) |

**Measured** (2026-09-20, `spec/paint-ink-vectors.json`, 1 176 cases over
`white × saturation × hue × alpha × dark`, reference resolved by real Chrome
and cross-checked against CSS Color 4 to within 1/255):

| | worst channel delta | cases over 1/255 | alpha |
|---|---|---|---|
| Swift vs Web | **2.04/255** | 240 / 1176 | exact (4.8e-09) |
| Kotlin vs Web | **2.00/255** | 168 / 1176 | 0.00137 (Compose quantises alpha) |

The worst case is `dark: true, white: 0.004` → lightness 0.996, which the Web
rounds to 1.00 (pure white) and the natives do not.

**This is accepted by decision (2026-09-20), not an open defect.** 2/255 is
invisible, and making three painters agree exactly would mean changing how at
least one of them emits colour. Do not "fix" it: doing so would break the
agreement recorded here. **A parity check over colour must be written against
this tolerance, not against zero.**

The check is `spec/paint-ink-vectors.json` with consumers in
`SinuaTests/InkVectorsTests.swift` and `:sinua-view`'s `InkVectorsTest.kt`
(a JVM test — no emulator). Both bound the worst delta at 3/255. **Their
stated limit:** a deliberate 3 % error in the Swift chroma constant only moves
the worst case to 3.95/255, so the bound catches structural mistakes — a wrong
hue sector or a swapped channel is *tens* of units — and moderate constant
drift, but a sub-1 % arithmetic change would pass. Regenerate the vectors with
`node packages/web/scripts/gen-ink-vectors.mjs`.

**Anti-aliasing exceptions** (agreed with families; only deliberate
additions, each with a reason). Every case must pass at full resolution:
mean ≤ 2/255, p99 ≤ 24/255. A listed case instead passes on full-resolution
mean ≤ 2 **and** half-resolution (2×2 box) p99 ≤ 24, and its full-resolution
p99 is reported as information. The whole metric isn't moved to half
resolution because that would also soften real thin-stroke offsets.
- `drifting-64-0.6-particles-liquid`: dense thin-stroke curls at ~2.4 px,
  where Chrome's anti-aliasing differs from CoreGraphics/Skia. Re-checked
  again after families' Q2 re-baseline (2026-09-19; anchored drift, now 24
  polylines instead of 30): web↔native full-resolution p99 is 24.1–28.1,
  but 11.3–12.1 at half resolution; iOS↔Android is 15.2 at full
  resolution; the means are ≤ 1.01. Still needed.

**Renderer-environment exceptions** (user decision, 2026-09-21; only
deliberate additions, each with its measurement). A listed case passes on
mean ≤ its own limit instead of 2/255; p99 stays ≤ 24/255. These are for a
render that has been measured to move with the *environment* (browser/OS build,
Android GPU backend), not with our paint code.
- `tracking-64-0.6-glow-blur-additive`: **mean ≤ 3/255.** The first CI run
  with the Android leg (2026-09-21) measured web↔Android dark mean **2.023**,
  against ≤ 0.97 locally. Measured, not assumed: the same Android render
  compared with a *local* Web render is 1.23, a local arm64 `swiftshader`
  emulator gives 0.53, and CI's Android differs from local Android only on the
  additive-blur renders (≤ 1.07, the other 38 renders ≤ 0.05). CI renders the
  Web in Linux headless Chrome and Android on an x86_64 `swiftshader`
  emulator, and additive blending over a blur is the one render that is
  sensitive to both. Everything else stays at 2/255, so a real paint change in
  this case's neighbours still fails.

The check is scripted in `packages/web/scripts/materials/` (`frames.mjs`
reads every materials golden case, `compare.cjs` holds the exception
list); the README covers producing the iOS/Android renders. Additive is invisible on a light paper by nature (it's
a dark-theme look). SwiftUI's blur radius equals σ, so
`blurRadiusPerSigma = 1.0` passes with margin; no fitting was needed.

### Per-vertex stroke colour (`Polyline.hues`, golden 1.6.0)

The painters follow families' rule (`docs/engine.md`, *Per-vertex stroke
colour*). For a polyline with one hue per point:
- every segment is a round-capped stroke with a linear gradient between its
  two vertices' inks, at alpha 1;
- a zero-length segment is a dot of diameter `w`;
- all segments go into **one layer**, composited once at `a`, with the
  polyline's blend.

Frames without `hues` keep exactly the old calls (a `hues` array whose
length doesn't match the points falls back to `hue`).

| | Layer | Blur at the composite |
|---|---|---|
| Web `paint.ts` | a reused scratch canvas (`OffscreenCanvas`, else a DOM canvas) with the target's transform, drawn only in the stroke's device-pixel box, then one `drawImage` with `globalAlpha *= a` (an outer cross-fade alpha is kept) and `lighter` for additive | `ctx.filter = blur(σ·scale px)`. A shadow, this painter's usual blur, has one colour. **Safari** has `filter` disabled by default through 27.x (caniuse), so there the layer is drawn sharp with a one-time warning, the same rule as blurred gradient fills |
| SwiftUI `FxPaint.swift` | `GraphicsContext.drawLayer` on a context copy with `opacity *= a` and `plusLighter` for additive | the copy's blur filter applies to the layer as a whole (exact) |
| Compose `FxPaint.kt` | `Canvas.saveLayer(box, Paint(alpha = a, PLUS/ADD))` | no layer blur on a framework `Paint`: `RenderEffect` needs a RenderNode / hardware canvas, and the Studio's PNG export is a software bitmap. Blurring each segment inside the layer **measured wrong**: the overlapping soft fringes add up to a wider, stronger halo, mean ~5/255 against Web/iOS. So a *blurred* per-vertex polyline is drawn as the usual one-path `BlurMaskFilter` stroke in the vertex-mean `hue`: exact geometry, and the hue sweep drops out. That measures within tolerance, because the sweep isn't visible under a halo-sized blur |

**Seam check** (iOS `PerVertexSeamTests`, Android `PerVertexSeamTest`).
A zigzag with sharp bends and a repeated point, at `a = 0.5`, is drawn
three ways: through the per-vertex path with all hues equal, as one stroke
(the no-seam reference), and as a naive per-segment composite (the
negative control, each segment straight at `a`).
- The layered render matches the one-stroke reference: mean 0.06/255,
  p99 2 (iOS) and 1 (Android).
- Pixels off by more than 24/255: **31 (iOS) and 27 (Android)** for the
  layered render, against **1044 and 1035** for the naive one.
- Those few remaining pixels are anti-aliasing on the outline at bends.
  Where two segments' AA edges overlap inside the layer, their coverages
  combine as 1−(1−c₁)(1−c₂), a touch darker than one stroker's coverage.
  It isn't the notch.
- A 16× zoom of `tracking-64-0.6-holo`'s alpha < 1 tracks (the iOS
  attachment) shows continuous tracks with no marks at the joints.

**Three-platform tolerance** (same metric; worst of light/dark; mean / p99):

| Case | web↔iOS | web↔Android | iOS↔Android |
|---|---|---|---|
| `tracking-64-0.6-holo` | 0.07 / 1.9 | 0.21 / 3.3 | 0.22 / 3.1 |
| `tracking-64-0.6-gradient3` | 0.13 / 1.9 | 0.19 / 3.1 | 0.26 / 3 |
| `locating-64-0.6-holo` | 0.14 / 2 | 0.18 / 2 | 0.22 / 2.9 |
| `completing-64-0.6-holo-interrupt` | 0.14 / 1.1 | 0.20 / 1.9 | 0.14 / 2 |
| `completing-64-0.6-holo-glow` (now with `hues`) | 0.14 / 1.2 | 0.31 / 2.2 | 0.36 / 2.8 |
| *synthetic* `x-…-holo-glowblur` (information) | 0.48 / 3 | 0.68 / 3 | 0.74 / 3.8 |
| *synthetic* `x-…-holo-glowblur-additive` (information) | 0.49 / 3 | 0.54 / 3 | 0.46 / 3 |

No golden case puts an effect run on a per-vertex polyline, so the two
synthetic cases do: `completing`, holo + glow with `glowMode 1` (+
`glowBlend 1`). They're listed in `frames.mjs` `SYNTHETIC` (keys start with
`x-`); `compare.cjs` reports them and never fails on them. `frames.mjs` now
also picks up any golden case whose frame has `hues`, which catches
`…-gradient3`.

## Performance and power

**Frame cap: `maxFps`** (Web `mount`/React, SwiftUI, Compose). This is real
frame pacing, not a timer. The animation clock stays wall time, so a cap
lowers smoothness, never speed.
- **Web and Android:** skip display frames with the shared pacer (Web
  `createFramePacer`, Kotlin `FramePacer`, Swift `FramePacer`, the same
  rule on all three):
  - draw when `now − last ≥ interval − 1 ms`;
  - then `last += interval · max(1, floor((now − last + 1) / interval))`.
  This is a fixed grid, so the average rate is exact (30 on 60 Hz or
  120 Hz, 24 on 60 Hz), with no drift and no burst after a stall. The
  "carry the remainder" variant (`last = now − ((now − last) % interval)`)
  looks equivalent but isn't: a frame just inside the 1 ms tolerance leaves
  `last` unmoved and the next frame draws too. The tests measured ~40 fps
  for a 30 cap on 60 Hz; the Studio saw 36–54.
- **iOS:** `TimelineView(.animation(minimumInterval: 1/maxFps))`. The system
  schedules at the interval, with no per-vsync wakeups.
- The Studio's preview loops use the same pacer (studio-ui-ux).

**Low power:**

| Platform | Signal | How the view follows it |
|---|---|---|
| iOS | `ProcessInfo.isLowPowerModeEnabled` + `NSProcessInfoPowerStateDidChange` | `lowPower: .auto` (default) via `LowPowerMonitor.shared`; `.on`/`.off` override it |
| Android | `PowerManager.isPowerSaveMode` + `ACTION_POWER_SAVE_MODE_CHANGED` | `lowPower = FxLowPower.AUTO` (default) via `rememberPowerSaveMode()` |
| Web | **none reliable** | the app sets `lowPower: true`. Why: the Battery Status API is Chromium-only (Firefox removed it in v52 as a fingerprinting vector; Safari never shipped it) and reports battery level/charging, not a saving mode; `prefers-reduced-data` isn't shipped anywhere (MDN, caniuse, webstatus.dev). `watchLowBattery(cb)` is an **opt-in heuristic** (level ≤ 20 %, not charging) where `getBattery` exists; it resolves `null` elsewhere |

**What low power does.** The same rule on every platform (`performanceFor`
/ `fxPerformance`):
1. **FX Spec 1.2 `performance` block:** the resolver gets `lowPower`
   (TS `ctx.lowPower`, `FxSpecPlayer.setLowPower`, UniFFI
   `resolveFxSpecWith(…, lowPower:)`). It reports the cap for that power
   state (`FxSpecResolved.maxFps`) and sheds `lowPower.disable` itself;
   a shed material stays shed even if it's bound.
2. **No `lowPower` block:** the host default is 30 fps with glow and
   particles off (`glowStrength` 0, `particleStrength` 0; both are their
   material's off switch, a strict no-op). Liquid is left to the spec/app,
   because it replaces dots rather than adding to them. This matches the
   recommended host default in `fx-spec.md`.
3. The view's own `maxFps` caps further, and the lowest cap wins.
   `performance.maxFps` also applies outside low power.

`onFrame` (every platform) reports each drawn frame's `dtMs`, `computeMs`
(engine) and `paintMs`. The native paint number is the time to record the
drawing, not GPU raster.

**Web transport.** A plain-state `mount` renders through the engine's
packed `Float64Array` (`frameWithOverridesPacked`, families) and paints it
with `drawPacked`: no JSON and no object per dot. `drawPacked` is
pixel-identical to `drawFrame` in Chrome (252 renders, 0 differing bytes,
including `colorMode: fixed`). The spec path goes through `FxSpecPlayer`
(cross-fades), which is on the JSON path until families switches its
internals to packed.

**Measure it on a device:** [`bench.md`](bench.md) (Web/iOS/Android bench apps, one result format).

### Android Views (XML): `SinuaViewLayout`

For apps still on XML layouts, `dev.sinua.view.SinuaViewLayout` is a classic
`View` (an `AbstractComposeView`, the documented way to put Compose inside a
View) that hosts the Compose `FxView` unchanged:

```xml
<dev.sinua.view.SinuaViewLayout
    android:id="@+id/orb"
    android:layout_width="160dp" android:layout_height="160dp"
    android:contentDescription="Assistant"
    app:sinuaPattern="speaking" app:sinuaTheme="auto" app:fxMaxFps="30" />
<!-- or app:sinuaSpecAsset="assistant.fxspec.json" app:sinuaState="listening" -->
```

```kotlin
val orb = findViewById<SinuaViewLayout>(R.id.orb)
orb.voice = LocalMicVoiceSource()   // every attribute is also a property; setting one recomposes
orb.pattern = "tracking"            // or orb.spec = json to switch to the spec path
```

- **Attributes:** `fxPattern`, `fxSize` (the engine resolves **20**, **32**
  and **64**; other sizes have no preset and draw nothing), `fxSpeed`, `fxSpec`
  (inline JSON) / `fxSpecAsset` (a file in `assets/`), `fxState` (the spec's lifecycle state),
  `fxTheme` (auto / light / dark), `fxPaused`, `fxReducedMotion` (auto /
  always / never), `fxLowPower` (auto / on / off), `fxMaxFps`, plus
  `android:contentDescription`.
- It needs a `ViewTreeLifecycleOwner` like any Compose-in-View: a
  `ComponentActivity`, AppCompat 1.3+ or Fragment 1.3+. Give each instance a
  unique id. The composition is disposed on detach (Compose's default).
- **Tested:** `SinuaViewLayoutTest` (instrumented, render-only) inflates it from
  XML. It checks that the attrs are parsed, that frames render while running
  (≈30 fps at `fxMaxFps` 30) and stop while `paused`, that `fxSpecAsset`
  loads, and that the spec and state paths switch from Kotlin.

### Hosting the SwiftUI FxView in UIKit

A `UIHostingController(rootView: FxView(…))` works in any UIKit app. FxView
runs its frame loop only while visible, not `paused`, and active:
- **Apps with a scene manifest** (`UIApplicationSceneManifest`, i.e. every
  SwiftUI `App` and scene-based UIKit app) use SwiftUI's `scenePhase ==
  .active`, as before. An inactive scene of an active app stays paused.
- **Apps without one** (app-delegate UIKit apps, React Native's default
  template) have `scenePhase == .background` forever inside a hosting
  controller. So FxView follows `UIApplication`'s own active state there
  (`didBecomeActive` / `willResignActive` / `didEnterBackground`, through
  `AppActivityMonitor`).
- Measured in such a host (`SceneLessHostTests`): 1 frame in 1.5 s before
  the fallback, 59 frames in 1 s after; `paused` still stops it.

### Composing with glass

FxView draws on a transparent background, so a platform "glass" surface
behind it is plain composition; nothing in the engine is involved.
**Checked:** the SwiftUI snippet type-checks against the iOS 27 SDK (target
iOS 17, so the fallback path compiles too), and the Compose snippet compiles
in `:sinua-view`. Neither was checked for looks on a device.

**SwiftUI.** Liquid Glass on iOS 26+, the classic material before it:

```swift
FxView(spec: spec)
    .frame(width: 120, height: 120)
    .padding(20)
    .modifier(GlassBackground())

struct GlassBackground: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content.glassEffect(.regular, in: .circle)
        } else {
            content.background(.ultraThinMaterial, in: Circle())
        }
    }
}
```

**Compose.** There's no true backdrop blur for a view here: that's
`RenderNode.setBackdropRenderEffect` in a newer SDK (37.2) than this build
targets. So blur **your own background layer** behind the orb.
`Modifier.blur` takes effect on API 31+; below that it's a no-op, and the
tint alone remains.

```kotlin
Box(Modifier.size(160.dp).clip(CircleShape)) {
    Image(backgroundPainter, null, Modifier.matchParentSize().blur(24.dp), contentScale = ContentScale.Crop)
    Box(Modifier.matchParentSize().background(Color.White.copy(alpha = 0.18f)))
    FxView(spec = spec)
}
```

**Web.** Real backdrop blur, on the element behind the canvas:

```css
.orb-glass { backdrop-filter: blur(24px) saturate(1.4); -webkit-backdrop-filter: blur(24px) saturate(1.4);
             background: rgb(255 255 255 / 0.18); border-radius: 50%; }
```
```html
<div class="orb-glass"><canvas id="orb" width="160" height="160"></canvas></div>
<!-- mount(document.getElementById("orb") as HTMLCanvasElement, { spec }) -->
```

Keep the orb's own ink readable on the glass: the `colorMode: "fixed"`
colours (`docs/engine.md`) don't mirror with the theme, which is what you
want on a tinted surface.

The Studios' export drawer prints these calls for the current design (Code: props; File: the `.fxspec.json` + how to load it), on all five platforms.

## Voice states without a spec (`pattern` + `state`)

Give a view a **pattern** and the agent's **state** and the built-in voice-state
behaviour moves that one shape through the conversation: `idle` breathes slowly and
dims (`ink`), `listening` draws inward as the user speaks, `thinking` stirs, `speaking`
swells. No FX Spec file is needed. The profile comes from the engine
(`voiceStateProfile(pattern, state)`, FX Spec 1.8) and covers every pattern.

```swift
SinuaOrb(pattern: .working, state: agentState)          // or FxView(pattern:state:)
```
```kotlin
SinuaOrb(pattern = SinuaOrbPattern.WORKING, state = agentState)
```
```tsx
<FxView pattern="working" state={agentState} />          // Web and React Native
```

With a `voice` attached the state defaults to that source's `AgentState`, so a bound
source needs no `state` at all. The profile also names which of your inputs drives the
audio: `micLevel` while listening, `agentVolume` while speaking. Pass it in `inputs` and
it becomes `audioLevel`; with a bound source its own level is used.

**Precedence** (the same on every platform):

| Case | What wins |
|---|---|
| `pattern` + `state`, no spec | the profile first, then **your `overrides`**, then the voice's live keys. Your values always win over the profile. |
| `spec` of FX Spec **1.8+** + `state` | **the file wins.** The resolver merges the profile into that state's entry and fills only what the file doesn't set. The view adds nothing. |
| `spec` of **1.7 or older** + `state` | unchanged: the file's `states` entry, or its base design when the file has no such key. No profile is applied, so an old file renders exactly as it did. |
| a `state` outside the five voice names (your own, e.g. `goalReached`) | no profile anywhere; the view draws as if no state were given. |

The state's speed rides the phase-continuous clock, so switching states changes the
rate without jumping the pose.

## Typed components (`SinuaOrb`, `SinuaRing`, `SinuaSignal`, `SinuaCore`, `SinuaBeacon`)

One component per engine object, generated from the parameter catalog
(`spec/parameters.json`). Each one is a thin wrapper over `FxView`, with no
painting of its own: typed props become the engine overrides, or a spec is
played as is. FxView stays the low-level layer.

```swift
SinuaRing(pattern: .tracking, progress: [0.2, 0.5], ringCount: 2, glow: .init(strength: 0.6))
SinuaOrb(spec: json, state: "listening", voice: source)   // must be an "object": "orb" spec
```
```kotlin
SinuaRing(pattern = SinuaRingPattern.TRACKING, progress = SinuaNumbers.of(0.2, 0.5), ringCount = 2, glow = SinuaGlow(strength = 0.6))
```
```tsx
<SinuaRing pattern="tracking" progress={[0.2, 0.5]} ringCount={2} glow={{ strength: 0.6 }} style={{ width: 64, height: 64 }} />
```

- **`pattern`** is an enum of the object's catalog patterns (required). **`size`** is 20, 32 or 64.
- **Parameters.** Pattern and value parameters are flat optional props (`progress`, `ringCount`, `strokeWidth`, …). Material parameters are one optional group per material (`glow`, `noise`, `pulse`, `gradient`, `color`, `liquid`, `particles`, `holographic`), shared by all five components. Unset props keep the pattern's tuned values. Choices are enums (`SinuaGlow.Blend.additive`; TS: `"additive"`), and booleans become 1 or 0.
- **One prop per catalog path.** Where patterns disagree, the prop takes the union, and each doc comment lists the per-pattern range. Ring `progress` takes a number (arc, gauge, segmented) or one value per ring (`tracking`, up to 4 → `progress0…3`; Swift literal `0.4` or `[0.2, 0.5]`, Kotlin `SinuaNumbers.of(…)`, TS `number | number[]`). The list does not set `ringCount`: set it yourself. `segment` is a list of up to 24 values. On orb, the `orbits` pattern's particle count is `orbitParticles` (catalog path; engine key `particles`), apart from the `particles` material group.
- **Spec path.** `spec` + `state` (the spec's lifecycle state) + `inputs` / `voiceLevelInput`. The spec's `object` must match the component. Otherwise the component draws nothing and calls `onError` (native, without an `onError`: a debug assertion on iOS, a logged error on Android; RN logs with `console.error`). An unreadable spec is passed through for FxView to report.
- **Common options.** `voice`, `voiceOverrides`, `speed`, `theme`, `paused`, `reducedMotion`, `maxFps`, `lowPower`, `onFrame` and the accessible label all pass through to FxView.
- **The pure mapping is public**, for tests and custom hosts: Swift `SinuaRing(…).overrides()`, Kotlin `SinuaRingProps(…).toOverrides()`, TS `sinuaRingOverrides(pattern, params)`.

**Where they live.**
- Swift: `packages/ios/Sources/Sinua/Generated/`, in the Sinua module.
- Kotlin: `packages/android/view/src/main/kotlin/dev/sinua/view/generated/` (`dev.sinua.view.generated`).
- React Native: `packages/react-native/src/generated/`, exported from the package index.
- The React and Web Component emitters (`<sinua-orb>` …) live in the same generator.

**The name is config.** The `Sinua` type prefix and the `sinua-` tag prefix exist
only in `scripts/codegen/config.mjs`. To rename, edit that file and run
`node scripts/codegen/generate.mjs`: every type, tag and file name follows, and
files from the old name are removed. The generated files are checked in, and
CI runs `node scripts/codegen/generate.mjs --check`, which fails when they are stale.
After a catalog change, regenerate and commit.

**The public API is frozen separately.** `--check` pins the files' *bytes*, so a
catalog edit plus a regeneration passes it by construction, including a renamed
parameter. `node scripts/codegen/api-snapshot.mjs` pins the *contract*: every
public declaration, member, parameter list and pattern id the 28 generated files
expose, on all five outputs, compared against `scripts/codegen/api-snapshot.json`.
Comments, whitespace and default values don't count. Renames, type changes and
parameter order (where calls are positional) do. CI runs it next to `--check`.
An intended API change is `node scripts/codegen/api-snapshot.mjs --update` plus
a review of that JSON diff. `generate.mjs` never writes the snapshot, so
regenerating can't make a breaking change pass. `node --test
'scripts/codegen/test/*.test.mjs'` covers the model (merging, bounds, choices,
renames), the prefix rule, `--check` and the RN mapping. XCTest
`TypedComponentsTests` renders `SinuaRing(…)` pixel-equal to
`FxView(pattern:overrides:)`, and the Kotlin JVM `TypedComponentsTest` checks
`toOverrides()`.

## Web Component — `<sinua-view>` (any framework, or none)

`import "@sinua/web/element"` defines **`<sinua-view>`**, a standard custom element over `mount()`. It has a canvas in an open shadow root and no painting of its own (`packages/web/src/element.ts`).
- **Names:** FX Spec 1.7. `pattern` is the visual and `state` the app lifecycle key.
- **SSR-safe:** importing on a server touches no `window` / `HTMLElement`, and `defineSinuaViewElement()` is a no-op there (`test/element.test.mjs`).

```html
<sinua-view pattern="breathing" theme="auto" style="width:160px"></sinua-view>
<script type="module">
  import "@sinua/web/element";
  const fx = document.querySelector("sinua-view");
  fx.spec = spec;                        // object | JSON text | a JSON import
  fx.inputs = { micLevel: 0.6 };          // the spec's bindings
  fx.addEventListener("fxframe", (e) => e.detail.dtMs);
</script>
```

- **Attributes** (scalars): `pattern`, `state`, `size`, `speed`, `theme`, `paused`, `reduced-motion`, `max-fps`, `low-power`, `label`, `voice-level-input`, `cross-fade`, and `spec` (JSON text).
- **Properties:** the same names camelCased, plus `spec`, `inputs`, `overrides`, `voice` (objects). `handle` / `voiceOverrides` are read-only.
- **Events:** `fxframe` (FxFrameStats, dispatched only while a listener is attached) and `fxerror` (diagnostics). Both bubble and are composed.
- **Lifecycle:** connect → `mount`, disconnect → `destroy`. Property changes batch into one `update()` per microtask.

| Framework | Setup | Objects / events | Types |
|---|---|---|---|
| Plain HTML | a module script | `el.spec = …`, `addEventListener("fxframe")` | `HTMLElementTagNameMap` (built in) |
| Vue 3 | `vue({ template: { compilerOptions: { isCustomElement: (t) => t === "sinua-view" } } })` | `:spec="spec"` (a property, since the element has it), `@fxframe` | `import type {} from "@sinua/web/types/vue"` |
| Svelte 5 | none | `{spec}`, `onfxframe={…}` | `import type {} from "@sinua/web/types/svelte"` |
| Solid | none | `prop:spec={spec}`, `on:fxframe={…}` | `import type {} from "@sinua/web/types/solid"` |
| Angular | `schemas: [CUSTOM_ELEMENTS_SCHEMA]` | `[spec]="spec"`, `(fxframe)="…"` | none: the schema turns off template checking for the tag |
| React 19 | none (or use `@sinua/web/react`'s `<FxView/>`) | `spec={spec}`, `onfxframe={…}` | `import type {} from "@sinua/web/types/react"` |

**Verified** (2026-09-19; Angular moved to the Angular CLI on 2026-09-20): minimal apps in `apps/examples/web-component/` for plain HTML, Vue 3, Svelte 5 and Solid (Vite, no wasm plugin) and Angular (standalone, zoneless, `ng build`, 460 kB initial, under the default 500 kB budget). Since 2026-09-20 the engine's wasm is inlined (`platforms/web.md`), so no bundler needs wasm configuration. Verified with Angular CLI 21 (`ng build`, Zone.js and zoneless), webpack 5 without `experiments`, and a plain module script.
- They were built against the packed `@sinua/web` tarball.
- In headless Chrome, each shows: a non-blank canvas, `fxframe` events arriving, the bound `inputs` property changing the frame, the render loop stopping on removal, and no console errors.
- The typings pass `vue-tsc`, `svelte-check` and `tsc` (Solid, React 19), and reject wrong values (`size={99}`, `theme="purple"`).

## React Native — `<FxView>` (a Fabric component over the native FxViews)

Built 2026-09-19 (studio-ui-ux, native-list item 3). It went the native-view
route recommended here, not react-native-skia. `@sinua/react-native`
exports `FxView`, a **Fabric native component** (Codegen spec
`src/specs/SinuaNativeComponent.ts`, `codegenConfig` in `package.json`)
that hosts each platform's own `FxView` unchanged. The frame loop, painter,
voice, power policy and accessibility are exactly the native ones, and no
per-frame traffic crosses to JS.

```tsx
import { FxView } from "@sinua/react-native";

<FxView spec={voiceAssistantSpec} state="listening" inputs={{ micLevel: 0.6 }}
        theme="auto" reducedMotion="auto" style={{ width: 160, height: 160 }}
        onFrame={({ dtMs }) => {}} />   // or pattern="speaking" size={64} overrides={{…}} speed={1}
```

**Props:** `spec` (JSON text or object) or `pattern` / `size` / `overrides` /
`speed`; `state` (the spec's lifecycle state), `inputs`, `voiceLevelInput`, `crossFade`, `theme`,
`paused`, `reducedMotion`, `maxFps` (0 = none), `lowPower`,
`accessibilityLabel`, and `voice` ("none" | "test" | "mic", the native test
tone or mic; the app must hold mic permission for "mic"). These are the
table above; maps cross as JSON text because Codegen has no free-form map
type. `onFrame` reports `{dtMs, computeMs, paintMs}`, throttled natively to
at most 4 events a second, and only while a handler is set.

**How it's built:**
- **iOS:** an Obj-C++ `SinuaComponentView` (no public header: it pulls
  in C++ React headers, and the pod's umbrella header is also read by the
  Swift half) → `FxHostView.swift`, a `UIHostingController` over the SwiftUI
  `FxView`. Props sit in an `ObservableObject`, so an update re-renders in
  place without a remount.
  - SwiftUI's `scenePhase` is never `.active` inside a scene-less UIKit app
    like React Native's. FxView only animates while it's active, so the host
    feeds `scenePhase` from `UIApplication`'s active/background notifications.
    *Since 2026-09-19 FxView does this itself* (see *Hosting the SwiftUI
    FxView in UIKit*), so this workaround is now redundant; removing it is
    studio-ui-ux's call.
- **Android:** `SinuaManager` (a `SimpleViewManager` + the Codegen
  delegate) → `FxHostView`, a `FrameLayout` with a `ComposeView` rendering
  the Compose `FxView`, with props in Compose state.
- **Packaging:** `build.sh` copies `Sinua` + `SinuaVoice` + `SinuaVoiceTypes`
  (packages/ios) and `dev.sinua.view` + `dev.sinua.voice`
  (packages/android) into the package, as it already did with the engine
  bindings.
  - iOS flattens them into the pod's one Swift module (the
    `import CoreEngine` / `import SinuaVoice` / `(@_exported) import
    SinuaVoiceTypes` lines are stripped).
  - `COPY_ONLY=1` skips re-running the Rust builds.
  - Metro compiles the TS sources (`"react-native": "src/index.ts"`), because
    the Codegen babel plugin needs the spec's types.
- **Consumer setup:**
  - Android: the app's buildscript needs
    `org.jetbrains.kotlin:compose-compiler-gradle-plugin` at its Kotlin
    version (the library applies `org.jetbrains.kotlin.plugin.compose`).
  - iOS: the pod uses `install_modules_dependencies`, and needs iOS 15+.

### Voice sources in React Native

`createVoiceSource` makes one of the native sources (the same code iOS and
Android ship) and hands back a handle. The app owns it: it connects it from a
user action, reads its state and releases it; a view only binds it.

```tsx
const voice = createVoiceSource({ vendor: "gemini", credential: tokenFromMyBackend });
voice.onStateChange(setAgentState);
voice.onError(console.error);
<SinuaOrb pattern="speaking" voice={voice} />;
await voice.connect();   // from a button press
```

- **Vendors:** `{ vendor: "livekit", url, token }`, `{ vendor: "openai", getCredential }` (or `credential`), `{ vendor: "gemini", credential }`, `{ vendor: "elevenlabs", credential }`, `{ vendor: "mic" }`, `{ vendor: "test" }`. The `voice="test" | "mic"` strings still work for the quick case.
- **Credentials** are passed once and live only in the native source: never in props, storage or logs. For OpenAI, `getCredential` is called again on every reconnect, because an `ek_` is single-use.
- **Errors:** iOS rejects `connect()`; Android's `connect()` returns as soon as the socket is opening, so a later failure arrives through `onError`. Handle both.
- **Vendor SDKs are opt-in**, so an app only downloads what it uses:
  - **iOS:** Gemini Live and ElevenLabs are in the default pod (Foundation only). For the others, add to the Podfile
    `source "https://github.com/livekit/podspecs.git"` and `pod "SinuaCore/LiveKit"` / `pod "SinuaCore/OpenAI"`.
    LiveKit's pods need two `post_install` tweaks, which the example Podfile carries: a deployment target of 15, and its public header dir on `HEADER_SEARCH_PATHS`.
  - **Android:** `sinua.voiceVendors=gemini,elevenlabs,livekit,openai` in the app's `gradle.properties` (the default is `gemini,elevenlabs`). Measured: adding LiveKit and OpenAI takes the example's debug APK from 156 MB to 195 MB.
  - A vendor that isn't in the build fails at `create`/`connect` with the line to add.
- **The mic still needs the platform's permission** (`NSMicrophoneUsageDescription`, `RECORD_AUDIO`), requested by your app.

**Verified** (example app, RN 0.87.1, New Architecture, Release builds, iOS
simulator + Android emulator):
- The static cells use `reducedMotion="always"`: FxView's frame at engine
  t = 0.6, which on the spec path is elapsed 0.6 / presetSpeed. They were
  compared against `@sinua/web` `drawFrame` of the same input.
- The live cell animates, pauses from JS through a prop update without a
  remount, and delivers `onFrame` at about 4/s.
- The existing module smoke test still passes.
- Voice (2026-09-20, Android emulator, `-no-audio`, no mic granted): the example's
  self-test (`example/voiceSelfTest.ts`) passes 4/4 in both vendor configurations —
  the test tone's states reach JS, a dead ElevenLabs endpoint surfaces as an `error`
  event, an absent vendor explains how to add it, and a released handle is unusable.
  On iOS both pod configurations build (with and without the LiveKit/OpenAI subspecs);
  the example *app* link currently fails inside React Native's own prebuilt Hermes,
  which is unrelated to this package.
