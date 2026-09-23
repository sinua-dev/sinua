//! Sinua's own frozen vectors (`spec/sinua-golden.json`) for every
//! state upstream's `spec/orbs-golden.json` does not cover: the additive
//! `orbs` states and every `signal`/`ring`/`beacon`/`core` state.
//!
//! **What this proves, and what it doesn't.** Upstream's set is an
//! external oracle -- vectors produced by a different implementation
//! (`thinking-orbs`' TS engine) that our port must reproduce. This set is
//! not: it was generated *by this engine*, so it can only say "output is
//! unchanged since the baseline", never "output is right". It is a
//! regression and cross-platform lock (the same role Jest's snapshot tests
//! play -- "either the change is unexpected, or the reference snapshot
//! needs to be updated"), catching silent drift from refactors like a
//! shared-helper extraction, and later giving the native FFI tests a set
//! of values to agree with.
//!
//! **Re-baselining is deliberate, never a way to make a red test green.**
//! Regenerate only with
//! `SINUA_GOLDEN_WRITE=1 cargo test -p core_engine --test sinua_golden -- --ignored`,
//! then **review the `digests` block** -- one named line per case -- and log
//! which states changed and why in the commit message (see
//! `docs/testing.md`). Without the env var the generator refuses to write.
//!
//! Read the digests, not the vectors: measured 2026-09-20, one constant
//! changed in one pattern moves 12 of 233 cases but rewrites 17,254 lines,
//! and **none** of those hunks carries a case name. That is why the review
//! step used to be unperformable, and why `digests` exists.
//!
//! `spec/orbs-golden.json`, `tests/golden.rs` and the nine ported modes are
//! deliberately untouched by this file.

use core_engine::{
    frame_with_overrides, parameter_catalog_json, resolved_opts, ColorMode, Fill, OrbFrame,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/sinua-golden.json");
const TOLERANCE: f64 = 1e-4;
const SIZES: [u32; 3] = [64, 32, 20];
const TIMES: [f64; 4] = [0.6, 1.7, 3.3, 5.1];
const DOT_STRIDE: usize = 8;
const LINE_STRIDE: usize = 9;

/// The nine states upstream's set covers (and this one must not).
const PORTED: [&str; 9] = [
    "working",
    "searching",
    "solving",
    "listening",
    "connecting",
    "weaving",
    "composing",
    "breathing",
    "shaping",
];

/// Override keys that make a case a *material* case: a post-process locked
/// on top of some state's geometry. Only these cases may borrow a ported
/// state (a depth-shaded sphere, `connecting`'s edges) -- they freeze what
/// `apply_color` / `apply_gradient` do to that geometry, not the ported
/// geometry itself, which stays upstream's to prove (tests/golden.rs).
const MATERIAL_KEYS: [&str; 3] = ["colorMix", "colorMode", "gradientStrength"];

/// Every state this set freezes -- the complement of `PORTED` across all
/// five families. `every_non_ported_state_is_frozen` checks each one
/// resolves; adding a state means adding it here (docs/testing.md).
const STATES: [&str; 25] = [
    // orbs, additive
    "glowing",
    "drifting",
    "speaking",
    "confirming",
    "initializing",
    "calibrating",
    "progressing",
    "concluding",
    "muted",
    // signal
    "signaling",
    "waveform",
    "scrolling",
    "metering",
    // ring
    "completing",
    "loading",
    "tracking",
    "stepping",
    "measuring",
    // beacon
    "notifying",
    "reconnecting",
    "locating",
    "scanning",
    "broadcasting",
    // core
    "generating",
    "typing",
];

/// (state, key tag, overrides) for an input-driven extra case.
type ExtraCase = (&'static str, &'static str, Vec<(String, f64)>);

struct Case {
    key: String,
    state: &'static str,
    size: u32,
    t: f64,
    overrides: Vec<(String, f64)>,
}

fn speaking_bands() -> Vec<(String, f64)> {
    let bands = [0.9, 0.8, 0.65, 0.5, 0.4, 0.3, 0.2, 0.12];
    let mut o = vec![
        ("voiceStateCode".to_string(), 4.0),
        ("audioBandCount".to_string(), bands.len() as f64),
    ];
    o.extend(
        bands
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("audioBand{i}"), *v)),
    );
    o
}

/// The single source of truth for what is frozen -- the generator writes
/// exactly these cases and the checker requires exactly these keys.
fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    // Plain `frame()` output: every state, upstream's sizes and times.
    for state in STATES {
        for size in SIZES {
            for t in TIMES {
                out.push(Case {
                    key: format!("{state}-{size}-{t}"),
                    state,
                    size,
                    t,
                    overrides: vec![],
                });
            }
        }
    }
    // Input-driven paths plain `frame()` never reaches (a 0% ring is just
    // a dot; signal without audio only runs its synthetic pattern).
    let s = |v: &[(&str, f64)]| {
        v.iter()
            .map(|(k, x)| (k.to_string(), *x))
            .collect::<Vec<_>>()
    };
    let history: Vec<(String, f64)> = {
        let mut h = vec![
            ("historyCount".to_string(), 24.0),
            ("historyPhase".to_string(), 0.4),
        ];
        h.extend((0..24).map(|i| (format!("history{i}"), ((i as f64 * 0.7).sin() * 0.5 + 0.5))));
        h
    };
    let extra: Vec<ExtraCase> = vec![
        ("completing", "progress37", s(&[("progress", 0.37)])),
        ("stepping", "progress60", s(&[("progress", 0.6)])),
        (
            "measuring",
            "progress70-marker",
            s(&[("progress", 0.7), ("marker", 1.0)]),
        ),
        (
            "tracking",
            "laps",
            s(&[("progress0", 0.8), ("progress1", 0.55), ("progress2", 1.2)]),
        ),
        ("signaling", "speaking-bands", speaking_bands()),
        ("waveform", "speaking-bands", speaking_bands()),
        ("metering", "speaking-bands", speaking_bands()),
        ("scrolling", "history", history),
        ("reconnecting", "quality66", s(&[("quality", 0.66)])),
        ("broadcasting", "level50", s(&[("level", 0.5)])),
        // Color system (2026-09-18, spec 1.1.0): `apply_color` on a line-heavy
        // orb (edges coloured), fixed mode + a lightness bias on a depth-
        // shaded sphere, the 3-stop gradient, and the gradient on lines.
        (
            "connecting",
            "color",
            s(&[
                ("colorMix", 1.0),
                ("colorHue", 200.0),
                ("colorSaturation", 0.8),
                ("colorLightness", 0.2),
            ]),
        ),
        (
            "working",
            "color-fixed",
            s(&[
                ("colorMix", 1.0),
                ("colorHue", 330.0),
                ("colorLightness", 0.3),
                ("colorMode", 1.0),
            ]),
        ),
        (
            "searching",
            "gradient3",
            s(&[
                ("gradientStrength", 1.0),
                ("gradientHue", 200.0),
                ("gradientHue2", 300.0),
                ("gradientHue3", 40.0),
                ("gradientMid", 0.4),
            ]),
        ),
        ("connecting", "gradient", s(&[("gradientStrength", 1.0)])),
        // Materials phase 1 (spec 1.2.0): the fill primitive and paint
        // effects -- shimmer's gradient-filled highlight, radar's filled
        // wedge, real-blur glow on dots and (additive) on polylines.
        ("generating", "highlight-fill", s(&[("highlightFill", 1.0)])),
        ("scanning", "trail-fill", s(&[("trailFill", 1.0)])),
        (
            "glowing",
            "glow-blur",
            s(&[("glowStrength", 1.0), ("glowMode", 1.0)]),
        ),
        (
            "tracking",
            "glow-blur-additive",
            s(&[("glowStrength", 0.8), ("glowMode", 1.0), ("glowBlend", 1.0)]),
        ),
        // Materials phase 2 (spec 1.3.0): liquid -- metaball contours of
        // the dots as dots, as outlines, and as a filled band with a hole.
        (
            "glowing",
            "liquid-dots",
            s(&[("liquidStrength", 1.0), ("liquidStyle", 2.0)]),
        ),
        ("metering", "liquid-outline", s(&[("liquidStrength", 1.0)])),
        (
            "speaking",
            "liquid-fill",
            s(&[("liquidStrength", 1.0), ("liquidStyle", 0.0)]),
        ),
        // Materials phase 3 (spec 1.4.0): particles -- drift, attract, orbit
        // from a stroke emitter, and composed with liquid.
        (
            "glowing",
            "particles-drift",
            // Explicit drift: `glowing`'s own default is a slow orbit (1.5.0).
            s(&[("particleStrength", 1.0), ("particleStyle", 0.0)]),
        ),
        (
            "speaking",
            "particles-attract",
            s(&[("particleStrength", 1.0), ("particleStyle", 1.0)]),
        ),
        (
            "completing",
            "particles-orbit",
            s(&[("particleStrength", 1.0), ("particleStyle", 2.0)]),
        ),
        (
            "drifting",
            "particles-liquid",
            s(&[("particleStrength", 1.0), ("liquidStrength", 1.0)]),
        ),
        // Materials phase 4 (spec 1.5.0): holographic-lite -- dots with z,
        // a fill with a hole, strokes under a glow, and blended over a gradient.
        ("glowing", "holo", s(&[("holoStrength", 1.0)])),
        (
            "speaking",
            "holo-fill",
            s(&[
                ("holoStrength", 1.0),
                ("liquidStrength", 1.0),
                ("liquidStyle", 0.0),
            ]),
        ),
        (
            "completing",
            "holo-glow",
            s(&[("holoStrength", 1.0), ("glowStrength", 0.8)]),
        ),
        (
            "drifting",
            "holo-gradient",
            s(&[("holoStrength", 0.5), ("gradientStrength", 1.0)]),
        ),
        // 1.6.0: per-vertex stroke colour -- holo and a 3-stop gradient
        // sweeping along ring tracks, a beacon's rings, and an interrupt
        // flash over a swept stroke (per-vertex hues mapped, then dropped
        // where they collapse).
        ("tracking", "holo", s(&[("holoStrength", 1.0)])),
        (
            "tracking",
            "gradient3",
            s(&[
                ("gradientStrength", 1.0),
                ("gradientHue", 200.0),
                ("gradientHue2", 300.0),
                ("gradientHue3", 40.0),
            ]),
        ),
        ("locating", "holo", s(&[("holoStrength", 1.0)])),
        (
            "completing",
            "holo-interrupt",
            s(&[("holoStrength", 1.0), ("interruptAge", 0.15)]),
        ),
    ];
    for (state, tag, overrides) in extra {
        out.push(Case {
            key: format!("{state}-64-0.6-{tag}"),
            state,
            size: 64,
            t: 0.6,
            overrides,
        });
    }
    out
}

fn render(c: &Case) -> OrbFrame {
    let o: HashMap<String, f64> = c.overrides.iter().cloned().collect();
    frame_with_overrides(c.state.to_string(), c.size, c.t, o)
        .unwrap_or_else(|| panic!("{}: state did not resolve", c.key))
}

fn color_mode_str(m: ColorMode) -> &'static str {
    match m {
        ColorMode::Ink => "ink",
        ColorMode::Fixed => "fixed",
    }
}

fn r6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn flat(values: impl IntoIterator<Item = f64>) -> Vec<Value> {
    values.into_iter().map(|v| json!(r6(v))).collect()
}

fn dot_rows(f: &OrbFrame) -> Vec<[f64; DOT_STRIDE]> {
    f.dots
        .iter()
        .map(|d| [d.x, d.y, d.z, d.r, d.white, d.a, d.saturation, d.hue])
        .collect()
}

#[test]
#[ignore = "writes spec/sinua-golden.json; run deliberately with SINUA_GOLDEN_WRITE=1"]
fn regenerate_sinua_golden() {
    assert_eq!(
        std::env::var("SINUA_GOLDEN_WRITE").as_deref(),
        Ok("1"),
        "refusing to re-baseline without SINUA_GOLDEN_WRITE=1 -- see docs/testing.md"
    );
    // Every (state, size) a case renders -- the 25 states at every size,
    // plus the ported states the material cases borrow (see MATERIAL_KEYS).
    let mut resolved = Map::new();
    for c in cases() {
        let key = format!("{}-{}", c.state, c.size);
        if resolved.contains_key(&key) {
            continue;
        }
        let r = resolved_opts(c.state.to_string(), c.size).expect("resolves");
        let opts: BTreeMap<String, f64> = r.opts.into_iter().collect();
        resolved.insert(
            key,
            json!({ "mode": r.mode, "speed": r.speed, "opts": opts }),
        );
    }
    let cases: Vec<Value> = cases()
        .iter()
        .map(|c| {
            let f = render(c);
            let mode = resolved_opts(c.state.to_string(), c.size).unwrap().mode;
            let overrides: BTreeMap<String, f64> = c.overrides.iter().cloned().collect();
            let mut case = json!({
                "key": c.key,
                "state": c.state,
                "size": c.size,
                "mode": mode,
                "t": c.t,
                "overrides": overrides,
                "dotCount": f.dots.len(),
                "lineCount": f.lines.len(),
                "polylineCount": f.polylines.len(),
                "dots": flat(dot_rows(&f).into_iter().flatten()),
                "colorMode": color_mode_str(f.color_mode),
                "lines": flat(f.lines.iter().flat_map(|l| [l.x1, l.y1, l.x2, l.y2, l.white, l.a, l.w, l.saturation, l.hue])),
                "polylines": f.polylines.iter().map(|p| {
                    let mut o = json!({
                        "style": flat([p.white, p.a, p.w, p.saturation, p.hue]),
                        "points": flat(p.points.iter().flat_map(|q| [q.x, q.y])),
                    });
                    // 1.6.0: only on strokes that have them.
                    if !p.hues.is_empty() {
                        o["hues"] = json!(flat(p.hues.iter().copied()));
                    }
                    o
                }).collect::<Vec<_>>(),
            });
            // 1.2.0: only on cases that have them, so older cases are unchanged.
            if !f.fills.is_empty() {
                case["fills"] = json!(f.fills.iter().map(fill_json).collect::<Vec<_>>());
            }
            if !f.effects.is_empty() {
                case["effects"] = json!(flat(f.effects.iter().flat_map(|e| {
                    [e.target as f64, e.start as f64, e.count as f64, e.blur, e.blend as f64]
                })));
            }
            case
        })
        .collect();
    let doc = json!({
        "specVersion": "1.7.0",
        "note": "Sinua's own regression / cross-platform lock for every state spec/orbs-golden.json does not cover. \
    Generated by this engine, so it proves 'unchanged since baseline', NOT correctness -- upstream's set remains the only external oracle. \
    Dot stride 8: x, y, z, r, white, a, saturation, hue (dots in draw order). Line stride 9: x1, y1, x2, y2, white, a, w, saturation, hue (1.1.0; was 7). \
    colorMode per case: 'ink' (renderer mirrors lightness on dark) or 'fixed'. \
    Polylines: style [white, a, w, saturation, hue] + points [x, y, ...]. \
    1.2.0 (materials phase 1): cases with fills carry `fills` [{style [white, a, saturation, hue, blur, blend], points [x, y, ...], gradient null | {kind, geom [x0, y0, x1, y1, r], stops [offset, white, a, saturation, hue, ...]}}]; \
    cases with effect runs carry `effects` [target, start, count, blur, blend, ...]. \
    1.3.0 (materials phase 2, liquid): a fill with inner rings carries `holes` [[x, y, ...], ...] (even-odd with `points`); absent when none. \
    1.4.0 (materials phase 3): particle cases -- no new fields (particles are dots). \
    1.5.0 (materials phase 4): holographic-lite cases -- no new fields (it recolours saturation/hue); the 4 particle cases re-baselined deliberately (particles on wall-clock time, per-state defaults, calmer motion, position-picked emitters). \
    1.6.0 (per-vertex stroke colour): a polyline whose vertex hues differ (holo / gradient on strokes) carries `hues` [h, ...], one per vertex; absent when none. 1.7.0 (size 32): the plain frame() cases at every shipped size, 64 / 32 / 20; every earlier case unchanged. Size 32 of the ported orbs is also held to upstream's own engine (spec/orbs-golden-32.json). Re-baseline only deliberately, with a LOG entry (docs/testing.md).",
        "tolerance": TOLERANCE,
        "sizes": SIZES,
        "times": TIMES,
        "resolved": resolved,
        // Read this block when reviewing a re-baseline: one line per case,
        // named, so "which states changed" is answerable without reading
        // 17,000 lines of floats. `digest_block_matches_the_vectors` keeps it
        // honest -- a digest map that disagreed with the cases would be worse
        // than none, because a reviewer would be reading a block that lies.
        "digests": digests_of(&cases),
        "cases": cases,
    });
    fs::write(PATH, serde_json::to_string(&doc).unwrap() + "\n").expect("write golden");
}

fn fill_json(x: &Fill) -> Value {
    let mut v = json!({
        "style": flat([x.white, x.a, x.saturation, x.hue, x.blur, x.blend as f64]),
        "points": flat(x.points.iter().flat_map(|q| [q.x, q.y])),
        "gradient": x.gradient.as_ref().map(|g| json!({
            "kind": g.kind,
            "geom": flat([g.x0, g.y0, g.x1, g.y1, g.r]),
            "stops": flat(g.stops.iter().flat_map(|s| [s.offset, s.white, s.a, s.saturation, s.hue])),
        })),
    });
    // 1.3.0: only fills that have holes carry the key (older cases unchanged).
    if !x.holes.is_empty() {
        v["holes"] = json!(x
            .holes
            .iter()
            .map(|r| flat(r.iter().flat_map(|q| [q.x, q.y])))
            .collect::<Vec<_>>());
    }
    v
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_f64().expect("number"))
        .collect()
}

/// Order-independent fallback for dots, same reasoning as upstream's test
/// (`tests/golden.rs`, docs/testing.md "z-sort tie-break"): copied rather
/// than shared because that file is deliberately left untouched.
fn nearest_match_ok(got: &[[f64; DOT_STRIDE]], expected: &[Vec<f64>]) -> Result<(), String> {
    let mut used = vec![false; expected.len()];
    for (i, g) in got.iter().enumerate() {
        let mut best: Option<(usize, f64)> = None;
        for (j, e) in expected.iter().enumerate() {
            if used[j] {
                continue;
            }
            let dist: f64 = (0..DOT_STRIDE).map(|f| (g[f] - e[f]).powi(2)).sum();
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((j, dist));
            }
        }
        let (j, _) = best.ok_or_else(|| format!("dot[{i}] has no golden dot left"))?;
        used[j] = true;
        for f in 0..DOT_STRIDE {
            if (g[f] - expected[j][f]).abs() >= TOLERANCE {
                return Err(format!(
                    "dot[{i}] field {f}: expected {}, got {}",
                    expected[j][f], g[f]
                ));
            }
        }
    }
    Ok(())
}

fn close(label: &str, got: f64, expected: f64) {
    assert!(
        (got - expected).abs() < TOLERANCE,
        "{label}: expected {expected}, got {got}"
    );
}

/// A stable per-case fingerprint over the vectors a parity test compares.
///
/// Why this exists: the workflow above asks a human to "review the JSON diff".
/// Measured, that review is not performable — one constant changed in one
/// pattern moves 12 of 233 cases but rewrites **17,254 lines** (12% of the
/// file), and the hunks carry no case name, because the numbers sit deep inside
/// arrays. So a re-baseline degraded to "regenerate and trust", which is what
/// this file's own header warns against.
///
/// The `digests` block fixes the artefact without touching the vectors: a
/// re-baseline now shows ~12 changed lines that each *name their case*, beside
/// the unreadable float lines. Read that block, not the floats.
///
/// FNV-1a over the case's canonical JSON. It is a **change detector, not a
/// security primitive** — collisions are irrelevant here because the vectors
/// themselves remain the thing under test; the digest only has to move when
/// they do.
fn digest(case: &Value) -> String {
    // Hash what a *reader* sees, not what the writer held in memory. One
    // `history` override does not survive serde_json's serialise -> parse
    // round trip: the writer holds `0.9272994540441407` and the file parses
    // back as `…408`. The vectors are safe from this because `flat` rounds
    // them through `r6`; the overrides map carries full f64 precision and is
    // not rounded. The difference is 1 ULP, far under the set's 1e-4
    // tolerance, so no parity test can see it -- but a digest taken before the
    // write would disagree with one taken after, and an index that disagrees
    // with what it indexes is worse than no index. Normalising here makes
    // writer and checker agree by construction.
    let normalised: Value = serde_json::from_str(&serde_json::to_string(case).unwrap()).unwrap();
    // `Value::Object` is a BTreeMap without the `preserve_order` feature, so
    // key order is already deterministic.
    let bytes = serde_json::to_string(&normalised).unwrap();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{h:016x}")
}

/// `{ case key -> digest }`, sorted, for the whole set.
fn digests_of(cases: &[Value]) -> BTreeMap<String, String> {
    cases
        .iter()
        .map(|c| (c["key"].as_str().expect("key").to_string(), digest(c)))
        .collect()
}

fn load() -> Value {
    let data = fs::read_to_string(PATH).expect(
        "read spec/sinua-golden.json (generate with SINUA_GOLDEN_WRITE=1 ... -- --ignored)",
    );
    serde_json::from_str(&data).expect("parse spec/sinua-golden.json")
}

#[test]
fn digest_block_matches_the_vectors() {
    // An index that can drift from what it indexes is worse than no index: a
    // reviewer would read `digests` and believe it. Recompute every entry from
    // the checked-in vectors.
    let golden = load();
    let cases = golden["cases"].as_array().expect("cases");
    let want = digests_of(cases);
    let got: BTreeMap<String, String> = golden["digests"]
        .as_object()
        .expect("digests -- re-baseline with SINUA_GOLDEN_WRITE=1 to add it")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().expect("digest string").to_string()))
        .collect();

    let stale: Vec<&String> = want
        .iter()
        .filter(|(k, v)| got.get(*k) != Some(*v))
        .map(|(k, _)| k)
        .collect();
    assert!(
        stale.is_empty(),
        "digests disagree with cases for {} case(s): {stale:?} -- regenerate, never hand-edit",
        stale.len()
    );
    assert_eq!(
        got.len(),
        want.len(),
        "the digest block lists cases that no longer exist: {:?}",
        got.keys()
            .filter(|k| !want.contains_key(*k))
            .collect::<Vec<_>>()
    );
}

#[test]
fn sinua_states_match_frozen_vectors() {
    let golden = load();
    // Name every case whose vectors moved, before the per-value assertions
    // below stop at the first one. "Log which states changed and why" needs the
    // list, not the first failure.
    if let Some(frozen) = golden["digests"].as_object() {
        let live: BTreeMap<String, String> = cases()
            .iter()
            .map(|c| {
                let f = render(c);
                (
                    c.key.clone(),
                    format!("{}:{}:{}", f.dots.len(), f.lines.len(), f.polylines.len()),
                )
            })
            .collect();
        let shape_changed: Vec<&String> =
            live.keys().filter(|k| !frozen.contains_key(*k)).collect();
        assert!(
            shape_changed.is_empty(),
            "cases missing from the digest block: {shape_changed:?} -- re-baseline deliberately"
        );
    }
    let by_key: HashMap<&str, &Value> = golden["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|c| (c["key"].as_str().expect("key"), c))
        .collect();
    let expected_cases = cases();
    assert_eq!(
        by_key.len(),
        expected_cases.len(),
        "the frozen file and cases() disagree on the case list -- re-baseline deliberately"
    );

    for c in &expected_cases {
        let g = by_key
            .get(c.key.as_str())
            .unwrap_or_else(|| panic!("{} missing from spec/sinua-golden.json", c.key));
        let ctx = &c.key;

        // Preset resolution is part of the lock.
        let r = resolved_opts(c.state.to_string(), c.size).unwrap();
        let gr = &golden["resolved"][format!("{}-{}", c.state, c.size)];
        assert_eq!(gr["mode"].as_str(), Some(r.mode.as_str()), "{ctx}: mode");
        assert_eq!(gr["speed"].as_f64(), Some(r.speed), "{ctx}: speed");
        let gopts = gr["opts"].as_object().expect("opts");
        assert_eq!(gopts.len(), r.opts.len(), "{ctx}: resolved opt count");
        for (k, v) in gopts {
            close(&format!("{ctx}: opt {k}"), r.opts[k], v.as_f64().unwrap());
        }

        let f = render(c);
        assert_eq!(
            f.dots.len() as u64,
            g["dotCount"].as_u64().unwrap(),
            "{ctx}: dot count"
        );
        assert_eq!(
            f.lines.len() as u64,
            g["lineCount"].as_u64().unwrap(),
            "{ctx}: line count"
        );
        assert_eq!(
            f.polylines.len() as u64,
            g["polylineCount"].as_u64().unwrap(),
            "{ctx}: polyline count"
        );

        // Dots: strict index order first, nearest-neighbour set match as the
        // fallback for benign z-sort tie-break reorderings.
        let got = dot_rows(&f);
        let exp: Vec<Vec<f64>> = nums(&g["dots"])
            .chunks(DOT_STRIDE)
            .map(|c| c.to_vec())
            .collect();
        let strict_ok = got
            .iter()
            .zip(&exp)
            .all(|(a, b)| (0..DOT_STRIDE).all(|k| (a[k] - b[k]).abs() < TOLERANCE));
        if !strict_ok {
            if let Err(e) = nearest_match_ok(&got, &exp) {
                panic!("{ctx}: {e}");
            }
        }

        // Lines and polylines are never sorted -- strict.
        let lines = nums(&g["lines"]);
        assert_eq!(
            g["colorMode"].as_str(),
            Some(color_mode_str(f.color_mode)),
            "{ctx}: colorMode"
        );
        for (i, l) in f.lines.iter().enumerate() {
            let row = [
                l.x1,
                l.y1,
                l.x2,
                l.y2,
                l.white,
                l.a,
                l.w,
                l.saturation,
                l.hue,
            ];
            for (k, v) in row.iter().enumerate() {
                close(
                    &format!("{ctx}: line[{i}].{k}"),
                    *v,
                    lines[i * LINE_STRIDE + k],
                );
            }
        }
        let polys = g["polylines"].as_array().expect("polylines");
        for (i, (p, gp)) in f.polylines.iter().zip(polys).enumerate() {
            let style = nums(&gp["style"]);
            for (k, v) in [p.white, p.a, p.w, p.saturation, p.hue].iter().enumerate() {
                close(&format!("{ctx}: polyline[{i}].style[{k}]"), *v, style[k]);
            }
            let pts = nums(&gp["points"]);
            assert_eq!(
                pts.len(),
                p.points.len() * 2,
                "{ctx}: polyline[{i}] vertex count"
            );
            for (j, q) in p.points.iter().enumerate() {
                close(&format!("{ctx}: polyline[{i}].pt[{j}].x"), q.x, pts[j * 2]);
                close(
                    &format!("{ctx}: polyline[{i}].pt[{j}].y"),
                    q.y,
                    pts[j * 2 + 1],
                );
            }
            // 1.6.0: per-vertex hues, absent = none.
            let hues = gp.get("hues").map(nums).unwrap_or_default();
            assert_eq!(p.hues.len(), hues.len(), "{ctx}: polyline[{i}] hue count");
            for (j, (h, gh)) in p.hues.iter().zip(&hues).enumerate() {
                close(&format!("{ctx}: polyline[{i}].hues[{j}]"), *h, *gh);
            }
        }

        // 1.2.0: fills and effect runs, compared whole (fills never reorder).
        let gf = g["fills"].as_array().cloned().unwrap_or_default();
        assert_eq!(f.fills.len(), gf.len(), "{ctx}: fill count");
        for (i, (x, gx)) in f.fills.iter().zip(&gf).enumerate() {
            let want = fill_json(x);
            for part in ["style", "points"] {
                let (a, b) = (nums(&want[part]), nums(&gx[part]));
                assert_eq!(a.len(), b.len(), "{ctx}: fill[{i}].{part} length");
                for (k, (u, v)) in a.iter().zip(&b).enumerate() {
                    close(&format!("{ctx}: fill[{i}].{part}[{k}]"), *u, *v);
                }
            }
            assert_eq!(
                want["gradient"].is_null(),
                gx["gradient"].is_null(),
                "{ctx}: fill[{i}] gradient"
            );
            // 1.3.0: holes (absent key = none).
            let gh = gx["holes"].as_array().cloned().unwrap_or_default();
            let wh = want["holes"].as_array().cloned().unwrap_or_default();
            assert_eq!(wh.len(), gh.len(), "{ctx}: fill[{i}] hole count");
            for (h, (a, b)) in wh.iter().zip(&gh).enumerate() {
                let (a, b) = (nums(a), nums(b));
                assert_eq!(a.len(), b.len(), "{ctx}: fill[{i}].holes[{h}] length");
                for (k, (u, v)) in a.iter().zip(&b).enumerate() {
                    close(&format!("{ctx}: fill[{i}].holes[{h}][{k}]"), *u, *v);
                }
            }
            if !want["gradient"].is_null() {
                assert_eq!(
                    want["gradient"]["kind"], gx["gradient"]["kind"],
                    "{ctx}: fill[{i}] kind"
                );
                for part in ["geom", "stops"] {
                    let (a, b) = (nums(&want["gradient"][part]), nums(&gx["gradient"][part]));
                    assert_eq!(a.len(), b.len(), "{ctx}: fill[{i}].gradient.{part} length");
                    for (k, (u, v)) in a.iter().zip(&b).enumerate() {
                        close(&format!("{ctx}: fill[{i}].gradient.{part}[{k}]"), *u, *v);
                    }
                }
            }
        }
        let ge = if g["effects"].is_null() {
            vec![]
        } else {
            nums(&g["effects"])
        };
        let fe: Vec<f64> = f
            .effects
            .iter()
            .flat_map(|e| {
                [
                    e.target as f64,
                    e.start as f64,
                    e.count as f64,
                    e.blur,
                    e.blend as f64,
                ]
            })
            .collect();
        assert_eq!(fe.len(), ge.len(), "{ctx}: effect count");
        for (k, (u, v)) in fe.iter().zip(&ge).enumerate() {
            close(&format!("{ctx}: effects[{k}]"), *u, *v);
        }
    }
}

/// `STATES` + `PORTED` must be *every* pattern the engine has -- not merely a
/// list whose entries all happen to resolve.
///
/// `every_non_ported_state_is_frozen` checks each listed state resolves, which
/// catches a name that has gone away. It cannot catch the opposite: a pattern
/// added to the engine and never added here. The header two screens up says
/// "adding a state means adding it here", and that is a rule, not a check --
/// the third copy of the same list, and the same shape as
/// A pre-launch review finding.
///
/// The comparison goes through `parameter_catalog_json()` because this is an
/// integration test and the family modules are private, so `presets::STATES`
/// is not reachable from here. That is not a weaker check: `catalog.rs`'s
/// `catalog_pattern_names_match_the_presets` pins the catalog to the presets,
/// so catalog == presets and this pins golden == catalog. If either link
/// breaks, one of the two tests fails and names the state.
#[test]
fn the_two_lists_together_are_every_pattern_the_engine_has() {
    let catalog: Value = serde_json::from_str(&parameter_catalog_json()).unwrap();
    let mut in_engine: Vec<String> = catalog["objects"]
        .as_array()
        .expect("the catalog has an objects array")
        .iter()
        .flat_map(|o| o["patterns"].as_array().expect("each object has patterns"))
        .map(|p| {
            p["id"]
                .as_str()
                .expect("each pattern has an id")
                .to_string()
        })
        .collect();
    let mut listed: Vec<String> = STATES
        .iter()
        .chain(PORTED.iter())
        .map(|s| (*s).to_string())
        .collect();
    in_engine.sort();
    listed.sort();

    let unfrozen: Vec<&String> = in_engine.iter().filter(|s| !listed.contains(s)).collect();
    let stale: Vec<&String> = listed.iter().filter(|s| !in_engine.contains(s)).collect();
    assert!(
        unfrozen.is_empty() && stale.is_empty(),
        "this file's lists and the engine disagree.\n  \
         in the engine but in neither STATES nor PORTED (add it, and freeze a \
         case for it): {unfrozen:?}\n  \
         listed here but not in the engine: {stale:?}"
    );
}

#[test]
fn every_non_ported_state_is_frozen() {
    let frozen: HashSet<&str> = STATES.iter().copied().collect();
    for s in PORTED {
        assert!(
            !frozen.contains(s),
            "{s} is upstream's -- it belongs in golden.rs only"
        );
    }
    for s in STATES {
        assert!(
            resolved_opts(s.to_string(), 64).is_some(),
            "{s} no longer resolves"
        );
    }
    // Plain (and non-material input) cases cover exactly the 25 states.
    let covered: HashSet<&str> = cases()
        .iter()
        .filter(|c| !PORTED.contains(&c.state))
        .map(|c| c.state)
        .collect();
    assert_eq!(
        covered, frozen,
        "every listed state has at least one frozen case"
    );
    // A ported state only ever appears under a material override.
    for c in cases().iter().filter(|c| PORTED.contains(&c.state)) {
        assert!(
            c.overrides
                .iter()
                .any(|(k, _)| MATERIAL_KEYS.contains(&k.as_str())),
            "{}: a ported state may only appear in a material case",
            c.key
        );
    }
}
