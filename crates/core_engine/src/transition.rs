//! State transitions (docs/fx-spec.md, *Transitions*; the design note is
//! sinua-studio/docs/agents/families/design-01-state-transitions.md).
//!
//! When a view changes state it asks this module how to get there. The
//! engine stays stateless: the caller keeps the clock (how far into the
//! transition it is), both sides' resolved design, and the phase; this
//! only says what to draw at that instant.
//!
//! Three techniques, best first:
//! - `params`: the same pattern on both sides (every voice state, most FX Spec
//!   states). Continuous parameters and the speed are interpolated, so the
//!   shape flows. Counts and choices can't be, so they swap in a short
//!   window in the middle ([`SWAP_START`]..[`SWAP_END`]) as a brief dissolve
//!   of two frames that share every continuous value -- a dissolve over the
//!   whole transition dims its middle (the prototype, 2026-09-29).
//! - `morph`: two lattice-sharing orb patterns (glowing / calibrating /
//!   progressing): the point-by-point [`crate::frame_transition_with_overrides`].
//! - `crossFade`: anything else, the old dissolve of two independent frames.

use std::collections::HashMap;

use crate::catalog;
use crate::reactive;

/// Where the structural swap starts and ends, as a share of the eased curve.
pub const SWAP_START: f64 = 0.4;
pub const SWAP_END: f64 = 0.6;
/// Without a spec's `transitions` block (and for plain views).
pub const DEFAULT_DURATION: f64 = 0.6;
pub const DEFAULT_CURVE: &str = "easeInOut";

/// The orb patterns that share one point lattice (`orbs::modes::transition`).
const LATTICE: [&str; 3] = ["glowing", "calibrating", "progressing"];

/// One side of a transition: the state (pattern) it draws, the speed
/// multiplier the caller runs it at, and its design overrides (a spec's
/// resolved overrides, or a voice-state profile merged under the app's).
/// Live runtime keys (audio, pointer) are not part of either side: the
/// caller spreads them over the result.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionSide {
    pub state: String,
    pub speed: f64,
    pub overrides: HashMap<String, f64>,
}

/// What to draw at one instant of a transition.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionMix {
    /// `params`, `morph` or `crossFade`.
    pub technique: String,
    /// The eased progress, `0..1`: the `morph`/`crossFade` blend.
    pub weight: f64,
    /// The speed multiplier to run the phase at now.
    pub speed: f64,
    /// `params`: the design to draw -- continuous keys interpolated, counts
    /// and choices at the from side. Empty for the other techniques.
    pub overrides: HashMap<String, f64>,
    /// `params`: the to side's value of every count/choice that differs.
    /// Empty = one frame; otherwise draw `overrides` and `overrides` +
    /// `structural_to`, dissolved by `swap`.
    pub structural_to: HashMap<String, f64>,
    /// `params`: the weight of the `structural_to` frame, `0..1`.
    pub swap: f64,
    /// Accumulated rate key (`pulsePeriodCycles`, ...) → its rate now, per second of
    /// engine time (design note 31, TS1). The view adds `rate × dt` to each and passes
    /// the sums with the overrides, so a rate that changes mid-session doesn't make the
    /// motion jump. Empty when the design sets no rate key.
    pub rates: HashMap<String, f64>,
}

/// The duration (seconds) and CSS keyword curve of one state change.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxTransition {
    pub duration: f64,
    pub curve: String,
    /// The spec's author wrote `curve` for this change (design note 31): a view
    /// keeps that curve, carrying the motion's velocity into it. Otherwise the view
    /// uses its own transition clock and `curve` is the 0.6 s default's name.
    pub authored: bool,
}

impl Default for FxTransition {
    fn default() -> Self {
        FxTransition {
            duration: DEFAULT_DURATION,
            curve: DEFAULT_CURVE.to_string(),
            authored: false,
        }
    }
}

/// The technique a pair of states gets.
pub fn technique(from: &str, to: &str) -> &'static str {
    if from == to {
        "params"
    } else if LATTICE.contains(&from) && LATTICE.contains(&to) {
        "morph"
    } else {
        "crossFade"
    }
}

fn is_hue(key: &str) -> bool {
    key == "hue" || key.ends_with("Hue")
}

/// Shortest way round the colour circle.
fn lerp_hue(a: f64, b: f64, w: f64) -> f64 {
    let d = ((b - a) % 360.0 + 540.0) % 360.0 - 180.0;
    a + d * w
}

/// `key` on `mode`: continuous (`number`) or structural (anything else).
/// A key the catalog doesn't know is treated as continuous.
fn is_structural(mode: &str, key: &str) -> bool {
    catalog::key_info(mode, key).is_some_and(|(t, _)| t != "number" && t != "number[]")
}

/// The value a side has for `key` when it doesn't set it: the preset's
/// resolved opt, then the catalog fallback, then 0.
fn effective(preset: &HashMap<String, f64>, mode: &str, key: &str) -> f64 {
    preset
        .get(key)
        .copied()
        .or_else(|| catalog::key_info(mode, key).and_then(|(_, f)| f))
        .unwrap_or(0.0)
}

/// What to draw at `progress` (`0..1`, linear time share) of the change
/// `from` → `to`, eased by `curve` (a CSS keyword; unknown = linear).
/// `None` if a side's state doesn't resolve.
pub fn mix(
    from: &TransitionSide,
    to: &TransitionSide,
    size: u32,
    progress: f64,
    curve: &str,
) -> Option<TransitionMix> {
    let tech = technique(&from.state, &to.state);
    let w = reactive::ease(curve, progress);
    let speed = if tech == "crossFade" {
        to.speed
    } else {
        from.speed + (to.speed - from.speed) * w
    };
    let out = TransitionMix {
        technique: tech.to_string(),
        weight: w,
        speed,
        overrides: HashMap::new(),
        structural_to: HashMap::new(),
        swap: 0.0,
        rates: HashMap::new(),
    };
    if tech != "params" {
        crate::resolve_state(&from.state, size)?;
        crate::resolve_state(&to.state, size)?;
        return Some(out);
    }
    let mut m = blend_core(&[from.clone(), to.clone()], &[1.0 - w, w], 1, size, true)?;
    m.technique = out.technique;
    m.weight = w;
    m.speed = speed;
    Some(m)
}

/// The rate keys (design note 31, TS1): the key, the accumulated key a mode reads in
/// place of `t × rate` ([`crate::primitives::cycles`]), and whether the key is a period
/// (rate = 1 / value). `warp` multiplies two of them: see [`rates`].
static RATES: &[(&str, &str, bool)] = &[
    ("pulsePeriod", "pulsePeriodCycles", true),
    ("noiseSpeed", "noiseSpeedCycles", false),
    ("holoSpeed", "holoSpeedCycles", false),
    ("spin", "spinCycles", false),
    ("scanMul", "scanMulCycles", false),
    ("holdDuration", "holdDurationCycles", true),
    ("surfaceSpeed", "surfaceSpeedCycles", false),
    ("hueSpeed", "hueSpeedCycles", false),
    ("jumpSpeed", "jumpSpeedCycles", false),
    ("period", "periodCycles", true),
    ("waveSpeed", "waveSpeedCycles", false),
    ("scanSpeed", "scanSpeedCycles", false),
];

/// The layout key of a count `mode` draws as a density (design note 31, TS7): its
/// elements fade in and out instead of the whole set being laid out again.
fn density_layout(mode: &str, key: &str) -> Option<&'static str> {
    match (mode, key) {
        ("aurora" | "chladni" | "eclipse", "nodeCount") => Some("nodeCountLayout"),
        ("spectrum", "barCount") => Some("barCountLayout"),
        ("sonar", "echoCount") => Some("echoCountLayout"),
        _ => None,
    }
}

/// The weighted sum of the sides' values of `key`.
#[inline(never)]
fn weighted(
    sides: &[TransitionSide],
    w: &[f64],
    preset: &HashMap<String, f64>,
    mode: &str,
    key: &str,
) -> f64 {
    sides
        .iter()
        .zip(w)
        .map(|(s, w)| side_value(s, sides, preset, mode, key) * w)
        .sum()
}

/// A side's value of `key`: its own, else the preset's / catalog's. A palette colour
/// (design note 19) a side lacks is another side's, so only its weight `.w` blends and
/// the hue never sweeps from 0.
#[inline(never)]
fn side_value(
    s: &TransitionSide,
    sides: &[TransitionSide],
    preset: &HashMap<String, f64>,
    mode: &str,
    key: &str,
) -> f64 {
    if let Some(v) = s.overrides.get(key) {
        return *v;
    }
    if key.starts_with("palette.") && !key.ends_with(".w") {
        if let Some(v) = sides.iter().find_map(|o| o.overrides.get(key)) {
            return *v;
        }
    }
    effective(preset, mode, key)
}

/// The weighted mix of one pattern's sides (`weights` ≥ 0, one per side; `target` is the
/// state the view is heading to). `None`
/// when the sides draw different patterns or a state doesn't resolve.
#[inline(never)]
pub fn blend(
    sides: &[TransitionSide],
    weights: &[f64],
    target: usize,
    size: u32,
) -> Option<TransitionMix> {
    let total: f64 = weights.iter().map(|w| w.max(0.0)).sum::<f64>().max(1e-12);
    let w: Vec<f64> = weights.iter().map(|x| x.max(0.0) / total).collect();
    blend_core(sides, &w, target, size, false)
}

/// [`blend`] and [`mix`]'s `params` in one key loop. `pair` is a two-sided state change
/// (`mix`): counts and choices stay the from side's with the to side's dissolved in by
/// the eased progress `w[1]`, numbers interpolate `a + (b − a) × w` (exact at the ends),
/// and densities swap like any count. Otherwise (a voice-state blend) counts and choices
/// come from the heaviest side and densities blend with their layout.
#[inline(never)]
fn blend_core(
    sides: &[TransitionSide],
    w: &[f64],
    target: usize,
    size: u32,
    pair: bool,
) -> Option<TransitionMix> {
    let first = sides.first()?;
    if sides.len() != w.len()
        || target >= sides.len()
        || sides.iter().any(|s| s.state != first.state)
    {
        return None;
    }
    let (mode, preset) = crate::resolve_state(&first.state, size)?;
    // The two heaviest sides (counts and choices come from them); a pair: from, to.
    let (mut i1, mut i2) = (0, if pair { 1 } else { usize::MAX });
    if !pair {
        for i in 1..w.len() {
            if w[i] > w[i1] {
                i2 = i1;
                i1 = i;
            } else if i2 == usize::MAX || w[i] > w[i2] {
                i2 = i;
            }
        }
    }
    let mut keys: Vec<&String> = sides.iter().flat_map(|s| s.overrides.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut out = TransitionMix {
        technique: "params".to_string(),
        weight: w[i1],
        speed: sides.iter().zip(w).map(|(s, w)| s.speed * w).sum(),
        overrides: HashMap::new(),
        structural_to: HashMap::new(),
        swap: 0.0,
        rates: HashMap::new(),
    };
    for key in keys {
        if key.ends_with("Layout") && out.overrides.contains_key(key.as_str()) {
            continue;
        }
        let val = |s: &TransitionSide| side_value(s, sides, &preset, mode, key);
        // A count the mode draws as a density (TS7), when the sides carry its layout
        // (the profile adds it for 1.13+): blended like any number. Otherwise it swaps.
        let layout = density_layout(mode, key)
            .filter(|l| !pair && sides.iter().any(|s| s.overrides.contains_key(*l)));
        let v = if let Some(layout) = layout {
            let max = sides
                .iter()
                .flat_map(|s| [val(s), s.overrides.get(layout).copied().unwrap_or(0.0)])
                .fold(f64::MIN, f64::max);
            out.overrides.insert(layout.to_string(), max);
            weighted(sides, w, &preset, mode, key)
        } else if catalog::arrives_at_once(mode, key) {
            // An arrival value (a character's `turnBlink`): the new state's, at once.
            val(&sides[target])
        } else if is_structural(mode, key) {
            let a = val(&sides[i1]);
            if i2 != usize::MAX {
                let b = val(&sides[i2]);
                if a != b {
                    out.structural_to.insert(key.clone(), b);
                }
            }
            a
        } else if is_hue(key) || (key.starts_with("palette.") && key.ends_with(".h")) {
            // Folded shortest-way blends: exact for two sides, close for three.
            let (mut h, mut seen) = (0.0, 0.0);
            for (s, w) in sides.iter().zip(w) {
                if *w > 0.0 {
                    let x = val(s);
                    h = if seen == 0.0 {
                        x
                    } else {
                        lerp_hue(h, x, w / (seen + w))
                    };
                    seen += w;
                }
            }
            if seen == 0.0 {
                val(&sides[i1])
            } else {
                h
            }
        } else if pair {
            let (a, b) = (val(&sides[0]), val(&sides[1]));
            a + (b - a) * w[1]
        } else {
            weighted(sides, w, &preset, mode, key)
        };
        out.overrides.insert(key.clone(), v);
    }
    if pair && w[1] >= 1.0 {
        // Endpoints are exact: at the end the counts and choices are the to side's.
        for (k, v) in out.structural_to.drain() {
            out.overrides.insert(k, v);
        }
    } else if !out.structural_to.is_empty() {
        let share = if pair {
            w[1]
        } else {
            w[i2] / (w[i1] + w[i2]).max(1e-12)
        };
        out.swap = ((share - SWAP_START) / (SWAP_END - SWAP_START)).clamp(0.0, 1.0);
    }
    out.rates = rates(
        mode,
        &out.overrides,
        crate::preset_speed(&first.state, size),
    );
    Some(out)
}

/// The rate of every rate key the design sets (see [`RATES`]).
#[inline(never)]
fn rates(mode: &str, o: &HashMap<String, f64>, preset_speed: f64) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let warp = mode == "warp";
    // The pulse, the noise and the holographic drift run in wall-clock seconds (engine
    // time over the preset speed): their cycles per second of engine time are scaled.
    let wall = if preset_speed > 0.0 {
        1.0 / preset_speed
    } else {
        1.0
    };
    for &(key, acc, period) in RATES {
        let v = if warp && key == "period" {
            // warp's stars run at `warpSpeed / period`.
            match (o.get("warpSpeed"), o.get("period")) {
                (Some(s), Some(p)) => Some(s / p.max(0.05)),
                _ => None,
            }
        } else {
            let k = if matches!(key, "pulsePeriod" | "noiseSpeed" | "holoSpeed") {
                wall
            } else {
                1.0
            };
            o.get(key)
                .map(|v| k * if period { 1.0 / v.max(0.05) } else { *v })
        };
        if let Some(v) = v {
            put(
                &mut out,
                if warp && key == "period" {
                    "warpCycles"
                } else {
                    acc
                },
                v,
            );
        }
    }
    out
}

#[inline(never)]
fn put(out: &mut HashMap<String, f64>, key: &str, v: f64) {
    out.insert(key.to_string(), v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(state: &str, speed: f64, o: &[(&str, f64)]) -> TransitionSide {
        TransitionSide {
            state: state.into(),
            speed,
            overrides: o.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn an_arrival_value_takes_the_new_states_value_at_once() {
        // A character's turn blink: on at once entering thinking, off at once leaving it.
        let listening = side("buzzy", 1.0, &[("turnBlink", 0.0), ("gazeX", 0.0)]);
        let thinking = side("buzzy", 1.0, &[("turnBlink", 1.0), ("gazeX", -8.0)]);
        let into = mix(&listening, &thinking, 64, 0.1, "linear").unwrap();
        assert_eq!(into.overrides["turnBlink"], 1.0);
        assert!(
            (into.overrides["gazeX"] + 0.8).abs() < 1e-12,
            "the gaze still slides"
        );
        let out_of = mix(&thinking, &listening, 64, 0.1, "linear").unwrap();
        assert_eq!(out_of.overrides["turnBlink"], 0.0);
        assert!(
            out_of.structural_to.is_empty() && out_of.swap == 0.0,
            "no dissolve for it"
        );
    }

    #[test]
    fn technique_per_pair() {
        assert_eq!(technique("glowing", "glowing"), "params");
        assert_eq!(technique("glowing", "progressing"), "morph");
        assert_eq!(technique("glowing", "scanning"), "crossFade");
        assert_eq!(technique("working", "searching"), "crossFade");
    }

    #[test]
    fn endpoints_are_exact_and_the_middle_interpolates() {
        let a = side("glowing", 0.6, &[("ink", 0.72), ("glowStrength", 0.1)]);
        let b = side("glowing", 1.15, &[("ink", 1.0), ("glowStrength", 0.4)]);
        let m0 = mix(&a, &b, 64, 0.0, "linear").unwrap();
        assert_eq!(m0.overrides["ink"], 0.72);
        assert_eq!(m0.speed, 0.6);
        let m1 = mix(&a, &b, 64, 1.0, "linear").unwrap();
        assert_eq!(m1.overrides["ink"], 1.0);
        assert_eq!(m1.overrides["glowStrength"], 0.4);
        assert_eq!(m1.speed, 1.15);
        let mid = mix(&a, &b, 64, 0.5, "linear").unwrap();
        assert!((mid.overrides["ink"] - 0.86).abs() < 1e-12);
        assert!((mid.speed - 0.875).abs() < 1e-12);
        assert_eq!(mid.technique, "params");
    }

    #[test]
    fn a_key_one_side_lacks_starts_from_its_effective_value() {
        // glowStrength has a catalog fallback; the idle side doesn't set it.
        let (_, fallback) = catalog::key_info("aurora", "glowStrength").unwrap();
        let a = side("glowing", 1.0, &[]);
        let b = side("glowing", 1.0, &[("glowStrength", 0.4)]);
        let m = mix(&a, &b, 64, 0.0, "linear").unwrap();
        assert_eq!(m.overrides["glowStrength"], fallback.unwrap_or(0.0));
        // A preset key starts from the preset's value, not the catalog's.
        let (mode, preset) = crate::resolve_state("glowing", 64).unwrap();
        assert_eq!(mode, "aurora");
        let n = preset["nodeCount"];
        let m = mix(
            &side("glowing", 1.0, &[]),
            &side("glowing", 1.0, &[("nodeCount", 220.0)]),
            64,
            0.0,
            "linear",
        )
        .unwrap();
        assert_eq!(m.overrides["nodeCount"], n);
    }

    #[test]
    fn counts_and_choices_swap_only_in_the_middle_window() {
        let a = side(
            "glowing",
            1.0,
            &[("nodeCount", 220.0), ("particleStyle", 1.0), ("ink", 0.7)],
        );
        let b = side(
            "glowing",
            1.0,
            &[("nodeCount", 260.0), ("particleStyle", 2.0), ("ink", 1.0)],
        );
        for (u, swap) in [(0.2, 0.0), (0.4, 0.0), (0.5, 0.5), (0.6, 1.0), (0.8, 1.0)] {
            let m = mix(&a, &b, 64, u, "linear").unwrap();
            assert!((m.swap - swap).abs() < 1e-12, "u={u}: {}", m.swap);
            assert_eq!(
                m.overrides["nodeCount"], 220.0,
                "the base frame keeps the from count"
            );
            assert_eq!(m.structural_to["nodeCount"], 260.0);
            assert_eq!(m.structural_to["particleStyle"], 2.0);
            assert!(
                !m.structural_to.contains_key("ink"),
                "continuous keys never swap"
            );
        }
        let end = mix(&a, &b, 64, 1.0, "linear").unwrap();
        assert!(end.structural_to.is_empty());
        assert_eq!(end.overrides["nodeCount"], 260.0);
        // Same counts on both sides: one frame, no swap.
        let same = mix(
            &side("glowing", 1.0, &[("nodeCount", 220.0)]),
            &side("glowing", 1.0, &[("nodeCount", 220.0)]),
            64,
            0.5,
            "linear",
        )
        .unwrap();
        assert!(same.structural_to.is_empty() && same.swap == 0.0);
    }

    #[test]
    fn hue_takes_the_short_way_round() {
        let a = side("glowing", 1.0, &[("colorHue", 350.0)]);
        let b = side("glowing", 1.0, &[("colorHue", 10.0)]);
        let m = mix(&a, &b, 64, 0.5, "linear").unwrap();
        assert!(
            (m.overrides["colorHue"].rem_euclid(360.0) - 0.0).abs() < 1e-9,
            "{}",
            m.overrides["colorHue"]
        );
    }

    #[test]
    fn the_curve_eases_the_weight() {
        let a = side("glowing", 1.0, &[("ink", 0.0)]);
        let b = side("glowing", 1.0, &[("ink", 1.0)]);
        let lin = mix(&a, &b, 64, 0.25, "linear").unwrap().weight;
        let eio = mix(&a, &b, 64, 0.25, "easeInOut").unwrap().weight;
        assert_eq!(lin, 0.25);
        assert!(eio < lin, "easeInOut starts slower");
        assert_eq!(mix(&a, &b, 64, 1.0, "easeInOut").unwrap().weight, 1.0);
        assert_eq!(mix(&a, &b, 64, 0.0, "easeOut").unwrap().weight, 0.0);
    }

    #[test]
    fn other_techniques_carry_only_the_blend_and_speed() {
        let m = mix(
            &side("glowing", 0.5, &[]),
            &side("progressing", 1.0, &[]),
            64,
            0.5,
            "linear",
        )
        .unwrap();
        assert_eq!(m.technique, "morph");
        assert!(m.overrides.is_empty());
        assert_eq!(m.speed, 0.75);
        let c = mix(
            &side("glowing", 0.5, &[]),
            &side("scanning", 1.0, &[]),
            64,
            0.5,
            "linear",
        )
        .unwrap();
        assert_eq!(c.technique, "crossFade");
        assert_eq!(
            c.speed, 1.0,
            "a cross-fade runs each side at its own speed; the new one is reported"
        );
        assert!(mix(
            &side("nope", 1.0, &[]),
            &side("glowing", 1.0, &[]),
            64,
            0.5,
            "linear"
        )
        .is_none());
    }

    #[test]
    fn every_voice_state_profile_key_is_classified_by_the_catalog() {
        // A key the catalog doesn't know would silently lerp; the profile's keys all must be known.
        let src: serde_json::Value =
            serde_json::from_str(include_str!("../../../spec/voice-state-profile.json")).unwrap();
        let mut keys = std::collections::BTreeSet::new();
        for s in src["states"].as_object().unwrap().values() {
            keys.extend(s["overrides"].as_object().unwrap().keys().cloned());
        }
        for (pattern, p) in src["patterns"].as_object().unwrap() {
            // `character`: every recipe from a file (1.12), which reads buzzy's keys.
            let pattern = if pattern == "character" {
                "buzzy"
            } else {
                pattern
            };
            let (mode, _) = crate::resolve_state(pattern, 64).unwrap();
            for s in p["states"].as_object().unwrap().values() {
                for k in s["overrides"]
                    .as_object()
                    .map(|o| o.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
                {
                    assert!(
                        catalog::key_info(mode, &k).is_some(),
                        "{pattern}/{mode}: {k} unknown to the catalog"
                    );
                }
            }
        }
        for k in keys {
            assert!(
                catalog::key_info("aurora", &k).is_some(),
                "generic profile key {k} unknown to the catalog"
            );
        }
    }

    /// The rate-key detector (design note 31): a key whose small change moves the frame
    /// far more at t = 300 s than at t = 10 s multiplies time, so a view changing it
    /// mid-session would make the motion jump. Every such key must be in [`RATES`]
    /// (accumulated by the view); a new pattern can't bring TS1 back unnoticed.
    #[test]
    fn every_key_that_multiplies_time_is_a_rate_key() {
        let cat: serde_json::Value =
            serde_json::from_str(include_str!("../../../spec/parameters.json")).unwrap();
        let defs = cat["definitions"].as_object().unwrap();
        // Materials on, so their keys are exercised too.
        let materials = [
            ("pulseStrength", 0.6),
            ("noiseStrength", 1.0),
            ("holoStrength", 1.0),
        ];
        let known: Vec<&str> = RATES.iter().map(|r| r.0).chain(["warpSpeed"]).collect();
        let shift = |f: &crate::OrbFrame, g: &crate::OrbFrame| -> Option<f64> {
            if f.dots.len() != g.dots.len()
                || f.lines.len() != g.lines.len()
                || f.polylines.len() != g.polylines.len()
            {
                return None;
            }
            let mut d = 0.0;
            for (a, b) in f.dots.iter().zip(&g.dots) {
                d += (a.x - b.x).abs() + (a.y - b.y).abs() + (a.a - b.a).abs() + (a.r - b.r).abs();
            }
            for (a, b) in f.lines.iter().zip(&g.lines) {
                d += (a.x1 - b.x1).abs()
                    + (a.y1 - b.y1).abs()
                    + (a.x2 - b.x2).abs()
                    + (a.y2 - b.y2).abs()
                    + (a.a - b.a).abs();
            }
            for (a, b) in f.polylines.iter().zip(&g.polylines) {
                if a.points.len() != b.points.len() {
                    return None;
                }
                d += a
                    .points
                    .iter()
                    .zip(&b.points)
                    .map(|(p, q)| (p.x - q.x).abs() + (p.y - q.y).abs())
                    .sum::<f64>();
                d += (a.a - b.a).abs();
            }
            Some(d)
        };
        let mut offenders = Vec::new();
        for &state in crate::orbs::presets::STATES {
            let Some((mode, preset)) = crate::resolve_state(state, 64) else {
                continue;
            };
            for (id, def) in defs {
                let (key, scope) = id.split_once('@').unwrap();
                if def["type"] != "number"
                    || (scope != mode && scope != "shared")
                    || known.contains(&key)
                {
                    continue;
                }
                let base = effective(&preset, mode, key);
                let v = if base == 0.0 { 0.5 } else { base };
                let at = |t: f64, x: f64| {
                    let mut o: HashMap<String, f64> =
                        materials.iter().map(|(k, v)| (k.to_string(), *v)).collect();
                    o.insert(key.to_string(), x);
                    crate::frame_with_overrides(state.to_string(), 64, t, o)
                };
                // Averaged over 3 s, so where a cycle happens to be doesn't decide.
                let sens = |t0: f64| -> Option<f64> {
                    (0..12)
                        .map(|k| t0 + k as f64 * 0.25)
                        .try_fold(0.0, |acc, t| {
                            Some(acc + shift(&at(t, v)?, &at(t, v * 1.002)?)?)
                        })
                };
                let (Some(s10), Some(s300)) = (sens(10.0), sens(300.0)) else {
                    continue;
                };
                if s300 > 1e-3 && s300 > 8.0 * s10 + 1e-6 {
                    offenders.push(format!(
                        "{state}/{key} ({s10:.4} at 10 s, {s300:.4} at 300 s)"
                    ));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "keys that multiply time but aren't rate keys: {offenders:#?}"
        );
    }

    /// TS2 / TS7: between voice states only a count a mode draws as a density may
    /// change (it fades); any other count or choice would swap the whole frame.
    #[test]
    fn voice_states_change_no_count_but_a_density() {
        let prof: serde_json::Value =
            serde_json::from_str(include_str!("../../../spec/voice-state-profile.json")).unwrap();
        let mut bad = Vec::new();
        for (pattern, entry) in prof["patterns"].as_object().unwrap() {
            let Some((mode, _)) = crate::resolve_state(pattern, 64) else {
                continue;
            };
            let Some(states) = entry["states"].as_object() else {
                continue;
            };
            let mut keys: Vec<&String> = states
                .values()
                .filter_map(|s| s["overrides"].as_object())
                .flat_map(|o| o.keys())
                .collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                if !is_structural(mode, key) || density_layout(mode, key).is_some() {
                    continue;
                }
                let vals: Vec<String> = crate::voice_state::VOICE_STATES
                    .iter()
                    .map(|s| {
                        states
                            .get(*s)
                            .and_then(|e| e["overrides"].get(key.as_str()))
                            .map_or("-".into(), |v| v.to_string())
                    })
                    .collect();
                let set: std::collections::HashSet<&String> =
                    vals.iter().filter(|v| *v != "-").collect();
                if set.len() > 1 {
                    bad.push(format!("{pattern}/{key}: {vals:?}"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "counts or choices that change between voice states: {bad:#?}"
        );
    }

    #[test]
    fn accumulated_cycles_equal_to_t_times_rate_draw_the_same_frame() {
        // The view's sums start at `t × rate`, so taking over from the old formula is seamless.
        for (state, key, acc, rate) in [
            ("concluding", "period", "periodCycles", 1.0 / 6.0),
            ("glowing", "surfaceSpeed", "surfaceSpeedCycles", 0.1),
        ] {
            let t = 123.4;
            let mut o: HashMap<String, f64> = HashMap::new();
            o.insert(key.into(), if key == "period" { 6.0 } else { 0.1 });
            let plain = crate::frame_with_overrides(state.into(), 64, t, o.clone()).unwrap();
            o.insert(acc.into(), t * rate);
            let acc_f = crate::frame_with_overrides(state.into(), 64, t, o).unwrap();
            let d: f64 = plain
                .dots
                .iter()
                .zip(&acc_f.dots)
                .map(|(a, b)| (a.x - b.x).abs() + (a.y - b.y).abs())
                .sum();
            assert!(d < 1e-6 * plain.dots.len().max(1) as f64, "{state}: {d}");
        }
    }

    #[test]
    fn a_voice_blend_at_one_weight_is_that_side_and_reports_rates() {
        let idle = side("concluding", 0.6, &[("period", 6.0), ("ink", 0.72)]);
        let speaking = side("concluding", 1.15, &[("period", 4.0), ("ink", 1.0)]);
        let m = blend(&[idle.clone(), speaking.clone()], &[1.0, 0.0], 1, 64).unwrap();
        assert_eq!(
            (m.overrides["period"], m.overrides["ink"], m.speed),
            (6.0, 0.72, 0.6)
        );
        assert!((m.rates["periodCycles"] - 1.0 / 6.0).abs() < 1e-12);
        let half = blend(&[idle, speaking], &[0.5, 0.5], 1, 64).unwrap();
        assert!(
            (half.overrides["period"] - 5.0).abs() < 1e-12 && (half.speed - 0.875).abs() < 1e-12
        );
        assert!(blend(
            &[side("glowing", 1.0, &[]), side("buzzy", 1.0, &[])],
            &[1.0, 0.0],
            0,
            64
        )
        .is_none());
    }

    #[test]
    fn a_voice_blend_takes_arrival_keys_from_the_target_and_lays_out_densities() {
        let listening = side("buzzy", 1.0, &[("turnBlink", 0.0)]);
        let thinking = side("buzzy", 1.0, &[("turnBlink", 1.0)]);
        // Barely started towards thinking: the blink is already thinking's.
        let m = blend(&[listening, thinking], &[0.95, 0.05], 1, 64).unwrap();
        assert_eq!(m.overrides["turnBlink"], 1.0);
        // glowing's profile varies its count (220 / 260 / 340): from 1.13 its voice states
        // carry the layout, so the count blends and the dots fade.
        let prof = |st: &str| {
            crate::voice_state::profile("glowing", st)
                .unwrap()
                .overrides
        };
        let (idle, thinking) = (prof("idle"), prof("thinking"));
        assert_eq!(
            (idle["nodeCountLayout"], thinking["nodeCountLayout"]),
            (340.0, 340.0)
        );
        let a = TransitionSide {
            state: "glowing".into(),
            speed: 1.0,
            overrides: idle,
        };
        let b = TransitionSide {
            state: "glowing".into(),
            speed: 1.0,
            overrides: thinking,
        };
        let m = blend(&[a, b], &[0.5, 0.5], 1, 64).unwrap();
        assert_eq!(
            (m.overrides["nodeCount"], m.overrides["nodeCountLayout"]),
            (280.0, 340.0)
        );
        assert!(m.structural_to.is_empty(), "a density never swaps");
        let mut o = m.overrides.clone();
        // The lattice alone (no glow halos, no particles).
        o.extend([
            ("glowStrength".to_string(), 0.0),
            ("particleStrength".to_string(), 0.0),
        ]);
        let f = crate::frame_with_overrides("glowing".into(), 64, 3.0, o).unwrap();
        assert_eq!(f.dots.len(), 280);
        // Without the layout (an older file's states) the count swaps as before.
        let a = side("glowing", 1.0, &[("nodeCount", 220.0)]);
        let b = side("glowing", 1.0, &[("nodeCount", 340.0)]);
        let old = blend(&[a, b], &[0.5, 0.5], 1, 64).unwrap();
        assert!(
            old.structural_to.contains_key("nodeCount")
                && !old.overrides.contains_key("nodeCountLayout")
        );
        let legacy = crate::voice_state::profile_for_minor("glowing", "idle", 12).unwrap();
        assert!(
            !legacy.overrides.contains_key("nodeCountLayout"),
            "1.12 files keep the swap"
        );
    }

    #[test]
    fn voice_state_changes_take_the_profiles_time_unless_the_file_says() {
        let t = crate::fx_spec::transition_for("{}", "listening", "speaking");
        assert_eq!((t.duration, t.authored), (0.25, false));
        assert_eq!(
            crate::fx_spec::transition_for("{}", "speaking", "idle").duration,
            0.9
        );
        assert_eq!(
            crate::fx_spec::transition_for("{}", "thinking", "speaking").duration,
            0.3
        );
        assert_eq!(
            crate::fx_spec::transition_for("{}", "ok", "error").duration,
            DEFAULT_DURATION
        );
        let own =
            r#"{"transitions":{"default":{"duration":0.7},"*->speaking":{"curve":"easeOut"}}}"#;
        let t = crate::fx_spec::transition_for(own, "listening", "speaking");
        assert_eq!(
            (t.duration, t.curve.as_str(), t.authored),
            (0.7, "easeOut", true)
        );
    }

    /// Every rate sum a view starts from `t × rate` draws exactly what the engine's own
    /// formula draws, on every pattern and voice state (design note 31): the moment a
    /// view takes over the sums can't move the picture (the pulse once did: it runs in
    /// wall-clock seconds, so its rate is over the preset speed).
    #[test]
    fn taking_over_the_sums_moves_nothing_on_any_pattern() {
        let mut bad = Vec::new();
        for &pattern in crate::orbs::presets::STATES {
            for state in ["idle", "listening", "thinking", "speaking"] {
                let Some(p) = crate::voice_state::profile(pattern, state) else {
                    continue;
                };
                let s = TransitionSide {
                    state: pattern.into(),
                    speed: p.speed,
                    overrides: p.overrides,
                };
                let m = blend(std::slice::from_ref(&s), &[1.0], 0, 64).unwrap();
                let t = 37.3;
                let plain = crate::frame_with_overrides(pattern.into(), 64, t, s.overrides.clone())
                    .unwrap();
                let mut o = s.overrides.clone();
                o.extend(m.rates.iter().map(|(k, r)| (k.clone(), t * r)));
                let summed = crate::frame_with_overrides(pattern.into(), 64, t, o).unwrap();
                let d = plain
                    .dots
                    .iter()
                    .zip(&summed.dots)
                    .map(|(a, b)| (a.x - b.x).abs() + (a.y - b.y).abs() + (a.a - b.a).abs())
                    .fold(0.0, f64::max)
                    + plain
                        .polylines
                        .iter()
                        .zip(&summed.polylines)
                        .map(|(a, b)| (a.a - b.a).abs())
                        .fold(0.0, f64::max);
                if plain.dots.len() != summed.dots.len() || d > 1e-6 {
                    bad.push(format!("{pattern}/{state}: {d}"));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
