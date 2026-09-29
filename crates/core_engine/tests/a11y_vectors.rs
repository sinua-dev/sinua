//! `spec/a11y-announce-vectors.json`: when a view speaks a state change, and what
//! it is called in each state (docs/fx-view.md, *Accessibility*). The Web, iOS and
//! Android tests drive the same sequences through their bindings with the same
//! host loop (a call on every change and at `recheck_at`), so every platform
//! speaks the same words at the same moments. Regenerate only on purpose, with
//! `A11Y_VECTORS_WRITE=1 cargo test -p core_engine --test a11y_vectors -- --ignored`.

use std::collections::HashMap;

use core_engine::{a11y_accessible_name, a11y_announce_step, AnnouncerState};
use serde_json::{json, Value};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spec/a11y-announce-vectors.json"
);

/// The host loop every platform runs: step on each change, and at `recheck_at`.
fn run(seq: &[(f64, Option<&str>)], end: f64) -> Vec<(f64, String)> {
    let mut s = AnnouncerState::default();
    let mut said = Vec::new();
    let mut pending: Option<f64> = None;
    let mut i = 0;
    let mut words: Option<String> = None;
    loop {
        let next = seq.get(i).map(|x| x.0);
        let t = match (next, pending) {
            (Some(c), Some(p)) => c.min(p),
            (Some(c), None) => c,
            (None, Some(p)) => p,
            (None, None) => break,
        };
        if t > end {
            break;
        }
        if next == Some(t) {
            words = seq[i].1.map(str::to_owned);
            i += 1;
        }
        let out = a11y_announce_step(s, words.clone(), t);
        s = out.state;
        pending = out.recheck_at;
        if let Some(w) = out.announce {
            said.push((t, w));
        }
    }
    said
}

/// A named sequence of `[t, words]` changes.
type Sequence = (&'static str, Vec<(f64, Option<&'static str>)>);

fn sequences() -> Vec<Sequence> {
    let flap = (0..16)
        .map(|k| {
            (
                k as f64 * 0.5,
                Some(if k % 2 == 0 { "Listening" } else { "Speaking" }),
            )
        })
        .collect();
    vec![
        (
            "a held change is spoken after 1 s",
            vec![(0.0, Some("Listening")), (2.0, Some("Speaking"))],
        ),
        ("a flapping conversation waits until it settles", flap),
        (
            "at most one announcement per 3 s",
            vec![
                (0.0, Some("Listening")),
                (1.0, Some("Thinking")),
                (2.5, Some("Speaking")),
                (4.0, Some("Listening")),
                (9.0, Some("Speaking")),
            ],
        ),
        (
            "a turn: listening, thinking, speaking, back",
            vec![
                (0.0, None),
                (1.0, Some("Listening")),
                (4.0, Some("Thinking")),
                (5.5, Some("Speaking")),
                (10.0, Some("Listening")),
            ],
        ),
        (
            "a barge-in cuts speaking short",
            vec![
                (0.0, Some("Listening")),
                (3.0, Some("Speaking")),
                (3.6, Some("Listening")),
                (8.0, Some("Speaking")),
            ],
        ),
        (
            "idle after a disconnect is never spoken",
            vec![
                (0.0, Some("Listening")),
                (2.0, Some("Speaking")),
                (8.0, None),
                (12.0, Some("Speaking")),
            ],
        ),
    ]
}

fn names() -> Vec<Value> {
    let spec: HashMap<String, String> = [
        ("listening".to_string(), "Coach is listening".to_string()),
        ("goalReached".to_string(), "Goal reached!".to_string()),
    ]
    .into_iter()
    .collect();
    let app: HashMap<String, String> = [("speaking".to_string(), "Koç konuşuyor".to_string())]
        .into_iter()
        .collect();
    let none = HashMap::new();
    let mut out = Vec::new();
    for (name, state, s, a) in [
        ("Coach", Some("listening"), &spec, &app),
        ("Coach", Some("speaking"), &spec, &app),
        ("Coach", Some("thinking"), &spec, &app),
        ("Coach", Some("idle"), &spec, &app),
        ("Coach", None, &spec, &app),
        ("", Some("initializing"), &none, &none),
        ("Steps", Some("goalReached"), &spec, &none),
        ("Steps", Some("goalReached"), &none, &none),
    ] {
        out.push(json!({
            "name": name, "state": state, "spec": s, "app": a,
            "expect": a11y_accessible_name(name.to_string(), state.map(str::to_owned), s.clone(), a.clone()),
        }));
    }
    out
}

fn build() -> Value {
    let cases: Vec<Value> = sequences()
        .into_iter()
        .map(|(name, seq)| {
            let said = run(&seq, 30.0);
            json!({
                "name": name,
                "end": 30.0,
                "steps": seq.iter().map(|(t, w)| json!([t, w])).collect::<Vec<_>>(),
                "said": said.iter().map(|(t, w)| json!([t, w])).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "note": "State-aware accessibility (docs/fx-view.md). `cases`: word sequences [t, words|null] and what the announcer says [t, words], with the host loop that steps on every change and at recheck_at. `names`: accessible_name inputs and results. Checked by crates/core_engine/tests/a11y_vectors.rs and each platform's tests.",
        "holdSeconds": core_engine::a11y::HOLD_S,
        "gapSeconds": core_engine::a11y::GAP_S,
        "cases": cases,
        "names": names(),
    })
}

#[test]
fn the_vectors_match_the_engine() {
    let text = std::fs::read_to_string(PATH)
        .unwrap_or_else(|_| panic!("{PATH} is missing -- write it (see this file's header)"));
    let want: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        want,
        build(),
        "spec/a11y-announce-vectors.json is stale; rewrite on purpose (see the header)"
    );
}

#[test]
#[ignore]
fn write() {
    if std::env::var("A11Y_VECTORS_WRITE").is_ok() {
        std::fs::write(PATH, serde_json::to_string_pretty(&build()).unwrap() + "\n").unwrap();
    }
}
