//! The conversation simulator (design note: sinua-studio/docs/agents/families/
//! design-02-conversation-simulator.md; docs/audio-pipeline.md, *Simulated
//! conversations*).
//!
//! A script of turns plays like a voice source: the agent state changes on a
//! timeline, and while someone talks the level and bands move like speech.
//! No audio is opened or produced anywhere. Everything here is a pure
//! function of time, so every platform's `SimulatedVoiceSource` shows the
//! same conversation (`spec/conversation-vectors.json` holds them to it).
//!
//! The speech curve is synthetic: a phrase envelope, syllable bursts around
//! 4 Hz and short word gaps, all seeded. No fast flutter -- a faster wobble
//! read as noise, not speech, on the site's hero (2026-09-28).

use serde_json::Value;

use crate::fx_spec::FxDiagnostic;
use crate::primitives::hash_d;

const VOICE_STATES: [&str; 5] = ["initializing", "idle", "listening", "thinking", "speaking"];
const SCRIPT_KEYS: [&str; 4] = ["name", "loop", "seed", "turns"];
const TURN_KEYS: [&str; 5] = ["state", "seconds", "voice", "bargeIn", "line"];
const RISE_S: f64 = 0.25;
const FALL_S: f64 = 0.3;
const GAP_S: f64 = 0.28;

const SAMPLES: [(&str, &str); 4] = [
    (
        "calendar",
        include_str!("../../../spec/conversations/calendar.json"),
    ),
    (
        "quick-answer",
        include_str!("../../../spec/conversations/quick-answer.json"),
    ),
    (
        "long-answer",
        include_str!("../../../spec/conversations/long-answer.json"),
    ),
    (
        "barge-in",
        include_str!("../../../spec/conversations/barge-in.json"),
    ),
];

/// One instant of a simulated conversation.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationFrame {
    /// False when the script has errors (then nothing else is meaningful).
    pub ok: bool,
    pub diagnostics: Vec<FxDiagnostic>,
    /// The agent state (`idle`, `listening`, …).
    pub state: String,
    /// The voice level now, `0..1` (0 while nobody talks).
    pub level: f64,
    /// Per-band levels, `0..1`, low frequency first.
    pub bands: Vec<f64>,
    /// The turn index.
    pub turn: u32,
    /// `0..1` through the current turn.
    pub progress: f64,
    /// How many characters of `line` are "said" by now.
    pub shown: u32,
    /// The turn's text (empty when it has none).
    pub line: String,
    /// The turn is a barge-in (the user talked over the agent).
    pub barge_in: bool,
    /// The script's length in seconds.
    pub total: f64,
    /// The script time shown (`t` wrapped when the script loops).
    pub time: f64,
}

struct Turn {
    state: String,
    seconds: f64,
    voice: Option<String>,
    barge_in: bool,
    line: String,
}

struct Script {
    looping: bool,
    seed: f64,
    turns: Vec<Turn>,
}

fn diag(out: &mut Vec<FxDiagnostic>, severity: &str, path: String, message: impl Into<String>) {
    out.push(FxDiagnostic {
        severity: severity.into(),
        path,
        message: message.into(),
    });
}

fn parse(json: &str, d: &mut Vec<FxDiagnostic>) -> Option<Script> {
    let Ok(doc) = serde_json::from_str::<Value>(json) else {
        diag(d, "error", String::new(), "not JSON");
        return None;
    };
    let Some(root) = doc.as_object() else {
        diag(d, "error", String::new(), "expected an object");
        return None;
    };
    for k in root.keys() {
        if !SCRIPT_KEYS.contains(&k.as_str()) {
            diag(d, "warning", format!("/{k}"), format!("unknown key `{k}`"));
        }
    }
    let Some(turns) = root.get("turns").and_then(Value::as_array) else {
        diag(d, "error", "/turns".into(), "expected a list of turns");
        return None;
    };
    if turns.is_empty() {
        diag(
            d,
            "error",
            "/turns".into(),
            "a script needs at least one turn",
        );
        return None;
    }
    let mut out = Vec::new();
    for (i, t) in turns.iter().enumerate() {
        let at = format!("/turns/{i}");
        let Some(o) = t.as_object() else {
            diag(d, "error", at, "expected an object");
            continue;
        };
        for k in o.keys() {
            if !TURN_KEYS.contains(&k.as_str()) {
                diag(
                    d,
                    "warning",
                    format!("{at}/{k}"),
                    format!("unknown key `{k}`"),
                );
            }
        }
        let state = match o.get("state").and_then(Value::as_str) {
            Some(s) => s.to_string(),
            None => {
                diag(d, "error", format!("{at}/state"), "missing `state`");
                continue;
            }
        };
        if !VOICE_STATES.contains(&state.as_str()) {
            diag(
                d,
                "warning",
                format!("{at}/state"),
                format!("`{state}` is not a voice state; views show it as the base design"),
            );
        }
        let seconds = match o.get("seconds").and_then(Value::as_f64) {
            Some(s) if (0.1..=60.0).contains(&s) => s,
            _ => {
                diag(
                    d,
                    "error",
                    format!("{at}/seconds"),
                    "expected seconds between 0.1 and 60",
                );
                continue;
            }
        };
        let voice = match o.get("voice") {
            None => None,
            Some(v) => match v.as_str() {
                Some(s @ ("user" | "agent")) => Some(s.to_string()),
                _ => {
                    diag(
                        d,
                        "error",
                        format!("{at}/voice"),
                        "expected `user` or `agent`",
                    );
                    None
                }
            },
        };
        if voice.is_some() && !matches!(state.as_str(), "listening" | "speaking") {
            diag(
                d,
                "warning",
                format!("{at}/voice"),
                format!("a voice in a `{state}` turn: views ignore audio there"),
            );
        }
        out.push(Turn {
            state,
            seconds,
            voice,
            barge_in: o.get("bargeIn").and_then(Value::as_bool).unwrap_or(false),
            line: o
                .get("line")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        });
    }
    if out.len() != turns.len() {
        return None;
    }
    Some(Script {
        looping: root.get("loop").and_then(Value::as_bool).unwrap_or(false),
        seed: root.get("seed").and_then(Value::as_f64).unwrap_or(0.0),
        turns: out,
    })
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The speech level `lt` seconds into a voiced turn of `len` seconds.
fn speech_level(voice: &str, seed: f64, turn: usize, lt: f64, len: f64) -> f64 {
    let k = turn as f64 + 1.0;
    let env = smoothstep(lt / RISE_S) * smoothstep((len - lt) / FALL_S);
    // Syllables: two detuned slow waves, so bursts aren't metronomic.
    let rate = 3.6 + 0.8 * hash_d(k, seed + 11.0);
    let p1 = std::f64::consts::TAU * hash_d(k, seed + 23.0);
    let p2 = std::f64::consts::TAU * hash_d(k, seed + 37.0);
    let s1 = 0.5 + 0.5 * (std::f64::consts::TAU * rate * lt + p1).sin();
    let s2 = 0.5 + 0.5 * (std::f64::consts::TAU * rate * 0.61 * lt + p2).sin();
    let syllable = (0.6 * s1 + 0.4 * s2).powf(1.4);
    // Word gaps: a soft dip at the end of each ~1-1.5 s word group (a sharper one
    // read as a stutter at 30 fps).
    let cycle = 1.0 + 0.5 * hash_d(k, seed + 51.0);
    let within = lt.rem_euclid(cycle);
    let dip = if within > cycle - GAP_S {
        let u = (within - (cycle - GAP_S)) / GAP_S;
        1.0 - 0.6 * (std::f64::consts::PI * u).sin()
    } else {
        1.0
    };
    let (lo, hi, depth) = if voice == "agent" {
        (0.35, 0.8, 0.7)
    } else {
        // A normal speaker (0.25-0.65 read as mumbling: signal barely moved while listening).
        (0.40, 0.78, 0.85)
    };
    let shaped = 1.0 - depth + depth * syllable;
    (env * (lo + (hi - lo) * shaped) * dip).clamp(0.0, 1.0)
}

/// Voice-shaped bands: most energy in the low mids, drifting slowly per band.
fn speech_bands(level: f64, seed: f64, lt: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            let peak = (-((x - 0.25) / 0.28).powi(2)).exp();
            let tilt = 0.55 * peak + 0.45 * (1.0 - x);
            let drift = 0.8
                + 0.4
                    * (0.5
                        + 0.5
                            * (std::f64::consts::TAU * 0.7 * lt
                                + std::f64::consts::TAU * hash_d(i as f64 + 1.0, seed + 71.0))
                            .sin());
            (level * tilt * drift * 1.3).clamp(0.0, 1.0)
        })
        .collect()
}

/// A simulated conversation at `t` seconds, with `band_count` bands. `loop`
/// scripts wrap; others hold their last turn at its end.
pub fn at(json: &str, t: f64, band_count: u32) -> ConversationFrame {
    let mut d = Vec::new();
    let script = parse(json, &mut d);
    let ok = script.is_some() && !d.iter().any(|x| x.severity == "error");
    let mut frame = ConversationFrame {
        ok,
        diagnostics: d,
        state: "idle".into(),
        level: 0.0,
        bands: vec![0.0; band_count as usize],
        turn: 0,
        progress: 0.0,
        shown: 0,
        line: String::new(),
        barge_in: false,
        total: 0.0,
        time: 0.0,
    };
    let Some(s) = script.filter(|_| ok) else {
        return frame;
    };
    let total: f64 = s.turns.iter().map(|t| t.seconds).sum();
    let t = t.max(0.0);
    let time = if s.looping {
        t.rem_euclid(total)
    } else {
        t.min(total)
    };
    let mut start = 0.0;
    let mut idx = s.turns.len() - 1;
    for (i, turn) in s.turns.iter().enumerate() {
        if time < start + turn.seconds {
            idx = i;
            break;
        }
        start += turn.seconds;
    }
    if idx == s.turns.len() - 1 && time >= total {
        start = total - s.turns[idx].seconds;
    }
    let turn = &s.turns[idx];
    let lt = (time - start).clamp(0.0, turn.seconds);
    let progress = lt / turn.seconds;
    let level = turn
        .voice
        .as_deref()
        .map(|v| speech_level(v, s.seed, idx, lt, turn.seconds))
        .unwrap_or(0.0);
    frame.state = turn.state.clone();
    frame.level = level;
    frame.bands = speech_bands(level, s.seed, lt, band_count as usize);
    frame.turn = idx as u32;
    frame.progress = progress;
    frame.shown = (turn.line.chars().count() as f64 * progress).round() as u32;
    frame.line = turn.line.clone();
    frame.barge_in = turn.barge_in;
    frame.total = total;
    frame.time = time;
    frame
}

/// The built-in sample names, in order.
pub fn sample_names() -> Vec<String> {
    SAMPLES.iter().map(|(n, _)| n.to_string()).collect()
}

/// A built-in sample's script JSON.
pub fn sample(name: &str) -> Option<String> {
    SAMPLES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, j)| j.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = r#"{ "loop": true, "seed": 7, "turns": [
        { "state": "idle", "seconds": 1 },
        { "state": "listening", "seconds": 2, "voice": "user", "line": "Hello there" },
        { "state": "speaking", "seconds": 2, "voice": "agent", "bargeIn": true } ] }"#;

    #[test]
    fn turns_follow_the_timeline_and_loop() {
        let f = at(SCRIPT, 0.5, 16);
        assert!(f.ok, "{:?}", f.diagnostics);
        assert_eq!((f.state.as_str(), f.turn, f.total), ("idle", 0, 5.0));
        assert_eq!(
            at(SCRIPT, 1.0, 16).state,
            "listening",
            "a boundary belongs to the next turn"
        );
        let s = at(SCRIPT, 3.5, 16);
        assert_eq!((s.state.as_str(), s.barge_in), ("speaking", true));
        assert_eq!(at(SCRIPT, 5.5, 16).state, "idle", "loops");
        assert!((at(SCRIPT, 5.5, 16).time - 0.5).abs() < 1e-12);
        let once = SCRIPT.replace(r#""loop": true"#, r#""loop": false"#);
        let end = at(&once, 99.0, 16);
        assert_eq!(
            (end.state.as_str(), end.progress),
            ("speaking", 1.0),
            "a non-looping script holds its end"
        );
    }

    #[test]
    fn silent_turns_are_silent_and_voiced_ones_move_like_speech() {
        assert_eq!(at(SCRIPT, 0.5, 16).level, 0.0);
        assert!(at(SCRIPT, 0.5, 16).bands.iter().all(|b| *b == 0.0));
        // Starts and ends at 0 (the phrase envelope), moves in between.
        assert_eq!(at(SCRIPT, 1.0, 16).level, 0.0);
        let levels: Vec<f64> = (0..80)
            .map(|i| at(SCRIPT, 1.3 + i as f64 * 0.02, 16).level)
            .collect();
        let (lo, hi) = levels
            .iter()
            .fold((1.0f64, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(hi > 0.55 && hi <= 0.78 + 1e-9, "user peak {hi}");
        assert!(hi - lo > 0.15, "it moves: {lo}..{hi}");
        // No fast flutter: frame-to-frame (30 fps) change stays small.
        let steps: Vec<f64> = (0..60)
            .map(|i| {
                let a = at(SCRIPT, 1.3 + i as f64 / 30.0, 16).level;
                let b = at(SCRIPT, 1.3 + (i + 1) as f64 / 30.0, 16).level;
                (a - b).abs()
            })
            .collect();
        let max_step = steps.iter().cloned().fold(0.0, f64::max);
        assert!(max_step < 0.2, "no jumps at 30 fps: {max_step}");
    }

    #[test]
    fn bands_are_voice_shaped_and_sized() {
        let f = at(SCRIPT, 2.0, 16);
        assert_eq!(f.bands.len(), 16);
        let low_mid = f.bands[2..6].iter().sum::<f64>() / 4.0;
        let top = f.bands[12..16].iter().sum::<f64>() / 4.0;
        assert!(low_mid > top, "more energy in the low mids");
        assert_eq!(at(SCRIPT, 2.0, 5).bands.len(), 5);
    }

    #[test]
    fn deterministic_and_seeded() {
        assert_eq!(at(SCRIPT, 2.345, 16), at(SCRIPT, 2.345, 16));
        let other = SCRIPT.replace(r#""seed": 7"#, r#""seed": 8"#);
        assert_ne!(at(SCRIPT, 2.345, 16).level, at(&other, 2.345, 16).level);
    }

    #[test]
    fn the_line_is_said_as_the_turn_goes() {
        assert_eq!(at(SCRIPT, 1.0, 16).shown, 0);
        assert_eq!(at(SCRIPT, 2.0, 16).shown, 6);
        assert_eq!(at(SCRIPT, 2.0, 16).line, "Hello there");
    }

    #[test]
    fn a_bad_script_is_reported() {
        let f = at(
            r#"{ "turns": [ { "state": "listening", "seconds": 0 } ] }"#,
            0.0,
            4,
        );
        assert!(!f.ok);
        assert_eq!(f.diagnostics[0].path, "/turns/0/seconds");
        let w = at(
            r#"{ "turns": [ { "state": "thinking", "seconds": 1, "voice": "user" } ], "extra": 1 }"#,
            0.0,
            4,
        );
        assert!(w.ok, "warnings only");
        assert_eq!(w.diagnostics.len(), 2);
        assert!(!at("nope", 0.0, 4).ok);
        assert!(!at(r#"{ "turns": [] }"#, 0.0, 4).ok);
    }

    #[test]
    fn the_samples_are_valid_and_voiced() {
        assert_eq!(
            sample_names(),
            ["calendar", "quick-answer", "long-answer", "barge-in"]
        );
        for name in sample_names() {
            let json = sample(&name).unwrap();
            let f = at(&json, 0.0, 16);
            assert!(
                f.ok && f.diagnostics.is_empty(),
                "{name}: {:?}",
                f.diagnostics
            );
            assert!((9.0..=16.0).contains(&f.total), "{name} is {}s", f.total);
            let states: std::collections::BTreeSet<String> = (0..200)
                .map(|i| at(&json, i as f64 * f.total / 200.0, 16).state)
                .collect();
            for s in ["idle", "listening", "thinking", "speaking"] {
                assert!(states.contains(s), "{name} shows {s}");
            }
        }
        assert!(sample("nope").is_none());
    }
}
