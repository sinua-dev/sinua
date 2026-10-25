# Parameters: the opt-key vocabulary and taxonomy

Every mode reads its tunables from one flat `opts` map (`string → f64`,
see [`engine.md`](engine.md#presets-and-the-opts-system)). This page is the
naming contract for those keys: a **shared vocabulary** so the same
concept has the same name in every family, and the **category** each key
belongs to in the product's parameter plan
(section 2: Appearance / Motion / Energy / Material / State / Reactive
Inputs). The category is *metadata*, not part of the key: a key is
`dotSize`, never `appearanceDotSize`. The Studio carries the category in
its knob registries (`group` on every orb knob in
the Web Studio's `families/orb/knobs.ts`, `*_KNOB_GROUPS` tables in the
other families', all typed by `families/knobGroups.ts`) and groups
sliders by it; the native Studios mirror those tables. An
[FX Spec](fx-spec.md) file's `params` section accepts exactly these
per-state keys; the loader rejects runtime inputs and section-owned keys there.

## The one exception: the nine ported modes keep upstream's names

`orbits`, `globe`, `rubik`, `wave`, `web`, `braid`, `ribbon`/`ring` and
`morph` are line-by-line ports of `thinking-orbs` (see
[`engine.md`](engine.md#provenance)), and their opt names (`orbitN`,
`ghostA`, `scanMul`, `dimBase`, `thr`, `lineW`, `nodeN`, `nodeR`, `rBase`,
`lanes`, `segs`, …) mirror upstream's source verbatim. They are **not**
renamed — not because a rename would break anything (the golden vectors
test rendered output, not key strings) but because that fidelity is the
point of a port. `webflow` (`drifting`) is `web`'s deliberate twin and
shares its opts space and Studio knobs, so it keeps `web`'s names too.
Everything else — the eight other additive orb modes, `signal`,
the post-processes and the materials — uses the
vocabulary below.

## Vocabulary rules

| Concept | Rule | Examples |
|---|---|---|
| a length as a fraction of `size` | name the thing, drop `Frac`; "relative to `size` unless noted" is the unit convention | `dotSize`, `strokeWidth`, `barWidth`, `minHeight`, `thickness`, `length`, `spacing`, `gap` |
| an alpha | `*Opacity` | `trackOpacity`, `haloOpacity` |
| a count | `*Count` | `barCount`, `pointCount`, `layerCount`, `dotCount`, `ringCount`, `nodeCount`, `starCount`, `echoCount` |
| an amplitude | `*Amplitude` | `amplitude`, `pulseAmplitude`, `driftAmplitude`, `bounceAmplitude`, `noiseAmplitude` |
| a repeating duration (seconds) | `period` / `*Period` | `period`, `interruptDuration` is the one-shot form |
| a one-shot duration (seconds) | `*Duration` | `holdDuration`, `interruptDuration` |
| a rate | `*Speed` | `surfaceSpeed`, `warpSpeed`, `jumpSpeed`, `hueSpeed`, `noiseSpeed` |
| a fade-out | `decay` / `*Width` for a spatial fade | `decay` (warp trail), `fadeWidth` (scroll's left edge) |
| color | `hue` (degrees), `saturation` (`0..1`); `white` is the engine's ink value, not an opt | every family |
| a reactive input | the plain noun, no prefix | `progress`, `level`, `audioLevel` |

Booleans are `0`/`1` (`once`, `muted`). Indexed inputs are `prefixN`
(`audioBand0..15`, `history0..N`).

`voiceStateProfile(pattern, state)` returns the keys the **voice-state
profile** sets for an agent state on that pattern (`speed`, the overrides,
and which app input drives `audioLevel`) -- the same values the FX Spec
resolver fills into a 1.8+ file. The data is `spec/voice-state-profile.json`,
versioned by `profileVersion` in this catalog.

## Categories

- **Appearance** — what it looks like at rest: color, opacity, size,
  density. `hue`, `saturation`, `*Opacity`, `*Size`, `*Width`, `*Count`,
  `dim`, and `ink` (1.8: the whole visual's opacity, `0..1`, default `1`,
  applied after glow — see [`fx-spec.md`](fx-spec.md#v18-ink-and-the-voice-state-direction)).
- **Motion** — how it moves: speed, amplitude, direction, timing.
  `period`, `*Speed`, `amplitude`, `*Amplitude` (positional), `delay`,
  `holdDuration`, `yaw`.
- **Energy** — how strongly it "lives": intensity, pulse, decay.
  `pulseAmplitude`, `echoCount`/`echoSpacing`, `ringCount` (sonar rings),
  `decay`, `fadeWidth`, `signals`; and the generic post-process envelopes
  `pulse*` (`apply_pulse`) and `decay*` (`apply_decay`) — see
  [`engine.md`](engine.md#pulse-and-decay-shared-energystate-envelopes).
  Not to be confused with same-stem *mode* keys: `hush`'s
  `pulseAmplitude` and `warp`'s `decay` are that mode's own.
- **Material** — the post-process materials any object wears
  ([`materials.md`](materials.md)): `glow*`, `noise*`, `gradient*`; plus a
  mode's own surface noise (`surfaceScale`).
- **State** — not opts: the state name itself (`frame("listening", …)`);
  `muted` is the one state-like opt (a cue flag).
- **Reactive Inputs** — see the contract below.

## Reactive Inputs: the public binding contract

A developer may compute *anything* as an input — step count, heart rate,
a download's progress — and pass it under any key an existing mode reads;
the raw opts map is always available as an escape hatch. The **curated**
set below is different: these are the keys we promise to keep supporting
as bind targets, the same way the voice pipeline limits itself to a fixed
`AgentState` vocabulary ([`audio-pipeline.md`](audio-pipeline.md)). A
small, stable target list is what lets the Studio build UI around
bindings, keeps exported snippets correct, and avoids a naive generic
binder producing bad results from opts with real interdependencies
(`glowRadius` without `glowStrength` does nothing, for instance).

| Target key | Range | Drives | Read by |
|---|---|---|---|
| `progress` | `0..1` | `dune`'s done part and plane of light / eclipse phase | `dune`, orb `progressing` |
| `level` | `0..1` | how full the volume is (a battery, a quota, a goal, or a swell) | `veil`, `crystal`, `comet`, `flow`, `cohesion`, `scan` |
| `quality` | `0..1` | how whole it holds (1 = whole, 0 = fallen apart) | `cohesion` |
| `audioLevel` (+ `audioStrength`, `-1..1`: positive swells outward, negative draws inward — 1.8's listening "inhale") | `0..1` | the radial breathing pulse on any state, including dot-less frames | `primitives::apply_audio_reactive` — input-agnostic under the hood: a fitness `energy` metric can drive it today by writing to `audioLevel`; a generalized key name is a possible future rename (see the log) |
| `audioBand0..15` + `audioBandCount` | `0..1` | per-band levels | `spectrum`, `crystal` |
| `voiceStateCode` | enum | the voice lifecycle; the light volumes count a bound voice only while it listens or speaks | `veil`, `crystal`, `dune`, `comet`, `flow`, `cohesion`, `scan` |
| `pointerX`/`pointerY`/`pointerRadius`/`pointerStrength` | engine units | pointer / touch scatter on dots, line endpoints and polyline vertices, so every family responds | `apply_pointer` |
| `interruptAge` (+ `interruptDuration`/`Strength`/`Tint`/`Hue`) | seconds | the one-shot barge-in flash | `apply_interrupt` |
| `muted` (+ `mutedTint`/`mutedHue`) | `0..1` | the mute / connection-lost cue | `apply_muted` |
| `pulseStrength` (+ `pulsePeriod`/`Opacity`/`Scale`/`Phase`) | `0..1` | a periodic pulse / breathe on any state | `apply_pulse` |
| `decayAge` (+ `decayDuration`/`Curve`/`Scale`) | seconds | a one-shot fade-out after an event | `apply_decay` |
| `glowStrength`, `noiseStrength`, `gradientStrength` | `0..1` | the three materials' master amounts | [`materials.md`](materials.md) |
| `colorMix` | `0..1` | how far the frame moves to the picked colour | `apply_color` |

These reactive/cue keys were deliberately **not** renamed in the retrofit:
they are the contract shared with the voice adapters (`VoiceOverrides`),
the native Studios' live overrides and every sample snippet.

### Binding a value: `bindReactiveInput` (`packages/core`)

`packages/core/src/reactive.ts` turns *any* app value into one of the
reactive targets above, already in range and with its companions:

```ts
import { bindReactiveInput, ReactiveBinding, frameWithOverrides } from "@sinua/core";

// Steps -> dune's progress (10k fills it), eased out.
frameWithOverrides("dune", 64, t,
  bindReactiveInput({ value: steps, target: "progress", input: [0, 10_000], curve: "easeOut" }));

// Heart rate -> the breathing swell on any state; emits audioStrength too.
const hr = new ReactiveBinding({ target: "audioLevel", input: [60, 180] }, { easeRate: 4 });
sensor.on("bpm", (bpm) => hr.push(bpm));             // 1 Hz is fine
frameWithOverrides("working", 64, t, { ...knobs, ...hr.overrides(dt) });  // per frame
```

- **Targets** (`REACTIVE_TARGETS`, each with its legal range, companions
  and what reads it): `progress`, `level`, `audioLevel`
  (+ `audioStrength` 0.18), `glowStrength`, `noiseStrength`,
  `gradientStrength`, `pulseStrength`, `colorMix`, `muted`.
- **Mapping** follows Motion's `interpolate()`: multi-stop `input`/`output`
  of equal length, input ascending or descending, `curve` one for all or
  one per segment (`linear`, the CSS keywords `ease`/`easeIn`/`easeOut`/
  `easeInOut`, or any `u => u'`). Output defaults to the target's range.
  **Always clamped** — input to its stops, output to the target's range —
  unlike Motion (clamp on by default) or d3 (off by default), because every
  target is bounded and extrapolating past it has no meaning. `NaN` maps
  to the range minimum; an invalid spec throws at construction.
- **Smoothing is caller-side and opt-in** (`ReactiveBinding`'s
  `easeRate`, the same `k = min(1, rate·dt)` easing `VoiceOverrides` uses,
  `dt` clamped to 0.1s) — the engine stays stateless.
- **Not bindable through it, on purpose:** event ages (`interruptAge`,
  `decayAge` — a moment, not a level: stamp the time and pass the age),
  structured feeds (`audioBand*`, `history*`, `voiceStateCode` — use
  `VoiceOverrides`; `pointer*`), and booleans (`once`). The raw
  `frameWithOverrides` opts map remains the escape hatch for everything.
- Tests: `packages/core` `npm test` (`node:test`, incl. an end-to-end check
  through the wasm engine).
- **The same contract in FX Spec files** (v1.1 `bindings`,
  [`fx-spec.md`](fx-spec.md#bindings)): the targets and mapping are ported
  to Rust (`core_engine::reactive`) so a spec binds identically on every
  platform. `spec/reactive-vectors.json` holds the two implementations
  together (table + mapped values): `tests/reactive_vectors.rs` and
  `packages/core/test/reactive-parity.test.mjs`. A change to a target's
  range or companions has to land in both (`reactive.rs`, `reactive.ts`).

## Parameter catalog (the source of truth, 2026-09-19)

Every tunable of every object and pattern lives in one machine-readable catalog. Rust owns it (`crates/core_engine/src/catalog.rs`), and the curated words and ranges sit in `catalog_source.json` next to it. Typed components (`SinuaOrb`, `SinuaSignal`, `<sinua-orb>`), the Studios and the docs' parameter tables are generated from it. The key tables below are a readable overview; **the catalog wins where they differ.**

| | Web (`@sinua/core/dev`) | Swift / Kotlin (UniFFI; development builds only, not in the published artefacts) |
|---|---|---|
| The catalog | `parameterCatalog()`, typed (`ParameterCatalog`) | `parameterCatalogJson()`: JSON text, one shape everywhere |
| Validate overrides | `checkOverrides(pattern, size, overrides)` | `checkOverrides(state:size:overrides:)` (`state` = the pattern id in the low-level API) |
| Checked-in copy | [`spec/parameters.json`](../spec/parameters.json) | same file |

**The words stay out of the runtime** (1.12): `parameterCatalog()` /
`parameterCatalogJson()` carry no `description` fields. The runtime embeds `catalog_source.json`
without its prose and minified (`crates/core_engine/build.rs`; the voice-state profile is minified
too), which keeps ~10 KB (gzip) out of every app. The descriptions are in `spec/parameters.json`
(written by a test from the full source, so it is unchanged), in the typed components' doc comments
(codegen reads that file, so an IDE shows them on hover) and on sinua.dev.

**Shape** (`catalogVersion` 1):
- **`objects[]`:** `{ id, label, component, patterns[] }`. A **pattern** is the visual (`breathing`, `crystal`, …). The app lifecycle key (`listening`, …) is a **state** (FX Spec `states`).
- **`patterns[]`:** `{ id, label, mode, speed, sizes, params: [{ ref, default: { "20", "32", "64" } }] }`.
  - Defaults come from the engine's resolved preset per size, else the definition's `fallback`; they're never hand-typed.
- **`definitions`:** keyed `<key>@<scope>`, where scope is the engine mode or `shared`. The same key can mean different things per mode, e.g. `period` or `nodeCount`. Fields:

  | field | meaning |
  |---|---|
  | `key` | the engine opt key |
  | `path` | the one name FX Spec bindings, typed props and docs use: `glow.strength`, `color.mix`, `lanes` |
  | `label`, `description` | one line, docs-ready (`description` only in `spec/parameters.json`, not the runtime catalog) |
  | `category` | appearance / motion / energy / material / input |
  | `group` | **value** = app data that becomes a direct prop (progress, level); **style** = the look |
  | `type` | number / integer / boolean / choice / `number[]` |
  | `min`, `max` | the **valid** range, enforced by `checkOverrides` |
  | `uiMin`, `uiMax`, `step` | the Studio slider range |
  | `unit`, `fallback` | |
  | `tier` | basic = a Studio knob; advanced = engine-only |
  | `choices`, `if` | Storybook-style conditional visibility |
  | `specPath` | where FX Spec writes it |
  | `aliases`, `deprecated` | older names: the live binding API (`bindReactiveInput`, `reactiveTargetKey`) still accepts them; in an FX Spec file they are errors that name the path (the 1.8 floor) |

- **`materials[]`** (glow, noise, pulse, gradient, colour), **`runtimeInputs[]`** (audio level and bands, history, peaks, pointer, interrupt, decay: fed by the SDK) and **`internalKeys`** (derived keys such as `rSizeMul`, accepted but not documented as tunables).
- **Arrays:** `dune`'s `envelope` is the one definition of type `number[]` (the engine keys `envelope0..63`).

**Where it comes from:**
- The curated half was seeded from the three Studios' knob registries: labels, UI ranges, groups and material choices (190 Studio knobs, the `basic` tier).
- Valid ranges and fallbacks were parsed from each mode's own `get(o, key, default).clamp(..)` calls.
- The remaining engine-read keys were written up as the `advanced` tier.
- **The shape follows:**
  - Storybook ArgTypes: control min/max/step, table category / default, conditional `if`;
  - the DTCG Format Module 2025.10: `description`, `deprecated`, and the naming rule (no leading `$`, no `{`, `}`, `.`), checked by a test;
  - Rive's data-binding view models: typed properties a runtime discovers by name.

**Tests:**
- Every key a mode reads has a definition, and every definition is used by a pattern or a material.
- The material definitions equal FX Spec's `material_keys` (path and valid range).
- `min ≤ uiMin ≤ uiMax ≤ max`, `fallback` in range, and choices spanning the range.
- Defaults equal `resolvedOpts`.
- `spec/parameters.json` equals the engine output byte for byte. Regenerate it with `PARAMS_CATALOG_WRITE=1 cargo test -p core_engine --test parameter_catalog -- --ignored`.
- `packages/core`, iOS and Android each parse it and exercise `checkOverrides`.

**`checkOverrides`** returns warnings only; the frame is unchanged, and a test proves it. The warnings:
- **an unknown key**, with a did-you-mean by edit distance or by a shared 4+ letter prefix. The catalog caught a real one: the Studio's orb "Line width" knob writes `lineWidth` on connecting/drifting, where `web` reads `lineW`;
- **a value outside the valid range;**
- **a fractional value** for a whole-number key;
- **a key renamed** in the retrofit.

Live inputs (`audioLevel`, pointer, interrupt, …) and the catalog's internal keys are not flagged.

## Key table by family (post-retrofit names)

Defaults are the engine's own `get(o, key, default)` fallbacks; ranges
are the Studio's slider ranges, not engine limits.

**Orb, additive modes** (the ported nine keep upstream names; `rMin`/`rsPow`
are the shared radius-scale keys every orb mode reads):

| State / mode | Keys |
|---|---|
| `glowing` / `aurora` | `nodeCount`, `nodeSize`, `surfaceScale`, `surfaceSpeed`, `hueOffset`, `hueSpread`, `hueSpeed`, `saturation`, `depthTone` (1 = front dots lighter than back ones, 0 = one tone; size and alpha keep the depth) |
| `speaking` / `spectrum` | `barCount`, `barDotCount`, `jumpSpeed`, `dotSize`, `hue`, `saturation`, `audioBand*` |
| `confirming` / `sonar` | `period`, `ringCount`, `echoCount`, `echoSpacing`, `coreSize` |
| `initializing` / `warp` | `starCount`, `period`, `warpSpeed`, `decay` |
| `calibrating` / `chladni` | `nodeCount`, `nodeSize`, `holdDuration` |
| `progressing` / `eclipse` | `nodeCount`, `nodeSize`, `progress` |
| `muted` / `hush` | `nodeCount`, `nodeSize`, `pulseAmplitude`, `period`, `dim`, `yaw`, `hue`, `saturation` |

`nodeCount`/`nodeSize` are in the size-preset scaling lists
(`COUNT_KEYS`/`RADIUS_KEYS` in `orbs/profiles.rs`) next to `nodeN`/`nodeR`,
so the 64/32/20 presets scale them exactly as before.

**Signal** ([`signal.md`](signal.md)): `veil` — `level`, `churn`,
`layerCount`; `crystal` — `level`, `churn`, `columnCount`; `dune` —
`progress`, `churn`, `envelope0..63`; `comet` — `level`, `churn`;
`flow` — `level`, `churn`; `cohesion` — `quality`, `level`, `churn`;
`scan` — `blips` (0..12), `level`, `churn`.

**Materials** ([`materials.md`](materials.md)): `glowStrength`,
`glowRadius`, `glowLayers`, `glowTint`, `glowHue`; `noiseStrength`,
`noiseAmplitude`, `noiseScale`, `noiseSpeed`, `noiseSeed`;
`gradientStrength`, `gradientAngle`, `gradientHue`, `gradientHue2`,
`gradientHue3`, `gradientMid`, `gradientSaturation`; `colorMix`,
`colorHue`, `colorSaturation`, `colorLightness`, `colorMode` (all
Material group, see [`materials.md`](materials.md#color-one-colour-for-the-whole-frame-design-notes-2026-09-18)).

**Envelopes** ([`engine.md`](engine.md#pulse-and-decay-shared-energystate-envelopes)):
`pulseStrength`, `pulsePeriod`, `pulseOpacity`, `pulseScale`,
`pulsePhase`; `decayAge`, `decayDuration`, `decayCurve` (`0` linear, `1`
quad, `2` exponential, `3` emphasized), `decayScale`.

## The retrofit map (old → new), 2026-09-18

Renamed in one pass across the engine, `packages/core`'s wasm build, the
Web Studio's registries and these docs; the native Studios' mirrors
(`Families.swift` / `Families.kt`) follow in `studio-ui-ux`'s pass. Old
names are simply unknown keys now — the engine ignores them and falls back
to its defaults, so a stale caller degrades silently, it doesn't crash.
[FX Spec](fx-spec.md#legacy-names-migrate-forward) files don't have that
problem: the spec loader applies this map per mode, maps an old name
forward with a "renamed" warning, and reports any other unknown key.

| Family | Old | New |
|---|---|---|
| aurora | `noiseScale`, `flowSpeed`, `nodeN`, `nodeR` | `surfaceScale`, `surfaceSpeed`, `nodeCount`, `nodeSize` |
| spectrum | `bars`, `barDots`, `jumpRate`, `dotR` | `barCount`, `barDotCount`, `jumpSpeed`, `dotSize` |
| sonar | `ringN`, `echoes`, `echoGap`, `coreR` | `ringCount`, `echoCount`, `echoSpacing`, `coreSize` |
| warp | `starN`, `trail` | `starCount`, `decay` |
| chladni | `nodeN`, `nodeR`, `holdTime` | `nodeCount`, `nodeSize`, `holdDuration` |
| eclipse | `nodeN`, `nodeR` | `nodeCount`, `nodeSize` |
| hush | `nodeN`, `nodeR`, `pulseAmp`, `pulsePeriod` | `nodeCount`, `nodeSize`, `pulseAmplitude`, `period` |
| signal | `barWidthFrac`, `minHeightFrac`, `fadeFrac`, `lineWidthFrac`, `amplitudeFrac`, `layers` | `barWidth`, `minHeight`, `fadeWidth`, `lineWidth`, `amplitude`, `layerCount` |
| materials | `noiseAmp` | `noiseAmplitude` |

One rename also removed a real collision: aurora's `noiseScale` no longer
shares a name with the noise *material*'s `noiseScale`. (The ring, beacon and
core families' rows left this map with the families in 0.1.0-beta.12.)
