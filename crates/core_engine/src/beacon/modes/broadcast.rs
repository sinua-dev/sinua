//! Beacon / Broadcast: the `((•))` "live / transmitting / on air" glyph
//! (`broadcasting`) -- a dot with radio-wave arcs either side that light up
//! in sequence.
//!
//! Geometry is Material Symbols' `sensors` icon (raw SVG, parsed by hand --
//! fetched): in its 960 box a center dot of r 80,
//! two arcs per side at centerline radii 200 and 360 with an 80 stroke,
//! each arc 90° centered on the horizontal axis. Scaled here so the outer
//! arc's edge sits on the family's 0.41·size silhouette; the first arc keeps
//! Material's one-stroke gap from the dot. Material's arc ends are butt-cut;
//! our paint contract rounds them.
//!
//! Motion is SF Symbols' Variable Color on `dot.radiowaves.left.and.right`
//! (WWDC23 "Animate symbols in your app" and the SF Symbols docs/articles):
//! layers light up in sequence -- `.iterative` one at a time, `.cumulative`
//! filling and holding, `.reversing` running back out; inactive layers
//! either dimmed (`dimInactiveLayers`) or hidden (`hideInactiveLayers`) --
//! and as a static value, "0.5 colorizes half the beams" (`level`). Apple
//! publishes no timing: `period` 1.2s is a first guess, like every tuned
//! number in the additive modes. A short cross-fade between steps (0.15 of
//! a step) keeps it from strobing. Stateless: a pure function of `t`.

use crate::primitives::{arc_polyline, finalize_frame, Dot, ModeOpts, OrbFrame, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub const MAX_WAVES: usize = 4;
const LIT_ALPHA: f64 = 0.95;
const FADE: f64 = 0.15;

/// Arc centerline radii, inner first: the first keeps Material's one-stroke
/// gap from the dot, the last sits on the silhouette, even spacing between.
pub(crate) fn wave_radii(size: f64, n: usize, dot_r: f64, stroke: f64) -> Vec<f64> {
    let r_out = size * 0.41 - stroke * 0.5;
    let r_in = (dot_r + stroke * 1.5).min(r_out);
    if n == 1 {
        return vec![(r_in + r_out) * 0.5];
    }
    (0..n)
        .map(|i| r_in + (r_out - r_in) * i as f64 / (n - 1) as f64)
        .collect()
}

/// The thickest stroke that still leaves Material's one-stroke gap between
/// `n` waves (pitch = 2 strokes) between the dot and the silhouette.
pub(crate) fn max_stroke(size: f64, n: usize, dot_r: f64) -> f64 {
    ((size * 0.41 - dot_r) / (2.0 * n as f64)).max(size * 0.005)
}

/// How lit wave `i` of `n` is (0..1) at sequence position `s` in `[0, n+1)`
/// -- step 0 is the dot-only beat, step `i + 1` belongs to wave `i`.
pub(crate) fn lit(i: usize, s: f64, cumulative: bool) -> f64 {
    let on = ((s - (i as f64 + 1.0)) / FADE).clamp(0.0, 1.0);
    if cumulative {
        on
    } else {
        on * (((i as f64 + 2.0) - s) / FADE).clamp(0.0, 1.0)
    }
}

pub fn frame_broadcast(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let n = get(o, "waveCount", 2.0)
        .round()
        .clamp(1.0, MAX_WAVES as f64) as usize;
    let both = get(o, "sides", 2.0) >= 1.5;
    let sweep = get(o, "waveSweep", 90.0).clamp(10.0, 170.0);
    let period = get(o, "period", 1.2).max(0.05);
    let cumulative = get(o, "cumulative", 0.0) >= 0.5;
    let reversing = get(o, "reversing", 0.0) >= 0.5;
    let inactive = get(o, "inactiveOpacity", 0.18).clamp(0.0, 1.0);
    let level = o.get("level").map(|v| v.clamp(0.0, 1.0));
    let dot_r = size * get(o, "dotSize", 0.07).clamp(0.01, 0.3);
    // Keep Material's one-stroke gap between waves (a pitch of two strokes):
    // with more waves the stroke thins instead of the arcs merging.
    let stroke =
        (size * get(o, "strokeWidth", 0.07).clamp(0.01, 0.2)).min(max_stroke(size, n, dot_r));
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let white = 0.15;

    // Where in the lighting sequence we are, `s` in [0, n + 1).
    let steps = (n + 1) as f64;
    let u = (t / period).rem_euclid(1.0);
    let pass = if reversing {
        if u < 0.5 {
            2.0 * u
        } else {
            2.0 * (1.0 - u)
        }
    } else {
        u
    };
    let s = (pass * steps).min(steps - 1e-9);

    let radii = wave_radii(size, n, dot_r, stroke);
    let centers: &[f64] = if both { &[90.0, 270.0] } else { &[90.0] };
    let mut polylines: Vec<Polyline> = Vec::with_capacity(n * centers.len());
    for (i, &r) in radii.iter().enumerate() {
        let amount = match level {
            // Static variable-color value: "0.5 colorizes half the beams",
            // with a partial last wave.
            Some(v) => (v * n as f64 - i as f64).clamp(0.0, 1.0),
            None => lit(i, s, cumulative),
        };
        let a = inactive + (LIT_ALPHA - inactive) * amount;
        for &c in centers {
            polylines.push(arc_polyline(
                cx,
                cy,
                r,
                c - sweep * 0.5,
                sweep,
                stroke,
                white,
                a,
                saturation,
                hue,
            ));
        }
    }

    let dot = Dot {
        x: cx,
        y: cy,
        z: 0.0,
        r: dot_r,
        white,
        a: LIT_ALPHA,
        saturation,
        hue,
    };
    finalize_frame(vec![dot], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{opts, Point};

    const SIZE: f64 = 64.0;

    /// The arc's bisector point direction: the mean of its two ends (the
    /// sampled middle vertex isn't exactly central for an odd segment count).
    fn mid(p: &Polyline) -> Point {
        let (a, b) = (&p.points[0], p.points.last().unwrap());
        Point {
            x: (a.x + b.x) * 0.5,
            y: (a.y + b.y) * 0.5,
        }
    }

    fn lit_count(f: &OrbFrame) -> usize {
        f.polylines.iter().filter(|p| p.a > 0.9).count()
    }

    #[test]
    fn default_is_a_dot_with_two_waves_each_side_mirrored() {
        let f = frame_broadcast(SIZE, 0.0, &opts(&[]));
        assert_eq!(f.dots.len(), 1);
        assert_eq!(f.polylines.len(), 4, "2 waves x 2 sides");
        // Pairs are (right, left) per wave, inner first.
        for w in 0..2 {
            let (r, l) = (&f.polylines[2 * w], &f.polylines[2 * w + 1]);
            let (mr, ml) = (mid(r), mid(l));
            assert!(
                (mr.y - 32.0).abs() < 1e-9 && mr.x > 32.0,
                "right arc centered on 3 o'clock"
            );
            assert!((ml.y - 32.0).abs() < 1e-9 && ml.x < 32.0, "left arc on 9");
            assert!((mr.x - 32.0 + (ml.x - 32.0)).abs() < 1e-9, "mirrored");
        }
        let radius =
            |p: &Polyline| ((p.points[0].x - 32.0).powi(2) + (p.points[0].y - 32.0).powi(2)).sqrt();
        let (r0, r1) = (radius(&f.polylines[0]), radius(&f.polylines[2]));
        assert!(r1 > r0 && r1 + f.polylines[2].w / 2.0 <= 0.41 * SIZE + 1e-9);
    }

    #[test]
    fn many_waves_thin_the_stroke_instead_of_merging() {
        let f = frame_broadcast(SIZE, 0.0, &opts(&[("waveCount", 4.0), ("sides", 1.0)]));
        let radius =
            |p: &Polyline| ((p.points[0].x - 32.0).powi(2) + (p.points[0].y - 32.0).powi(2)).sqrt();
        for w in f.polylines.windows(2) {
            let gap = radius(&w[1]) - radius(&w[0]) - w[0].w;
            assert!(
                gap >= w[0].w - 1e-9,
                "at least a stroke of clear space between waves"
            );
        }
        let two = frame_broadcast(SIZE, 0.0, &opts(&[]));
        assert!(
            (two.polylines[0].w - 0.07 * SIZE).abs() < 1e-9,
            "the default stroke is untouched"
        );
    }

    #[test]
    fn one_side_and_wave_count_clamp() {
        let right = frame_broadcast(SIZE, 0.0, &opts(&[("sides", 1.0)]));
        assert_eq!(right.polylines.len(), 2);
        assert!(right.polylines.iter().all(|p| mid(p).x > 32.0));
        let many = frame_broadcast(SIZE, 0.0, &opts(&[("waveCount", 9.0)]));
        assert_eq!(many.polylines.len(), 2 * MAX_WAVES);
    }

    #[test]
    fn iterative_lights_one_wave_at_a_time_after_a_dot_only_beat() {
        // n = 2 -> 3 steps of 0.4s each with period 1.2.
        let o = opts(&[("period", 1.2)]);
        assert_eq!(
            lit_count(&frame_broadcast(SIZE, 0.2, &o)),
            0,
            "dot-only beat"
        );
        let step1 = frame_broadcast(SIZE, 0.6, &o);
        assert_eq!(lit_count(&step1), 2, "inner wave, both sides");
        assert!(step1.polylines[0].a > 0.9 && step1.polylines[2].a < 0.5);
        let step2 = frame_broadcast(SIZE, 1.0, &o);
        assert_eq!(lit_count(&step2), 2);
        assert!(
            step2.polylines[2].a > 0.9 && step2.polylines[0].a < 0.5,
            "moved outward"
        );
    }

    #[test]
    fn cumulative_only_ever_adds_waves_on_the_way_out() {
        let o = opts(&[("cumulative", 1.0), ("waveCount", 3.0), ("period", 1.0)]);
        let mut prev = 0;
        for k in 0..100 {
            let c = lit_count(&frame_broadcast(SIZE, k as f64 / 100.0, &o));
            assert!(c >= prev, "fill and hold");
            prev = c;
        }
        assert_eq!(prev, 6, "all three waves both sides by the end");
    }

    #[test]
    fn reversing_is_symmetric_and_the_period_repeats() {
        let o = opts(&[("reversing", 1.0), ("period", 2.0)]);
        assert_eq!(
            frame_broadcast(SIZE, 0.5, &o),
            frame_broadcast(SIZE, 1.5, &o)
        );
        let p = opts(&[("period", 2.0)]);
        assert_eq!(
            frame_broadcast(SIZE, 0.5, &p),
            frame_broadcast(SIZE, 2.5, &p)
        );
    }

    #[test]
    fn level_is_a_static_variable_color_value() {
        let a = frame_broadcast(SIZE, 0.1, &opts(&[("level", 0.5)]));
        let b = frame_broadcast(SIZE, 0.9, &opts(&[("level", 0.5)]));
        assert_eq!(a, b, "independent of t");
        assert!(
            a.polylines[0].a > 0.9 && a.polylines[1].a > 0.9,
            "inner wave lit"
        );
        assert!(
            a.polylines[2].a < 0.5 && a.polylines[3].a < 0.5,
            "outer wave inactive"
        );
    }

    #[test]
    fn hide_inactive_culls_and_grey_by_default() {
        let f = frame_broadcast(SIZE, 0.6, &opts(&[("inactiveOpacity", 0.0)]));
        assert_eq!(f.polylines.len(), 2, "only the lit wave remains");
        assert!(f.dots[0].saturation == 0.0 && f.polylines.iter().all(|p| p.saturation == 0.0));
        let g = frame_broadcast(SIZE, 0.6, &opts(&[("saturation", 0.7), ("hue", 10.0)]));
        assert!(g.dots[0].hue == 10.0 && g.polylines.iter().all(|p| p.hue == 10.0));
    }
}
