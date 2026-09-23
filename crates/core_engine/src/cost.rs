//! A render-cost *proxy* for a frame, for the Studio's cost badge and for
//! hosts deciding what to shed: how many primitives a renderer has to issue
//! (`elements`) and how much ink it lays down (`coverage`: total primitive
//! area / canvas area -- an overdraw proxy, since every renderer here paints
//! plain source-over fills and strokes). Glow's halo copies are ordinary
//! primitives in the frame, so a glowing object counts them automatically.
//!
//! This is not a measurement: paint cost depends on the renderer, the
//! device and the output resolution. The class thresholds were picked from
//! the distribution over every state (docs/engine.md, *Cost estimate*) and
//! sanity-checked against the Web frame timings of the performance pack.

use std::collections::HashMap;

use crate::primitives::OrbFrame;

/// A frame's (or a spec's) estimated render cost.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxCost {
    /// dots + lines + polylines + fills: draw calls a canvas-style renderer
    /// issues (plus `n - 1` for each per-vertex-hue polyline's segments).
    pub elements: u32,
    pub dots: u32,
    pub lines: u32,
    pub polylines: u32,
    /// Total polyline vertices.
    pub points: u32,
    /// Filled shapes (materials phase 1).
    pub fills: u32,
    /// Sum of primitive areas / canvas area (1.0 = the canvas covered once).
    pub coverage: f64,
    /// `"light"`, `"medium"` or `"heavy"`.
    pub class: String,
    /// The same object with every material off (glow / noise / pulse /
    /// gradient strength 0): `elements - base_elements` and `coverage -
    /// base_coverage` are what the materials add -- the "why" behind a class.
    pub base_elements: u32,
    pub base_coverage: f64,
    /// Blur work proxy: sum over blurred elements of (bounding box grown by
    /// 3 sigma each side) x sigma, / canvas area -- a Gaussian's cost grows
    /// with the area it touches and its radius. 0 when nothing is blurred.
    pub blur_load: f64,
}

/// Class cut points: (elements, coverage). A frame is `heavy` if it passes
/// either heavy bound, else `medium` if it passes either medium bound.
/// From `cost_distribution` (every state at 64, plain and with full glow):
/// no state exceeds 566 elements / 0.38 coverage without glow; with glow the
/// element counts cluster at <= 690, 817-932, 1171-1277 and 2028-2606, so
/// the bounds sit in the gaps (700, 1600). Coverage: plain <= 0.38, glow
/// 0.12-2.56 -- 1.0 = the canvas painted over once, 3.0 = three times.
pub const MEDIUM: (u32, f64) = (700, 1.0);
pub const HEAVY: (u32, f64) = (1600, 3.0);
/// Blur-load bounds (see `FxCost::blur_load`), from `cost_distribution`'s
/// glow-blur column at 64: orbs 0.04-1.99, signal 2.8-3.75, rings/beacons
/// (few but large blurred strokes) 5.7-12.5, shimmer 31, locating's one
/// canvas-sized halo 103 -- bounds in the gaps. Per-call overhead is
/// `elements`' job; this only measures area x radius.
pub const MEDIUM_BLUR: f64 = 2.0;
pub const HEAVY_BLUR: f64 = 5.0;

pub fn class_of(elements: u32, coverage: f64, blur_load: f64) -> &'static str {
    if elements > HEAVY.0 || coverage > HEAVY.1 || blur_load > HEAVY_BLUR {
        "heavy"
    } else if elements > MEDIUM.0 || coverage > MEDIUM.1 || blur_load > MEDIUM_BLUR {
        "medium"
    } else {
        "light"
    }
}

fn shoelace(pts: &[crate::primitives::Point]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (&pts[i], &pts[(i + 1) % n]);
        s += a.x * b.y - b.x * a.y;
    }
    (s * 0.5).abs()
}

/// (bbox w + 6 sigma) x (bbox h + 6 sigma) x sigma.
fn blurred(w: f64, h: f64, sigma: f64) -> f64 {
    if sigma <= 0.0 {
        0.0
    } else {
        (w + 6.0 * sigma) * (h + 6.0 * sigma) * sigma
    }
}

pub fn frame_cost(f: &OrbFrame, size: u32) -> FxCost {
    let seg = |x1: f64, y1: f64, x2: f64, y2: f64| ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let mut area = 0.0;
    for d in &f.dots {
        area += std::f64::consts::PI * d.r * d.r;
    }
    for l in &f.lines {
        area += seg(l.x1, l.y1, l.x2, l.y2) * l.w;
    }
    let mut points = 0;
    for p in &f.polylines {
        points += p.points.len();
        for w in p.points.windows(2) {
            area += seg(w[0].x, w[0].y, w[1].x, w[1].y) * p.w;
        }
    }
    for x in &f.fills {
        area += shoelace(&x.points);
        for ring in &x.holes {
            area -= shoelace(ring);
        }
    }
    // Blur: fills carry their own sigma; effect runs blur ranges of a list.
    let bbox = |pts: &mut dyn Iterator<Item = (f64, f64)>| {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in pts {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        if x1 < x0 {
            (0.0, 0.0)
        } else {
            (x1 - x0, y1 - y0)
        }
    };
    let mut blur = 0.0;
    for x in &f.fills {
        let (w, h) = bbox(&mut x.points.iter().map(|p| (p.x, p.y)));
        blur += blurred(w, h, x.blur);
    }
    for e in &f.effects {
        let range = e.start as usize..(e.start + e.count) as usize;
        match e.target {
            0 => {
                for d in f.dots.get(range).unwrap_or(&[]) {
                    blurred_add(&mut blur, 2.0 * d.r, 2.0 * d.r, e.blur);
                }
            }
            1 => {
                for l in f.lines.get(range).unwrap_or(&[]) {
                    let (w, h) = ((l.x2 - l.x1).abs() + l.w, (l.y2 - l.y1).abs() + l.w);
                    blurred_add(&mut blur, w, h, e.blur);
                }
            }
            _ => {
                for p in f.polylines.get(range).unwrap_or(&[]) {
                    let (w, h) = bbox(&mut p.points.iter().map(|q| (q.x, q.y)));
                    blurred_add(&mut blur, w + p.w, h + p.w, e.blur);
                }
            }
        }
    }
    let canvas = size as f64 * size as f64;
    let blur_load = blur / canvas;
    // A per-vertex-hue polyline paints one gradient stroke per segment into
    // a layer, then composites the layer: (n - 1) strokes + 1 composite
    // where a plain polyline is 1 stroke -- n - 1 extra draw calls.
    let per_vertex: usize = f
        .polylines
        .iter()
        .filter(|p| !p.hues.is_empty())
        .map(|p| p.points.len().saturating_sub(1))
        .sum();
    let elements =
        (f.dots.len() + f.lines.len() + f.polylines.len() + f.fills.len() + per_vertex) as u32;
    let coverage = area / canvas;
    FxCost {
        elements,
        dots: f.dots.len() as u32,
        lines: f.lines.len() as u32,
        polylines: f.polylines.len() as u32,
        points: points as u32,
        coverage,
        class: class_of(elements, coverage, blur_load).to_string(),
        base_elements: elements,
        base_coverage: coverage,
        fills: f.fills.len() as u32,
        blur_load,
    }
}

fn blurred_add(acc: &mut f64, w: f64, h: f64, sigma: f64) {
    *acc += blurred(w, h, sigma);
}

/// Elapsed seconds the estimate samples (scaled by the state's speed): the
/// max over them keeps a badge from flickering as an animation breathes.
pub const SAMPLE_TIMES: [f64; 4] = [0.4, 1.3, 2.7, 4.1];

/// The worst of `SAMPLE_TIMES` for `render(t)`.
pub fn sampled(size: u32, mut render: impl FnMut(f64) -> Option<OrbFrame>) -> Option<FxCost> {
    let mut worst: Option<FxCost> = None;
    for t in SAMPLE_TIMES {
        let c = frame_cost(&render(t)?, size);
        worst = Some(match worst {
            None => c,
            Some(w) => {
                let elements = w.elements.max(c.elements);
                let coverage = w.coverage.max(c.coverage);
                let blur_load = w.blur_load.max(c.blur_load);
                FxCost {
                    elements,
                    dots: w.dots.max(c.dots),
                    lines: w.lines.max(c.lines),
                    polylines: w.polylines.max(c.polylines),
                    points: w.points.max(c.points),
                    coverage,
                    class: class_of(elements, coverage, blur_load).to_string(),
                    base_elements: w.base_elements.max(c.base_elements),
                    base_coverage: w.base_coverage.max(c.base_coverage),
                    fills: w.fills.max(c.fills),
                    blur_load,
                }
            }
        });
    }
    worst
}

/// Cost of `(state, size)` with `overrides` (the Studio's live knobs).
/// The material master keys a baseline turns off.
/// Liquid is a material too (naming split): the baseline turns it off.
const MATERIAL_STRENGTHS: [&str; 6] = [
    "glowStrength",
    "noiseStrength",
    "pulseStrength",
    "gradientStrength",
    "liquidStrength",
    "particleStrength",
];

/// `sampled` for `(state, size, speed, overrides)`, with the no-materials
/// baseline filled in.
fn with_baseline(
    state: &str,
    size: u32,
    speed: f64,
    overrides: &HashMap<String, f64>,
) -> Option<FxCost> {
    let run = |ov: &HashMap<String, f64>| {
        sampled(size, |t| {
            crate::frame_with_overrides(state.to_string(), size, t * speed, ov.clone())
        })
    };
    let mut cost = run(overrides)?;
    let mut bare = overrides.clone();
    for k in MATERIAL_STRENGTHS {
        bare.insert(k.to_string(), 0.0);
    }
    let base = run(&bare)?;
    cost.base_elements = base.elements;
    cost.base_coverage = base.coverage;
    Some(cost)
}

/// FX Spec cost: the spec resolved for `(state, inputs, low_power)`, sampled
/// at its own time scale -- so a low-power host sees what shedding saves.
pub fn estimate_spec(
    json: &str,
    state: Option<&str>,
    inputs: &HashMap<String, f64>,
    low_power: bool,
) -> Option<FxCost> {
    let r = crate::fx_spec::resolve_full(json, state, inputs, low_power);
    if !r.ok {
        return None;
    }
    let speed = crate::resolved_opts(r.state.clone(), r.size)?.speed * r.speed;
    with_baseline(&r.state, r.size, speed, &r.overrides)
}

pub fn estimate(state: &str, size: u32, overrides: &HashMap<String, f64>) -> Option<FxCost> {
    let speed = crate::resolved_opts(state.to_string(), size)?.speed;
    with_baseline(state, size, speed, overrides)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::all_states;

    #[test]
    fn known_states_land_in_their_class() {
        let glow: HashMap<String, f64> = HashMap::from([("glowStrength".to_string(), 1.0)]);
        let none = HashMap::new();
        let class = |s: &str, g: bool| {
            estimate(s, 64, if g { &glow } else { &none })
                .unwrap()
                .class
        };
        // Every state is light without glow.
        for s in all_states() {
            assert_eq!(class(s, false), "light", "{s}");
        }
        assert_eq!(class("working", true), "heavy");
        assert_eq!(class("breathing", true), "heavy");
        assert_eq!(class("glowing", true), "medium");
        assert_eq!(class("tracking", true), "medium", "overdraw, not count");
        assert_eq!(class("scanning", true), "light");
        // Stable across time: the estimate is the max over fixed samples.
        let a = estimate("working", 64, &glow).unwrap();
        assert_eq!(a, estimate("working", 64, &glow).unwrap());
        assert!(a.elements > 1600 && a.dots > 0 && a.points == 0);
        // The baseline explains the class: working's glow multiplies elements.
        assert_eq!(
            a.base_elements,
            estimate("working", 64, &none).unwrap().elements
        );
        assert!(a.elements > 3 * a.base_elements && a.coverage > a.base_coverage);
        let plain = estimate("working", 64, &none).unwrap();
        assert_eq!(
            (plain.base_elements, plain.base_coverage),
            (plain.elements, plain.coverage)
        );
        // Spec cost: low power sheds glow + noise, and the estimate shows it.
        let spec = include_str!("../../../spec/examples/status-beacon-power.fxspec.json");
        let hi = estimate_spec(spec, None, &none, false).unwrap();
        let lo = estimate_spec(spec, None, &none, true).unwrap();
        assert!(lo.elements < hi.elements);
        assert_eq!(
            (lo.elements, lo.base_elements),
            (hi.base_elements, hi.base_elements)
        );
        assert!(estimate_spec("{", None, &none, false).is_none());
        // Real-blur glow: half the elements of stacked, blur load instead.
        let blur_glow: HashMap<String, f64> = HashMap::from([
            ("glowStrength".to_string(), 1.0),
            ("glowMode".to_string(), 1.0),
        ]);
        let wb = estimate("working", 64, &blur_glow).unwrap();
        assert!(wb.elements < a.elements && wb.blur_load > 0.0 && a.blur_load == 0.0);
        assert_eq!(wb.class, "medium", "1032 elements; stacked glow was heavy");
        assert_eq!(estimate("tracking", 64, &blur_glow).unwrap().class, "heavy");
        // Low power (blurScale 0) sheds the blur: glow falls back to stacked.
        let mut shed = blur_glow.clone();
        shed.insert("blurScale".to_string(), 0.0);
        assert_eq!(estimate("working", 64, &shed).unwrap().elements, a.elements);
        // Fills count: shimmer's fill mode is two fills, no polylines.
        let sh = estimate(
            "generating",
            64,
            &HashMap::from([("highlightFill".to_string(), 1.0)]),
        )
        .unwrap();
        assert_eq!((sh.fills, sh.polylines, sh.elements), (2, 0, 2));
        assert!(sh.coverage > 0.0);
        // Liquid counts as a material in the baseline.
        let liq = estimate(
            "drifting",
            64,
            &HashMap::from([("liquidStrength".to_string(), 1.0)]),
        )
        .unwrap();
        let plain_drift = estimate("drifting", 64, &HashMap::new()).unwrap();
        assert_eq!(
            (liq.base_elements, liq.base_coverage),
            (plain_drift.elements, plain_drift.coverage)
        );
        assert_ne!(liq.elements, liq.base_elements);
        let sc = estimate("scrolling", 64, &HashMap::new()).unwrap();
        assert!(sc.polylines > 0 && sc.points > sc.polylines);
    }

    /// How the thresholds were chosen:
    /// `cargo test -p core_engine cost_distribution -- --ignored --nocapture`.
    #[test]
    #[ignore = "prints the per-state cost distribution the class thresholds come from"]
    fn cost_distribution() {
        let glow: HashMap<String, f64> = HashMap::from([("glowStrength".to_string(), 1.0)]);
        let blur: HashMap<String, f64> = HashMap::from([
            ("glowStrength".to_string(), 1.0),
            ("glowMode".to_string(), 1.0),
        ]);
        for s in all_states() {
            let a = estimate(s, 64, &HashMap::new()).unwrap();
            let g = estimate(s, 64, &glow).unwrap();
            let b = estimate(s, 64, &blur).unwrap();
            println!(
                "{s:14} plain {:5} el {:6.2} cov {:6} | glow {:5} el {:6.2} cov {:6} | blur {:5} el {:6.2} cov {:6.2} load {:6}",
                a.elements, a.coverage, a.class, g.elements, g.coverage, g.class,
                b.elements, b.coverage, b.blur_load, b.class
            );
        }
    }
}
