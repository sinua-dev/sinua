//! Color-conversion vectors for FX Spec (`spec/fx-color-vectors.json`):
//! hex -> CSS Color 4 HSL -> the engine keys a spec resolves to
//! (`colorHue`/`colorSaturation`, rounded like the Studio), plus gradient
//! stops -> unwrapped `gradientHue`/`2`/`3` both ways round.
//!
//! The point is agreement, not a snapshot of Rust: the same file is checked
//! against the web Studio's own converter (`kit/color.ts`)
//! and the wasm build in `packages/core/test/fx-color-parity.test.mjs`, so
//! "a hex picked in the Studio renders as that color from a spec" is a
//! tested property. The Swift/Kotlin Studio mirrors (`ColorMath.*`) can
//! read the same file.
//!
//! Regenerate only with
//! `FX_COLOR_VECTORS_WRITE=1 cargo test -p core_engine --test fx_color_vectors -- --ignored`
//! and review the diff (docs/testing.md); without the env var the
//! generator refuses to write.

use core_engine::{fx_color_to_hsl, resolve_fx_spec};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spec/fx-color-vectors.json"
);

/// Hand-picked edge cases, then a deterministic spread.
fn hexes() -> Vec<String> {
    let mut v: Vec<String> = [
        "#000000", "#ffffff", "#808080", "#7f8080", "#010101",
        "#fefefe", // greys / near-greys
        "#ff0000", "#00ff00", "#0000ff", "#ffff00", "#00ffff", "#ff00ff", // primaries
        "#ff0001", "#ff00fe", // hue near 360 (wrap)
        "#00FFB2", "#E54666", "#6E56CF", "#FFB224", "#33cc66", // brand/example colors
        "#abc", "#F0A", // 3-digit
        "#19a1e6", "#123456", "#fedcba", "#0a0b0c",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // xorshift32, fixed seed: stable across runs and platforms.
    let mut x: u32 = 0x9e37_79b9;
    for _ in 0..25 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push(format!("#{:06x}", x & 0x00ff_ffff));
    }
    v
}

fn gradients() -> Vec<(Vec<&'static str>, &'static str)> {
    let mut out = Vec::new();
    for path in ["short", "long"] {
        for stops in [
            vec!["#6E56CF", "#E54666"],
            vec!["#ff00ff", "#ffb224"], // 300 -> ~40: crosses 360 the short way
            vec!["#ffb224", "#ff00ff"],
            vec!["#6E56CF", "#E54666", "#FFB224"],
            vec!["#00ffb2", "#0000ff", "#ff0000"],
            vec!["#808080", "#ff0000"], // achromatic first stop -> hue 0
            vec!["#e54666", "#e54666"], // equal stops: no long-way flip
        ] {
            out.push((stops, path));
        }
    }
    out
}

fn spec_with_color(hex: &str) -> String {
    json!({ "fxSpec": "1.8", "object": "orb", "pattern": "working", "color": hex }).to_string()
}

fn spec_with_gradient(stops: &[&str], path: &str) -> String {
    json!({ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "gradient": { "stops": stops, "path": path } })
    .to_string()
}

fn color_row(hex: &str) -> Value {
    let c = fx_color_to_hsl(hex.to_string()).expect("valid hex");
    let r = resolve_fx_spec(spec_with_color(hex));
    assert!(r.ok, "{hex}: {:?}", r.diagnostics);
    json!({
        "input": hex,
        "hex": c.hex,
        "h": if c.achromatic { Value::Null } else { json!(c.h) },
        "s": c.s,
        "l": c.l,
        "colorHue": r.overrides["colorHue"],
        "colorSaturation": r.overrides["colorSaturation"],
    })
}

fn gradient_row(stops: &[&str], path: &str) -> Value {
    let r = resolve_fx_spec(spec_with_gradient(stops, path));
    assert!(r.ok, "{stops:?}: {:?}", r.diagnostics);
    let keys: BTreeMap<&str, f64> = ["gradientHue", "gradientHue2", "gradientHue3"]
        .into_iter()
        .filter_map(|k| r.overrides.get(k).map(|v| (k, *v)))
        .collect();
    json!({ "stops": stops, "path": path, "engine": keys })
}

fn load() -> Value {
    serde_json::from_str(&fs::read_to_string(PATH).expect("spec/fx-color-vectors.json")).unwrap()
}

#[test]
fn engine_matches_the_color_vectors() {
    let doc = load();
    let colors = doc["colors"].as_array().unwrap();
    assert!(!colors.is_empty());
    for row in colors {
        let input = row["input"].as_str().unwrap();
        let got = color_row(input);
        // Rounded keys must match exactly; raw h/s/l within 1e-12 (serde_json's
        // default float parsing isn't round-trip exact in the last ulp).
        for k in ["hex", "colorHue", "colorSaturation"] {
            assert_eq!(got[k], row[k], "{input}: {k}");
        }
        assert_eq!(
            got["h"].is_null(),
            row["h"].is_null(),
            "{input}: achromatic"
        );
        for k in ["h", "s", "l"] {
            if row[k].is_null() {
                continue;
            }
            let (g, e) = (got[k].as_f64().unwrap(), row[k].as_f64().unwrap());
            assert!((g - e).abs() < 1e-12, "{input}: {k} {g} vs {e}");
        }
    }
    for row in doc["gradients"].as_array().unwrap() {
        let stops: Vec<&str> = row["stops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let got = gradient_row(&stops, row["path"].as_str().unwrap());
        assert_eq!(got["engine"], row["engine"], "{stops:?} {}", row["path"]);
    }
}

#[test]
#[ignore = "writes spec/fx-color-vectors.json; run deliberately with FX_COLOR_VECTORS_WRITE=1"]
fn regenerate_fx_color_vectors() {
    assert_eq!(
        std::env::var("FX_COLOR_VECTORS_WRITE").as_deref(),
        Ok("1"),
        "refusing to write without FX_COLOR_VECTORS_WRITE=1 -- see docs/testing.md"
    );
    let doc = json!({
        "specVersion": "1.0.0",
        "note": "FX Spec color vectors: hex -> CSS Color 4 HSL (h null = achromatic) -> colorHue (h rounded to 1 dp, JS Math.round) / colorSaturation (s to 3 dp); gradient stops -> unwrapped gradientHue/2/3. Checked by crates/core_engine/tests/fx_color_vectors.rs and packages/core/test/fx-color-parity.test.mjs (Studio kit/color.ts + wasm).",
        "colors": hexes().iter().map(|h| color_row(h)).collect::<Vec<_>>(),
        "gradients": gradients().iter().map(|(s, p)| gradient_row(s, p)).collect::<Vec<_>>(),
    });
    fs::write(PATH, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
