//! `spec/effect-vectors.json`: each one-shot effect (docs/fx-view.md, *One-shot
//! effects*) on three patterns at fixed ages, normal and reduced, as a frame
//! summary (counts, alpha sum, the mean hue of saturated items, the centroid x).
//! The Web, iOS and Android tests render the same frames through their bindings
//! and compare the same summary, so every platform plays the same effect.
//! Regenerate only on purpose, with
//! `EFFECT_VECTORS_WRITE=1 cargo test -p core_engine --test effect_vectors -- --ignored`.

use std::collections::HashMap;

use core_engine::{effect_info, frame_with_overrides, OrbFrame};
use serde_json::{json, Value};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spec/effect-vectors.json"
);
const PATTERNS: [&str; 3] = ["glowing", "completing", "waveform"];
const EFFECTS: [&str; 3] = ["success", "error", "celebrate"];
const AGES: [f64; 4] = [0.05, 0.3, 0.6, 1.2];
const T: f64 = 1.0;

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// The summary every platform computes the same way.
fn summary(f: &OrbFrame) -> Value {
    let mut alpha = 0.0;
    let (mut hue, mut nh) = (0.0, 0usize);
    let (mut sx, mut n) = (0.0, 0usize);
    for d in &f.dots {
        alpha += d.a;
        if d.saturation > 0.0 {
            hue += d.hue;
            nh += 1;
        }
        sx += d.x;
        n += 1;
    }
    for l in &f.lines {
        alpha += l.a;
        if l.saturation > 0.0 {
            hue += l.hue;
            nh += 1;
        }
        sx += l.x1 + l.x2;
        n += 2;
    }
    for p in &f.polylines {
        alpha += p.a;
        if p.saturation > 0.0 {
            hue += p.hue;
            nh += 1;
        }
        for q in &p.points {
            sx += q.x;
            n += 1;
        }
    }
    json!({
        "dots": f.dots.len(), "lines": f.lines.len(), "polylines": f.polylines.len(),
        "alpha": r6(alpha),
        "hue": if nh == 0 { 0.0 } else { r6(hue / nh as f64) },
        "cx": if n == 0 { 0.0 } else { r6(sx / n as f64) },
    })
}

fn build() -> Value {
    let mut cases = Vec::new();
    for pattern in PATTERNS {
        for name in EFFECTS {
            let code = effect_info(name.to_string()).unwrap().code;
            for age in AGES {
                for reduced in [false, true] {
                    let o: HashMap<String, f64> = [
                        ("effectCode".to_string(), code as f64),
                        ("effectAge".to_string(), age),
                        ("effectReduced".to_string(), if reduced { 1.0 } else { 0.0 }),
                    ]
                    .into_iter()
                    .collect();
                    let f = frame_with_overrides(pattern.to_string(), 64, T, o).unwrap();
                    cases.push(json!({
                        "pattern": pattern, "effect": name, "age": age, "reduced": reduced,
                        "summary": summary(&f),
                    }));
                }
            }
        }
    }
    json!({
        "note": "One-shot effects (docs/fx-view.md): frame_with_overrides(pattern, 64, t = 1, { effectCode, effectAge, effectReduced }) summarised as { dots, lines, polylines, alpha (sum), hue (mean over saturated items), cx (mean x over dots, line ends and polyline points) }, rounded to 1e-6. Checked by crates/core_engine/tests/effect_vectors.rs and each platform's tests.",
        "size": 64, "t": T,
        "effects": EFFECTS.iter().map(|n| { let i = effect_info(n.to_string()).unwrap(); json!({ "name": n, "code": i.code, "duration": i.duration, "words": i.words }) }).collect::<Vec<_>>(),
        "cases": cases,
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
        "spec/effect-vectors.json is stale; rewrite on purpose (see the header)"
    );
}

#[test]
#[ignore]
fn write() {
    if std::env::var("EFFECT_VECTORS_WRITE").is_ok() {
        std::fs::write(PATH, serde_json::to_string_pretty(&build()).unwrap() + "\n").unwrap();
    }
}
