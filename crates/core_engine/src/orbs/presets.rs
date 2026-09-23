//! The shipped tunings: nine states x three sizes, baked from upstream's
//! tuning session. Ported from `thinking-orbs/src/presets.ts` (the source
//! module itself, not `spec/orbs-spec.json` -- at the point this was ported,
//! the extracted spec's `enums.sizes` only listed `[64, 20]` while
//! `presets.ts` already shipped a third size, `32`, for every mode. Treat
//! `presets.ts` as ground truth if the two ever disagree again.

use crate::orbs::profiles::{base_profiles, opts, scale_counts, scale_radii, ModeOpts};
use std::collections::HashMap;

// Not yet consumed internally -- reserved for the future uniffi-bindgen /
// enum-export pass so Swift/Kotlin/TS get a real `OrbState` enum instead of
// a bare string.
#[allow(dead_code)]
pub const STATES: &[&str] = &[
    "working",
    "searching",
    "solving",
    "listening",
    "connecting",
    "weaving",
    "composing",
    "breathing",
    "shaping",
    // Not part of the original 9 ported states -- see each mode's header
    // comment in `orbs/modes/` (aurora, webflow, spectrum, sonar, warp,
    // chladni, eclipse, crystallize).
    "glowing",
    "drifting",
    "speaking",
    "confirming",
    "initializing",
    "calibrating",
    "progressing",
    "concluding",
    // Not a port either -- the "mic muted / connection lost" resting state,
    // see `orbs/modes/hush.rs`.
    "muted",
];

pub fn state_to_mode(state: &str) -> Option<&'static str> {
    Some(match state {
        "working" => "orbits",
        "searching" => "globe",
        "solving" => "rubik",
        "listening" => "wave",
        "connecting" => "web",
        "weaving" => "braid",
        "composing" => "ribbon",
        "breathing" => "ring",
        "shaping" => "morph",
        "glowing" => "aurora",
        "drifting" => "webflow",
        "speaking" => "spectrum",
        "confirming" => "sonar",
        "initializing" => "warp",
        "calibrating" => "chladni",
        "progressing" => "eclipse",
        "concluding" => "crystallize",
        "muted" => "hush",
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub struct Preset {
    pub speed: f64,
    pub count: f64,
    pub size: f64,
    /// Extra mode opts merged verbatim after scaling.
    pub extra: Option<ModeOpts>,
}

fn preset(speed: f64, count: f64, size: f64) -> Preset {
    Preset {
        speed,
        count,
        size,
        extra: None,
    }
}

fn preset_extra(speed: f64, count: f64, size: f64, extra: &[(&str, f64)]) -> Preset {
    Preset {
        speed,
        count,
        size,
        extra: Some(opts(extra)),
    }
}

/// mode -> size -> Preset. Exported so a future `scripts/extract-spec`
/// equivalent can regenerate `spec/orbs-spec.json` from this, same as upstream.
pub fn presets() -> HashMap<&'static str, HashMap<u32, Preset>> {
    HashMap::from([
        (
            "orbits",
            HashMap::from([
                (64, preset(1.885, 1.0, 1.0)),
                (32, preset(2.9072, 0.4251, 1.6849)),
                (20, preset(3.9, 0.238, 2.4)),
            ]),
        ),
        (
            "globe",
            HashMap::from([
                (
                    64,
                    preset_extra(2.015, 0.42, 1.15, &[("scanMul", 4.08), ("dimBase", 0.45)]),
                ),
                (
                    32,
                    preset_extra(
                        2.3803,
                        0.1839,
                        1.4769,
                        &[("scanMul", 4.2301), ("dimBase", 0.45)],
                    ),
                ),
                (
                    20,
                    preset_extra(2.665, 0.105, 1.75, &[("scanMul", 4.335), ("dimBase", 0.45)]),
                ),
            ]),
        ),
        (
            "rubik",
            HashMap::from([
                (64, preset(1.82, 0.35, 1.05)),
                (32, preset(1.8964, 0.1537, 1.4951)),
                (20, preset(1.95, 0.088, 1.9)),
            ]),
        ),
        (
            "wave",
            HashMap::from([
                (64, preset(4.388, 0.341, 1.0)),
                (32, preset(4.1512, 0.169, 1.3232)),
                (20, preset(3.998, 0.105, 1.6)),
            ]),
        ),
        (
            "web",
            HashMap::from([
                (64, preset(3.315, 1.35, 0.95)),
                (32, preset(5.0104, 0.4942, 1.2571)),
                (20, preset(6.63, 0.25, 1.52)),
            ]),
        ),
        (
            "braid",
            HashMap::from([
                (64, preset(1.625, 0.5, 1.0)),
                (32, preset(2.2234, 0.2056, 1.2011)),
                (20, preset(2.75, 0.1125, 1.36)),
            ]),
        ),
        (
            "ribbon",
            HashMap::from([
                (
                    64,
                    preset_extra(
                        2.34,
                        0.25,
                        0.85,
                        &[("spin", 0.0), ("bandMul", 3.9), ("wobMul", 1.0)],
                    ),
                ),
                (
                    32,
                    preset_extra(
                        2.7776,
                        0.0969,
                        0.9766,
                        &[("spin", 0.0), ("bandMul", 4.49), ("wobMul", 1.0)],
                    ),
                ),
                (
                    20,
                    preset_extra(
                        3.12,
                        0.051,
                        1.073,
                        &[("spin", 0.0), ("bandMul", 4.94), ("wobMul", 1.0)],
                    ),
                ),
            ]),
        ),
        (
            "ring",
            HashMap::from([
                (
                    64,
                    preset_extra(
                        3.24,
                        0.25,
                        0.956,
                        &[("spin", 0.0), ("bandMul", 3.627), ("wobMul", 0.368)],
                    ),
                ),
                (
                    32,
                    preset_extra(
                        3.5517,
                        0.0678,
                        1.31,
                        &[("spin", 0.0), ("bandMul", 3.8265), ("wobMul", 0.4751)],
                    ),
                ),
                (
                    20,
                    preset_extra(
                        3.78,
                        0.028,
                        1.622,
                        &[("spin", 0.0), ("bandMul", 3.968), ("wobMul", 0.565)],
                    ),
                ),
            ]),
        ),
        (
            "morph",
            HashMap::from([
                (64, preset_extra(2.405, 0.702, 0.395, &[("spread", 1.45)])),
                (
                    32,
                    preset_extra(2.2057, 0.5937, 0.6916, &[("spread", 1.45)]),
                ),
                (20, preset_extra(2.08, 0.53, 1.011, &[("spread", 1.45)])),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/aurora.rs`'s header. Hand-picked,
            // not upstream-tuned: same caveat the feature status carries
            // for the Studio's knob ranges.
            "aurora",
            HashMap::from([
                (64, preset(1.6, 1.0, 1.0)),
                (32, preset(1.9, 0.5, 1.3)),
                (20, preset(2.2, 0.28, 1.6)),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/hush.rs`'s header. Same count/
            // size scaling as `aurora` (same Fibonacci lattice); speed stays
            // 1.0 because its only motion, the breathing pulse, is specified
            // in seconds (`period`) and shouldn't run faster when small.
            "hush",
            HashMap::from([
                (64, preset(1.0, 1.0, 1.0)),
                (32, preset(1.0, 0.5, 1.3)),
                (20, preset(1.0, 0.28, 1.6)),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/spectrum.rs`'s header. `barCount`/
            // `barDotCount`/`dotSize` aren't count/radius keys `scale_counts`/
            // `scale_radii` know about, so they're set directly per size via
            // `extra` rather than left to the scaling machinery.
            "spectrum",
            HashMap::from([
                (
                    64,
                    preset_extra(
                        1.0,
                        1.0,
                        1.0,
                        &[("barCount", 24.0), ("barDotCount", 6.0), ("dotSize", 1.0)],
                    ),
                ),
                (
                    32,
                    preset_extra(
                        1.0,
                        1.0,
                        1.0,
                        &[("barCount", 16.0), ("barDotCount", 4.0), ("dotSize", 0.85)],
                    ),
                ),
                (
                    20,
                    preset_extra(
                        1.0,
                        1.0,
                        1.0,
                        &[("barCount", 12.0), ("barDotCount", 3.0), ("dotSize", 0.7)],
                    ),
                ),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/sonar.rs`'s header.
            "sonar",
            HashMap::from([
                (
                    64,
                    preset_extra(1.0, 1.0, 1.0, &[("ringCount", 48.0), ("coreSize", 1.3)]),
                ),
                (
                    32,
                    preset_extra(1.0, 1.0, 1.0, &[("ringCount", 32.0), ("coreSize", 1.1)]),
                ),
                (
                    20,
                    preset_extra(1.0, 1.0, 1.0, &[("ringCount", 20.0), ("coreSize", 0.9)]),
                ),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/warp.rs`'s header.
            "warp",
            HashMap::from([
                (64, preset_extra(1.0, 1.0, 1.0, &[("starCount", 120.0)])),
                (32, preset_extra(1.0, 1.0, 1.0, &[("starCount", 70.0)])),
                (20, preset_extra(1.0, 1.0, 1.0, &[("starCount", 45.0)])),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/chladni.rs`'s header. `nodeCount`/
            // `nodeSize` reused from `web`'s/`aurora`'s existing count/radius
            // keys, same as `aurora`.
            "chladni",
            HashMap::from([
                (64, preset(1.0, 1.0, 1.0)),
                (32, preset(1.0, 0.5, 1.3)),
                (20, preset(1.0, 0.28, 1.6)),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/eclipse.rs`'s header.
            "eclipse",
            HashMap::from([
                (64, preset(1.0, 1.0, 1.0)),
                (32, preset(1.0, 0.5, 1.3)),
                (20, preset(1.0, 0.28, 1.6)),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/crystallize.rs`'s header. Vertex
            // count is fixed by the polyhedron (12 or 6), not size-scaled --
            // no `count` multiplier needed.
            "crystallize",
            HashMap::from([
                (64, preset(1.0, 1.0, 1.0)),
                (32, preset(1.0, 1.0, 1.3)),
                (20, preset(1.0, 1.0, 1.6)),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/webflow.rs`'s header. Deliberately
            // IDENTICAL preset numbers to "web" at every size -- the only
            // variable between `connecting` and `drifting` should be the
            // noise function, nothing else.
            "webflow",
            HashMap::from([
                (64, preset(3.315, 1.35, 0.95)),
                (32, preset(5.0104, 0.4942, 1.2571)),
                (20, preset(6.63, 0.25, 1.52)),
            ]),
        ),
    ])
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// Resolve a (state, size) pair to its mode + fully-scaled draw options.
/// Unlike upstream this does not memoize -- callers that resolve the same
/// pair every frame should cache the `Resolved` themselves.
pub fn resolve_preset(state: &str, size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    let presets = presets();
    let preset = presets.get(mode)?.get(&size)?;
    let base = base_profiles();
    let mut o = base.get(mode)?.clone();

    if preset.count != 1.0 {
        o = scale_counts(&o, preset.count);
    }
    if preset.size != 1.0 {
        o = scale_radii(&o, preset.size);
    }
    if let Some(extra) = &preset.extra {
        for (k, v) in extra {
            o.insert(k.clone(), *v);
        }
    }

    Some(Resolved {
        mode,
        speed: preset.speed,
        opts: o,
    })
}
