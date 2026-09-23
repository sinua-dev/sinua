//! Ring / Spinner: the indeterminate activity indicator (`loading`) -- the
//! classic chasing arc, for waits with no known end (Vercel's Geist
//! guidance: "indeterminate, single-action waits of roughly one to three
//! seconds"; and "mount the Spinner only after the action starts", since
//! a partial rotation sitting idle reads as jank -- a caller-side rule
//! this engine can't enforce, noted in `docs/ring.md`).
//!
//! The motion model is Material Components Web's `mdc-circular-progress`,
//! read from its SCSS: one **arc cycle of 1333ms** during which the arc's
//! head sweeps out 270 degrees with the standard easing and, in the
//! second half, the tail follows with the same easing -- so the arc
//! grows, then shrinks back to (almost) nothing -- while the whole thing
//! keeps rotating. MDC gets successive cycles to start at different
//! clock positions with a 216-degree "start rotation interval" plus a
//! linear container rotation; here the same effect comes from a
//! continuous base rotation of `270 + ROT_PER_CYCLE` degrees per cycle
//! (the 270 is the tail's own travel, which keeps the arc's start
//! continuous across the cycle boundary; `ROT_PER_CYCLE = 225` makes eight
//! cycles visit eight distinct orientations). Compose M3 uses a different
//! parameterization of the same idea (6000ms, arc 0.10-0.87 of a turn,
//! 1080-degree global rotation); MDC's is the simpler to transcribe
//! exactly. No track by default (Material's indeterminate variant has
//! none); `trackOpacity > 0` adds one.

use crate::primitives::{arc_polyline, standard_ease};
use crate::primitives::{finalize_frame, frac, ModeOpts, OrbFrame, Polyline};
use crate::ring::modes::arc::layout;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// MDC's `$arc-time`.
pub const CYCLE_S: f64 = 1.333;
/// MDC's `$arc-size`.
pub const ARC_DEG: f64 = 270.0;
/// Extra base rotation per cycle on top of the tail's 270-degree travel.
pub const ROT_PER_CYCLE: f64 = 225.0;
/// The arc never fully vanishes: a 2-degree minimum keeps a round dot.
const MIN_SWEEP_DEG: f64 = 2.0;

/// `(start_deg, sweep_deg)` of the arc at time `t` -- pulled out so the
/// tests (and any future ring variant) can reason about the motion
/// without a frame.
pub fn arc_at(t: f64) -> (f64, f64) {
    let cycles = (t / CYCLE_S).floor();
    let u = frac(t / CYCLE_S);
    let head = ARC_DEG * standard_ease((2.0 * u).min(1.0));
    let tail = ARC_DEG * standard_ease((2.0 * u - 1.0).max(0.0));
    let base = (ARC_DEG + ROT_PER_CYCLE) * cycles + ROT_PER_CYCLE * u;
    let start = base + tail;
    let sweep = (head - tail).max(MIN_SWEEP_DEG);
    (start, sweep)
}

pub fn frame_spinner(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let track_alpha = get(o, "trackOpacity", 0.0).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;

    let (start, sweep) = arc_at(t);
    let mut polylines: Vec<Polyline> = Vec::with_capacity(2);
    if track_alpha > 0.0 {
        polylines.push(arc_polyline(
            l.cx,
            l.cy,
            l.r,
            0.0,
            360.0,
            l.stroke,
            white,
            track_alpha,
            0.0,
            0.0,
        ));
    }
    polylines.push(arc_polyline(
        l.cx, l.cy, l.r, start, sweep, l.stroke, white, 0.95, saturation, hue,
    ));

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    #[test]
    fn sweep_stays_within_the_material_arc_range() {
        for i in 0..200 {
            let (_, sweep) = arc_at(i as f64 * 0.037);
            assert!(
                (MIN_SWEEP_DEG..=ARC_DEG + 1e-9).contains(&sweep),
                "t={}: sweep {sweep}",
                i as f64 * 0.037
            );
        }
    }

    #[test]
    fn arc_grows_in_the_first_half_of_a_cycle_and_shrinks_in_the_second() {
        let s = |u: f64| arc_at(u * CYCLE_S).1;
        assert!(s(0.1) < s(0.3) && s(0.3) < s(0.5), "growing");
        assert!(s(0.5) > s(0.7) && s(0.7) > s(0.95), "shrinking");
        assert!(
            (s(0.5) - ARC_DEG).abs() < 1e-6,
            "fully extended at the half-way point"
        );
    }

    #[test]
    fn start_is_continuous_across_a_cycle_boundary_and_visits_new_orientations() {
        let (before, _) = arc_at(CYCLE_S - 1e-6);
        let (after, _) = arc_at(CYCLE_S + 1e-6);
        assert!(
            (after - before).abs() < 0.01,
            "no jump at the boundary: {before} -> {after}"
        );
        let (s0, _) = arc_at(0.0);
        let (s1, _) = arc_at(CYCLE_S);
        assert!(((s1 - s0) - (ARC_DEG + ROT_PER_CYCLE)).abs() < 1e-6);
        assert!(
            (s1 - s0).rem_euclid(360.0) > 1.0,
            "the next cycle starts somewhere else on the clock"
        );
    }

    #[test]
    fn renders_one_round_capped_polyline_and_is_deterministic() {
        let o = opts(&[("rMin", 0.3)]);
        let a = frame_spinner(64.0, 0.4, &o);
        let b = frame_spinner(64.0, 0.4, &o);
        assert_eq!(a, b);
        assert_eq!(a.polylines.len(), 1, "no track by default");
        assert!(a.dots.is_empty() && a.lines.is_empty());
        let mut with_track = opts(&[("rMin", 0.3)]);
        with_track.insert("trackOpacity".to_string(), 0.2);
        assert_eq!(frame_spinner(64.0, 0.4, &with_track).polylines.len(), 2);
    }
}
