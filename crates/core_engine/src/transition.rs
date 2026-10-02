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
}

/// The duration (seconds) and CSS keyword curve of one state change.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxTransition {
    pub duration: f64,
    pub curve: String,
}

impl Default for FxTransition {
    fn default() -> Self {
        FxTransition {
            duration: DEFAULT_DURATION,
            curve: DEFAULT_CURVE.to_string(),
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
    let mut out = TransitionMix {
        technique: tech.to_string(),
        weight: w,
        speed,
        overrides: HashMap::new(),
        structural_to: HashMap::new(),
        swap: 0.0,
    };
    if tech != "params" {
        crate::resolve_state(&from.state, size)?;
        crate::resolve_state(&to.state, size)?;
        return Some(out);
    }
    let (mode, preset) = crate::resolve_state(&to.state, size)?;
    let mut keys: Vec<&String> = from.overrides.keys().chain(to.overrides.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        // A palette colour (design note 19) missing on one side takes the other
        // side's: only its weight `.w` blends, so the hue never sweeps from 0.
        let colour = key.starts_with("palette.") && !key.ends_with(".w");
        let other = |o: &HashMap<String, f64>| if colour { o.get(key).copied() } else { None };
        let a = from
            .overrides
            .get(key)
            .copied()
            .or_else(|| other(&to.overrides))
            .unwrap_or_else(|| effective(&preset, mode, key));
        let b = to
            .overrides
            .get(key)
            .copied()
            .or_else(|| other(&from.overrides))
            .unwrap_or_else(|| effective(&preset, mode, key));
        if colour && key.ends_with(".h") {
            out.overrides.insert(key.clone(), lerp_hue(a, b, w));
            continue;
        }
        if catalog::arrives_at_once(mode, key) {
            // An arrival value (a character's `turnBlink`): the new state's, at once.
            out.overrides.insert(key.clone(), b);
        } else if is_structural(mode, key) {
            out.overrides.insert(key.clone(), a);
            if a != b {
                out.structural_to.insert(key.clone(), b);
            }
        } else if is_hue(key) {
            out.overrides.insert(key.clone(), lerp_hue(a, b, w));
        } else {
            out.overrides.insert(key.clone(), a + (b - a) * w);
        }
    }
    // Endpoints are exact: at the end the structural keys are the to side's.
    if w >= 1.0 {
        for (k, v) in out.structural_to.drain() {
            out.overrides.insert(k, v);
        }
    } else if !out.structural_to.is_empty() {
        out.swap = ((w - SWAP_START) / (SWAP_END - SWAP_START)).clamp(0.0, 1.0);
    }
    Some(out)
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
}
