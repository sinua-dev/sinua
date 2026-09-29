//! `spec/conversation-vectors.json`: every built-in sample conversation sampled
//! at fixed times (state, level, 8 bands). The Web, iOS and Android simulator
//! tests read the same file through their bindings, so the four platforms play
//! the same conversation. Regenerate only on purpose, with
//! `CONVERSATION_VECTORS_WRITE=1 cargo test -p core_engine --test conversation_vectors -- --ignored`.

use core_engine::{conversation_at, conversation_sample, conversation_sample_names};
use serde_json::{json, Value};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spec/conversation-vectors.json"
);
const BANDS: u32 = 8;
const STEP: f64 = 0.37;

fn build() -> Value {
    let mut cases = Vec::new();
    for name in conversation_sample_names() {
        let script = conversation_sample(name.clone()).unwrap();
        let total = conversation_at(script.clone(), 0.0, BANDS).total;
        let mut t = 0.0;
        while t < total + 1.0 {
            let f = conversation_at(script.clone(), t, BANDS);
            cases.push(json!({
                "sample": name, "t": t, "state": f.state, "turn": f.turn,
                "level": f.level, "bands": f.bands, "shown": f.shown, "bargeIn": f.barge_in,
            }));
            t = ((t + STEP) * 1e6).round() / 1e6;
        }
    }
    json!({
        "note": "Simulated conversations (docs/audio-pipeline.md): each built-in sample at fixed times, 8 bands. Checked by crates/core_engine/tests/conversation_vectors.rs and each platform's simulator tests.",
        "bands": BANDS,
        "tolerance": 1e-9,
        "cases": cases,
    })
}

#[test]
fn the_vectors_match_the_engine() {
    let text = std::fs::read_to_string(PATH)
        .unwrap_or_else(|_| panic!("{PATH} is missing -- write it (see this file's header)"));
    let want: Value = serde_json::from_str(&text).unwrap();
    let got = build();
    let (w, g) = (
        want["cases"].as_array().unwrap(),
        got["cases"].as_array().unwrap(),
    );
    assert_eq!(w.len(), g.len(), "case count");
    for (a, b) in w.iter().zip(g) {
        assert_eq!(a["state"], b["state"], "{} at {}", a["sample"], a["t"]);
        assert_eq!(a["turn"], b["turn"]);
        assert_eq!(a["shown"], b["shown"]);
        assert!(
            (a["level"].as_f64().unwrap() - b["level"].as_f64().unwrap()).abs() < 1e-9,
            "{} at {}",
            a["sample"],
            a["t"]
        );
        for (x, y) in a["bands"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["bands"].as_array().unwrap())
        {
            assert!((x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9);
        }
    }
}

#[test]
#[ignore]
fn write_the_vectors() {
    assert_eq!(
        std::env::var("CONVERSATION_VECTORS_WRITE").as_deref(),
        Ok("1"),
        "set CONVERSATION_VECTORS_WRITE=1"
    );
    std::fs::write(PATH, serde_json::to_string_pretty(&build()).unwrap() + "\n").unwrap();
}
