//! Parity test against upstream's own engine (the external oracle):
//! `spec/orbs-golden.json` (upstream's frozen golden, vendored verbatim; sizes
//! 64 and 20) and `spec/orbs-golden-32.json` (the same engine, unmodified, at
//! 32 -- `scripts/oracle/README.md`). All 9 states / 9 modes at every shipped
//! size, at all 4 frozen timestamps -- every `orbs` mode is ported.

use core_engine::{frame, frame_with_overrides, resolve_preset, Dot};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

fn load_golden(name: &str) -> Value {
    let path = format!("{}/../../spec/{name}", env!("CARGO_MANIFEST_DIR"));
    let data = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read spec/{name}: {e}"));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("parse spec/{name}: {e}"))
}

const DOT_FIELDS: [&str; 6] = ["x", "y", "z", "r", "white", "a"];

fn dot_row(d: &Dot) -> [f64; 6] {
    [d.x, d.y, d.z, d.r, d.white, d.a]
}

fn parse_stride(flat: &[Value], stride: usize) -> Vec<Vec<f64>> {
    flat.chunks(stride)
        .map(|c| c.iter().map(|v| v.as_f64().unwrap()).collect())
        .collect()
}

/// First-mismatch message for a strict, index-aligned comparison, or `None`
/// if everything is within tolerance.
fn strict_mismatch(got: &[[f64; 6]], expected: &[Vec<f64>], tol: f64) -> Option<String> {
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        for (field, name) in DOT_FIELDS.iter().enumerate() {
            if (g[field] - e[field]).abs() >= tol {
                return Some(format!(
                    "dot[{i}].{name} mismatch: expected {}, got {}",
                    e[field], g[field]
                ));
            }
        }
    }
    None
}

/// Order-independent fallback: greedily pairs each computed dot with its
/// nearest not-yet-claimed golden dot (by squared distance across all 6
/// fields) and checks that pairing's tolerance, instead of requiring one
/// exact index order. A lexicographic tuple sort was tried first and
/// rejected -- modes with several near-identical dots (e.g. `ring`'s
/// parallel lanes tracing almost the same circle) have near-tied *x*, which
/// scrambles a tuple sort's pairing the same way near-degenerate z scrambles
/// `finalize_frame`'s: on `globe` (no such degeneracy) it produced a false
/// mismatch entirely from bad pairing, not a value error. Nearest-neighbor
/// matching is what the underlying claim actually is -- the same set of
/// dots was produced, independent of the order platforms happen to sort
/// near-tied keys in.
fn nearest_match_mismatch(got: &[[f64; 6]], expected: &[Vec<f64>], tol: f64) -> Option<String> {
    let mut used = vec![false; expected.len()];
    for (i, g) in got.iter().enumerate() {
        let mut best: Option<(usize, f64)> = None;
        for (j, e) in expected.iter().enumerate() {
            if used[j] {
                continue;
            }
            let dist: f64 = (0..6).map(|f| (g[f] - e[f]).powi(2)).sum();
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((j, dist));
            }
        }
        let Some((j, _)) = best else {
            return Some(format!(
                "dot[{i}]: no unmatched golden dot left to pair with"
            ));
        };
        used[j] = true;
        let e = &expected[j];
        for (field, name) in DOT_FIELDS.iter().enumerate() {
            if (g[field] - e[field]).abs() >= tol {
                return Some(format!(
                    "dot[{i}] nearest-matched golden[{j}] but .{name} differs: expected {}, got {}",
                    e[field], g[field]
                ));
            }
        }
    }
    None
}

#[test]
fn orbs_family_matches_golden_vectors() {
    // Every shipped size, each from one oracle file, each file covering all 9
    // states -- so a truncated or mis-sized file can't pass.
    let mut sizes = std::collections::BTreeSet::new();
    for (name, want_sizes) in [
        ("orbs-golden.json", vec![20, 64]),
        ("orbs-golden-32.json", vec![32]),
    ] {
        let got = check_golden(name);
        assert_eq!(got, want_sizes, "{name}: sizes covered");
        sizes.extend(got);
    }
    // The sizes the engine ships (`fx_spec::SIZES`).
    assert_eq!(
        sizes.into_iter().collect::<Vec<_>>(),
        [20, 32, 64],
        "the oracle covers every size the engine ships"
    );
}

/// Checks one oracle file case by case; returns the sizes it covered.
fn check_golden(name: &str) -> Vec<u32> {
    let golden = load_golden(name);
    let mut sizes = std::collections::BTreeSet::new();
    let tolerance = golden["tolerance"].as_f64().expect("tolerance");
    let mut checked_cases = 0;
    let mut checked_states = std::collections::HashSet::new();

    for case in golden["cases"].as_array().expect("cases") {
        let state = case["state"].as_str().expect("state");
        let size = case["size"].as_u64().expect("size") as u32;
        let t = case["t"].as_f64().expect("t");
        let expected_mode = case["mode"].as_str().expect("mode");
        let expected_dots_flat = case["dots"].as_array().expect("dots");
        let expected_lines_flat = case["lines"].as_array().expect("lines");
        let expected_dot_count = case["dotCount"].as_u64().expect("dotCount") as usize;
        let expected_line_count = case["lineCount"].as_u64().expect("lineCount") as usize;
        let ctx = || format!("{state}-{size} t={t}");

        let resolved = resolve_preset(state, size).expect("resolve_preset");
        assert_eq!(resolved.mode, expected_mode, "mode mismatch for {}", ctx());

        let golden_resolved = &golden["resolved"][format!("{state}-{size}")];
        assert_eq!(
            golden_resolved["speed"].as_f64().expect("golden speed"),
            resolved.speed,
            "speed mismatch for {}",
            ctx()
        );
        for (k, v) in golden_resolved["opts"].as_object().expect("golden opts") {
            let expect = v.as_f64().expect("opt value");
            let got = *resolved
                .opts
                .get(k)
                .unwrap_or_else(|| panic!("resolve_preset({state}, {size}) missing opt `{k}`"));
            assert!(
                (expect - got).abs() < 1e-9,
                "opt `{k}` mismatch for {}: expected {expect}, got {got}",
                ctx()
            );
        }

        let got = frame(state.to_string(), size, t)
            .unwrap_or_else(|| panic!("frame({state}, {size}, {t}) returned None"));
        assert_eq!(
            got.dots.len(),
            expected_dot_count,
            "dot count mismatch for {}",
            ctx()
        );
        assert_eq!(
            got.lines.len(),
            expected_line_count,
            "line count mismatch for {}",
            ctx()
        );
        assert_eq!(
            expected_dots_flat.len(),
            expected_dot_count * 6,
            "golden dot stride mismatch (expected stride 6)"
        );
        assert_eq!(
            expected_lines_flat.len(),
            expected_line_count * 7,
            "golden line stride mismatch (expected stride 7)"
        );

        let got_dots: Vec<[f64; 6]> = got.dots.iter().map(dot_row).collect();
        let expected_dots = parse_stride(expected_dots_flat, 6);
        if let Some(strict_err) = strict_mismatch(&got_dots, &expected_dots, tolerance) {
            if let Some(set_err) = nearest_match_mismatch(&got_dots, &expected_dots, tolerance) {
                panic!(
                    "{}: {strict_err}  (nearest-match comparison also failed: {set_err})",
                    ctx()
                );
            }
            // Index order differs but the same set of dots was produced --
            // a benign z-sort tie-break artifact, not a value error.
        }

        // `finalize_frame` never sorts `lines` (only filters by alpha), so
        // generation order is deterministic and a strict check suffices.
        for (i, line) in got.lines.iter().enumerate() {
            let base = i * 7;
            let e: Vec<f64> = expected_lines_flat[base..base + 7]
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let got_row = [
                line.x1, line.y1, line.x2, line.y2, line.white, line.a, line.w,
            ];
            let names = ["x1", "y1", "x2", "y2", "white", "a", "w"];
            for (field, name) in names.iter().enumerate() {
                assert!(
                    (got_row[field] - e[field]).abs() < tolerance,
                    "line[{i}].{name} mismatch for {}: expected {}, got {}",
                    ctx(),
                    e[field],
                    got_row[field]
                );
            }
        }

        checked_states.insert(state.to_string());
        sizes.insert(size);
        checked_cases += 1;
    }

    assert!(
        checked_cases > 0,
        "{name}: no cases found in golden vectors"
    );
    assert_eq!(
        checked_states.len(),
        9,
        "{name}: expected all 9 states to be covered"
    );
    sizes.into_iter().collect()
}

/// Sanity check for `frame_with_overrides` (added for the Studio's live
/// parameter tweaking -- see the Studio). No golden fixture exists for
/// arbitrary overrides, so this checks internal consistency rather than
/// comparing against a frozen vector: an override actually changes the
/// output, an empty override map reproduces the stock `frame()` exactly,
/// and overriding a non-structural key (scanMul) never changes dot count.
#[test]
fn frame_with_overrides_sanity() {
    let stock = frame("searching".to_string(), 64, 0.6).expect("stock frame");

    let empty = frame_with_overrides("searching".to_string(), 64, 0.6, HashMap::new())
        .expect("empty-override frame");
    assert_eq!(
        empty, stock,
        "empty overrides must reproduce the stock frame exactly"
    );

    let mut overrides = HashMap::new();
    overrides.insert("scanMul".to_string(), 8.0);
    let overridden = frame_with_overrides("searching".to_string(), 64, 0.6, overrides)
        .expect("overridden frame");
    assert_eq!(
        overridden.dots.len(),
        stock.dots.len(),
        "scanMul is not a count-affecting key -- dot count must be unchanged"
    );
    assert_ne!(
        overridden, stock,
        "a large scanMul override must actually change the rendered frame"
    );

    assert!(
        frame_with_overrides("not-a-real-state".to_string(), 64, 0.6, HashMap::new()).is_none(),
        "an unknown state must still resolve to None with overrides"
    );
}
