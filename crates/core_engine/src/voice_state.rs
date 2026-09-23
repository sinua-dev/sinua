//! The **voice-state profile**: what each agent state (`initializing`,
//! `idle`, `listening`, `thinking`, `speaking`) does to *any* pattern.
//!
//! One shape stays on screen for a whole conversation and the state changes
//! how it behaves -- the product decision behind FX Spec 1.8 (the user, via
//! the orchestrator, 2026-09-20). The
//! language, from the review page that compared all 34 patterns:
//!
//! - **idle** rests: slower, lighter ink, a long breath, no audio.
//! - **listening** takes in: the mic *draws the shape inward* (a negative
//!   `audioStrength`) and particles flow in with it.
//! - **thinking** works quietly: a faster inner churn, no audio.
//! - **speaking** gives out: the agent's voice swells the shape outward,
//!   particles drift out, the glow comes up.
//!
//! The data lives in `spec/voice-state-profile.json` -- checked in, so the
//! Studio, the docs site's gallery and this engine all read the same floats
//! -- and is parsed once here, the way `catalog.rs` parses its own source.
//! This module only *returns* values: `frame()` is untouched and the engine
//! stays stateless. The FX Spec resolver fills them into a 1.8+ file's
//! voice-state entries for keys the file doesn't set (`fx_spec.rs`), and a
//! view without a spec asks for them directly.
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The five keys this profile knows, in lifecycle order. They are also the
/// `states` keys an FX Spec is expected to use for a voice agent, and the
/// names LiveKit's `AgentState` uses.
pub const VOICE_STATES: [&str; 5] = ["initializing", "idle", "listening", "thinking", "speaking"];

/// Bumped when the profile's values change, so a tool can cache them and
/// tell "tuned by the user" from "came with the engine" (the Studio's ask).
pub fn profile_version() -> u64 {
    source()["profileVersion"].as_u64().unwrap_or(0)
}

fn source() -> &'static Value {
    static SRC: OnceLock<Value> = OnceLock::new();
    SRC.get_or_init(|| {
        serde_json::from_str(include_str!("../../../spec/voice-state-profile.json"))
            .expect("spec/voice-state-profile.json parses")
    })
}

/// One state's behaviour on one pattern.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceStateProfile {
    /// Multiplies the pattern's tuned speed (1 = unchanged).
    pub speed: f64,
    /// Engine keys to apply -- `ink`, `audioStrength`, particles, glow, and
    /// whatever a per-pattern entry tunes.
    pub overrides: HashMap<String, f64>,
    #[cfg_attr(target_arch = "wasm32", serde(rename = "audioInput"))]
    /// Which app input a view should feed into `audioLevel` for this state
    /// (`micLevel` while listening, `agentVolume` while speaking), or `None`
    /// when the state ignores audio. Not an engine key.
    pub audio_input: Option<String>,
}

fn numbers(v: Option<&Value>) -> HashMap<String, f64> {
    v.and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n)))
                .collect()
        })
        .unwrap_or_default()
}

/// The profile for `state` on `pattern`: the generic state, with the
/// pattern's own entry over it (a per-pattern `speed` or override wins).
/// `None` for an unknown state -- an app's own key, like "recording", is
/// simply not part of the voice language and gets the file's design as-is.
pub fn profile(pattern: &str, state: &str) -> Option<VoiceStateProfile> {
    let src = source();
    let generic = src["states"].get(state)?;
    let own = src["patterns"]
        .get(pattern)
        .and_then(|p| p["states"].get(state));

    let mut overrides = numbers(generic.get("overrides"));
    for (k, v) in numbers(own.and_then(|o| o.get("overrides"))) {
        overrides.insert(k, v);
    }
    let speed = own
        .and_then(|o| o["speed"].as_f64())
        .or_else(|| generic["speed"].as_f64())
        .unwrap_or(1.0);
    let audio_input = own
        .and_then(|o| o["audioInput"].as_str())
        .or_else(|| generic["audioInput"].as_str())
        .map(str::to_string);
    Some(VoiceStateProfile {
        speed,
        overrides,
        audio_input,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_voice_state_has_a_generic_profile() {
        for s in VOICE_STATES {
            let p = profile("working", s).unwrap_or_else(|| panic!("{s} has no profile"));
            assert!(p.speed > 0.0, "{s}");
            assert!(p.overrides.contains_key("ink"), "{s} sets ink");
        }
        assert!(profile("working", "recording").is_none(), "unknown state");
    }

    #[test]
    fn the_two_talking_states_move_in_opposite_directions() {
        // The cue that makes listening and speaking tell apart: the mic
        // draws the shape in, the agent's voice pushes it out.
        let listening = profile("glowing", "listening").unwrap();
        let speaking = profile("glowing", "speaking").unwrap();
        assert!(listening.overrides["audioStrength"] < 0.0);
        assert!(speaking.overrides["audioStrength"] > 0.0);
        assert_eq!(listening.audio_input.as_deref(), Some("micLevel"));
        assert_eq!(speaking.audio_input.as_deref(), Some("agentVolume"));
        assert!(profile("glowing", "thinking")
            .unwrap()
            .audio_input
            .is_none());
    }

    #[test]
    fn idle_rests_and_the_active_states_come_to_full_ink() {
        let idle = profile("glowing", "idle").unwrap();
        assert!(idle.overrides["ink"] < 1.0 && idle.speed < 1.0);
        for s in ["listening", "thinking", "speaking"] {
            assert_eq!(profile("glowing", s).unwrap().overrides["ink"], 1.0, "{s}");
        }
    }

    #[test]
    fn a_pattern_entry_overrides_the_generic_state() {
        // `glowing` is the showcase base: its tuned values must win, and
        // the keys it doesn't name still come from the generic state.
        let generic = profile("working", "thinking").unwrap();
        let tuned = profile("glowing", "thinking").unwrap();
        assert!(!generic.overrides.contains_key("surfaceSpeed"));
        assert_eq!(tuned.overrides["surfaceSpeed"], 0.55, "aurora's own churn");
        assert_ne!(
            tuned.overrides["glowStrength"], generic.overrides["glowStrength"],
            "a tuned value wins over the generic one"
        );
        assert_eq!(
            tuned.overrides["pulsePeriod"], generic.overrides["pulsePeriod"],
            "an untouched generic key still applies"
        );
    }

    #[test]
    fn every_pattern_and_state_named_in_the_file_exists_in_the_engine() {
        let src = source();
        for (pattern, entry) in src["patterns"].as_object().unwrap() {
            assert!(
                crate::resolved_opts(pattern.clone(), 64).is_some(),
                "`patterns.{pattern}` is not a pattern this engine has"
            );
            for state in entry["states"].as_object().unwrap().keys() {
                assert!(
                    VOICE_STATES.contains(&state.as_str()),
                    "`patterns.{pattern}.states.{state}` is not a voice state"
                );
            }
        }
        for state in src["states"].as_object().unwrap().keys() {
            assert!(VOICE_STATES.contains(&state.as_str()), "{state}");
        }
    }

    #[test]
    fn every_override_key_is_one_the_catalog_knows() {
        // A typo here would silently do nothing at render time.
        let src = source();
        let mut blocks: Vec<&Value> = src["states"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|s| s.get("overrides"))
            .collect();
        for p in src["patterns"].as_object().unwrap().values() {
            blocks.extend(
                p["states"]
                    .as_object()
                    .unwrap()
                    .values()
                    .filter_map(|s| s.get("overrides")),
            );
        }
        for b in blocks {
            for key in b.as_object().unwrap().keys() {
                assert!(
                    crate::catalog::knows_key(key),
                    "`{key}` is in the profile but not in the parameter catalog"
                );
            }
        }
    }
}
