//! Ring / Arc: the determinate progress ring (`completing`). A `progress:
//! 0..1` opt sweeps a round-capped arc clockwise from 12 o'clock over a
//! faint track -- the only input is the number, so it fits a download, an
//! upload, a long tool call, a daily goal, anything.
//!
//! Built from Material 3's circular progress indicator, read from source
//! (androidx `ProgressIndicator.kt`: start angle 270 = 12 o'clock, sweep
//! `progress * 360`, `StrokeCap.Round`, and M3 expressive's
//! `TrackActiveSpace` -- the track stops short of the indicator's ends by a
//! small gap) and MDC Web's 4px-on-48px stroke (`strokeWidth` 0.085). Two
//! consequences worth knowing: at `progress = 0` the indicator is a
//! zero-length arc, which the paint contract draws as a round dot at 12
//! o'clock (Material's exact look), and at `progress = 1` the track has
//! nowhere left to be and disappears. Apple's Activity rings were looked
//! at and deliberately not imitated (their HIG reserves them for
//! Move/Exercise/Stand and forbids restyling).

use crate::primitives::arc_polyline;
use crate::primitives::{finalize_frame, ModeOpts, OrbFrame, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Shared layout for both ring modes: stroke, centerline radius (kept
/// inside the same 0.82 silhouette every family uses), and the track gap
/// as an angle.
pub(crate) struct Layout {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub stroke: f64,
    pub gap_deg: f64,
}

pub(crate) fn layout(size: f64, o: &ModeOpts) -> Layout {
    let stroke = size * get(o, "strokeWidth", 0.085).clamp(0.01, 0.4);
    let r = (size * 0.41 - stroke * 0.5).max(size * 0.05);
    // The gap is specified in stroke widths (M3's `TrackActiveSpace` is
    // one stroke width at the default size); convert arc length -> angle.
    let gap = get(o, "gap", 1.0).max(0.0) * stroke;
    Layout {
        cx: size * 0.5,
        cy: size * 0.5,
        r,
        stroke,
        gap_deg: (gap / r).to_degrees(),
    }
}

pub fn frame_arc(size: f64, _t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let progress = get(o, "progress", 0.0).clamp(0.0, 1.0);
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;

    let sweep = progress * 360.0;
    let mut polylines: Vec<Polyline> = Vec::with_capacity(2);

    // Track first (underneath): the complement of the indicator, shortened
    // by `gap_deg` at both ends. Gone entirely once the indicator plus its
    // two gaps cover the full circle.
    let track_sweep = 360.0 - sweep - 2.0 * l.gap_deg;
    if track_alpha > 0.0 && track_sweep > 0.0 {
        polylines.push(arc_polyline(
            l.cx,
            l.cy,
            l.r,
            sweep + l.gap_deg,
            track_sweep,
            l.stroke,
            white,
            track_alpha,
            0.0,
            0.0,
        ));
    }
    polylines.push(arc_polyline(
        l.cx, l.cy, l.r, 0.0, sweep, l.stroke, white, 0.95, saturation, hue,
    ));

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;
    use crate::primitives::ring_point;

    const SIZE: f64 = 64.0;

    fn with_progress(p: f64) -> ModeOpts {
        opts(&[("progress", p), ("rMin", 0.3)])
    }

    /// The indicator is always the last polyline (drawn on top).
    fn indicator(frame: &OrbFrame) -> &Polyline {
        frame.polylines.last().unwrap()
    }

    #[test]
    fn zero_progress_is_a_dot_at_twelve_over_a_nearly_full_track() {
        let frame = frame_arc(SIZE, 0.0, &with_progress(0.0));
        assert_eq!(frame.polylines.len(), 2, "track + indicator");
        let ind = indicator(&frame);
        assert_eq!(ind.points.len(), 2);
        assert_eq!(
            ind.points[0], ind.points[1],
            "zero sweep collapses to a dot"
        );
        assert!(
            (ind.points[0].x - 32.0).abs() < 1e-9 && ind.points[0].y < 32.0,
            "the dot sits at 12 o'clock"
        );
        let track = &frame.polylines[0];
        assert!(track.a < ind.a, "track is fainter than the indicator");
        assert!(
            track.points.len() > 60,
            "track covers (almost) the whole circle"
        );
    }

    #[test]
    fn a_quarter_sweeps_clockwise_to_three_oclock() {
        let frame = frame_arc(SIZE, 0.0, &with_progress(0.25));
        let ind = indicator(&frame);
        let l = layout(SIZE, &with_progress(0.25));
        let want = ring_point(l.cx, l.cy, l.r, 90.0);
        let end = ind.points.last().unwrap();
        assert!(
            (end.x - want.x).abs() < 1e-9 && (end.y - want.y).abs() < 1e-9,
            "end of a 25% arc is at 3 o'clock"
        );
        assert!(end.x > l.cx, "clockwise means the right-hand side first");
    }

    #[test]
    fn full_progress_closes_the_ring_and_drops_the_track() {
        let frame = frame_arc(SIZE, 0.0, &with_progress(1.0));
        assert_eq!(frame.polylines.len(), 1, "no room left for a track");
        let ind = indicator(&frame);
        let (first, last) = (&ind.points[0], ind.points.last().unwrap());
        assert!(
            (first.x - last.x).abs() < 1e-9 && (first.y - last.y).abs() < 1e-9,
            "a closed ring"
        );
    }

    #[test]
    fn track_keeps_a_gap_from_both_indicator_ends() {
        let o = with_progress(0.5);
        let frame = frame_arc(SIZE, 0.0, &o);
        let l = layout(SIZE, &o);
        let track = &frame.polylines[0];
        let track_start = ring_point(l.cx, l.cy, l.r, 180.0 + l.gap_deg);
        let track_end = ring_point(l.cx, l.cy, l.r, 360.0 - l.gap_deg);
        assert!(
            (track.points[0].x - track_start.x).abs() < 1e-9
                && (track.points[0].y - track_start.y).abs() < 1e-9
        );
        let last = track.points.last().unwrap();
        assert!((last.x - track_end.x).abs() < 1e-9 && (last.y - track_end.y).abs() < 1e-9);
        assert!(l.gap_deg > 0.0);
    }

    #[test]
    fn stroke_scales_with_size() {
        let big = indicator(&frame_arc(64.0, 0.0, &with_progress(0.5))).w;
        let small = indicator(&frame_arc(20.0, 0.0, &with_progress(0.5))).w;
        assert!((big / small - 3.2).abs() < 1e-9);
    }
}
