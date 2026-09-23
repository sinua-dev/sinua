# FX Spec v1

One versioned JSON file for **one object** — in one state (1.0), or across its whole lifecycle with data bindings (1.1) — what the Studio exports, what an app loads, what a designer hands off. Every platform loads it through the same Rust code (`crates/core_engine/src/fx_spec.rs`), so a spec renders identically on Web, iOS, Android and React Native, and a mistake in it is reported instead of silently ignored.

Before FX Spec, a design was a code snippet pasted into four codebases: a typo'd opt key was ignored, and a key renamed by the taxonomy retrofit (`nodeN` → `nodeCount` on `glowing`) went silently inert.

- Schema (editor aid): [`spec/fx-spec-1.schema.json`](../spec/fx-spec-1.schema.json)
- Examples: [`spec/examples/`](../spec/examples/)
- Color vectors: [`spec/fx-color-vectors.json`](../spec/fx-color-vectors.json); binding vectors: [`spec/reactive-vectors.json`](../spec/reactive-vectors.json)
- 1.1 (states + bindings): see [*v1.1: lifecycle states and bindings*](#v11-lifecycle-states-and-bindings) below.

## Shape

```json
{
  "$schema": "../fx-spec-1.schema.json",
  "fxSpec": "1.0",
  "name": "Glowing radar",
  "object": "beacon",
  "state": "scanning",
  "size": 64,
  "speed": 1,
  "color": { "value": "#33cc66", "mix": 0.8, "lightness": 0, "mode": "ink" },
  "gradient": { "stops": ["#6E56CF", "#E54666"], "angle": 90, "strength": 1, "saturation": 0.8, "mid": 0.5, "path": "short" },
  "materials": {
    "glow":  { "strength": 0.6, "radius": 3, "layers": 4, "tint": 0, "hue": 200 },
    "noise": { "strength": 0.3, "amplitude": 0.04, "scale": 2, "speed": 0.4, "seed": 0 },
    "pulse": { "strength": 0.5, "period": 1.2, "opacity": 0.5, "scale": 0.1, "phase": 0 }
  },
  "params": { "blipCount": 4 }
}
```

| Key | Required | Meaning |
|---|---|---|
| `fxSpec` | yes | `"major.minor"`: `"1.0"`, or `"1.1"` for `states`/`bindings` (see *Versioning*) |
| `object` | yes | `orb` \| `signal` \| `ring` \| `beacon` \| `core` — must be the state's family |
| `state` | yes | an engine state (`working`, `metering`, `tracking`, `scanning`, …) |
| `size` | no | `20` \| `32` \| `64`, default `64` |
| `speed` | no | multiplier on the preset's tuned speed, default `1`. Not an opt: `frameFromFxSpec` applies `elapsed × presetSpeed × speed` for you, so the classic `t * speed` mistake ([engine.md](engine.md)) can't happen |
| `color` | no | a color (shorthand for `{ value, mix: 1 }`) or `{ value, mix, lightness, mode }` → the Color system keys ([materials.md](materials.md)) |
| `gradient` | no | 2–3 `stops`, `angle`, `strength` (default `1`), `saturation`, `mid`, `path` `short`\|`long` |
| `materials` | no | `glow` / `noise` / `pulse`, each key → the prefixed engine key (`noise.amplitude` → `noiseAmplitude`) |
| `params` | no | per-state engine opts ([parameters.md](parameters.md)), numbers only |
| `name`, `description`, `$schema` | no | metadata; `$schema` lets editors complete and validate |
| `bindings`, `states` | no | 1.1 — see [below](#v11-lifecycle-states-and-bindings) |

### Colors

A color is a hex string (`"#RRGGBB"`, `"#RGB"`, `#` optional) or a **W3C Design Tokens (DTCG 2025.10) color value**: `{ "colorSpace", "components", "alpha"?, "hex"? }`.

- `srgb` (components 0..1) and `hsl` (`[H 0..360, S 0..100, L 0..100]`) convert exactly.
- Any other DTCG color space (`oklch`, `display-p3`, …) uses its `hex` fallback, with a warning. Without one, it's an error. That leaves room for OKLCH later without breaking files.
- `"none"` as a component means missing; a `none` hue is achromatic.
- `alpha ≠ 1` gives a warning and is ignored: engine colors are opaque, and element alpha comes from the mode.

**Hex → engine keys** is a port of the Studio's `kit/color.ts`, which is itself the CSS Color 4 sample code (`better-rgbToHsl`, `hslToRgb`). It uses the Studio's rounding: `colorHue` = h to 1 dp, `colorSaturation` = s to 3 dp, with JS `Math.round` semantics. The hex's lightness isn't sent; `color.lightness` is the bias. Gradient stops go through the same `unwrapStops` (short or long way round), because the engine's hue ramp doesn't wrap. A spec exported by the Studio therefore renders exactly what the Studio showed. `spec/fx-color-vectors.json` holds this in place (see *Tests*).

## Validation

Diagnostics are `{ severity: "error" | "warning", path, message }`. `path` is a JSON Pointer (RFC 6901), e.g. `/params/orbitn`. `ok` is false if any diagnostic is an error, and `frameFromFxSpec` then returns `null`.

- **Unknown keys are always reported**, with a "did you mean" hint (edit distance ≤ 2): ``unknown key `orbitn` -- did you mean `orbitN`?``.
- **`object` / `state` / `size`**: an unknown state, a state from another family, or a size other than 20/32/64 are errors.
- **`params` allowlist**: the keys the state's mode actually reads, plus the resolved preset's own opts (so the ported orb modes' upstream names work), plus the shared `rMin`/`rsPow`.
  - Indexed design values (`progress0..3` on `tracking`, `segment0..` on `stepping`) are allowed.
  - A unit test re-scans every mode source for `get(o, "…")` so the table can't drift.
- **Runtime inputs are rejected with a reason**: `pointer*`, `audio*`, `voiceStateCode`, `history*`, `interrupt*`, `muted`/`mutedT*`, `decay*` (the fade event; `warp`'s own `decay` opt is fine), `peakN`. These are live data, not design. The bindable ones (`audioLevel`, `muted`, …) go under `bindings` (1.1), and the message says so.
- **Section-owned keys in `params` are errors** that point at the section: `colorHue` → `/color`, `glowStrength` → `/materials/glow`, and so on.
- Numbers are range-checked where the engine has a range (`mix` 0..1, `lightness` −1..1, `mid` 0.05..0.95, …).

### Old names are errors

1.0–1.7 were never published, so the runtime doesn't carry their names forward. The floor (below) removed both kinds of alias before the first release, which kept them from becoming a compatibility promise (docs/release-roadmap.md, decision 0.1):

- **The names FX Spec 1.7 replaced** are errors that say the new name: ``/state``: ``` `state` is `pattern` ```; ``/bindings/glowStrength``: ``` `glowStrength` is `glow.strength` ```. They aren't honoured.
- **The 2026-09-18 retrofit renames** ([parameters.md](parameters.md#the-retrofit-map-old--new-2026-09-18)) are plain unknown keys now. `aurora`'s old `nodeN` is ``unknown key `nodeN` ``. `web`'s own, current `nodeN` is unaffected.

## Versioning

This follows glTF 2.0's `asset.version` rule: `"major.minor"`. A major version may break compatibility. Minor versions must be backward **and** forward compatible.

- Major ≠ 1 → error; this runtime doesn't load it.
- **The floor is 1.8** (`fx_spec.rs`'s `FLOOR_MINOR`). A `1.0`–`1.7` file is one error at `/fxSpec` (``FX Spec 1.7 isn't supported; this runtime reads 1.8 and later``) and nothing resolves. Those minors were never published; their acceptance was dropped before the first release instead of becoming a promise (docs/release-roadmap.md, decision 0.1). A missing or malformed `fxSpec` is an error too, and the rest of the file is still read as the current minor so its other problems show.
- The runtime is **1.8** (`RUNTIME_MINOR`). A file claiming this runtime's minor → unknown keys are **errors**.
- A **newer 1.x** file (`1.9`) → unknown keys are **warnings** and the rest renders (graceful degradation).
- Every supported minor resolves **identically** under a newer runtime: one identity lock per minor (`spec/fx-spec-1.<minor>-resolved.json`) freezes that runtime's output for every example, and a test holds every later runtime to it. Today that is one lock, `spec/fx-spec-1.8-resolved.json`, over all 13 examples; the 1.0–1.7 locks went with the floor. Capture one with `FX_SPEC_LOCK_WRITE=1 cargo test -p core_engine --test fx_spec_lock -- --ignored`, once that minor is stable and before anything using it is published. A missing lock for the current minor fails `the_current_runtimes_lock_is_present_and_still_matches`; a genuine mid-bump window is declared by setting `BUMP_IN_PROGRESS_TO` in `crates/core_engine/tests/fx_spec_lock.rs`, so it is a visible edit rather than an inference from an absent file.
- **A key added in a later minor is gated automatically.** `spec/fx-spec-1.8-keys.json`
  freezes every key path a 1.8 file may use (103 today, built from the resolver's own
  tables). A new key (a material, a section key, a binding target, something low power
  can shed) must get a row in `fx_spec.rs`'s `SINCE` with the minor that adds it.
  `every_key_is_a_1_8_key_or_has_a_since_minor` fails by name until it does, and the
  one gate in `resolve` then turns it into an error in older files ("`materials.frost`
  needs "fxSpec": "1.9" (this file says 1.8)") and drops it. Removing a frozen key fails
  too: 1.8 files would break.
- `migrate` in `fx_spec.rs` reads the 1.7 grammar into engine names (catalog-path binding targets, array params) and reports the replaced names. A later minor that renames something plugs in there.

## API

(1.2 adds `lowPower` to every `…With` call and to `ctx`, and the cost helpers `estimateCost` / `fxSpecCost`. See *v1.2* and [engine.md](engine.md#cost-estimate).)

All platforms are thin wrappers over the same Rust functions:

| Rust (UniFFI, iOS/Android) | Web (`@sinua/core`) / RN (`@sinua/react-native`, async) |
|---|---|
| `resolve_fx_spec(json)` / `resolve_fx_spec_with(json, state, inputs)` → `FxSpecResolved { ok, state, size, speed, overrides, diagnostics, state_key, state_keys, inactive_bindings }` | `resolveFxSpec(spec \| json, ctx?)` |
| `frame_from_fx_spec(json, elapsed)` / `frame_from_fx_spec_with(json, elapsed, state, inputs)` → `Option<OrbFrame>` | `frameFromFxSpec(spec \| json, elapsed, ctx?)` |
| `fx_color_to_hsl(color) -> Option<FxHsl { h, s, l, achromatic, hex }>` | `fxColorToHsl(color)` |

`ctx` is `{ state?: string, inputs?: { [name]: number } }`. Omitting it is the 1.0 call. Swift/Kotlin call the UniFFI functions directly: `resolveFxSpec(json:)`, `resolveFxSpecWith(json:state:inputs:)`, `frameFromFxSpecWith(json:elapsed:state:inputs:)`, `fxColorToHsl(color:)`. The wasm names are `*_json` and take the context as JSON.

```ts
import { frameFromFxSpec, resolveFxSpec } from "@sinua/core";
const r = resolveFxSpec(specJson);
if (!r.ok) console.warn(r.diagnostics);        // [{ severity, path: "/params/…", message }]
const frame = frameFromFxSpec(specJson, performance.now() / 1000);  // speed already applied
```

`resolveFxSpec` returns plain `frameWithOverrides` inputs, so a renderer that already drives `frameWithOverrides` (e.g. to add live Reactive Inputs on top) can use `r.state`, `r.size` and `r.overrides` directly. It must then do the `t × presetSpeed × r.speed` itself.

## Tests

- **Rust:** `fx_spec.rs` unit tests cover resolution, typos with hints, version rules and the 1.8 floor, the replaced names as errors, object/pattern/size, runtime and section keys, DTCG colors, the params-table drift check, schema/validator key agreement, and material keys existing in the engine.
  - Every example must resolve cleanly, and `frame_from_fx_spec` must equal `frame_with_overrides` at the scaled time.
- **Color vectors:** `tests/fx_color_vectors.rs` checks the engine against `spec/fx-color-vectors.json` (50 colors, 14 gradients).
  - Regenerate only with `FX_COLOR_VECTORS_WRITE=1 cargo test -p core_engine --test fx_color_vectors -- --ignored`.
  - `packages/core/test/fx-color-parity.test.mjs` checks the **Studio's own `kit/color.ts`** (imported directly; Node strips the types) and the wasm build against the same file. That test is what keeps "Studio pick = spec render" true.
- **Platform parity:** wasm (`fx-spec.test.mjs`), iOS (`FxSpecTests.swift`, reads `spec/examples` via `#filePath`) and Android (`FxSpecTests.kt`, via the androidTest `spec/` assets) each render every example and compare against `frameWithOverrides`.
  - For state-map files, the wasm, iOS and Android tests render **every `states` key** with the same fixed inputs map.
  - The RN example app runs one spec round trip through the bridge (`runFxSpecCheck`), plus a lifecycle call (state, binding, inactive binding; keyed `talking`, outside the voice-state profile). Its inlined spec is kept equal to `spec/examples/beacon-radar-glow.fxspec.json` by `sinua-checkpoints.test.mjs`.
- **Lifecycle states and bindings (Rust):**
  - The 1.8 identity lock (`v1_8_examples_resolve_identically`).
  - Merge semantics (`null` deletes, arrays replace, entry beats base) and fallback; a `null` keeps the voice-state profile out.
  - Binding targets, ranges and curves; missing inputs; companions; static-vs-bound conflicts.
  - Entry-local diagnostic paths.
- **Binding vectors:** `tests/reactive_vectors.rs` (`REACTIVE_VECTORS_WRITE=1` to regenerate) and `packages/core/test/reactive-parity.test.mjs` hold the Rust mapper and `reactive.ts` to the same file: the targets table plus 24 cases (264 values). `fx-player.test.mjs` covers the caller loop.

The sections from here on record what each minor added. Since the 1.8 floor their **gating** and **identity-lock** notes are history: a file declares 1.8 or later and gets all of it.

## v1.1: lifecycle states and bindings

```json
{ "fxSpec": "1.8", "object": "orb", "pattern": "breathing",
  "color": { "value": "#6E56CF", "mix": 0.7 }, "materials": { "glow": { "strength": 0.25, "radius": 3 } },
  "bindings": { "muted": { "input": "micMuted" } },
  "states": {
    "idle": {},
    "listening": { "pattern": "listening", "bindings": { "audioLevel": { "input": "micLevel", "curve": "easeOut" } } },
    "thinking":  { "pattern": "working", "materials": { "glow": null } },
    "speaking":  { "pattern": "speaking", "speed": 1.1, "materials": { "glow": { "strength": 0.4 } },
                   "bindings": { "audioLevel": { "input": "agentVolume", "inputRange": [0, 0.8] } } } } }
```
(`spec/examples/voice-assistant.fxspec.json`, written in today's names: the **advanced** form, a different pattern per state. The showcase is `voice-assistant-glowing.fxspec.json`, which keeps one pattern -- `glowing` -- and lets the states change how it behaves. `fitness-rings.fxspec.json` is the data-driven one: steps, water and active minutes drive `tracking`'s rings, and heart rate drives the glow.)

**The engine stays stateless.** The caller says which lifecycle state is current and passes its input values on every call. The spec never tracks time, history or transitions.

### `states`
- **Keys are free-form.** The convention is LiveKit's `AgentState` names (`initializing`, `idle`, `listening`, `thinking`, `speaking`), which `VoiceSource` already uses (`voice.ts`).
- **The top level is the base design**, and each entry is merged over it with **RFC 7396 JSON Merge Patch**: objects merge, `null` deletes, arrays (`gradient.stops`, ranges) replace whole. `"materials": { "glow": null }` turns the shared glow off in one state; an empty entry is the base under a name.
- An entry may set `state`, `speed`, `color`, `gradient`, `materials`, `params` and `bindings`. `object` and `size` are file-wide (an error in an entry). An entry's `state` must belong to the file's `object`.
- **Fallback.** With no state passed, the base design renders silently. A key that isn't in `states` also renders the base, with a warning at `/states`. It is never an error, the same way Figma's variable modes fall back to the collection's default mode. A file without `states` ignores a requested state.
- **Validation paths** are the entry's own (`/states/speaking/params/…`). Inherited keys were already checked on the base. An inherited `param` that the entry's mode doesn't read is simply not applied in that state (e.g. `tracking`'s `ringCount` under a `completing` entry). Params meant for one mode belong in that state's entry.

### `bindings`
- **Keyed by target**, so each target is bound once and a state can override or delete (`null`) one: `"audioLevel": { "input": "micLevel", "inputRange": [0, 1], "outputRange": [0.1, 1], "curve": "easeOut" }`.
- **Targets come only from the curated Reactive-Inputs contract** ([parameters.md](parameters.md#reactive-inputs-the-public-binding-contract)): `progress`, `progress0..3`, `quality`, `accuracy`, `audioLevel`, `glowStrength`, `noiseStrength`, `gradientStrength`, `pulseStrength`, `colorMix`, `muted`. Anything else is an error with a hint. Raw opts can't be bound from a spec.
- **Input names are the app's own** (`micLevel`, `steps`, `heartRate`); the engine never interprets them.
- **Mapping:** the same contract as `bindReactiveInput`, following Motion's `interpolate()`:
  - multi-stop `inputRange`/`outputRange` (field names as in RN's `Animated.interpolate`), ascending or descending;
  - one CSS keyword curve for all segments or one per segment (`linear`, `ease`, `easeIn`, `easeOut`, `easeInOut`);
  - an omitted `outputRange` is the target's range, except `progress0..3`: their range is 0..3 (extra laps), but they **default to `[0, 1]`, one lap = the goal**. Laps are opt-in with a multi-stop output, e.g. `"inputRange": [0, 10000, 30000], "outputRange": [0, 1, 3]` (the fitness example's steps ring);
  - always clamped to the target's range; NaN maps to the minimum.
  - Companions ride along: `audioLevel` brings `audioStrength` 0.18 unless the block sets it.
  - It runs in Rust (`core_engine::reactive`), with `spec/reactive-vectors.json` holding it equal to `reactive.ts`.
- **Missing input → inactive.** The target keeps its static or engine value and is listed in `inactiveBindings`; nothing is guessed.
- Warnings:
  - a mode-specific target the state's mode doesn't read (e.g. `quality` on a ring);
  - a target also set statically in the same block (while its input is present, the binding wins). One exemption: a `colorMix` binding next to a bare `color.value` or `color.mix: 0` doesn't warn. That colour is only what the binding mixes toward, and the Studios export exactly this pattern. An explicit non-zero `color.mix` does warn.
- Runtime keys that are bindable (`audioLevel`, `muted`) are rejected in `params` with a pointer to `bindings.<key>`.

### Caller loop
State cross-fades and value easing stay caller-side. `FxSpecPlayer` (`@sinua/core`, `fxPlayer.ts`) packages the recommended loop for the web; native callers follow the same steps:

```ts
const player = new FxSpecPlayer(specJson, { inputEaseRate: { heartRate: 4 } });
agent.onStateChange((s) => player.setState(s));            // "listening", "speaking", ...
mic.onLevel((v) => player.setInput("micLevel", v));
// every rendered frame:
const { frame, previous, blend } = player.frame(elapsedS, dtS, voice.overrides(dtS));
previous ? drawCrossDissolve(ctx, previous, frame, blend) : draw(ctx, frame);
```

1. **On a state change,** keep the previous key and cross-fade its frame into the new one over **250 ms, cubic ease-out**. This is the Studio's fade (`SignalStudio.tsx`), after LiveKit's `0.25s ease-out` state transition. A change mid-fade restarts from the state showing now.
2. **Ease slow inputs** yourself if they jump (`k = min(1, rate·dt)`, dt clamped to 0.1 s, the `ReactiveBinding`/`VoiceOverrides` rule). The first value is taken as-is.
3. **Render** with `resolve…With(state, inputs)`, then `frameWithOverrides(state, size, elapsed × presetSpeed × speed, { ...overrides, ...extra })`. `extra` is for runtime keys the spec doesn't own, such as `VoiceOverrides`' spectrum bands and `voiceStateCode`.
4. Per-state `speed` changes the time scale, so an object's phase can jump when it switches state. The cross-fade hides that. The real point-by-point `frameTransition` exists only for the `glowing`/`calibrating`/`progressing` trio.

Parsing happens on every call. A spec is a few KB of JSON, which is cheap next to rendering.

## v1.2: `performance`

```json
"performance": { "maxFps": 30, "lowPower": { "maxFps": 15, "disable": ["glow", "noise"] } }
```
(`spec/examples/status-beacon-power.fxspec.json`)

- **File-wide.** Top level only; inside a `states` entry it's an error. The block needs `"fxSpec": "1.2"`; in a 1.0 or 1.1 file it's an error and isn't honoured. 1.0 and 1.1 files resolve identically: `spec/fx-spec-1.1-resolved.json` froze the 1.1 runtime's output for every example and state before 1.2 landed, next to the 1.0 lock.
- **`maxFps`** (1..120) is the cap the **host** should pace its render loop to. The engine never paces frames.
- **`lowPower`** says what to do when the **host** reports low power. The spec can't detect that itself:
  - iOS: `ProcessInfo.isLowPowerModeEnabled` + `NSProcessInfoPowerStateDidChange`. Apple's Energy Guide asks apps to "reduce the use of animations" and "lower frame rates" in Low Power Mode.
  - Android: `PowerManager.isPowerSaveMode`.
  - Web: no Baseline battery API (MDN), so it's an app policy.
  - The host passes `ctx.lowPower` (TS/RN) or `lowPower:` (Swift/Kotlin, default `false`).
  - `maxFps` is the lower cap. `disable` is a subset of `glow`, `noise`, `pulse` and `gradient`: their `*Strength` is forced to 0 **after** bindings, so a bound `glowStrength` is shed too, and it isn't reported as an inactive binding.
- **Resolution** adds `maxFps` (the effective cap: `lowPower.maxFps` under low power when set, else `maxFps`, else `null` = the host's default) and `disabledMaterials`.
- **Validation:**
  - unknown keys get hints;
  - a material that can't be shed is an error;
  - a low-power cap above the normal cap is a warning.
- **No block → host policy.** The recommended default when a spec says nothing is 30 fps with glow off under low power, since glow multiplies elements 3.4–3.9× (see *Cost estimate* in [engine.md](engine.md#cost-estimate)). `FxSpecPlayer` takes `lowPower` / `setLowPower(on)`, and the cap is in `resolved.maxFps`.

## v1.3: materials phase 1

- **`materials.glow.mode`**: `"stacked"` (default, concentric copies) or `"blur"` (one Gaussian-blurred halo per element).
- **`materials.glow.blend`**: `"normal"` or `"additive"`. Additive is a dark-theme look.
  - Both map to `glowMode` / `glowBlend`.
- **`performance.lowPower.disable`** also accepts **`"blur"`**. Under low power it sets `blurScale` 0: every blur σ goes to 0, and glow's blur mode falls back to the stacked copies.
- New mode params (plain `params`, checked by the params table): **`highlightFill`** (shimmer: a real gradient band) and **`trailFill`** (radar: a filled wedge).
- **Gating:** glow `mode`/`blend` and shedding `"blur"` need `"fxSpec": "1.3"`; in an older file they're errors and aren't honoured. 1.0/1.1/1.2 files resolve identically: `spec/fx-spec-1.2-resolved.json` froze all 8 older examples × every state × both power states before 1.3 landed.
- **Examples:** `shimmer-gradient.fxspec.json` and `radar-wedge-blur.fxspec.json` (wedge + additive blur glow; low power sheds blur and caps at 20 fps).
- Paint semantics: [engine.md](engine.md#paint-contract-fills-and-effects-materials-phase-1-2026-09-18).

## v1.4: liquid

- **`materials.liquid`** `{ strength, reach, threshold, cells, style: "outline" | "dots" | "fill", spacing, width, keep: bool, blur }` maps to the `liquid*` engine keys. Metaball contours of the frame's dots: [materials.md](materials.md#liquid-metaball-contours-materials-phase-2-2026-09-19). `liquid*` keys in `params` are errors that point here.
- **`performance.lowPower.disable`** accepts **`"liquid"`** (→ `liquidStrength` 0).
- Unset liquid keys take the state's **tuned defaults** (e.g. `working`: reach 4, threshold 0.4). Keys set in `materials.liquid` override them. `liquidSuitability(state)` reports the defaults and a recommended / ok / notRecommended verdict; see [materials.md](materials.md#liquid-metaball-contours-materials-phase-2-2026-09-19).
- **Gating:** both need `"fxSpec": "1.4"`. 1.0–1.3 files resolve identically: `spec/fx-spec-1.3-resolved.json` froze the 10 older examples × states × power before 1.4.
- **Example:** `liquid-orb.fxspec.json`:
  - outline on a glowing orb;
  - `thinking` → soft filled blobs;
  - `speaking` → a filled band with a hole, with the dots kept on top;
  - low power sheds liquid at 24 fps.

## v1.5: particles

- **`materials.particles`** `{ strength, count, size, spread, life, style: "drift" | "attract" | "orbit" | "rise", seed }` → the `particle*` engine keys. It's a stateless particle layer emitted from the state's own geometry: [materials.md](materials.md#particles-a-stateless-particle-layer-materials-phase-3-2026-09-19-reworked-the-same-day). `particle*` keys in `params` point here.
- **`performance.lowPower.disable`** accepts **`"particles"`** (→ `particleStrength` 0).
- **Gating:** both need `"fxSpec": "1.5"`. 1.0–1.4 files resolve identically (`spec/fx-spec-1.4-resolved.json`, 11 examples × states × power, captured before).
- **Example:** `particles-orb.fxspec.json` (drift; listening attracts; speaking drifts with additive blur glow; low power sheds particles and blur).
- **Recommended host low-power default** (when a spec has no `performance` block): 30 fps with `{ glowStrength: 0, particleStrength: 0 }`. Liquid is left to the spec or app, because it replaces dots rather than adding to them.
- No painter change: particles are ordinary dots.
- **Engine rework (same day, applies to 1.5 files too):** particle time is **wall-clock**. The engine divides the state's preset speed back out, so `life` is real seconds; the spec's own `speed` still scales it. There are calmer base defaults (count 28, size 0.8, spread 0.18, life 4.5), and **each state has its own defaults for unset keys** (`particleDefaults(state)`). None of this touches spec resolution: `fx-spec-1.5-resolved.json` still holds.
- **Pulse and noise followed (same day, user go):** `pulse.period` and `noise.speed` are real seconds on every state too. A spec's `speed` multiplies all four material clocks together with the geometry. See [materials.md, *Material time*](materials.md#material-time-2026-09-19).

## v1.6: holographic, particle sync/audio

- **`materials.holographic`** `{ strength, hue, span, saturation, depth, facing, speed }` maps to the `holo*` engine keys: a foil hue sweep by depth, facing and time over each element's kept lightness ([materials.md](materials.md#holographic-a-hue-sweep-over-kept-lightness-materials-phase-4-2026-09-19)). `holo*` keys in `params` point here.
- It's **not sheddable**, because it recolours only: `disable: ["holographic"]` is an error saying there's nothing to shed.
- **`materials.particles`** gains **`sync`** (0..1, 1 = a burst each life) and **`audio`** (0..1, brightness follows the host's `audioLevel` input when present).
- **Gating:** all three need `"fxSpec": "1.6"`. 1.0–1.5 files resolve identically (`spec/fx-spec-1.5-resolved.json`, 12 examples × states × power, captured before).
- **Example:** `holo-orb.fxspec.json` (a composing orb in foil; speaking leans on facing, with a blurred glow; low power sheds the glow and keeps the holographic).

## v1.7: the parameter catalog's names

One name everywhere: a spec, a typed component prop and the docs all use the [parameter catalog](parameters.md#parameter-catalog-the-source-of-truth-2026-09-19)'s `path`s. The orchestrator and the user made this decision on 2026-09-19.

| 1.0–1.6 (an error since the 1.8 floor) | 1.7 |
|---|---|
| `"state"` (the visual, top level and inside `states.*`) | `"pattern"`. The lifecycle key stays the `states` map key, and FxView's `specState` becomes `state` |
| binding target `glowStrength` / `noiseStrength` / `gradientStrength` / `pulseStrength` / `colorMix` | `glow.strength` / `noise.strength` / `gradient.strength` / `pulse.strength` / `color.mix` |
| binding target `progress0` … `progress3` | `progress[0]` … `progress[3]` |
| params `progress0..3` / `segment0..23` | `"progress": [..]` (tracking, 1–4 values) / `"segment": [..]` (stepping, 1–24 values) |

Unchanged: `audioLevel`, `muted`, `quality`, `accuracy`, `progress`, free input names, `inputRange` / `outputRange` / `curve`, `null` deletes, and the LiveKit `AgentState` keys.

**At the time** this was a minor, not 2.0: the old names stayed as aliases (with a deprecation warning in a 1.7 file), and the identity locks proved every older file resolved byte-identically. **Since the 1.8 floor the old names are errors that name the new one** (see *Old names are errors* above), because nothing had been published that would need them.

**Resolver output is unchanged:** `FxSpecResolved.state` is still the pattern id, and binding targets are reported with their engine keys (`inactiveBindings`). TS's `bindReactiveInput` / `reactiveMapper` / `ReactiveBinding` accept either name (`reactiveTargetKey`, `REACTIVE_TARGET_PATHS`) -- that is the live binding API, not a file, so it isn't affected by the floor.

## v1.8: `ink` and the voice-state direction

**`ink`** (top level, and per `states` entry): how present the whole visual is, `0..1`, default `1`. It multiplies the alpha of every dot, line, polyline and fill *after* glow, so a faded visual fades as one thing, halos included. It changes no geometry and no ink value -- opacity only -- which is what keeps it mode-agnostic.

```jsonc
{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing", "ink": 1,
  "states": { "idle": { "ink": 0.8 }, "listening": {} } }
```

It exists because "how present is this right now" is a **state** cue, not a colour choice: a voice assistant rests below full ink when idle and comes to full ink when it starts listening. Callers used to fake it outside the engine (CSS opacity), which no native view and no export could reproduce.

**A signed `audioStrength`** (runtime input, not a file value): positive swells the frame outward as `audioLevel` rises -- an agent *speaking* -- and **negative draws it inward**, the "inhale" that reads as *listening*. Brightness follows the magnitude in both directions, so an inhale is a tightening, not a fade. Its range in the parameter catalog is now `-1..1`. This is the cue that separates listening from speaking: with both swelling outward, they measured the same on every pattern (the voice-state review).

### The voice-state profile

1.8 also ships **the state language itself**. For a `states` key that is one of `initializing`, `idle`, `listening`, `thinking` or `speaking`, the resolver fills in what that state does to that pattern -- tempo, `ink`, the audio direction, particles, glow -- from `spec/voice-state-profile.json`, compiled into the engine (`crates/core_engine/src/voice_state.rs`, `voiceStateProfile(pattern, state)`).

It is **fill-only**: every key the file sets, in the base or in the entry, wins; a `null` in a patch removes the key and opts it out of the profile too -- a whole section's `null` (`"materials": { "glow": null }`) opts out every key it removed. (Until the floor landed the runtime didn't honour that opt-out: a profile could put back what a `null` had removed. No 1.8 example relied on it; `voice-assistant`'s `thinking` entry, with no glow, is the case that exposed it.) So a file patches the language instead of restating it -- `spec/examples/voice-assistant-glowing.fxspec.json` is 51 lines and carries only its identity (the pinned brand colour, the glow, which input drives each state).

```jsonc
// Everything below comes from the profile: idle rests at 0.8 ink and moves
// slower, listening comes to full ink while the mic draws the shape inward.
{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
  "states": { "idle": {}, "listening": { "bindings": { "audioLevel": { "input": "micLevel" } } } } }
```

A state name outside that set (your own `"recording"`) is left exactly as written. Patterns that need their own reading -- `muted` dims itself, `calibrating` is too fine at 64 px -- carry a per-pattern entry in the same file, and what still doesn't read is listed there under `exceptions` with the measurements.

Views use the same profile for plain input (`pattern` + `state`, no file): merge it *under* the app's own overrides. A state's speed multiplier is safe to switch live: the views' clock is phase-continuous (`docs/fx-view.md`), so the pose carries over and runs on at the new rate instead of jumping. A tool that shows "tuned by you" versus "from the profile" compares against `profileVersion` in the parameter catalog.

**When the floor moved the examples to 1.8,** their lifecycle entries keyed by a voice state picked the profile up: the lock rows that changed are exactly those, and every changed value is the profile's own (`audioStrength` included -- the profile runs before a binding's companion default, so speaking's `0.5` stands where the companion would have written `0.18`). Every other row, and every row of `voice-assistant-glowing` (already 1.8), came out byte-identical. `ink` defaults to 1 and no preset sets `audioStrength`, so callers without a file render as before; both golden sets pass unchanged.

## Not in v1.1

- Real OKLCH / wide-gamut conversion (today these use the `hex` fallback).
- Transitions *declared in the spec* (per-pair durations and curves). Today the fade is the caller's fixed 250 ms.
- Multiple objects or a scene per file.
