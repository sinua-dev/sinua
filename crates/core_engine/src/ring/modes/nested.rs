//! Ring / Nested: several values as concentric rings, one value per ring
//! (`tracking`) -- steps, active minutes and sleep; a quota per resource;
//! any small set of goals read together. Outermost ring is index 0, each
//! reads its own `progress{i}`, and a value past 1 keeps going for another
//! lap instead of saturating.
//!
//! Prior art (fetched): Google
//! Fit's two concentric rings (Heart Points + Steps, a double arrow once a
//! goal is surpassed); MKRingProgressView (values above 100% as extra laps,
//! a shadow under the line end so the head reads over its own tail, a
//! backdrop ring per ring, a grouped three-ring example); swdevnotes'
//! SwiftUI rings (past-360° sweep, end-cap circle). **Deliberately not
//! Apple's Activity rings**: Apple's HIG says "Don't attempt to replicate
//! or modify Activity rings for other purposes" and "Never use Activity
//! rings to display other types of data", so none of their signature look
//! is a default here -- no red/green/cyan triad, no along-the-ring
//! gradient, no tip arrows, no black disc. Grey ink by default (inner
//! rings a touch lighter so they stay distinguishable), one `hue` when
//! saturated, `hueStep` for opt-in distinct hues. See `docs/ring.md`.
//!
//! Geometry follows `arc`: 12 o'clock start, clockwise, round caps, the
//! same 0.41·size silhouette. Differences: the track is a full backdrop
//! circle (a gapped M3-style track can't survive a second lap), and past
//! 100% the ring draws a full lap, then a paper-colored **separation halo**
//! at the head, then the top arc -- the halo is MKRingProgressView's
//! end-cap shadow re-expressed as a cut-out, because ink is mirrored on dark
//! themes (a dark shadow would turn into a light glow there) while paper
//! mirrors to paper. Stateless: `t` is unused; filling animations are the
//! caller driving `progress{i}` (e.g. through `ReactiveBinding`).

use crate::primitives::{
    arc_polyline, finalize_frame, indexed_opt, ring_point, ModeOpts, OrbFrame, Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub const MAX_RINGS: usize = 4;
/// The innermost ring's centerline never gets closer to the center than
/// this (fraction of size) -- the stroke shrinks instead, so four rings
/// still leave a hole.
pub const MIN_INNER_RADIUS: f64 = 0.12;
const OUTER_EDGE: f64 = 0.41;

/// Centerline radii (outer first) and the stroke width actually used.
pub(crate) fn ring_radii(
    size: f64,
    count: usize,
    stroke_frac: f64,
    spacing: f64,
) -> (Vec<f64>, f64) {
    let pitch = 1.0 + spacing; // center-to-center distance, in strokes
    let max_stroke = size * (OUTER_EDGE - MIN_INNER_RADIUS) / (0.5 + (count as f64 - 1.0) * pitch);
    let stroke = (size * stroke_frac).min(max_stroke);
    let r0 = size * OUTER_EDGE - stroke * 0.5;
    let radii = (0..count).map(|i| r0 - i as f64 * stroke * pitch).collect();
    (radii, stroke)
}

pub fn frame_nested(size: f64, _t: f64, o: &ModeOpts) -> OrbFrame {
    let count = get(o, "ringCount", 3.0)
        .round()
        .clamp(1.0, MAX_RINGS as f64) as usize;
    let stroke_frac = get(o, "strokeWidth", 0.07).clamp(0.01, 0.4);
    let spacing = get(o, "spacing", 0.25).clamp(0.0, 2.0);
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let max_laps = get(o, "maxLaps", 3.0).clamp(1.0, 10.0);
    let hue = get(o, "hue", 200.0);
    let hue_step = get(o, "hueStep", 0.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let (radii, stroke) = ring_radii(size, count, stroke_frac, spacing);

    let mut polylines: Vec<Polyline> = Vec::with_capacity(count * 4);
    for (i, &r) in radii.iter().enumerate() {
        let p = indexed_opt(o, "progress", i)
            .unwrap_or(0.0)
            .clamp(0.0, max_laps);
        let white = 0.15 + 0.07 * i as f64;
        let h = (hue + hue_step * i as f64).rem_euclid(360.0);
        if track_alpha > 0.0 {
            polylines.push(arc_polyline(
                cx,
                cy,
                r,
                0.0,
                360.0,
                stroke,
                white,
                track_alpha,
                0.0,
                0.0,
            ));
        }
        let ind =
            |sweep: f64| arc_polyline(cx, cy, r, 0.0, sweep, stroke, white, 0.95, saturation, h);
        if p <= 1.0 {
            polylines.push(ind(p * 360.0));
            continue;
        }
        // Past one lap: the full lap underneath, a paper-colored halo just
        // under the head so it reads as passing over its own tail, then the
        // remaining partial lap on top (a whole number of laps = full ring).
        let frac = p - p.floor();
        let top = if frac < 1e-9 { 360.0 } else { frac * 360.0 };
        polylines.push(ind(360.0));
        let head = ring_point(cx, cy, r, top);
        polylines.push(Polyline {
            points: vec![head.clone(), head],
            white: 1.0,
            a: 0.85,
            w: stroke * 1.35,
            saturation: 0.0,
            hue: 0.0,
            hues: Vec::new(),
        });
        polylines.push(ind(top));
    }

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    const SIZE: f64 = 64.0;

    fn dist(p: &crate::primitives::Point) -> f64 {
        ((p.x - 32.0).powi(2) + (p.y - 32.0).powi(2)).sqrt()
    }

    /// Clockwise angle from 12 o'clock of a point, in `[0, 360)`.
    fn angle(p: &crate::primitives::Point) -> f64 {
        (p.x - 32.0)
            .atan2(-(p.y - 32.0))
            .to_degrees()
            .rem_euclid(360.0)
    }

    /// Indicator polylines (opaque, not tracks and not the halo).
    fn indicators(f: &OrbFrame) -> Vec<&Polyline> {
        f.polylines
            .iter()
            .filter(|p| p.a > 0.9 && p.white < 0.9)
            .collect()
    }

    #[test]
    fn empty_rings_are_dots_at_twelve_on_concentric_tracks() {
        let f = frame_nested(SIZE, 0.0, &opts(&[("ringCount", 3.0)]));
        assert_eq!(f.polylines.len(), 6, "track + indicator per ring");
        let ind = indicators(&f);
        assert_eq!(ind.len(), 3);
        let mut prev = f64::INFINITY;
        for p in &ind {
            assert_eq!(p.points[0], p.points[1], "0% is a round dot");
            assert!(
                (p.points[0].x - 32.0).abs() < 1e-9 && p.points[0].y < 32.0,
                "at 12"
            );
            let r = dist(&p.points[0]);
            assert!(r < prev, "outer ring first, radii shrink inward");
            assert!(r + p.w / 2.0 <= 0.41 * SIZE + 1e-9, "inside the silhouette");
            prev = r;
        }
    }

    #[test]
    fn each_ring_sweeps_its_own_value() {
        let f = frame_nested(
            SIZE,
            0.0,
            &opts(&[
                ("ringCount", 3.0),
                ("progress0", 0.25),
                ("progress1", 0.5),
                ("progress2", 0.75),
            ]),
        );
        let ind = indicators(&f);
        for (p, want) in ind.iter().zip([90.0, 180.0, 270.0]) {
            let end = p.points.last().unwrap();
            assert!((angle(end) - want).abs() < 1e-6, "ends at {want}°");
        }
    }

    #[test]
    fn a_second_lap_draws_full_ring_halo_then_the_top_arc() {
        let f = frame_nested(SIZE, 0.0, &opts(&[("ringCount", 1.0), ("progress0", 1.25)]));
        // track, full lap, halo, top arc
        assert_eq!(f.polylines.len(), 4);
        let lap = &f.polylines[1];
        let halo = &f.polylines[2];
        let top = &f.polylines[3];
        assert!(
            {
                let a = angle(lap.points.last().unwrap());
                a.min(360.0 - a) < 1e-6
            },
            "full lap closes at 12"
        );
        assert_eq!(halo.points[0], halo.points[1]);
        assert!(
            (angle(&halo.points[0]) - 90.0).abs() < 1e-6,
            "halo sits on the head"
        );
        assert!(
            halo.white > 0.99 && halo.w > top.w,
            "paper-colored, wider than the stroke"
        );
        assert!((angle(top.points.last().unwrap()) - 90.0).abs() < 1e-6);
    }

    #[test]
    fn exactly_two_laps_is_a_full_top_ring_and_values_are_capped() {
        let two = frame_nested(SIZE, 0.0, &opts(&[("ringCount", 1.0), ("progress0", 2.0)]));
        let top = two.polylines.last().unwrap();
        assert!(
            top.points.len() > 80,
            "a full circle, not a zero-length arc"
        );
        let capped = frame_nested(
            SIZE,
            0.0,
            &opts(&[("ringCount", 1.0), ("progress0", 9.5), ("maxLaps", 3.0)]),
        );
        let at_cap = frame_nested(
            SIZE,
            0.0,
            &opts(&[("ringCount", 1.0), ("progress0", 3.0), ("maxLaps", 3.0)]),
        );
        assert_eq!(capped, at_cap);
    }

    #[test]
    fn four_rings_shrink_the_stroke_to_keep_a_hole() {
        let (radii, stroke) = ring_radii(SIZE, 4, 0.2, 0.25);
        assert!(stroke < 0.2 * SIZE, "a thick stroke is shrunk");
        let inner = *radii.last().unwrap();
        assert!(inner >= MIN_INNER_RADIUS * SIZE - 1e-9);
        let (_, thin) = ring_radii(SIZE, 3, 0.07, 0.25);
        assert!(
            (thin - 0.07 * SIZE).abs() < 1e-9,
            "the default stroke is untouched"
        );
    }

    #[test]
    fn tracks_off_count_clamped_and_grey_by_default() {
        let f = frame_nested(
            SIZE,
            0.0,
            &opts(&[("ringCount", 9.0), ("trackOpacity", 0.0)]),
        );
        assert_eq!(f.polylines.len(), MAX_RINGS, "clamped to 4, no tracks");
        assert!(
            f.polylines.iter().all(|p| p.saturation == 0.0),
            "grey by default"
        );
        let one = frame_nested(
            SIZE,
            0.0,
            &opts(&[("ringCount", 0.0), ("trackOpacity", 0.0)]),
        );
        assert_eq!(one.polylines.len(), 1, "at least one ring");
    }

    #[test]
    fn hue_step_gives_each_ring_its_own_hue() {
        let f = frame_nested(
            SIZE,
            0.0,
            &opts(&[
                ("ringCount", 3.0),
                ("saturation", 0.8),
                ("hue", 200.0),
                ("hueStep", 40.0),
                ("trackOpacity", 0.0),
            ]),
        );
        let hues: Vec<f64> = f.polylines.iter().map(|p| p.hue).collect();
        assert_eq!(hues, vec![200.0, 240.0, 280.0]);
    }
}
