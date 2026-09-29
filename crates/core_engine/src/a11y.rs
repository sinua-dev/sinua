//! State-aware accessibility (docs/fx-view.md, *Accessibility*): what a view is
//! called in each state, and when a state change is spoken.
//!
//! ```json
//! "accessibility": {
//!   "name": "Coach",
//!   "states": { "listening": "Coach is listening", "speaking": "Coach is speaking" },
//!   "announce": true
//! }
//! ```
//!
//! The words: the app's own `labels[state]` win, then the file's
//! `accessibility.states[state]`, then the built-in words for the voice states
//! ("<name>, listening"), then the plain name. The same function on every
//! platform, so a screen reader hears the same thing everywhere.
//!
//! The timing: a conversation flips listening / speaking every few seconds, and
//! speaking each flip is noise. [`announce_step`] is a pure step the views call
//! on every state change and again at `recheck_at`: a state is spoken once it has
//! held for [`HOLD_S`], at least [`GAP_S`] after the last announcement, and only
//! if it differs from the last one spoken. The first state a view shows is not
//! spoken (it is already in the name), and a state with no words (`idle`, an
//! app key without a label) never is. The views keep the [`AnnouncerState`]
//! between calls, as they keep easing and cross-fades (docs/fx-spec.md, *Caller
//! loop*).

use std::collections::HashMap;

use serde_json::Value;

/// A state must hold this long before it is spoken.
pub const HOLD_S: f64 = 1.0;
/// At most one announcement per this many seconds.
pub const GAP_S: f64 = 3.0;

/// The `accessibility` block's keys.
pub(crate) const KEYS: [&str; 3] = ["name", "states", "announce"];

/// The voice states' built-in words (FX Spec 1.9). `idle` has none: an idle
/// view is just its name.
pub fn default_words(state: &str) -> Option<&'static str> {
    match state {
        "initializing" => Some("Starting"),
        "listening" => Some("Listening"),
        "thinking" => Some("Thinking"),
        "speaking" => Some("Speaking"),
        _ => None,
    }
}

/// A file's `accessibility` block, read leniently (a bad type reads as absent;
/// `check` reports it).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
pub struct FxAccessibility {
    pub name: Option<String>,
    pub states: HashMap<String, String>,
    pub announce: Option<bool>,
}

/// The `accessibility` block of an FX Spec JSON text (empty for bad JSON or no block).
pub fn read(json: &str) -> FxAccessibility {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return FxAccessibility::default();
    };
    let Some(a) = root.get("accessibility").and_then(Value::as_object) else {
        return FxAccessibility::default();
    };
    FxAccessibility {
        name: a.get("name").and_then(Value::as_str).map(str::to_owned),
        states: a
            .get("states")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default(),
        announce: a.get("announce").and_then(Value::as_bool),
    }
}

/// The words for `state`, or `None` when it has none (then the view is just its
/// name, and nothing is spoken). App words, then the file's, then the built-in.
pub fn state_words(
    name: &str,
    state: Option<&str>,
    spec_words: &HashMap<String, String>,
    app_words: &HashMap<String, String>,
) -> Option<String> {
    let s = state?;
    if let Some(w) = app_words.get(s).filter(|w| !w.trim().is_empty()) {
        return Some(w.clone());
    }
    if let Some(w) = spec_words.get(s).filter(|w| !w.trim().is_empty()) {
        return Some(w.clone());
    }
    let d = default_words(s)?;
    Some(if name.trim().is_empty() {
        d.to_owned()
    } else {
        format!("{name}, {}", d.to_lowercase())
    })
}

/// The view's accessible name in `state`: its state words, else `name`.
pub fn accessible_name(
    name: &str,
    state: Option<&str>,
    spec_words: &HashMap<String, String>,
    app_words: &HashMap<String, String>,
) -> String {
    state_words(name, state, spec_words, app_words).unwrap_or_else(|| name.to_owned())
}

/// What [`announce_step`] remembers between calls. Start from `Default`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase", default))]
pub struct AnnouncerState {
    /// Whether any state has been seen yet (the first one is not spoken).
    pub started: bool,
    /// The words showing now, and since when.
    pub current: Option<String>,
    pub since: f64,
    /// The last words spoken, and when.
    pub last: Option<String>,
    pub last_at: f64,
}

/// One [`announce_step`]: the next state, the words to speak now (if any), and
/// when to call again (a state still waiting for its hold or for the gap).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct AnnounceStep {
    pub state: AnnouncerState,
    pub announce: Option<String>,
    pub recheck_at: Option<f64>,
}

/// Feed the words showing now (`None` = nothing to say: idle, an unlabelled
/// app state) at `now` seconds. Call it on every change and at `recheck_at`.
pub fn announce_step(prev: AnnouncerState, words: Option<String>, now: f64) -> AnnounceStep {
    let mut s = prev;
    if !s.started {
        // The first state is already in the view's name: remember it as said.
        s.started = true;
        s.current = words.clone();
        s.since = now;
        s.last = words;
        // "Long ago": finite, so the state crosses JSON (the wasm form) intact.
        s.last_at = -1.0e9;
        return AnnounceStep {
            state: s,
            announce: None,
            recheck_at: None,
        };
    }
    if s.current != words {
        s.current = words;
        s.since = now;
    }
    let Some(w) = s.current.clone() else {
        // Nothing to say now. A later state with words is compared with the
        // last one spoken, so "listening, idle, listening" isn't repeated.
        return AnnounceStep {
            state: s,
            announce: None,
            recheck_at: None,
        };
    };
    if s.last.as_deref() == Some(w.as_str()) {
        return AnnounceStep {
            state: s,
            announce: None,
            recheck_at: None,
        };
    }
    let due = (s.since + HOLD_S).max(s.last_at + GAP_S);
    // A small tolerance: a host timer firing a hair early still counts.
    if now + 1e-6 >= due {
        s.last = Some(w.clone());
        s.last_at = now;
        return AnnounceStep {
            state: s,
            announce: Some(w),
            recheck_at: None,
        };
    }
    AnnounceStep {
        state: s,
        announce: None,
        recheck_at: Some(due),
    }
}

/// A validation problem in the `accessibility` block (mapped to diagnostics by
/// `fx_spec`).
pub(crate) enum Problem {
    Error(String, String),
    Warning(String, String),
    Unknown(String, String, &'static [&'static str]),
}

/// Checks a file's `accessibility` block against its `states` keys.
pub(crate) fn check(a: &Value, state_keys: &[String]) -> Vec<Problem> {
    let mut out = Vec::new();
    let Some(obj) = a.as_object() else {
        out.push(Problem::Error(
            "/accessibility".into(),
            "expected an object { name?, states?, announce? }".into(),
        ));
        return out;
    };
    for (k, v) in obj {
        let at = format!("/accessibility/{k}");
        match k.as_str() {
            "name" => {
                if !v.is_string() {
                    out.push(Problem::Error(at, "expected a string".into()));
                }
            }
            "announce" => {
                if !v.is_boolean() {
                    out.push(Problem::Error(at, "expected true or false".into()));
                }
            }
            "states" => {
                let Some(m) = v.as_object() else {
                    out.push(Problem::Error(
                        at,
                        "expected an object of state -> words".into(),
                    ));
                    continue;
                };
                for (s, w) in m {
                    let sat = format!("{at}/{}", s.replace('~', "~0").replace('/', "~1"));
                    if !w.is_string() {
                        out.push(Problem::Error(sat.clone(), "expected a string".into()));
                    }
                    if default_words(s).is_none()
                        && s != "idle"
                        && !state_keys.iter().any(|k| k == s)
                    {
                        out.push(Problem::Warning(
                            sat,
                            format!("`{s}` is neither a voice state nor a key of `states`, so these words are never used"),
                        ));
                    }
                }
            }
            _ => out.push(Problem::Unknown(at, k.clone(), &KEYS)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn app_words_win_then_the_file_then_the_built_in() {
        let spec = words(&[("listening", "Coach is listening")]);
        let app = words(&[("listening", "Koç dinliyor")]);
        let none = HashMap::new();
        assert_eq!(
            accessible_name("Coach", Some("listening"), &spec, &app),
            "Koç dinliyor"
        );
        assert_eq!(
            accessible_name("Coach", Some("listening"), &spec, &none),
            "Coach is listening"
        );
        assert_eq!(
            accessible_name("Coach", Some("speaking"), &spec, &none),
            "Coach, speaking"
        );
        assert_eq!(
            accessible_name("", Some("thinking"), &none, &none),
            "Thinking"
        );
        assert_eq!(
            accessible_name("Coach", Some("idle"), &spec, &none),
            "Coach"
        );
        assert_eq!(accessible_name("Coach", None, &spec, &none), "Coach");
        // An app state with words of its own.
        let goal = words(&[("goalReached", "Goal reached!")]);
        assert_eq!(
            accessible_name("Steps", Some("goalReached"), &goal, &none),
            "Goal reached!"
        );
        assert_eq!(
            accessible_name("Steps", Some("goalReached"), &none, &none),
            "Steps"
        );
    }

    #[test]
    fn reads_the_block_leniently() {
        let a = read(
            r#"{"fxSpec":"1.9","accessibility":{"name":"Coach","states":{"listening":"Go","x":3},"announce":false}}"#,
        );
        assert_eq!(a.name.as_deref(), Some("Coach"));
        assert_eq!(a.states.len(), 1);
        assert_eq!(a.announce, Some(false));
        assert_eq!(read("not json"), FxAccessibility::default());
    }

    fn run(seq: &[(f64, Option<&str>)], end: f64) -> Vec<(f64, String)> {
        let mut s = AnnouncerState::default();
        let mut said = Vec::new();
        let mut pending: Option<f64> = None;
        let mut i = 0;
        let mut words: Option<String> = None;
        // Drive exactly as a host does: on each change, and at recheck_at.
        loop {
            let next_change = seq.get(i).map(|x| x.0);
            let t = match (next_change, pending) {
                (Some(c), Some(p)) => c.min(p),
                (Some(c), None) => c,
                (None, Some(p)) => p,
                (None, None) => break,
            };
            if t > end {
                break;
            }
            if next_change == Some(t) {
                words = seq[i].1.map(str::to_owned);
                i += 1;
            }
            let out = announce_step(s, words.clone(), t);
            s = out.state;
            pending = out.recheck_at;
            if let Some(w) = out.announce {
                said.push((t, w));
            }
        }
        said
    }

    fn said_pairs(v: &[(f64, String)]) -> Vec<(f64, &str)> {
        v.iter().map(|(t, w)| (*t, w.as_str())).collect()
    }

    #[test]
    fn the_first_state_is_not_spoken_and_a_held_change_is() {
        let said = run(&[(0.0, Some("Listening")), (2.0, Some("Speaking"))], 10.0);
        assert_eq!(said, vec![(3.0, "Speaking".to_string())]);
    }

    #[test]
    fn a_flapping_conversation_is_spoken_at_most_once_per_gap() {
        // listening/speaking flip every 0.5 s for 6 s: nothing holds 1 s until the
        // last flip (Speaking at 5.5), which is spoken once it settles.
        let flap: Vec<(f64, Option<&str>)> = (0..12)
            .map(|k| {
                (
                    k as f64 * 0.5,
                    Some(if k % 2 == 0 { "Listening" } else { "Speaking" }),
                )
            })
            .collect();
        assert_eq!(said_pairs(&run(&flap, 20.0)), vec![(6.5, "Speaking")]);
        // Changes held 1.5 s each: one announcement per 3 s at most.
        let slow = [
            (0.0, Some("Listening")),
            (1.0, Some("Thinking")),
            (2.5, Some("Speaking")),
            (4.0, Some("Listening")),
        ];
        // Thinking is spoken after its 1 s hold. Speaking (from 2.5) waits for the
        // 3 s gap (5.0); by then it's Listening, which differs from the last
        // words spoken (Thinking), so Listening is spoken then.
        assert_eq!(
            said_pairs(&run(&slow, 20.0)),
            vec![(2.0, "Thinking"), (5.0, "Listening")]
        );
    }

    #[test]
    fn idle_is_never_spoken_and_does_not_repeat_the_last_words() {
        let said = run(
            &[
                (0.0, Some("Listening")),
                (2.0, Some("Speaking")),
                (8.0, None),
                (12.0, Some("Speaking")),
            ],
            30.0,
        );
        assert_eq!(said, vec![(3.0, "Speaking".to_string())]);
    }

    #[test]
    fn check_reports_types_unknown_keys_and_dead_words() {
        let v: Value = serde_json::from_str(
            r#"{"name":3,"announce":"yes","states":{"listening":"ok","goalReached":"yay","nope":"x"},"extra":1}"#,
        )
        .unwrap();
        let p = check(&v, &["goalReached".to_string()]);
        let errors = p.iter().filter(|p| matches!(p, Problem::Error(..))).count();
        let warnings: Vec<String> = p
            .iter()
            .filter_map(|p| match p {
                Problem::Warning(at, _) => Some(at.clone()),
                _ => None,
            })
            .collect();
        let unknown = p
            .iter()
            .filter(|p| matches!(p, Problem::Unknown(..)))
            .count();
        assert_eq!(errors, 2);
        assert_eq!(warnings, vec!["/accessibility/states/nope".to_string()]);
        assert_eq!(unknown, 1);
    }
}
