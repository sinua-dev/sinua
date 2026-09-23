//! Sinua core geometry engine: shared math for every shape family
//! shipped as a single crate so iOS/Android/RN/Web ports stay in sync by
//! construction. `orbs` (ported from `thinking-orbs`,
//! https://github.com/Jakubantalik/Libraries.dev, MIT, Jakub Antalik; see
//! `third_party/thinking-orbs/LICENSE`) is the first family; `signal` (see
//! `signal/mod.rs`) is the second, a sibling, not a mode inside `orbs`.
//! `crate::primitives` holds what both actually share (`Dot`/`Line`/
//! `OrbFrame`, noise, the mode-agnostic pointer/audio post-processes) --
//! see that module's header for why it was pulled out of `orbs::core`
//! rather than left there for `signal` to reach into.
//! `spec/orbs-golden.json` holds the parity vectors `tests/golden.rs`
//! validates the `orbs` family against; `signal` has no golden vector
//! (nothing to port against), same tradeoff as `orbs`' own additive modes.

mod beacon;
mod catalog;
mod core_fx;
mod cost;
mod fx_spec;
mod liquid;
mod orbs;
mod particles;
mod primitives;
pub mod reactive;
mod ring;
mod signal;
mod voice_state;
// Only the wasm bridge packs frames; native tests still cover the layout.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod transport;

use std::collections::HashMap;

pub use cost::FxCost;
pub use fx_spec::{FxDiagnostic, FxHsl, FxSpecResolved};
pub use liquid::LiquidSuitability;
pub use orbs::presets::{resolve_preset, Resolved};
pub use primitives::{
    ColorMode, Dot, EffectRun, Fill, FillGradient, GradientStop, Line, OrbFrame, Point, Polyline,
};
pub use voice_state::{VoiceStateProfile, VOICE_STATES};

#[cfg(not(target_arch = "wasm32"))]
uniffi::setup_scaffolding!();

/// Dispatch a resolved mode to its geometry function. Shared by `frame` and
/// `frame_with_overrides` so the override path can't drift from the stock
/// one -- it's the exact same dispatch, just with a different `opts` map.
/// Covers both families (`orbs` and `signal`) in one place so pointer/
/// audio-reactive post-processing and the single public `frame`/
/// `frame_with_overrides` entry points stay unified rather than duplicated
/// per family. Every mode's output passes through `apply_pointer`/
/// `apply_audio_reactive` afterward, mode-agnostically -- a no-op unless
/// `opts` carries `pointerStrength`/`audioStrength` (no preset sets either,
/// so plain `frame()`/every golden test is unaffected).
///
/// `t` is the mode's clock: callers pass `elapsed * presetSpeed` (times a
/// spec's own `speed`). `preset_speed` undoes the preset part for the
/// post-processes that live in **wall-clock seconds** -- `pulsePeriod`,
/// `noiseSpeed`, particles' `particleLife` and holographic's `holoSpeed` --
/// so a pulse or a particle takes the same real time on a 3.3x `drifting`
/// as on a 1x ring (orchestrator 2026-09-19: "too fast" on fast orbs;
/// pulse/noise followed with the user's go). A spec's `speed` still scales
/// them: it is the user's "whole object slower/faster" knob. Interrupt and
/// decay don't read `t` at all -- their `interruptAge`/`decayAge` are
/// seconds on the caller's clock by design (a one-shot event's age), and
/// liquid has no time input (it follows the geometry).
fn render(
    mode: &str,
    size: u32,
    t: f64,
    preset_speed: f64,
    opts: &primitives::ModeOpts,
) -> Option<OrbFrame> {
    let wall = if preset_speed > 0.0 {
        t / preset_speed
    } else {
        t
    };
    let frame = match mode {
        "orbits" => orbs::modes::orbits::frame_orbits(size as f64, t, opts),
        "globe" => orbs::modes::globe::frame_globe(size as f64, t, opts),
        "rubik" => orbs::modes::rubik::frame_rubik(size as f64, t, opts),
        "wave" => orbs::modes::wave::frame_wave(size as f64, t, opts),
        "web" => orbs::modes::web::frame_web(size as f64, t, opts),
        "braid" => orbs::modes::braid::frame_braid(size as f64, t, opts),
        // ring shares ribbon's geometry upstream (registry.ts: `ring:
        // frameRibbon`) -- the `faceOn` opt in its base profile switches it.
        "ribbon" | "ring" => orbs::modes::ribbon::frame_ribbon(size as f64, t, opts),
        "morph" => orbs::modes::morph::frame_morph(size as f64, t, opts),
        // Not a port -- see orbs/modes/aurora.rs's header for why it has no
        // golden vector, unlike every other arm above.
        "aurora" => orbs::modes::aurora::frame_aurora(size as f64, t, opts),
        // Not a port either -- a deliberate near-twin of "web" for an A/B
        // comparison of vnoise vs. perlin3, see webflow.rs's header.
        "webflow" => orbs::modes::webflow::frame_webflow(size as f64, t, opts),
        // Not ports -- new shape concepts from docs/effects-research.md,
        // see each module's header comment.
        "spectrum" => orbs::modes::spectrum::frame_spectrum(size as f64, t, opts),
        "sonar" => orbs::modes::sonar::frame_sonar(size as f64, t, opts),
        "warp" => orbs::modes::warp::frame_warp(size as f64, t, opts),
        "chladni" => orbs::modes::chladni::frame_chladni(size as f64, t, opts),
        "eclipse" => orbs::modes::eclipse::frame_eclipse(size as f64, t, opts),
        "crystallize" => orbs::modes::crystallize::frame_crystallize(size as f64, t, opts),
        // Not a port -- the `muted` resting state, see hush.rs's header.
        "hush" => orbs::modes::hush::frame_hush(size as f64, t, opts),
        // The `signal` family -- a sibling to `orbs`, see `signal/mod.rs`.
        "bar" => signal::modes::bar::frame_bar(size as f64, t, opts),
        "waveform" => signal::modes::waveform::frame_waveform(size as f64, t, opts),
        "scroll" => signal::modes::scroll::frame_scroll(size as f64, t, opts),
        "matrix" => signal::modes::matrix::frame_matrix(size as f64, t, opts),
        // The `ring` family -- the third sibling, see `ring/mod.rs`.
        "arc" => ring::modes::arc::frame_arc(size as f64, t, opts),
        "spinner" => ring::modes::spinner::frame_spinner(size as f64, t, opts),
        "nested" => ring::modes::nested::frame_nested(size as f64, t, opts),
        "segmented" => ring::modes::segmented::frame_segmented(size as f64, t, opts),
        "gauge" => ring::modes::gauge::frame_gauge(size as f64, t, opts),
        // The `beacon` family -- the fourth sibling, see `beacon/mod.rs`.
        "ping" => beacon::modes::ping::frame_ping(size as f64, t, opts),
        "pulse" => beacon::modes::pulse::frame_pulse(size as f64, t, opts),
        "halo" => beacon::modes::halo::frame_halo(size as f64, t, opts),
        "radar" => beacon::modes::radar::frame_radar(size as f64, t, opts),
        "broadcast" => beacon::modes::broadcast::frame_broadcast(size as f64, t, opts),
        // The `core` family (module `core_fx`, see its header for the name).
        "shimmer" => core_fx::modes::shimmer::frame_shimmer(size as f64, t, opts),
        "dots" => core_fx::modes::dots::frame_dots(size as f64, t, opts),
        _ => return None,
    };
    let frame = primitives::apply_pointer(frame, opts);
    let frame = primitives::apply_audio_reactive(frame, opts);
    // A periodic pulse swells geometry, so it runs before the materials copy
    // or color it -- a glow halo then breathes with its source.
    let frame = primitives::apply_pulse(frame, wall, opts);
    // Particles (materials phase 3) emit from the geometry so far -- after
    // pulse so they breathe with it, before noise/liquid/colour/glow so
    // those jitter, melt, tint and halo them like any other dot.
    // Per-mode particle defaults fill in only what the caller left unset.
    let frame = particles::apply_particles(
        frame,
        size as f64,
        wall,
        &particles::with_mode_defaults(mode, opts),
    );
    // Materials (see `docs/materials.md`), in dependency order: noise moves
    // geometry, so it runs before anything that copies geometry; gradient
    // colors elements by their final position and runs before glow so a
    // halo inherits its source's ramped color; glow splices halos in last
    // of the three.
    let frame = primitives::apply_noise(frame, size as f64, wall, opts);
    // Liquid (materials phase 2) melts the (jittered) dots into metaball
    // contours before colour/gradient/glow, so those apply to the liquid.
    // Per-mode tuned liquid defaults fill in only what the caller left unset.
    let frame = liquid::apply_liquid(frame, size as f64, &liquid::with_mode_defaults(mode, opts));
    // Colour is the base tint (and carries the ink|fixed paint mode); the
    // gradient is more specific and wins where it's set; holographic-lite
    // (materials phase 4) is the top tint, a hue sweep by depth/facing/time
    // over the kept lightness; glow inherits all three.
    let frame = primitives::apply_color(frame, opts);
    let frame = primitives::apply_gradient(frame, size as f64, opts);
    let frame = primitives::apply_holo(frame, size as f64, wall, opts);
    let frame = primitives::apply_glow(frame, opts);
    // `ink` (FX Spec 1.8) fades the finished visual as one thing, halos
    // included -- hence after glow. It sits *before* the barge-in flash and
    // the mute cue on purpose: those two are events the user must notice,
    // so they read at full strength even on a visual resting at low ink.
    let frame = primitives::apply_ink(frame, opts);
    // The one-shot barge-in flash sits on top of everything above; a one-shot
    // decay then fades everything drawn (flash included); the mute cue runs
    // last so its dimming has the final word even mid-flash.
    let frame = primitives::apply_interrupt(frame, opts);
    let frame = primitives::apply_decay(frame, opts);
    let frame = primitives::apply_muted(frame, opts);
    // Last: `blurScale` (low power) scales / strips every blur sigma.
    Some(primitives::apply_blur_scale(frame, opts))
}

/// Tries every family's own preset resolution in turn (today: `orbs`, then
/// `signal`) and normalizes the result to one shape -- the one place
/// `frame`/`frame_with_overrides`/`resolved_opts` need to know that more
/// than one family exists at all.
struct AnyResolved {
    mode: &'static str,
    speed: f64,
    opts: primitives::ModeOpts,
}

fn resolve_any(state: &str, size: u32) -> Option<AnyResolved> {
    if let Some(r) = orbs::presets::resolve_preset(state, size) {
        return Some(AnyResolved {
            mode: r.mode,
            speed: r.speed,
            opts: r.opts,
        });
    }
    if let Some(r) = signal::presets::resolve_preset(state, size) {
        return Some(AnyResolved {
            mode: r.mode,
            speed: r.speed,
            opts: r.opts,
        });
    }
    if let Some(r) = ring::presets::resolve_preset(state, size) {
        return Some(AnyResolved {
            mode: r.mode,
            speed: r.speed,
            opts: r.opts,
        });
    }
    if let Some(r) = beacon::presets::resolve_preset(state, size) {
        return Some(AnyResolved {
            mode: r.mode,
            speed: r.speed,
            opts: r.opts,
        });
    }
    let r = core_fx::presets::resolve_preset(state, size)?;
    Some(AnyResolved {
        mode: r.mode,
        speed: r.speed,
        opts: r.opts,
    })
}

/// Resolve a `(state, size)` pair -- from any family, see `resolve_any` --
/// and render one frame at time `t` (seconds). Returns `None` if no family
/// has a preset for `state`.
///
/// Native builds (iOS/Android/RN) export this directly through UniFFI. Web
/// has no `#[uniffi::export]` here -- see `wasm::frame_json` below, which
/// wraps this in a JSON bridge for `wasm-bindgen` instead.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame(state: String, size: u32, t: f64) -> Option<OrbFrame> {
    let resolved = resolve_any(&state, size)?;
    render(resolved.mode, size, t, resolved.speed, &resolved.opts)
}

/// Same as `frame`, but overlays `overrides` onto the resolved preset's opts
/// before rendering -- e.g. `{"scanMul": 8.0}` on `searching` speeds up the
/// scan sweep without touching any other tuned value. Built for the Studio
/// (the Studio): live parameter tweaking has no other entry point, since
/// `frame` always renders the stock preset.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame_with_overrides(
    state: String,
    size: u32,
    t: f64,
    overrides: HashMap<String, f64>,
) -> Option<OrbFrame> {
    let resolved = resolve_any(&state, size)?;
    let mut opts = resolved.opts;
    opts.extend(overrides);
    render(resolved.mode, size, t, resolved.speed, &opts)
}

/// `resolved_opts`'s native/UniFFI counterpart to `wasm::resolved_opts_json`
/// below -- returns `{ mode, speed, opts }` for a `(state, size)` pair
/// without rendering anything. A native Studio (iOS/Android) needs this to
/// seed a slider's starting position and to compute `t * speed` (see
/// engine.md's callout); until this was added, only the wasm/JSON path had
/// it, which is the gap the native Studios surfaced.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
pub struct ResolvedOpts {
    pub mode: String,
    pub speed: f64,
    pub opts: HashMap<String, f64>,
}

/// A resolved family's `mode` is `&'static str`, not UniFFI-compatible
/// directly as a Record field -- hence this owned-`String` wrapper type.
/// Works across every family via `resolve_any`, not just `orbs`.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn resolved_opts(state: String, size: u32) -> Option<ResolvedOpts> {
    let r = resolve_any(&state, size)?;
    Some(ResolvedOpts {
        mode: r.mode.to_string(),
        speed: r.speed,
        opts: r.opts,
    })
}

/// Real point-by-point morph between `from_state` and `to_state` at
/// `blend` (`0..1`, `0` = fully `from_state`, `1` = fully `to_state`).
/// Returns `None` if either state doesn't resolve, *or* if the pair isn't
/// one of the three lattice-sharing modes this supports (`glowing`,
/// `calibrating`, `progressing` -- see `orbs::modes::transition`'s header
/// for why only these three) -- a caller should fall back to
/// cross-dissolving two independent `frame()` calls in that case (`apps/
/// studio`'s transition panel does exactly this).
///
/// `t` is plain elapsed seconds here, deliberately **not** scaled by
/// either state's own `speed` (unlike `frame`/`frame_with_overrides`) --
/// the two endpoints have different tuned speeds and picking one over the
/// other for a transition preview isn't an obviously-right call, so this
/// keeps it simple instead of guessing.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame_transition(
    from_state: String,
    to_state: String,
    size: u32,
    t: f64,
    blend: f64,
) -> Option<OrbFrame> {
    let from = orbs::presets::resolve_preset(&from_state, size)?;
    let to = orbs::presets::resolve_preset(&to_state, size)?;
    orbs::modes::transition::frame_transition(
        from.mode,
        to.mode,
        size as f64,
        t,
        blend,
        &from.opts,
        &to.opts,
    )
}

/// The **voice-state profile** for `state` on `pattern` (docs/fx-spec.md,
/// *v1.8*): the speed multiplier and engine overrides that make one shape
/// read as `idle` / `listening` / `thinking` / `speaking`, plus which app
/// input drives `audioLevel` there. `None` for a key outside the voice
/// language, so an app's own state names are left alone.
///
/// Views use it for plain input (`pattern` + `state`, no spec file): merge
/// it *under* the app's own overrides. The FX Spec resolver already applies
/// it to a 1.8+ file's voice-state entries, filling only what the file
/// doesn't set, so callers with a spec need not call this at all.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn voice_state_profile(pattern: String, state: String) -> Option<VoiceStateProfile> {
    voice_state::profile(&pattern, &state)
}

/// The FX Spec minor this runtime speaks (`1.<minor>`): a file above it
/// still resolves, with unknown keys reported as warnings rather than
/// errors. The identity-lock test (`tests/fx_spec_lock.rs`) captures a
/// lock per minor.
pub fn fx_spec_runtime_minor() -> u64 {
    fx_spec::RUNTIME_MINOR
}

/// Parse, validate and resolve an FX Spec v1 document (`docs/fx-spec.md`,
/// `spec/fx-spec-1.schema.json`) to `(state, size, speed, overrides)` plus
/// diagnostics with JSON-Pointer paths. Unknown keys are always reported
/// (errors in a 1.0 file, warnings in a newer 1.x one); `ok` is false if
/// any diagnostic is an error.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn resolve_fx_spec(json: String) -> FxSpecResolved {
    fx_spec::resolve(&json)
}

/// Render an FX Spec at `elapsed` seconds of wall time. Unlike `frame`,
/// the time scaling is done for you: `elapsed * presetSpeed * spec.speed`.
/// Returns `None` if the spec has errors (see `resolve_fx_spec` for why).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame_from_fx_spec(json: String, elapsed: f64) -> Option<OrbFrame> {
    frame_from_fx_spec_with(json, elapsed, None, HashMap::new(), false)
}

/// FX Spec v1.1+: resolve for the caller's current lifecycle `state` (a key
/// of the spec's `states`; `None` or an unknown key = the base design), app
/// `inputs` (drive the spec's `bindings`; a missing input leaves its binding
/// inactive, listed in `inactive_bindings`) and the host's power state
/// (`low_power`: apply the spec's 1.2 `performance.lowPower` -- shed
/// materials, lower `max_fps`). Stateless: the caller owns easing,
/// cross-fades and frame pacing (docs/fx-spec.md, *Caller loop*).
// `low_power` defaults to false, so Swift/Kotlin callers from before 1.2
// (the Studios, FxView) compile unchanged.
#[cfg_attr(
    not(target_arch = "wasm32"),
    uniffi::export(default(low_power = false))
)]
pub fn resolve_fx_spec_with(
    json: String,
    state: Option<String>,
    inputs: HashMap<String, f64>,
    low_power: bool,
) -> FxSpecResolved {
    fx_spec::resolve_full(&json, state.as_deref(), &inputs, low_power)
}

/// `frame_from_fx_spec` for a lifecycle state and app inputs (see
/// `resolve_fx_spec_with`). `None` if the spec has errors.
// `low_power` defaults to false, so Swift/Kotlin callers from before 1.2
// (the Studios, FxView) compile unchanged.
#[cfg_attr(
    not(target_arch = "wasm32"),
    uniffi::export(default(low_power = false))
)]
pub fn frame_from_fx_spec_with(
    json: String,
    elapsed: f64,
    state: Option<String>,
    inputs: HashMap<String, f64>,
    low_power: bool,
) -> Option<OrbFrame> {
    let spec = fx_spec::resolve_full(&json, state.as_deref(), &inputs, low_power);
    if !spec.ok {
        return None;
    }
    let preset_speed = resolve_any(&spec.state, spec.size)?.speed;
    frame_with_overrides(
        spec.state,
        spec.size,
        elapsed * preset_speed * spec.speed,
        spec.overrides,
    )
}

/// A spec color (`"#RRGGBB"`, `"#RGB"`, or a DTCG color object as JSON) as
/// CSS Color 4 HSL -- the same conversion the spec resolver uses, exported
/// so the Studios' own converters can be parity-tested against it.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn fx_color_to_hsl(color: String) -> Option<FxHsl> {
    fx_spec::color_to_hsl(&color)
}

/// Render-cost proxy of `(state, size)` with `overrides` -- element count
/// and coverage (primitive area / canvas area), the max over fixed sample
/// times, classed light / medium / heavy (`cost.rs`; docs/engine.md, *Cost
/// estimate*). A proxy for a Studio badge or a host's shedding decision,
/// not a measurement. `None` if the state doesn't resolve.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn estimate_cost(state: String, size: u32, overrides: HashMap<String, f64>) -> Option<FxCost> {
    cost::estimate(&state, size, &overrides)
}

/// `estimate_cost` for an FX Spec resolved with `(state, inputs,
/// low_power)`. `None` if the spec has errors.
// `low_power` defaults to false, so Swift/Kotlin callers from before 1.2
// (the Studios, FxView) compile unchanged.
#[cfg_attr(
    not(target_arch = "wasm32"),
    uniffi::export(default(low_power = false))
)]
pub fn fx_spec_cost(
    json: String,
    state: Option<String>,
    inputs: HashMap<String, f64>,
    low_power: bool,
) -> Option<FxCost> {
    cost::estimate_spec(&json, state.as_deref(), &inputs, low_power)
}

/// How well the liquid material suits `state` (`"recommended"` / `"ok"` /
/// `"notRecommended"`, a one-line reason, and the state's tuned liquid
/// defaults) -- from contact sheets, see docs/materials.md. For a Studio
/// badge. `None` only for an unknown state.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn liquid_suitability(state: String) -> Option<LiquidSuitability> {
    let mode = resolve_any(&state, 64)?.mode;
    Some(liquid::suitability(mode))
}

/// Every particle knob's default on `state` -- the engine-wide defaults with
/// the state's own over them (ring states rise, scanning attracts, notifying
/// bursts, speaking follows `audioLevel`, ...; see `particles::mode_defaults`
/// and docs/materials.md). They apply only to keys a caller leaves unset, so
/// the Studio shows them as the knob defaults. `None` only for an unknown
/// state.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn particle_defaults(state: String) -> Option<HashMap<String, f64>> {
    let mode = resolve_any(&state, 64)?.mode;
    Some(particles::defaults_for(mode))
}

/// The **parameter catalog** (docs/parameters.md, *Parameter catalog*) as
/// JSON text: every object, pattern and tunable with labels, descriptions,
/// ranges, groups, paths and per-size defaults. `spec/parameters.json` is a
/// checked-in copy. Parse it on the caller's side (one stable JSON shape
/// on every platform rather than a deep tree of UniFFI records).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn parameter_catalog_json() -> String {
    catalog::catalog_json()
}

/// Validates engine overrides for a pattern against the catalog: unknown
/// keys (with a did-you-mean), out-of-range values, fractional values for
/// whole-number keys, renamed keys. Warnings only -- the frame is unaffected.
/// `state` is the pattern id, as everywhere in the low-level API.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn check_overrides(
    state: String,
    size: u32,
    overrides: HashMap<String, f64>,
) -> Vec<fx_spec::FxDiagnostic> {
    catalog::check_overrides(&state, size, &overrides)
}

/// Web bridge: `wasm-pack build --target web` exposes these as JS functions.
/// JSON over the wire, not a zero-copy buffer -- good enough for the smoke
/// harness in `apps/smoke`; revisit if/when a real perf ceiling shows up.
#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::collections::HashMap;
    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen]
    pub fn frame_json(state: String, size: u32, t: f64) -> String {
        match crate::frame(state, size, t) {
            Some(f) => serde_json::to_string(&f).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn frame_json_with_overrides(
        state: String,
        size: u32,
        t: f64,
        overrides_json: String,
    ) -> String {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        match crate::frame_with_overrides(state, size, t, overrides) {
            Some(f) => serde_json::to_string(&f).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn frame_transition_json(
        from_state: String,
        to_state: String,
        size: u32,
        t: f64,
        blend: f64,
    ) -> String {
        match crate::frame_transition(from_state, to_state, size, t, blend) {
            Some(f) => serde_json::to_string(&f).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    /// `{ state?: string, inputs?: { [name]: number } }` -- FX Spec v1.1's
    /// call context; empty or unparseable = no state, no inputs.
    #[derive(serde::Deserialize, Default)]
    struct FxContext {
        state: Option<String>,
        #[serde(default)]
        inputs: HashMap<String, f64>,
        #[serde(default, rename = "lowPower")]
        low_power: bool,
    }

    fn fx_context(ctx_json: &str) -> FxContext {
        serde_json::from_str(ctx_json).unwrap_or_default()
    }

    #[wasm_bindgen]
    pub fn resolve_fx_spec_json(json: String, ctx_json: String) -> String {
        let ctx = fx_context(&ctx_json);
        serde_json::to_string(&crate::resolve_fx_spec_with(
            json,
            ctx.state,
            ctx.inputs,
            ctx.low_power,
        ))
        .unwrap_or_else(|_| "null".to_string())
    }

    #[wasm_bindgen]
    pub fn frame_from_fx_spec_json(json: String, elapsed: f64, ctx_json: String) -> String {
        let ctx = fx_context(&ctx_json);
        match crate::frame_from_fx_spec_with(json, elapsed, ctx.state, ctx.inputs, ctx.low_power) {
            Some(f) => serde_json::to_string(&f).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn fx_color_to_hsl_json(color: String) -> String {
        match crate::fx_color_to_hsl(color) {
            Some(c) => serde_json::to_string(&c).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    // --- Packed transport (see transport.rs): the same frames as the
    // `*_json` functions, as one `Float64Array` instead of a JSON string.

    #[wasm_bindgen]
    pub fn frame_packed(state: String, size: u32, t: f64) -> Box<[f64]> {
        crate::transport::pack(crate::frame(state, size, t).as_ref()).into_boxed_slice()
    }

    #[wasm_bindgen]
    pub fn frame_packed_with_overrides(
        state: String,
        size: u32,
        t: f64,
        overrides_json: String,
    ) -> Box<[f64]> {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        crate::transport::pack(crate::frame_with_overrides(state, size, t, overrides).as_ref())
            .into_boxed_slice()
    }

    #[wasm_bindgen]
    pub fn frame_from_fx_spec_packed(json: String, elapsed: f64, ctx_json: String) -> Box<[f64]> {
        let ctx = fx_context(&ctx_json);
        let f = crate::frame_from_fx_spec_with(json, elapsed, ctx.state, ctx.inputs, ctx.low_power);
        crate::transport::pack(f.as_ref()).into_boxed_slice()
    }

    #[wasm_bindgen]
    pub fn estimate_cost_json(state: String, size: u32, overrides_json: String) -> String {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        match crate::estimate_cost(state, size, overrides) {
            Some(c) => serde_json::to_string(&c).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn fx_spec_cost_json(json: String, ctx_json: String) -> String {
        let ctx = fx_context(&ctx_json);
        match crate::fx_spec_cost(json, ctx.state, ctx.inputs, ctx.low_power) {
            Some(c) => serde_json::to_string(&c).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn parameter_catalog_json() -> String {
        crate::parameter_catalog_json()
    }

    /// `voice_state_profile` as JSON (`null` for a state outside the voice
    /// language): `{ speed, overrides, audioInput }`.
    #[wasm_bindgen]
    pub fn voice_state_profile_json(pattern: String, state: String) -> String {
        match crate::voice_state_profile(pattern, state) {
            Some(p) => serde_json::to_string(&p).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn check_overrides_json(state: String, size: u32, overrides_json: String) -> String {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        serde_json::to_string(&crate::check_overrides(state, size, overrides))
            .unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn particle_defaults_json(state: String) -> String {
        match crate::particle_defaults(state) {
            Some(d) => serde_json::to_string(&d).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn liquid_suitability_json(state: String) -> String {
        match crate::liquid_suitability(state) {
            Some(s) => serde_json::to_string(&s).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    /// The tuned default opts for `(state, size)`, before any override --
    /// `frame`/`frame_with_overrides` only return the rendered frame, not
    /// the values that produced it, so a caller building an override UI
    /// (the Studio's sliders) has no other way to know what "no override"
    /// actually resolves to.
    #[wasm_bindgen]
    pub fn resolved_opts_json(state: String, size: u32) -> String {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            mode: &'a str,
            speed: f64,
            opts: &'a HashMap<String, f64>,
        }
        match crate::resolve_any(&state, size) {
            Some(r) => serde_json::to_string(&Out {
                mode: r.mode,
                speed: r.speed,
                opts: &r.opts,
            })
            .unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }
}

/// Every pattern this engine has, from the five families' own `STATES`.
///
/// One place to ask the question, because the answer used to be spelled out
/// independently in `cost.rs`'s tests and `liquid.rs`'s, and asserted as a bare
/// `34` in two more. A hard-coded count agrees
/// with itself even when two lists hold different *names*, which is the failure
/// that matters: `catalog_parameter_names_match_the_presets` compares the names.
#[cfg(test)]
pub(crate) fn all_states() -> Vec<&'static str> {
    [
        crate::orbs::presets::STATES,
        crate::signal::presets::STATES,
        crate::ring::presets::STATES,
        crate::beacon::presets::STATES,
        crate::core_fx::presets::STATES,
    ]
    .concat()
}

#[cfg(test)]
mod wall_clock_tests {
    use super::*;

    #[test]
    fn particles_run_on_wall_clock_seconds_not_the_preset_clock() {
        // `drifting` runs its mode clock ~3.3x: callers pass
        // t = elapsed * presetSpeed. The particles must be exactly those of
        // the unscaled elapsed time -- i.e. apply_particles(base, elapsed).
        let size = 64;
        let r = resolve_any("drifting", size).unwrap();
        assert!(r.speed > 3.0, "drifting is a fast preset: {}", r.speed);
        let elapsed = 1.7;
        let on = HashMap::from([("particleStrength".to_string(), 1.0)]);
        let with =
            frame_with_overrides("drifting".into(), size, elapsed * r.speed, on.clone()).unwrap();
        let base = frame_with_overrides("drifting".into(), size, elapsed * r.speed, HashMap::new())
            .unwrap();
        let mut opts = r.opts.clone();
        opts.extend(on);
        let opts = particles::with_mode_defaults(r.mode, &opts);
        let want = particles::apply_particles(base.clone(), size as f64, elapsed, &opts);
        assert_eq!(with.dots, want.dots);
        assert!(with.dots.len() > base.dots.len());
    }

    #[test]
    fn pulse_and_noise_run_on_wall_clock_seconds() {
        // `breathing` runs its mode clock 3.24x; a 2 s pulse and the noise
        // drift must be those of the unscaled elapsed time.
        let size = 64;
        let r = resolve_any("breathing", size).unwrap();
        assert!(r.speed > 3.0);
        let elapsed = 0.7;
        let t = elapsed * r.speed;
        // Exactly what render computes (t / speed, ~elapsed up to rounding).
        let wall = t / r.speed;
        assert!((wall - elapsed).abs() < 1e-12);
        let base = frame_with_overrides("breathing".into(), size, t, HashMap::new()).unwrap();
        let pulse = HashMap::from([
            ("pulseStrength".to_string(), 1.0),
            ("pulseScale".to_string(), 0.2),
        ]);
        let got = frame_with_overrides("breathing".into(), size, t, pulse.clone()).unwrap();
        assert_eq!(got, primitives::apply_pulse(base.clone(), wall, &pulse));
        let noise = HashMap::from([("noiseStrength".to_string(), 1.0)]);
        let got = frame_with_overrides("breathing".into(), size, t, noise.clone()).unwrap();
        assert_eq!(
            got,
            primitives::apply_noise(base, size as f64, wall, &noise)
        );
    }

    #[test]
    fn particle_defaults_are_per_state_and_explicit_keys_win() {
        let d = |st: &str| particle_defaults(st.into()).unwrap();
        assert_eq!(d("tracking")["particleStyle"], 3.0, "ring states rise");
        assert_eq!(d("scanning")["particleStyle"], 1.0, "scanning attracts");
        assert_eq!(d("notifying")["particleSync"], 1.0, "notifying bursts");
        assert_eq!(
            d("speaking")["particleAudio"],
            1.0,
            "speaking follows audio"
        );
        assert_eq!(d("breathing")["particleStyle"], 2.0, "ambient orbs orbit");
        assert_eq!(d("working")["particleLife"], 4.5, "base defaults elsewhere");
        assert_eq!(d("working").len(), particles::BASE_DEFAULTS.len());
        assert!(particle_defaults("nope".into()).is_none());
        // A key the caller sets wins over the state's default.
        let on = |extra: &[(&str, f64)]| {
            let mut o = HashMap::from([("particleStrength".to_string(), 1.0)]);
            o.extend(extra.iter().map(|(k, v)| (k.to_string(), *v)));
            frame_with_overrides("tracking".into(), 64, 2.0, o).unwrap()
        };
        let rise = on(&[]);
        let explicit = on(&[
            ("particleStyle", 3.0),
            ("particleCount", 12.0),
            ("particleLife", 5.0),
            ("particleSpread", 0.2),
        ]);
        assert_eq!(rise, explicit, "unset keys take the state's defaults");
        assert_ne!(
            on(&[("particleStyle", 0.0)]),
            rise,
            "an explicit style wins"
        );
        // Off: the defaults never touch a frame.
        assert_eq!(
            frame_with_overrides("tracking".into(), 64, 2.0, HashMap::new()),
            frame("tracking".into(), 64, 2.0)
        );
    }
}
