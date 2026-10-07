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

pub mod a11y;
mod beacon;
mod catalog;
/// For the test that writes spec/parameters.json (the catalog with its words).
#[doc(hidden)]
pub use catalog::catalog_json_from_source;
mod character;
mod conversation;
mod core_fx;
mod cost;
pub mod effects;
mod fx_spec;
mod orbs;
mod primitives;
pub mod reactive;
mod ring;
mod rules;
mod signal;
mod transition;
mod voice_state;
// Only the wasm bridge packs frames; native tests still cover the layout.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod transport;

use std::collections::HashMap;

pub use cost::FxCost;
pub use fx_spec::{FxDiagnostic, FxHsl, FxSpecResolved};
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
/// `noiseSpeed` -- so a pulse takes the same real time on a 3.3x `drifting`
/// as on a 1x ring (orchestrator 2026-09-19: "too fast" on fast orbs;
/// pulse/noise followed with the user's go). A spec's `speed` still scales
/// them: it is the user's "whole object slower/faster" knob. Interrupt and
/// decay don't read `t` at all -- their `interruptAge`/`decayAge` are
/// seconds on the caller's clock by design (a one-shot event's age).
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
        // A head and shoulders of dots (design note 32).
        "silhouette" => orbs::modes::silhouette::frame_silhouette(size as f64, t, opts),
        // The `signal` family -- a sibling to `orbs`, see `signal/mod.rs`.
        "bar" => signal::modes::bar::frame_bar(size as f64, t, opts),
        "waveform" => signal::modes::waveform::frame_waveform(size as f64, t, opts),
        "scroll" => signal::modes::scroll::frame_scroll(size as f64, t, opts),
        "matrix" => signal::modes::matrix::frame_matrix(size as f64, t, opts),
        "playback" => signal::modes::playback::frame_playback(size as f64, t, opts),
        // The `ring` family -- the third sibling, see `ring/mod.rs`.
        "arc" => ring::modes::arc::frame_arc(size as f64, t, opts),
        "spinner" => ring::modes::spinner::frame_spinner(size as f64, t, opts),
        "nested" => ring::modes::nested::frame_nested(size as f64, t, opts),
        "segmented" => ring::modes::segmented::frame_segmented(size as f64, t, opts),
        "gauge" => ring::modes::gauge::frame_gauge(size as f64, t, opts),
        "speaker" => ring::modes::speaker::frame_speaker(size as f64, t, opts),
        // The `beacon` family -- the fourth sibling, see `beacon/mod.rs`.
        "ping" => beacon::modes::ping::frame_ping(size as f64, t, opts),
        "pulse" => beacon::modes::pulse::frame_pulse(size as f64, t, opts),
        "halo" => beacon::modes::halo::frame_halo(size as f64, t, opts),
        "radar" => beacon::modes::radar::frame_radar(size as f64, t, opts),
        "broadcast" => beacon::modes::broadcast::frame_broadcast(size as f64, t, opts),
        // The `core` family (module `core_fx`, see its header for the name).
        "shimmer" => core_fx::modes::shimmer::frame_shimmer(size as f64, t, opts),
        "dots" => core_fx::modes::dots::frame_dots(size as f64, t, opts),
        // The `character` family (faces with voice states), see `character/mod.rs`.
        "buzzy" => {
            character::recipe::frame("buzzy", size as f64, t, opts).expect("a built-in recipe")
        }
        "hum" => character::recipe::frame("hum", size as f64, t, opts).expect("a built-in recipe"),
        "wisp" => {
            character::recipe::frame("wisp", size as f64, t, opts).expect("a built-in recipe")
        }
        "chirp" => {
            character::recipe::frame("chirp", size as f64, t, opts).expect("a built-in recipe")
        }
        "cuppa" => {
            character::recipe::frame("cuppa", size as f64, t, opts).expect("a built-in recipe")
        }
        "bean" => {
            character::recipe::frame("bean", size as f64, t, opts).expect("a built-in recipe")
        }
        "beep" => {
            character::recipe::frame("beep", size as f64, t, opts).expect("a built-in recipe")
        }
        // A recipe from an FX Spec (FX Spec 1.12): its registry key.
        k => character::registry::frame(k, size as f64, t, opts)?,
    };
    Some(post(frame, mode, size, wall, opts))
}

/// Everything after a mode's geometry: pointer, audio, pulse,
/// materials, ink and the one-shot cues, in their fixed order. Shared by
/// [`render`] and the lattice morph ([`frame_transition_with_overrides`]),
/// so a morph frame is finished exactly like any other.
fn post(
    frame: OrbFrame,
    mode: &str,
    size: u32,
    wall: f64,
    opts: &primitives::ModeOpts,
) -> OrbFrame {
    let frame = primitives::apply_pointer(frame, opts);
    // A character moves with the voice itself (the mouth, a squash: `character/rig.rs`);
    // a bound voice's `audioStrength` would also swell the whole body.
    let frame = if effects::draws_own(mode) {
        frame
    } else {
        primitives::apply_audio_reactive(frame, opts)
    };
    // A periodic pulse swells geometry, so it runs before the materials copy
    // or color it -- a glow halo then breathes with its source.
    let frame = primitives::apply_pulse(frame, wall, opts);
    // Materials (see `docs/materials.md`), in dependency order: noise moves
    // geometry, so it runs before anything that copies geometry; gradient
    // colors elements by their final position and runs before glow so a
    // halo inherits its source's ramped color; glow splices halos in last
    // of the three.
    let frame = primitives::apply_noise(frame, size as f64, wall, opts);
    // Colour is the base tint (and carries the ink|fixed paint mode); the
    // gradient is more specific and wins where it's set; glow inherits both.
    // A character's palette is drawn, not tinted: a frame-wide colour or
    // gradient would repaint its eyes and screen (`hue` turns its shell). The
    // FX Spec rejects both sections on a character; this covers raw overrides.
    let (frame, tint) = if effects::draws_own(mode) {
        (frame, false)
    } else {
        (primitives::apply_color(frame, opts), true)
    };
    let frame = if tint {
        primitives::apply_gradient(frame, size as f64, opts)
    } else {
        frame
    };
    let frame = primitives::apply_glow(frame, opts);
    // `ink` (FX Spec 1.8) fades the finished visual as one thing, halos
    // included -- hence after glow. It sits *before* the barge-in flash and
    // the mute cue on purpose: those two are events the user must notice,
    // so they read at full strength even on a visual resting at low ink.
    let frame = primitives::apply_ink(frame, opts);
    // The one-shot barge-in flash sits on top of everything above; a one-shot
    // decay then fades everything drawn (flash included); the mute cue runs
    // last so its dimming has the final word even mid-flash.
    // A character startles instead (eyes wide, a hop back: `character/rig.rs`);
    // the generic flash darkens its whole palette.
    let frame = if effects::draws_own(mode) {
        frame
    } else {
        primitives::apply_interrupt(frame, opts)
    };
    // A one-shot feedback effect (success / error / celebrate) the view is
    // playing: on top of the flash, before the decay and the mute cue.
    // A character draws the effect as its own expression (`character/rig.rs`).
    let frame = if effects::draws_own(mode) {
        frame
    } else {
        effects::apply_effect(frame, size as f64, opts, effects::plays_in_place(mode))
    };
    let frame = primitives::apply_decay(frame, opts);
    // A character keeps its colours when muted: it squints and fades a little
    // itself (`character/rig.rs`, the mode), instead of the generic grey-out.
    let frame = if effects::draws_own(mode) {
        frame
    } else {
        primitives::apply_muted(frame, opts)
    };
    // Last: `blurScale` (low power) scales / strips every blur sigma.
    primitives::apply_blur_scale(frame, opts)
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

/// A state's preset speed: the factor between its engine time and the wall-clock
/// seconds its post-processes run in (see [`render`]).
pub(crate) fn preset_speed(state: &str, size: u32) -> f64 {
    resolve_any(state, size).map_or(1.0, |r| r.speed)
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
    if let Some(r) = character::presets::resolve_preset(state, size) {
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

/// A state's mode and its resolved preset opts (any family).
pub(crate) fn resolve_state(
    state: &str,
    size: u32,
) -> Option<(&'static str, primitives::ModeOpts)> {
    let r = resolve_any(state, size)?;
    Some((r.mode, r.opts))
}

pub use transition::{FxTransition, TransitionMix, TransitionSide};

/// The voice states' weighted mix of one pattern (design note 31): `sides` are the
/// pattern's voice-state sides (profile merged under the app's overrides), `weights`
/// one per side (the view moves them with its transition clock), `target` the index
/// of the state it is heading to (arrival keys take its value at once). Continuous
/// keys blend by weight (hues the short way round); counts and choices come from the
/// heaviest side, with the second's in `structural_to` dissolved by `swap`. `rates`
/// is filled as in [`transition_mix`]. `None` if the sides draw different patterns.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn voice_blend(
    sides: Vec<TransitionSide>,
    weights: Vec<f64>,
    target: u32,
    size: u32,
) -> Option<TransitionMix> {
    transition::blend(&sides, &weights, target as usize, size)
}

/// What to draw at `progress` (`0..1` of the transition's duration, linear)
/// of a state change -- the one transition system every view uses
/// (docs/fx-spec.md, *Transitions*). `technique` says how: `params` (same
/// pattern: draw `overrides`, plus `overrides` + `structural_to` dissolved by
/// `swap` when counts/choices differ), `morph` (lattice pair:
/// [`frame_transition_with_overrides`] at `weight`) or `crossFade` (two
/// frames at `weight`). `curve` is a CSS keyword. `None` if a state doesn't
/// resolve.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn transition_mix(
    from: TransitionSide,
    to: TransitionSide,
    size: u32,
    progress: f64,
    curve: String,
) -> Option<TransitionMix> {
    transition::mix(&from, &to, size, progress, &curve)
}

/// The lattice morph with each side's own overrides, finished like any frame
/// (materials, ink, cues): geometry morphs point by point at `blend`, and the
/// post-processing runs on the two sides' continuous keys interpolated
/// (counts/choices from the nearer side). `None` unless both states are
/// lattice-sharing orb patterns (glowing / calibrating / progressing).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame_transition_with_overrides(
    from: TransitionSide,
    to: TransitionSide,
    size: u32,
    t: f64,
    blend: f64,
) -> Option<OrbFrame> {
    // A loadout change on one character (design note 25).
    if let Some(f) = character::wear::frame_wear(&from, &to, size, t, blend) {
        return Some(f);
    }
    let a = orbs::presets::resolve_preset(&from.state, size)?;
    let b = orbs::presets::resolve_preset(&to.state, size)?;
    let mut oa = a.opts.clone();
    oa.extend(from.overrides.clone());
    let mut ob = b.opts.clone();
    ob.extend(to.overrides.clone());
    let blend = blend.clamp(0.0, 1.0);
    let geometry =
        orbs::modes::transition::frame_transition(a.mode, b.mode, size as f64, t, blend, &oa, &ob)?;
    let mut post_opts = if blend < 0.5 { oa.clone() } else { ob.clone() };
    for (k, vb) in &ob {
        if let Some(va) = oa.get(k) {
            if catalog::key_info(b.mode, k).is_none_or(|(ty, _)| ty == "number") {
                post_opts.insert(k.clone(), va + (vb - va) * blend);
            }
        }
    }
    let speed = a.speed + (b.speed - a.speed) * blend;
    let wall = if speed > 0.0 { t / speed } else { t };
    Some(post(geometry, b.mode, size, wall, &post_opts))
}

/// The duration and curve for the state change `from` → `to` in an FX Spec
/// (1.9 `transitions`): the exact pair, then `from->*`, then `*->to`, then
/// `default`; without the block, 0.6 s `easeInOut`. `None` for a side is
/// the base design (`""`).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn fx_spec_transition(json: String, from: Option<String>, to: Option<String>) -> FxTransition {
    fx_spec::transition_for(
        &json,
        from.as_deref().unwrap_or(""),
        to.as_deref().unwrap_or(""),
    )
}

/// The `states` key an FX Spec's 1.9 `rules` pick for the app's `inputs`,
/// given the state rendered last (`previous`, for hysteresis): the first rule
/// that holds, else `None` -- keep the caller's own state. Pure; the caller
/// keeps `previous`.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn fx_spec_derive_state(
    json: String,
    inputs: HashMap<String, f64>,
    previous: Option<String>,
) -> Option<String> {
    rules::derive(&json, &inputs, previous.as_deref())
}

pub use a11y::{AnnounceStep, AnnouncerState, FxAccessibility};
pub use effects::EffectInfo;

/// A one-shot feedback effect by name (`success`, `error`, `celebrate`;
/// docs/fx-view.md, *One-shot effects*): the code and duration a view feeds
/// as `effectCode` / `effectAge` runtime keys, and the words it speaks. `None`
/// for an unknown name.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn effect_info(name: String) -> Option<EffectInfo> {
    effects::info(&name)
}

/// An FX Spec's 1.9 `accessibility` block (docs/fx-view.md, *Accessibility*):
/// the view's name, per-state words and whether changes are spoken. Empty for
/// bad JSON or no block; `resolve_fx_spec` reports problems.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn fx_spec_accessibility(json: String) -> FxAccessibility {
    a11y::read(&json)
}

/// A view's accessible name in `state`: the app's words for it, else the
/// file's `accessibility.states`, else the built-in words for a voice state
/// ("<name>, listening"), else `name`. The same on every platform.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn a11y_accessible_name(
    name: String,
    state: Option<String>,
    spec_words: HashMap<String, String>,
    app_words: HashMap<String, String>,
) -> String {
    a11y::accessible_name(&name, state.as_deref(), &spec_words, &app_words)
}

/// The words for `state` (as `a11y_accessible_name`), or `None` when it has
/// none -- then nothing is spoken. Feed these to `a11y_announce_step`.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn a11y_state_words(
    name: String,
    state: Option<String>,
    spec_words: HashMap<String, String>,
    app_words: HashMap<String, String>,
) -> Option<String> {
    a11y::state_words(&name, state.as_deref(), &spec_words, &app_words)
}

/// One step of the announcer: call on every change of the words showing and
/// again at `recheck_at`. Speaks a state once it has held 1 s, at most once per
/// 3 s, never the first one. Pure; keep the returned state.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn a11y_announce_step(prev: AnnouncerState, words: Option<String>, now: f64) -> AnnounceStep {
    a11y::announce_step(prev, words, now)
}

pub use conversation::ConversationFrame;

/// A simulated conversation at `t` seconds (docs/audio-pipeline.md,
/// *Simulated conversations*): the agent state and a speech-like level and
/// `band_count` bands, from a script of turns. A pure function of time, so
/// every platform's `SimulatedVoiceSource` plays the same conversation. No
/// audio anywhere.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn conversation_at(json: String, t: f64, band_count: u32) -> ConversationFrame {
    conversation::at(&json, t, band_count)
}

/// The built-in sample conversations' names (`calendar`, `quick-answer`,
/// `long-answer`, `barge-in`).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn conversation_sample_names() -> Vec<String> {
    conversation::sample_names()
}

/// A built-in sample conversation's script JSON.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn conversation_sample(name: String) -> Option<String> {
    conversation::sample(&name)
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

/// A character's palette override (design note 19), resolved for a view: the
/// engine opts (`palette.<slot>.h/.s/.l/.w`) and what went wrong or reads badly.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxPaletteResult {
    pub overrides: HashMap<String, f64>,
    pub diagnostics: Vec<fx_spec::FxDiagnostic>,
}

/// `palette` (a JSON object: slots or roles -> hex or DTCG colour, `theme`,
/// `dark`; design notes 19, 23) on `pattern`, through the FX Spec's own
/// `palette` rules, so a view and a file paint alike. The dark variant comes
/// back as `palette.dark.<slot>.*`, picked when the view passes `dark`.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn palette_overrides(pattern: String, palette_json: String) -> FxPaletteResult {
    let palette: serde_json::Value =
        serde_json::from_str(&palette_json).unwrap_or(serde_json::Value::Null);
    let doc = serde_json::json!({
        "fxSpec": "1.13", "object": "character", "pattern": pattern, "palette": palette,
    });
    let r = fx_spec::resolve(&doc.to_string());
    FxPaletteResult {
        overrides: r
            .overrides
            .into_iter()
            .filter(|(k, _)| k.starts_with("palette."))
            .collect(),
        diagnostics: r
            .diagnostics
            .into_iter()
            .filter(|d| d.path.starts_with("/palette"))
            .collect(),
    }
}

/// A built-in character's recipe (design note 18), its JSON as the engine
/// carries it (minified, without `$comment`); `None` for another id. The
/// Studio's editor starts from it ("Customize").
pub fn character_recipe(id: &str) -> Option<&'static str> {
    character::recipe::RECIPES
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, json)| *json)
}

/// A character's expression (design note 16) as the engine opts that weigh
/// it: the named one 1, the others 0; `"none"` all 0. `None` for an unknown
/// name. A view sets these (eased over a change) to draw `expression`.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn expression_overrides(name: String) -> Option<HashMap<String, f64>> {
    let known = name == "none" || character::rig::EXPRESSIONS.iter().any(|(n, _)| *n == name);
    known.then(|| {
        character::rig::EXPRESSIONS
            .iter()
            .map(|(n, k)| (k.to_string(), if *n == name { 1.0 } else { 0.0 }))
            .collect()
    })
}

/// How a pattern lays out in a view's box: `"box"` -- engine space follows
/// the box ratio, which the view passes as the `aspect` input (width
/// `size * aspect`, height `size`) and fills the box with -- or `"square"`,
/// a square centred in the box (every pattern without a catalog `layout`,
/// and an unknown one).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn pattern_layout(pattern: String) -> String {
    catalog::layout_of(&pattern).to_string()
}

/// Signal `playing`: the playback position under a touch. `x` is the touch's
/// distance from the box's left edge over the box's height, the box is `aspect`
/// wide (width / height); the result is `0..1`, on the same row of bars the
/// pattern draws. Views use it for drag-to-seek.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn playback_seek_progress(aspect: f64, x: f64) -> f64 {
    signal::modes::playback::seek_progress(aspect, x)
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

/// A loadout applied to an FX Spec (design note 25): the file with the end
/// user's choices in it, and warnings for whatever it no longer offers.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn apply_loadout(spec: String, loadout: String) -> LoadoutApplied {
    let (spec, diagnostics) = fx_spec::apply_loadout(&spec, &loadout);
    LoadoutApplied { spec, diagnostics }
}

/// [`apply_loadout`]'s result.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct LoadoutApplied {
    pub spec: String,
    pub diagnostics: Vec<FxDiagnostic>,
}

/// Loads a catalog pack (design note 26): `{ "catalog": 1, "namespace": "...",
/// "cosmetics": [...], "palettes": {...} }`. Files then name its items as
/// `"<namespace>:<id>"`. Loading a namespace again replaces it; an error loads nothing.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn load_catalog(json: String) -> Vec<FxDiagnostic> {
    character::catalog::load(&json)
}

/// Forgets a catalog pack's items; whether it had any.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn unload_catalog(namespace: String) -> bool {
    character::catalog::unload(&namespace)
}

/// What a file's wardrobe offers `character`, for a picker (design note 25).
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn cosmetics_for(spec: String, character: String) -> Vec<FxDiagnostic> {
    fx_spec::cosmetics_for(&spec, &character)
}

/// A thumbnail (design note 25): the spec with `loadout` (may be empty) drawn
/// once in a still pose (no glance, no blink) at `turn_yaw` (radians, 0 =
/// facing), without touching the live characters' registry.
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export)]
pub fn frame_still(spec: String, loadout: String, size: u32, turn_yaw: f64) -> Option<OrbFrame> {
    let spec = if loadout.is_empty() {
        spec
    } else {
        fx_spec::apply_loadout(&spec, &loadout).0
    };
    character::registry::preview(|| {
        let r = fx_spec::resolve_full(&spec, None, &HashMap::new(), false);
        if !r.ok {
            return None;
        }
        let mut o = r.overrides;
        o.insert("still".into(), 1.0);
        o.insert("turnYaw".into(), turn_yaw);
        frame_with_overrides(r.state, size, 1.0, o)
    })
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
#[cfg_attr(all(not(target_arch = "wasm32"), feature = "dev"), uniffi::export)]
pub fn estimate_cost(state: String, size: u32, overrides: HashMap<String, f64>) -> Option<FxCost> {
    cost::estimate(&state, size, &overrides)
}

/// `estimate_cost` for an FX Spec resolved with `(state, inputs,
/// low_power)`. `None` if the spec has errors.
// `low_power` defaults to false, so Swift/Kotlin callers from before 1.2
// (the Studios, FxView) compile unchanged.
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "dev"),
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

/// The **parameter catalog** (docs/parameters.md, *Parameter catalog*) as
/// JSON text: every object, pattern and tunable with labels, descriptions,
/// ranges, groups, paths and per-size defaults. `spec/parameters.json` is a
/// checked-in copy. Parse it on the caller's side (one stable JSON shape
/// on every platform rather than a deep tree of UniFFI records).
#[cfg_attr(all(not(target_arch = "wasm32"), feature = "dev"), uniffi::export)]
pub fn parameter_catalog_json() -> String {
    catalog::catalog_json()
}

/// Validates engine overrides for a pattern against the catalog: unknown
/// keys (with a did-you-mean), out-of-range values, fractional values for
/// whole-number keys, renamed keys. Warnings only -- the frame is unaffected.
/// `state` is the pattern id, as everywhere in the low-level API.
#[cfg_attr(all(not(target_arch = "wasm32"), feature = "dev"), uniffi::export)]
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

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn frame_json(state: String, size: u32, t: f64) -> String {
        match crate::frame(state, size, t) {
            Some(f) => serde_json::to_string(&f).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[cfg(feature = "dev")]
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
    pub fn frame_transition_packed(
        from_state: String,
        to_state: String,
        size: u32,
        t: f64,
        blend: f64,
    ) -> Box<[f64]> {
        let f = crate::frame_transition(from_state, to_state, size, t, blend);
        crate::transport::pack(f.as_ref()).into_boxed_slice()
    }

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn conversation_at_json(json: String, t: f64, band_count: u32) -> String {
        serde_json::to_string(&crate::conversation_at(json, t, band_count))
            .unwrap_or_else(|_| "null".to_string())
    }

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn conversation_samples_json() -> String {
        serde_json::to_string(&crate::conversation_sample_names())
            .unwrap_or_else(|_| "[]".to_string())
    }

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn conversation_sample_json(name: String) -> String {
        crate::conversation_sample(name).unwrap_or_else(|| "null".to_string())
    }

    #[wasm_bindgen]
    pub fn voice_blend_json(
        sides_json: String,
        weights: Vec<f64>,
        target: u32,
        size: u32,
    ) -> String {
        let Ok(sides) = serde_json::from_str::<Vec<crate::TransitionSide>>(&sides_json) else {
            return "null".to_string();
        };
        match crate::voice_blend(sides, weights, target, size) {
            Some(m) => serde_json::to_string(&m).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn transition_mix_json(
        from_json: String,
        to_json: String,
        size: u32,
        progress: f64,
        curve: String,
    ) -> String {
        let (Ok(from), Ok(to)) = (
            serde_json::from_str::<crate::TransitionSide>(&from_json),
            serde_json::from_str::<crate::TransitionSide>(&to_json),
        ) else {
            return "null".to_string();
        };
        match crate::transition_mix(from, to, size, progress, curve) {
            Some(m) => serde_json::to_string(&m).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn frame_transition_with_overrides_packed(
        from_json: String,
        to_json: String,
        size: u32,
        t: f64,
        blend: f64,
    ) -> Box<[f64]> {
        let f = match (
            serde_json::from_str::<crate::TransitionSide>(&from_json),
            serde_json::from_str::<crate::TransitionSide>(&to_json),
        ) {
            (Ok(from), Ok(to)) => crate::frame_transition_with_overrides(from, to, size, t, blend),
            _ => None,
        };
        crate::transport::pack(f.as_ref()).into_boxed_slice()
    }

    #[wasm_bindgen]
    pub fn load_catalog_json(json: String) -> String {
        serde_json::to_string(&crate::load_catalog(json)).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn unload_catalog_json(namespace: String) -> bool {
        crate::unload_catalog(namespace)
    }

    #[wasm_bindgen]
    pub fn apply_loadout_json(spec: String, loadout: String) -> String {
        serde_json::to_string(&crate::apply_loadout(spec, loadout))
            .unwrap_or_else(|_| "null".to_string())
    }

    #[wasm_bindgen]
    pub fn cosmetics_for_json(spec: String, character: String) -> String {
        serde_json::to_string(&crate::cosmetics_for(spec, character))
            .unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn frame_still_packed(
        spec: String,
        loadout: String,
        size: u32,
        turn_yaw: f64,
    ) -> Box<[f64]> {
        let f = crate::frame_still(spec, loadout, size, turn_yaw);
        crate::transport::pack(f.as_ref()).into_boxed_slice()
    }

    #[wasm_bindgen]
    pub fn fx_spec_transition_json(json: String, from: String, to: String) -> String {
        let opt = |s: String| if s.is_empty() { None } else { Some(s) };
        serde_json::to_string(&crate::fx_spec_transition(json, opt(from), opt(to)))
            .unwrap_or_else(|_| "null".to_string())
    }

    /// `inputs_json`: `{ [name]: number }`; `previous`: `""` = none. Returns
    /// the derived state key, or `""` when no rule holds.
    /// `{ code, duration, words }` for a known effect name, else `null`.
    #[wasm_bindgen]
    pub fn effect_info_json(name: String) -> String {
        serde_json::to_string(&crate::effect_info(name)).unwrap_or_else(|_| "null".to_string())
    }

    /// The spec's `accessibility` block as JSON (`{ name, states, announce }`).
    #[wasm_bindgen]
    pub fn fx_spec_accessibility_json(json: String) -> String {
        serde_json::to_string(&crate::fx_spec_accessibility(json))
            .unwrap_or_else(|_| "{}".to_string())
    }

    /// `state`: `""` = none; `spec_json` / `app_json`: `{ [state]: words }`.
    #[wasm_bindgen]
    pub fn a11y_accessible_name_json(
        name: String,
        state: String,
        spec_json: String,
        app_json: String,
    ) -> String {
        let spec: HashMap<String, String> = serde_json::from_str(&spec_json).unwrap_or_default();
        let app: HashMap<String, String> = serde_json::from_str(&app_json).unwrap_or_default();
        crate::a11y_accessible_name(name, (!state.is_empty()).then_some(state), spec, app)
    }

    /// As `a11y_accessible_name_json`; `""` = no words.
    #[wasm_bindgen]
    pub fn a11y_state_words_json(
        name: String,
        state: String,
        spec_json: String,
        app_json: String,
    ) -> String {
        let spec: HashMap<String, String> = serde_json::from_str(&spec_json).unwrap_or_default();
        let app: HashMap<String, String> = serde_json::from_str(&app_json).unwrap_or_default();
        crate::a11y_state_words(name, (!state.is_empty()).then_some(state), spec, app)
            .unwrap_or_default()
    }

    /// `prev_json`: the last step's `state` (or `{}`); `words`: `""` = none.
    /// Returns `{ state, announce: string|null, recheckAt: number|null }`.
    #[wasm_bindgen]
    pub fn a11y_announce_step_json(prev_json: String, words: String, now: f64) -> String {
        let prev: crate::AnnouncerState = serde_json::from_str(&prev_json).unwrap_or_default();
        let out = crate::a11y_announce_step(prev, (!words.is_empty()).then_some(words), now);
        serde_json::to_string(&out).unwrap_or_else(|_| "null".to_string())
    }

    #[wasm_bindgen]
    pub fn fx_spec_derive_state_json(
        json: String,
        inputs_json: String,
        previous: String,
    ) -> String {
        let inputs: HashMap<String, f64> = serde_json::from_str(&inputs_json).unwrap_or_default();
        let previous = if previous.is_empty() {
            None
        } else {
            Some(previous)
        };
        crate::fx_spec_derive_state(json, inputs, previous).unwrap_or_default()
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

    #[cfg(feature = "dev")]
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

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn estimate_cost_json(state: String, size: u32, overrides_json: String) -> String {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        match crate::estimate_cost(state, size, overrides) {
            Some(c) => serde_json::to_string(&c).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn fx_spec_cost_json(json: String, ctx_json: String) -> String {
        let ctx = fx_context(&ctx_json);
        match crate::fx_spec_cost(json, ctx.state, ctx.inputs, ctx.low_power) {
            Some(c) => serde_json::to_string(&c).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[cfg(feature = "dev")]
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

    /// `character_recipe` (`""` for an id that isn't a built-in character).
    #[wasm_bindgen]
    pub fn character_recipe_json(id: String) -> String {
        crate::character_recipe(&id).unwrap_or("").to_string()
    }

    /// `expression_overrides` as JSON (`null` for an unknown name).
    #[wasm_bindgen]
    pub fn expression_overrides_json(name: String) -> String {
        match crate::expression_overrides(name) {
            Some(m) => serde_json::to_string(&m).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn pattern_layout_json(pattern: String) -> String {
        crate::pattern_layout(pattern)
    }

    #[wasm_bindgen]
    pub fn playback_seek_progress_json(aspect: f64, x: f64) -> f64 {
        crate::playback_seek_progress(aspect, x)
    }

    #[cfg(feature = "dev")]
    #[wasm_bindgen]
    pub fn check_overrides_json(state: String, size: u32, overrides_json: String) -> String {
        let overrides: HashMap<String, f64> =
            serde_json::from_str(&overrides_json).unwrap_or_default();
        serde_json::to_string(&crate::check_overrides(state, size, overrides))
            .unwrap_or_else(|_| "[]".to_string())
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

/// Every pattern this engine has, from the six families' own `STATES`.
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
        crate::character::presets::STATES,
    ]
    .concat()
}

#[cfg(test)]
mod wall_clock_tests {
    use super::*;

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
}
