//! Reactive-binding vectors (`spec/reactive-vectors.json`): the curated
//! targets table and value -> target mappings, produced by the Rust port
//! (`core_engine::reactive`, what FX Spec `bindings` run) and checked
//! against `packages/core/src/reactive.ts` in
//! `packages/core/test/reactive-parity.test.mjs` -- one contract, two
//! implementations held together (reactive.ts keeps its own copy because it
//! also accepts JS function curves). Every case goes through a real spec
//! (`resolve_fx_spec_with`), so the file also locks the spec plumbing.
//!
//! Regenerate only with
//! `REACTIVE_VECTORS_WRITE=1 cargo test -p core_engine --test reactive_vectors -- --ignored`
//! and review the diff (docs/testing.md).

use core_engine::reactive::REACTIVE_TARGETS;
use core_engine::resolve_fx_spec_with;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spec/reactive-vectors.json"
);

/// A state whose mode reads the target (so the spec raises no warning).
fn host(target: &str) -> (&'static str, &'static str) {
    match target {
        "progress" => ("ring", "completing"),
        t if t.starts_with("progress") => ("ring", "tracking"),
        "quality" => ("beacon", "reconnecting"),
        "accuracy" => ("beacon", "locating"),
        _ => ("orb", "working"),
    }
}

struct Case {
    target: &'static str,
    input: Option<Vec<f64>>,
    output: Option<Vec<f64>>,
    curve: Option<Value>,
    values: Vec<f64>,
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    let sweep = |lo: f64, hi: f64| -> Vec<f64> {
        let mut out = vec![lo - (hi - lo) * 0.5];
        out.extend((0..=8).map(|i| lo + (hi - lo) * i as f64 / 8.0));
        out.push(hi + (hi - lo) * 0.5);
        out
    };
    // Defaults on every target (input [0,1] -> the target's range).
    for t in &REACTIVE_TARGETS {
        v.push(Case {
            target: t.name,
            input: None,
            output: None,
            curve: None,
            values: sweep(0.0, 1.0),
        });
    }
    // Every keyword curve.
    for c in ["linear", "ease", "easeIn", "easeOut", "easeInOut"] {
        v.push(Case {
            target: "progress",
            input: Some(vec![0.0, 10_000.0]),
            output: None,
            curve: Some(json!(c)),
            values: sweep(0.0, 10_000.0),
        });
    }
    // Multi-stop with per-segment curves; descending input; clamped output;
    // lap-range targets; companions.
    v.push(Case {
        target: "glowStrength",
        input: Some(vec![60.0, 100.0, 160.0]),
        output: Some(vec![0.0, 0.2, 0.8]),
        curve: Some(json!(["linear", "easeIn"])),
        values: sweep(60.0, 160.0),
    });
    v.push(Case {
        target: "quality",
        input: Some(vec![300.0, 20.0]),
        output: None,
        curve: Some(json!("easeOut")),
        values: sweep(20.0, 300.0),
    });
    v.push(Case {
        target: "progress1",
        input: Some(vec![0.0, 2500.0]),
        output: Some(vec![0.0, 5.0]),
        curve: None,
        values: sweep(0.0, 2500.0),
    });
    // A goal ring with laps: one lap at the goal, three at 3x (the
    // multi-stop way to opt into laps; the default output is one lap).
    v.push(Case {
        target: "progress0",
        input: Some(vec![0.0, 10_000.0, 30_000.0]),
        output: Some(vec![0.0, 1.0, 3.0]),
        curve: Some(json!(["easeOut", "linear"])),
        values: sweep(0.0, 30_000.0),
    });
    v.push(Case {
        target: "audioLevel",
        input: Some(vec![0.0, 0.8]),
        output: Some(vec![0.1, 1.0]),
        curve: Some(json!("ease")),
        values: sweep(0.0, 0.8),
    });
    v.push(Case {
        target: "colorMix",
        input: Some(vec![0.0, 0.25, 0.5, 1.0]),
        output: Some(vec![1.0, 0.2, 0.9, 0.0]),
        curve: Some(json!(["easeInOut", "linear", "easeOut"])),
        values: sweep(0.0, 1.0),
    });
    v
}

fn run(c: &Case) -> Vec<Value> {
    let (object, state) = host(c.target);
    let mut b = serde_json::Map::new();
    b.insert("input".into(), json!("x"));
    if let Some(i) = &c.input {
        b.insert("inputRange".into(), json!(i));
    }
    if let Some(o) = &c.output {
        b.insert("outputRange".into(), json!(o));
    }
    if let Some(cv) = &c.curve {
        b.insert("curve".into(), cv.clone());
    }
    // The file names a target by its catalog path (FX Spec 1.7); the vectors,
    // and the resolved overrides, use the engine key.
    let path = match c.target {
        "glowStrength" | "noiseStrength" | "gradientStrength" | "pulseStrength" => {
            format!("{}.strength", c.target.trim_end_matches("Strength"))
        }
        "colorMix" => "color.mix".to_string(),
        t => match t.strip_prefix("progress").filter(|i| !i.is_empty()) {
            Some(i) => format!("progress[{i}]"),
            None => t.to_string(),
        },
    };
    let spec = json!({ "fxSpec": "1.8", "object": object, "pattern": state,
                       "bindings": { path: Value::Object(b) } })
    .to_string();
    c.values
        .iter()
        .map(|x| {
            let r = resolve_fx_spec_with(
                spec.clone(),
                None,
                HashMap::from([("x".to_string(), *x)]),
                false,
            );
            assert!(
                r.ok && r.diagnostics.is_empty(),
                "{}: {:?}",
                c.target,
                r.diagnostics
            );
            let out: BTreeMap<String, f64> = r.overrides.into_iter().collect();
            json!({ "value": x, "overrides": out })
        })
        .collect()
}

fn doc() -> Value {
    let targets: BTreeMap<&str, Value> = REACTIVE_TARGETS
        .iter()
        .map(|t| {
            let comp: BTreeMap<&str, f64> = t.companions.iter().cloned().collect();
            (
                t.name,
                json!({ "range": [t.range.0, t.range.1],
                        "defaultOutput": [t.default_output.0, t.default_output.1],
                        "companions": comp }),
            )
        })
        .collect();
    let cases: Vec<Value> = cases()
        .iter()
        .map(|c| {
            json!({ "target": c.target, "inputRange": c.input, "outputRange": c.output,
                    "curve": c.curve, "results": run(c) })
        })
        .collect();
    json!({
        "specVersion": "1.0.0",
        "note": "Reactive-binding vectors: targets table + value -> overrides through FX Spec bindings (core_engine::reactive). Checked by crates/core_engine/tests/reactive_vectors.rs and packages/core/test/reactive-parity.test.mjs (reactive.ts). null range/curve = the default.",
        "targets": targets,
        "cases": cases,
    })
}

#[test]
fn engine_matches_the_reactive_vectors() {
    let want: Value =
        serde_json::from_str(&fs::read_to_string(PATH).expect("spec/reactive-vectors.json"))
            .unwrap();
    let got = doc();
    assert_eq!(got["targets"], want["targets"]);
    let (gc, wc) = (
        got["cases"].as_array().unwrap(),
        want["cases"].as_array().unwrap(),
    );
    assert_eq!(gc.len(), wc.len());
    for (g, w) in gc.iter().zip(wc) {
        for (gr, wr) in g["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(w["results"].as_array().unwrap())
        {
            let (go, wo) = (
                gr["overrides"].as_object().unwrap(),
                wr["overrides"].as_object().unwrap(),
            );
            assert_eq!(
                go.keys().collect::<Vec<_>>(),
                wo.keys().collect::<Vec<_>>(),
                "{}",
                w["target"]
            );
            for (k, v) in go {
                let (a, b) = (v.as_f64().unwrap(), wo[k].as_f64().unwrap());
                assert!(
                    (a - b).abs() < 1e-12,
                    "{} @ {}: {k} {a} vs {b}",
                    w["target"],
                    wr["value"]
                );
            }
        }
    }
}

#[test]
#[ignore = "writes spec/reactive-vectors.json; run deliberately with REACTIVE_VECTORS_WRITE=1"]
fn regenerate_reactive_vectors() {
    assert_eq!(
        std::env::var("REACTIVE_VECTORS_WRITE").as_deref(),
        Ok("1"),
        "refusing to write without REACTIVE_VECTORS_WRITE=1 -- see docs/testing.md"
    );
    fs::write(PATH, serde_json::to_string_pretty(&doc()).unwrap() + "\n").unwrap();
}
