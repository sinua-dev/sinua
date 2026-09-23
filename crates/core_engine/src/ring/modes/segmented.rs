//! Ring / Segmented: a ring divided into `segmentCount` equal segments that
//! fill in order (`stepping`) -- onboarding steps, "3 of 5 done", battery
//! bars, or "N statuses, some seen".
//!
//! Transcribed from androidx Wear Material 3's
//! `SegmentedCircularProgressIndicator` (raw source, see
//! fetched): segments start at 12 o'clock
//! (`StartAngle = 270`), a continuous `progress` fills
//! `segmentCount · progress` segments with the last one partially filled,
//! and the gap between segments is `2·asin((stroke + gapSize) / (2r))` --
//! the `+ stroke` term eats the two round caps, so the *visible* gap is
//! exactly `gapSize`. The default gap is M3's
//! `calculateRecommendedGapSize(strokeWidth) = strokeWidth / 3`. M3's
//! per-segment `segmentValue(i) -> Boolean` variant is generalized to
//! `segment{i}: 0..1` (1/0 = WhatsApp-style seen/unseen, as in 3llomi's
//! `CircularStatusView` with its per-portion colors). Not transcribed:
//! M3's `allowProgressOverflow` wrap-around -- `tracking` (`nested`) already
//! covers "more than 100%" with laps, a second overflow scheme would be
//! redundant.
//!
//! Stroke and radius come from `arc`'s shared `layout`, so a segmented ring
//! and a plain one line up exactly. Unlike `arc`, an empty segment draws no
//! indicator at all -- `arc`'s 0% dot says "started", an empty step
//! shouldn't. Stateless: `t` is unused.

use super::arc::layout;
use crate::primitives::{arc_polyline, finalize_frame, indexed_opt, ModeOpts, OrbFrame, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub const MAX_SEGMENTS: usize = 24;

/// The angle a gap takes on a ring of centerline radius `r`, such that the
/// visible gap between two round-capped segment ends is `gap_px`
/// (Wear M3's `asin((stroke + gap) / (size - stroke))`, `size - stroke`
/// being the centerline diameter).
pub(crate) fn gap_sweep_deg(r: f64, stroke: f64, gap_px: f64) -> f64 {
    let s = ((stroke + gap_px) / (2.0 * r)).min(1.0);
    2.0 * s.asin().to_degrees()
}

pub fn frame_segmented(size: f64, _t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let n = get(o, "segmentCount", 5.0)
        .round()
        .clamp(1.0, MAX_SEGMENTS as f64) as usize;
    let progress = get(o, "progress", 0.0).clamp(0.0, 1.0);
    let gap = get(o, "gap", 1.0 / 3.0).max(0.0) * l.stroke;
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;

    let pitch = 360.0 / n as f64;
    // One segment is a whole ring: no boundary, so no gap.
    let gs = if n == 1 {
        0.0
    } else {
        gap_sweep_deg(l.r, l.stroke, gap)
    };
    let sweep = pitch - gs;

    let mut polylines: Vec<Polyline> = Vec::with_capacity(n * 2);
    for i in 0..n {
        // Too many segments for this stroke: collapse to a dot at the
        // segment's middle instead of a negative sweep, shrunk so that
        // neighbouring dots still keep the requested gap instead of
        // overlapping into a blob.
        let (start, seg, w) = if sweep > 0.0 {
            (i as f64 * pitch + gs * 0.5, sweep, l.stroke)
        } else {
            let chord = 2.0 * l.r * (pitch.to_radians() * 0.5).sin();
            (
                (i as f64 + 0.5) * pitch,
                0.0,
                (chord - gap).clamp(l.stroke * 0.1, l.stroke),
            )
        };
        let fill = indexed_opt(o, "segment", i)
            .unwrap_or(n as f64 * progress - i as f64)
            .clamp(0.0, 1.0);
        if track_alpha > 0.0 {
            polylines.push(arc_polyline(
                l.cx,
                l.cy,
                l.r,
                start,
                seg,
                w,
                white,
                track_alpha,
                0.0,
                0.0,
            ));
        }
        if fill > 0.0 {
            polylines.push(arc_polyline(
                l.cx,
                l.cy,
                l.r,
                start,
                seg * fill,
                w,
                white,
                0.95,
                saturation,
                hue,
            ));
        }
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

    fn indicators(f: &OrbFrame) -> Vec<&Polyline> {
        f.polylines.iter().filter(|p| p.a > 0.9).collect()
    }

    fn tracks(f: &OrbFrame) -> Vec<&Polyline> {
        f.polylines.iter().filter(|p| p.a < 0.5).collect()
    }

    #[test]
    fn empty_shows_only_tracks_and_full_lights_every_segment() {
        let empty = frame_segmented(SIZE, 0.0, &opts(&[("segmentCount", 5.0)]));
        assert_eq!(tracks(&empty).len(), 5);
        assert!(indicators(&empty).is_empty(), "an empty step isn't started");
        let full = frame_segmented(
            SIZE,
            0.0,
            &opts(&[("segmentCount", 5.0), ("progress", 1.0)]),
        );
        let ind = indicators(&full);
        assert_eq!(ind.len(), 5);
        for (p, t) in ind.iter().zip(tracks(&full)) {
            assert_eq!(p.points, t.points, "a full segment covers its whole track");
        }
    }

    #[test]
    fn progress_fills_whole_segments_then_part_of_the_next() {
        let half4 = frame_segmented(
            SIZE,
            0.0,
            &opts(&[("segmentCount", 4.0), ("progress", 0.5)]),
        );
        assert_eq!(indicators(&half4).len(), 2);
        // 5 segments at 50%: two full, the third half full.
        let o = opts(&[("segmentCount", 5.0), ("progress", 0.5)]);
        let f = frame_segmented(SIZE, 0.0, &o);
        let ind = indicators(&f);
        assert_eq!(ind.len(), 3);
        let l = layout(SIZE, &o);
        let gs = gap_sweep_deg(l.r, l.stroke, l.stroke / 3.0);
        let start = 2.0 * 72.0 + gs / 2.0;
        let want = start + 0.5 * (72.0 - gs);
        assert!((angle(ind[2].points.last().unwrap()) - want).abs() < 1e-6);
    }

    #[test]
    fn the_visible_gap_is_the_requested_one_and_straddles_twelve() {
        let o = opts(&[
            ("segmentCount", 4.0),
            ("progress", 1.0),
            ("trackOpacity", 0.0),
        ]);
        let f = frame_segmented(SIZE, 0.0, &o);
        let l = layout(SIZE, &o);
        let end0 = f.polylines[0].points.last().unwrap();
        let start1 = &f.polylines[1].points[0];
        let chord = ((end0.x - start1.x).powi(2) + (end0.y - start1.y).powi(2)).sqrt();
        // Centerline chord = stroke (two half caps) + the visible gap.
        assert!((chord - (l.stroke + l.stroke / 3.0)).abs() < 1e-9);
        // The last segment ends as far before 12 as the first starts after it.
        let first = angle(&f.polylines[0].points[0]);
        let last = angle(f.polylines[3].points.last().unwrap());
        assert!((first - (360.0 - last)).abs() < 1e-6);
    }

    #[test]
    fn per_segment_values_override_progress() {
        let f = frame_segmented(
            SIZE,
            0.0,
            &opts(&[
                ("segmentCount", 3.0),
                ("progress", 1.0),
                ("segment0", 1.0),
                ("segment1", 0.0),
                ("segment2", 1.0),
                ("trackOpacity", 0.0),
            ]),
        );
        let lit: Vec<f64> = f.polylines.iter().map(|p| angle(&p.points[0])).collect();
        assert_eq!(lit.len(), 2, "segment 1 is unseen");
        assert!(lit[0] < 120.0 && lit[1] > 240.0, "segments 0 and 2");
    }

    #[test]
    fn one_segment_is_a_full_ring_and_crowded_segments_become_dots() {
        let one = frame_segmented(
            SIZE,
            0.0,
            &opts(&[
                ("segmentCount", 1.0),
                ("progress", 1.0),
                ("trackOpacity", 0.0),
            ]),
        );
        assert_eq!(one.polylines.len(), 1);
        let p = &one.polylines[0];
        assert!(
            (angle(p.points.last().unwrap()) - 360.0).abs() < 1e-6
                || angle(p.points.last().unwrap()) < 1e-6
        );
        let crowded = frame_segmented(
            SIZE,
            0.0,
            &opts(&[
                ("segmentCount", 24.0),
                ("strokeWidth", 0.3),
                ("gap", 2.0),
                ("progress", 1.0),
            ]),
        );
        let dots: Vec<&Polyline> = crowded.polylines.iter().filter(|p| p.a > 0.9).collect();
        for p in &dots {
            assert!(p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
            assert_eq!(p.points[0], p.points[1], "collapsed to a dot");
        }
        // Neighbouring dots don't overlap: center distance >= diameter.
        let (a, b) = (&dots[0].points[0], &dots[1].points[0]);
        let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
        assert!(
            d >= dots[0].w - 1e-9,
            "dots keep apart ({d} vs {})",
            dots[0].w
        );
    }

    #[test]
    fn shares_arcs_stroke_and_radius() {
        let o = opts(&[
            ("segmentCount", 1.0),
            ("progress", 0.25),
            ("trackOpacity", 0.0),
        ]);
        let seg = frame_segmented(SIZE, 0.0, &o);
        let arc = super::super::arc::frame_arc(
            SIZE,
            0.0,
            &opts(&[("progress", 0.25), ("trackOpacity", 0.0)]),
        );
        assert_eq!(seg.polylines.last().unwrap(), arc.polylines.last().unwrap());
    }
}
