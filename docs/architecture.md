# Architecture

## Why this exists

Sinua is a shared, cross-platform visual-effects engine: one Rust core, consumed identically from Web, iOS, Android, and React Native. The founding decision (see the project's early design discussion) was that **the value isn't raw performance — it's eliminating drift**. If four platforms each hand-maintain their own port of the same animation math, they diverge: a bug fix lands on one, a "feel" tweak lands on another, and eventually no two platforms render the same thing for the same state. Putting the math in one place, compiled natively everywhere, makes that structurally impossible instead of a discipline problem.

This is why Rust was chosen over "just write it in TypeScript and run that everywhere": TypeScript only runs natively on Web. Native (Swift/Kotlin) and React Native need real compiled binaries, and Rust is the one language that compiles cleanly to all of iOS, Android, and WebAssembly from a single source tree — see [`development.md`](development.md) for the toolchain that makes that true in practice.

**Note on the founding document's framing**: the original plan justified Rust primarily on JS single-thread/GC performance at "1,000–50,000 particle" scale. The engine actually built renders 24–600 dots per frame — well under a scale where that would matter — so that specific justification doesn't hold up in practice. The drift-elimination argument above is the one that actually mattered once real decisions had to be made (it's what motivated the golden-vector test suite, UniFFI, and every "keep this a faithful transcription" note in [`engine.md`](engine.md)); treat it as the real reason, not the performance framing.

## Repo layout

```
Sinua/
├── crates/
│   ├── core_engine/       # The engine itself -- see engine.md, signal.md
│   │   └── src/
│   │       ├── primitives.rs  # Family-agnostic: Dot/Line/OrbFrame, noise,
│   │       │                   # apply_pointer/apply_audio_reactive,
│   │       │                   # mute/interrupt cues, materials
│   │       │                   # (glow/noise/gradient) -- materials.md
│   │       ├── orbs/           # First family -- see engine.md
│   │       └── signal/         # Second family -- see signal.md
│   └── uniffi-bindgen/    # CLI that generates Swift/Kotlin bindings from core_engine
├── packages/
│   ├── core/              # TypeScript/wasm bridge  -- platforms/web.md
│   ├── ios/                # Swift Package           -- platforms/ios.md
│   ├── android/            # Gradle library module   -- platforms/android.md
│   └── react-native/       # RN native module bridge -- platforms/react-native.md
├── apps/
│   ├── site/                 # the documentation site
│   └── examples/             # <sinua-view> in HTML, Vue, Svelte, Solid, Angular
├── spec/
│   ├── orbs-spec.json        # Vendored from upstream: presets, ranges, paint contract
│   └── orbs-golden.json      # Vendored from upstream: frozen per-frame test vectors
├── third_party/
│   └── thinking-orbs/LICENSE # MIT attribution for the vendored engine (see engine.md)
└── docs/                     # You are here
```

## The "family" pattern

`crates/core_engine` holds more than one shape family: `orbs` (ported from `thinking-orbs`, see [`engine.md`](engine.md)), `signal` (a linear/dockable audio-reactive family, see [`signal.md`](signal.md)), `ring` (a flat circular progress/activity indicator family, see [`ring.md`](ring.md)) `beacon` (discrete-event attention cues, see [`beacon.md`](beacon.md)) and `core` (inline "generating…" indicators, see [`core.md`](core.md); Rust module `core_fx`, since a module literally named `core` would shadow the `core` crate) — the full Orb/Core/Ring/Signal/Beacon set from `docs/gpt.md` (`Orbit` remains deferred). `lib.rs`'s `resolve_any` tries each family's own `resolve_preset` in that order, so state names must be unique across families. The same hub rule kept applying as families arrived: the arc sampling and cubic-bezier easing that `ring` wrote first moved into `primitives.rs` the moment `beacon` needed them, rather than `beacon` importing from `ring`.

The module boundary was originally drawn assuming a second family would need **its own** primitives entirely (a border-beam gradient has nothing in common with an orb's dot cloud) — that held for the families evaluated and deferred (see [`engine.md`](engine.md#other-families-considered)), but `signal` turned out to be a case where the *same* `Dot`/`Line` primitives genuinely fit (a linear bar layout is still just points and lines, arranged differently). Rather than have `signal` import from `orbs::core` — which would make `orbs` (the *first* family, not a shared-code root) a false dependency hub as more families pile on — the actually-family-agnostic pieces (`Dot`, `Line`, `Point`/`Polyline`, `OrbFrame`, `finalize_frame`, the noise functions, `lerp`/`lerp_hue`, and the mode-agnostic `apply_pointer`/`apply_audio_reactive` post-processes) were pulled into a neutral `crates/core_engine/src/primitives.rs`, which both `orbs` and `signal` import from as siblings. (`Polyline` — one continuous round-capped stroke — is the one primitive that isn't part of the `thinking-orbs` port at all; it was added for `signal` and no `orbs` mode emits it, see [`signal.md`](signal.md#the-polyline-primitive-why-signal-needed-one).) What's genuinely `orbs`-specific (`fib_dir`, the Fibonacci *sphere* lattice; `Proj`, the sphere yaw/tilt/orthographic projection) stayed in `orbs::core` — `signal`'s flat layout uses neither. This extraction was done while only two families existed specifically because it's cheap now and would only get more expensive (more files importing from the wrong place) as `Core`/`Ring`/`Beacon` are added later.

- The Web Studio's UI shell (`App.tsx`) already renders every family's panel hidden-not-unmounted with a `visible` prop that pauses its animation loop, in a top-level `FAMILIES` array — adding `signal` was one more array entry and one more `<FamilyStudio visible={...} />`, not a shell restructure. Each family still gets its own Studio panel component (`OrbStudio.tsx`, `SignalStudio.tsx`) rather than one shared one with conditionals, since each family's controls (per-state knobs vs. a bar-style picker) are genuinely different shapes of UI.
- The engine's public functions (`frame`, `frame_with_overrides`, `resolved_opts`) are **shared across every family** through one dispatch (`lib.rs`'s `resolve_any`/`render`), not duplicated per family — a state string resolves against `orbs::presets` first, then `signal::presets`, so callers (Web/iOS/Android/RN) never need a second function name for a second family. Each platform package still exports one set of bindings covering all families, not one per family.

## The UniFFI + wasm-bindgen split

Every function in `crates/core_engine/src/lib.rs` is exported twice, through two completely different mechanisms, gated by `#[cfg(target_arch = "wasm32")]`:

- **Native (iOS/Android/RN)**: `#[uniffi::export]`. [UniFFI](https://mozilla.github.io/uniffi-rs/) generates the Swift and Kotlin binding code (a Swift/Kotlin file plus, for Swift, a C header) from the Rust source via reflection over a compiled dylib — see `crates/uniffi-bindgen`.
- **Web**: `#[wasm_bindgen]`, inside a `mod wasm { ... }` block that only compiles for `wasm32-unknown-unknown`. wasm-bindgen and UniFFI's scaffolding are mutually incompatible in the same compilation (UniFFI's scaffolding doesn't build for wasm32 at all), so the `uniffi` crate itself is a **target-specific dependency** — it is not present in the wasm32 dependency graph, and `wasm-bindgen`/`serde` are not present in the native one. See `crates/core_engine/Cargo.toml`'s `[target.'cfg(...)'.dependencies]` split.

This means the two paths carry data differently:
- Native functions take/return real typed values (`HashMap<String, f64>`, UniFFI `Record` structs) across the FFI boundary.
- The wasm path is JSON-over-the-wire: every function has a `..._json` counterpart that serializes its result with `serde_json` and returns a `String`. This was a deliberate simplicity-over-performance tradeoff (see [`engine.md`](engine.md#api-surface)) — revisit only if a real perf ceiling shows up, not preemptively.

## Cross-platform parity proof, not assertion

Every platform's binding is tested against the **same frozen data**: `spec/orbs-golden.json`, vendored from upstream and never regenerated locally. `crates/core_engine/tests/golden.rs` checks the Rust engine against it directly; `packages/ios`, `packages/android`, and `packages/react-native` each carry a thinner "does the FFI round-trip correctly" test that checks specific golden values through their respective bridges (real XCTest / instrumented JUnit / on-device smoke test runs, not mocks — see [`testing.md`](testing.md)). The claim "all four platforms render identically" is backed by this test suite, not by code review of four separately-written ports.
