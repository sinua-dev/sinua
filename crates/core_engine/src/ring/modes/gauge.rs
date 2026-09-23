//! Ring / Gauge: an open-bottom value arc (`measuring`) -- speedometer,
//! battery or volume dial, a Watch complication, a CPU/temperature widget.
//! A `progress: 0..1` value along a `sweep`-degree arc whose opening is
//! centered at 6 o'clock.
//!
//! Prior art (fetched): androidx Wear
//! Material 3's progress samples open the ring at the bottom with
//! `startAngle = 120f, endAngle = 60f` (a 300° sweep) and round a tiny value
//! up to the stroke width (our 0% dot); SwiftUI's `accessoryCircular` gauge
//! style is "an open ring with a marker ... at the gauge's current value"
//! (no fill); Home Assistant's `ha-gauge` is a 180° arc with an arc mode and
//! a needle mode; AG Charts' radial gauge shows the value as "a bar, a
//! needle or both". Default sweep 270° -- the orchestrator's brief, between
//! HA's 180° and Wear M3's 300°. Both presentations are opts: `fill` (the
//! bar, Wear M3) and `marker` (a dot at the value, SwiftUI), together or
//! apart. Not done: a needle and severity levels (see `docs/ring.md`).
//!
//! The marker reuses `nested`'s paper-colored separation halo (a halo under
//! an ink dot) so it cuts itself out of the track and fill in both themes.
//! Stroke/radius come from `arc`'s shared `layout`. Stateless.

use super::arc::layout;
use crate::primitives::{arc_polyline, finalize_frame, ring_point, ModeOpts, OrbFrame, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub const MIN_SWEEP: f64 = 90.0;
pub const MAX_SWEEP: f64 = 330.0;

/// Start angle (clockwise from 12) of a `sweep`-degree arc whose opening
/// is centered at 6 o'clock.
pub(crate) fn gauge_start(sweep: f64) -> f64 {
    180.0 + (360.0 - sweep) * 0.5
}

pub fn frame_gauge(size: f64, _t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let progress = get(o, "progress", 0.0).clamp(0.0, 1.0);
    let sweep = get(o, "sweep", 270.0).clamp(MIN_SWEEP, MAX_SWEEP);
    let fill = get(o, "fill", 1.0) >= 0.5;
    let marker = get(o, "marker", 0.0) >= 0.5;
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;
    let start = gauge_start(sweep);

    let mut polylines: Vec<Polyline> = Vec::with_capacity(4);
    if track_alpha > 0.0 {
        polylines.push(arc_polyline(
            l.cx,
            l.cy,
            l.r,
            start,
            sweep,
            l.stroke,
            white,
            track_alpha,
            0.0,
            0.0,
        ));
    }
    if fill {
        polylines.push(arc_polyline(
            l.cx,
            l.cy,
            l.r,
            start,
            progress * sweep,
            l.stroke,
            white,
            0.95,
            saturation,
            hue,
        ));
    }
    if marker {
        let at = ring_point(l.cx, l.cy, l.r, start + progress * sweep);
        polylines.push(Polyline {
            points: vec![at.clone(), at.clone()],
            white: 1.0,
            a: 0.9,
            w: l.stroke * 1.9,
            saturation: 0.0,
            hue: 0.0,
            hues: Vec::new(),
        });
        polylines.push(Polyline {
            points: vec![at.clone(), at],
            white,
            a: 0.95,
            w: l.stroke * 1.3,
            saturation,
            hue,
            hues: Vec::new(),
        });
    }

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{opts, Point};

    const SIZE: f64 = 64.0;

    fn angle(p: &Point) -> f64 {
        (p.x - 32.0)
            .atan2(-(p.y - 32.0))
            .to_degrees()
            .rem_euclid(360.0)
    }

    fn near(a: f64, b: f64) -> bool {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d) < 1e-6
    }

    #[test]
    fn default_opens_at_the_bottom_from_seven_thirty_to_four_thirty() {
        let f = frame_gauge(SIZE, 0.0, &opts(&[("progress", 1.0)]));
        let track = &f.polylines[0];
        let (a, b) = (&track.points[0], track.points.last().unwrap());
        assert!(near(angle(a), 225.0) && near(angle(b), 135.0));
        assert!((a.x - 32.0 + (b.x - 32.0)).abs() < 1e-9, "mirror in x");
        assert!(
            (a.y - b.y).abs() < 1e-9 && a.y > 32.0,
            "both ends below center"
        );
        // Passing through 12: some vertex sits straight up.
        assert!(track.points.iter().any(|p| near(angle(p), 0.0)));
    }

    #[test]
    fn progress_walks_the_sweep() {
        let half = frame_gauge(SIZE, 0.0, &opts(&[("progress", 0.5)]));
        assert!(
            near(angle(half.polylines[1].points.last().unwrap()), 0.0),
            "50% = 12"
        );
        let full = frame_gauge(SIZE, 0.0, &opts(&[("progress", 1.0)]));
        assert_eq!(
            full.polylines[1].points.last(),
            full.polylines[0].points.last(),
            "100% ends where the track does"
        );
        let zero = frame_gauge(SIZE, 0.0, &opts(&[("progress", 0.0)]));
        assert_eq!(
            zero.polylines[1].points[0], zero.polylines[1].points[1],
            "0% is a dot"
        );
    }

    #[test]
    fn sweep_is_clamped() {
        let wide = frame_gauge(SIZE, 0.0, &opts(&[("sweep", 360.0), ("progress", 1.0)]));
        let w = &wide.polylines[0];
        assert!(near(angle(&w.points[0]), gauge_start(330.0)));
        let narrow = frame_gauge(SIZE, 0.0, &opts(&[("sweep", 10.0), ("progress", 1.0)]));
        assert!(near(
            angle(&narrow.polylines[0].points[0]),
            gauge_start(90.0)
        ));
    }

    #[test]
    fn marker_only_is_the_open_ring_with_a_dot() {
        let f = frame_gauge(
            SIZE,
            0.0,
            &opts(&[("progress", 0.25), ("fill", 0.0), ("marker", 1.0)]),
        );
        assert_eq!(f.polylines.len(), 3, "track, halo, dot -- no bar");
        let (halo, dot) = (&f.polylines[1], &f.polylines[2]);
        assert_eq!(halo.points[0], halo.points[1]);
        assert!(
            halo.white > 0.99 && halo.w > dot.w,
            "paper halo, wider than the dot"
        );
        assert!(near(angle(&dot.points[0]), 225.0 + 0.25 * 270.0));
    }

    #[test]
    fn with_both_the_marker_draws_over_the_fill() {
        let f = frame_gauge(SIZE, 0.0, &opts(&[("progress", 0.6), ("marker", 1.0)]));
        assert_eq!(f.polylines.len(), 4);
        assert_eq!(
            f.polylines[3].points[0],
            *f.polylines[1].points.last().unwrap(),
            "marker sits on the fill's head, drawn last"
        );
    }

    #[test]
    fn shares_arcs_stroke_and_radius() {
        let o = opts(&[("progress", 1.0)]);
        let g = frame_gauge(SIZE, 0.0, &o);
        let l = layout(SIZE, &o);
        let p = &g.polylines[0].points[0];
        let r = ((p.x - 32.0).powi(2) + (p.y - 32.0).powi(2)).sqrt();
        assert!((r - l.r).abs() < 1e-9 && (g.polylines[0].w - l.stroke).abs() < 1e-9);
    }
}
