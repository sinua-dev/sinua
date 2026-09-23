# Sinua docs

Sinua is a shared, cross-platform visual-effects engine: one Rust core (`crates/core_engine`), consumed identically from Web, iOS, Android, and React Native — proven identical by testing all four against the same frozen data, not by code review.

This is the engineering documentation. The repository's front door — what Sinua is, the licence, and how to build from source — is the root [README.md](../README.md).



## Start here

- **[architecture.md](architecture.md)** — why Rust, the repo layout, the "family" pattern (one engine, room for more than `orbs`), the UniFFI/wasm-bindgen split every platform binding is built on.
- **[engine.md](engine.md)** — the `orbs` engine itself: provenance (ported from `thinking-orbs`, MIT), the 9 states/modes, the presets/opts system, the public API, and the one integration detail every caller needs to get right (`t * speed`).
- **[signal.md](signal.md)** — the `signal` family: a sibling to `orbs`, not a mode inside it — a linear, dockable audio-reactive strip (`bar`/`waveform`/`scroll`/`matrix` styles) for voice-AI product surfaces.
- **[ring.md](ring.md)** — the `ring` family: a flat circular progress ring (`completing`, driven by a generic `progress` opt) an indeterminate activity spinner (`loading`), concentric multi-value rings (`tracking`, generic — not Apple's Activity rings), segmented (`stepping`) and gauge (`measuring`) — built from Material/Wear M3 source and deliberately not voice-specific.
- **[beacon.md](beacon.md)** — the `beacon` family: discrete-event attention cues — a notification ping (`notifying`), a pulsing connection-quality dot (`reconnecting`), a location halo (`locating`), a radar sweep (`scanning`), a `((•))` broadcast glyph (`broadcasting`) — built from Tailwind's ping/pulse, Material's badge, LiveKit's quality levels and Google Maps' blue dot.
- **[parameters.md](parameters.md)** — the opt-key naming contract: the shared vocabulary every non-ported mode uses (`dotSize`, `period`, `trackOpacity`, `*Count`, `*Amplitude`), the taxonomy categories as knob metadata, the curated Reactive-Inputs binding contract, and the 2026-09-18 old→new rename map. The nine ported orb modes keep upstream's names.
- **[fx-view.md](fx-view.md)** — the drop-in renderer components: `@sinua/web` (`mount(canvas, …)` + optional React `<FxView/>`), SwiftUI `FxView` (`Sinua` product), Compose `FxView` (`:sinua-view`) — one line to draw an FX Spec or a state, optionally driven by a `VoiceSource`; the Studios paint through the same package painters. RN planned.
- **[fx-spec.md](fx-spec.md)** — FX Spec v1: one versioned JSON file per object+state (color as hex or W3C DTCG, gradient, materials, params), parsed, validated and migrated once in Rust and loaded identically on every platform; unknown keys are always reported, never silently ignored.
- **[materials.md](materials.md)** — glow / noise / gradient as family-agnostic **materials**: three more opts-activated post-processes in `primitives.rs` (`apply_glow`, `apply_noise`, `apply_gradient`) any object wears in any state — layered-halo glow, Perlin jitter, a CSS-convention linear color ramp — with the technique sources and the chain order.
- **[core.md](core.md)** — the `core` family: inline "generating…" indicators — a skeleton-shimmer sweep (`generating`) and the chat typing dots (`typing`) — built from react-loading-skeleton's and VS Code's progress-bar CSS; deliberately small-footprint, not another orb.
- **[audio-pipeline.md](audio-pipeline.md)** — the voice-reactive audio pipeline: the `VoiceSource` interface, why transport diversity (WebRTC/WebSocket/HTTP2) needs one adapter per transport shape, and the real `AudioAnalysis` extraction math (log-spaced bands, smoothing, a real mic-mode bug and its fix).

## Platforms

- **[platforms/web.md](platforms/web.md)** — `packages/core` (the TS/wasm package) — `apps/smoke` was removed 2026-09-18.
- **[platforms/ios.md](platforms/ios.md)** — `packages/ios`, the Swift Package.
- **[platforms/android.md](platforms/android.md)** — `packages/android`, the Gradle library module.
- **[platforms/react-native.md](platforms/react-native.md)** — `packages/react-native`, the classic-bridge module reusing the iOS/Android native binaries directly (no wasm, no reimplementation).

## Product direction and research

- **[effects-research.md](effects-research.md)** — research dump of math/shapes for new modes and families; most of its "new `orbs` modes" were built as the additive states.



## Working on this repo

- **[development.md](development.md)** — toolchain setup, in dependency order, for each platform.
- **[testing.md](testing.md)** — the layered testing approach (prove the engine once in Rust, prove each bridge thinly against it), and two real cross-language comparison bugs found while building that layer.
- **[bench.md](bench.md)** — the real-device bench harness: shared cases in `spec/bench/`, bench apps for Web / iOS / Android (`apps/fx-bench-*`), one result schema, and `scripts/bench/report.mjs` (tables, compare, cost badge vs measured). Simulator/emulator/desktop runs are not device numbers.
- **[lessons-learned.md](lessons-learned.md)** — a scannable list of everything that went wrong (or almost did), with pointers to the full story. Check here first before re-debugging something that smells familiar.
- **[publishing.md](publishing.md)** — how to release (one `VERSION`, `scripts/release/build.sh`, the tag-triggered `release.yml` and its secrets), what a consumer gets from each package, what is still missing, and the release-day order.

## Attribution

Sinua is licensed under Apache-2.0 (root [`LICENSE`](../LICENSE) and [`NOTICE`](../NOTICE)).

9 of the `orbs` family's 18 states (working, searching, solving, listening, connecting, weaving, composing, breathing, shaping), together with the shared sphere helpers in `orbs::core` (projection, Fibonacci lattice, value noise), are a line-by-line Rust port of [`thinking-orbs`](https://github.com/Jakubantalik/Libraries.dev) by Jakub Antalik (MIT). The other 25 states across `orbs`, `signal`, `ring`, `core` and `beacon` are Sinua's own. The MIT text is in `third_party/thinking-orbs/LICENSE` and ships as `THIRD_PARTY_LICENSES` in every package and app that contains the compiled engine. See [`engine.md`](engine.md#provenance) for what "port" means here and how correctness is proven against it.

**[sources.md](sources.md)** has the full list of everything pulled from that upstream repo — what was actually vendored under MIT vs. what was only read as an architecture reference (with a licensing caveat on the latter that's worth reading before assuming it extends further than it does).
