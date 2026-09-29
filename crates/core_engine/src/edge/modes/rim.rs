//! Edge / Rim: a glow round the inside edge of the box (`framing`).
//!
//! Prior art: Siri on iOS 18 (a multicoloured glow that runs round the
//! screen's edge and swells with the voice) and Gemini Live's edge light.
//! Both hug the device's rounded screen corners, keep the outer edge pinned
//! to the screen and grow *inward* with the voice, and let colour flow
//! round the frame.
//!
//! **Geometry.** One closed rounded rectangle, sampled evenly by arc length
//! from the top centre clockwise, with its *outer* edge on the box: the
//! stroke's centreline sits `w / 2` inside, so a thicker stroke (the voice,
//! `reach`) grows inward only and never leaves the box. The corner radius
//! is `cornerRadius` times the box's shorter side (0.12 by default -- about
//! an iPhone's screen corner at full screen). Four fainter strokes inside it
//! stand in for the inward fade (there's no inner-shadow primitive); the
//! glow material, which the voice-state profile turns on, softens all
//! three.
//!
//! **Colour.** Unlike the other families this one is colourful by default
//! (`saturation` 0.7): a grey rim reads as a border, not a glow. Hue varies
//! round the frame by `hueSpread` degrees and flows with `flowSpeed` turns
//! per second, as per-vertex `hues` (the paint contract's per-vertex
//! stroke). `saturation` 0 gives the grey ink like everything else.
//!
//! **States** (set by the profile, `spec/voice-state-profile.json`): the
//! rim is invisible at rest (`idleOpacity` 0), faint while listening and
//! swells with the voice; `shimmer` sends a bright segment round the frame
//! while thinking. The generic radial swell (`audioStrength`) is off for
//! this pattern: scaling the frame about its centre would pull it off the
//! screen's edge. The mode reads `audioLevel` itself.
//!
//! Box layout (`edge/mod.rs`): width `size * aspect`, height `size`.
//! Stateless: a pure function of `t` and the opts.

use std::f64::consts::PI;

use crate::primitives::{finalize_frame, ModeOpts, OrbFrame, Point, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

const PEAK_ALPHA: f64 = 0.95;
/// Seconds for the shimmer segment to go once round.
const SHIMMER_PERIOD: f64 = 2.8;
/// The shimmer segment's length, as a fraction of the perimeter.
const SHIMMER_LEN: f64 = 0.18;
/// The inward fade: (offset in stroke widths, alpha relative to the rim).
/// Four narrow steps, so the fade reads as a gradient rather than bands.
const FADE: [(f64, f64); 4] = [(0.7, 0.42), (1.4, 0.24), (2.1, 0.12), (2.8, 0.05)];

/// The box and the rim's resting geometry for these opts.
pub(crate) struct Layout {
    pub w: f64,
    pub h: f64,
    /// The shorter side, which every other dimension is a fraction of.
    pub m: f64,
    /// Outer corner radius.
    pub corner: f64,
}

pub(crate) fn layout(size: f64, o: &ModeOpts) -> Layout {
    let aspect = get(o, "aspect", 1.0).clamp(0.125, 8.0);
    let (w, h) = (size * aspect, size);
    let m = w.min(h);
    Layout {
        w,
        h,
        m,
        corner: m * get(o, "cornerRadius", 0.12).clamp(0.0, 0.5),
    }
}

/// A closed rounded rectangle inset by `inset` from the box, radius
/// `radius` (already reduced by the inset), from the top centre clockwise.
/// Straight runs are sampled every ~`step` (enough for the hue to flow
/// smoothly along them); each corner gets `CORNER_STEPS` points whatever
/// its size, so corners stay round while long edges stay cheap -- every
/// vertex of a per-vertex-hue stroke is a draw call (`cost.rs`). Returns
/// the points (the first repeated at the end) and each point's position
/// round the loop, `0..1` by arc length.
pub(crate) fn rounded_rect(
    l: &Layout,
    inset: f64,
    radius: f64,
    step: f64,
) -> (Vec<Point>, Vec<f64>) {
    const CORNER_STEPS: usize = 6;
    let (x0, y0, x1, y1) = (inset, inset, l.w - inset, l.h - inset);
    let r = radius.clamp(0.0, ((x1 - x0).min(y1 - y0) * 0.5).max(0.0));
    let (sw, sh) = ((x1 - x0 - 2.0 * r).max(0.0), (y1 - y0 - 2.0 * r).max(0.0));
    let arc = PI * 0.5 * r;
    let total = (2.0 * (sw + sh) + 4.0 * arc).max(1e-9);
    let cx = (x0 + x1) * 0.5;
    let step = step.max(1e-6);

    let mut points: Vec<Point> = vec![Point { x: cx, y: y0 }];
    let mut along: Vec<f64> = vec![0.0];
    let mut dist = 0.0;
    let straight =
        |points: &mut Vec<Point>, along: &mut Vec<f64>, dist: &mut f64, to: Point, len: f64| {
            let from = points.last().unwrap().clone();
            let n = ((len / step).ceil() as usize).max(1);
            for i in 1..=n {
                let u = i as f64 / n as f64;
                points.push(Point {
                    x: from.x + (to.x - from.x) * u,
                    y: from.y + (to.y - from.y) * u,
                });
                along.push((*dist + len * u) / total);
            }
            *dist += len;
        };
    // (corner centre, start angle) for each quarter, clockwise from top-right.
    let corners = [
        (x1 - r, y0 + r, -PI * 0.5, Point { x: x1, y: y1 - r }, sh),
        (x1 - r, y1 - r, 0.0, Point { x: x0 + r, y: y1 }, sw),
        (x0 + r, y1 - r, PI * 0.5, Point { x: x0, y: y0 + r }, sh),
        (x0 + r, y0 + r, PI, Point { x: cx, y: y0 }, sw * 0.5),
    ];
    straight(
        &mut points,
        &mut along,
        &mut dist,
        Point { x: x1 - r, y: y0 },
        sw * 0.5,
    );
    for (ccx, ccy, a0, next, next_len) in corners {
        if r > 0.0 {
            for i in 1..=CORNER_STEPS {
                let u = i as f64 / CORNER_STEPS as f64;
                points.push(corner(ccx, ccy, r, a0 + PI * 0.5 * u));
                along.push((dist + arc * u) / total);
            }
            dist += arc;
        }
        straight(&mut points, &mut along, &mut dist, next, next_len);
    }
    // Close exactly on the first point.
    if let (Some(first), Some(last)) = (points.first().cloned(), points.last_mut()) {
        *last = first;
    }
    if let Some(u) = along.last_mut() {
        *u = 1.0;
    }
    (points, along)
}

fn corner(cx: f64, cy: f64, r: f64, angle: f64) -> Point {
    Point {
        x: cx + r * angle.cos(),
        y: cy + r * angle.sin(),
    }
}

pub fn frame_rim(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0);
    let idle = get(o, "idleOpacity", 0.35).clamp(0.0, 1.0);
    let thickness = l.m * get(o, "thickness", 0.015).clamp(0.002, 0.2);
    let reach = l.m * get(o, "reach", 0.035).clamp(0.0, 0.2);
    let shimmer = get(o, "shimmer", 0.0).clamp(0.0, 1.0);
    let hue = get(o, "hue", 265.0);
    let spread = get(o, "hueSpread", 80.0).clamp(0.0, 360.0);
    let flow = get(o, "flowSpeed", 0.12);
    let saturation = get(o, "saturation", 0.7).clamp(0.0, 1.0);
    let white = 0.15;

    let w = thickness + reach * level;
    let rim_a = idle + (PEAK_ALPHA - idle) * (level * 1.5).min(1.0);
    let base_a = rim_a * (1.0 - 0.4 * shimmer);
    let step = (l.m * 0.06).max(0.5);
    let hue_at = |u: f64| hue + spread * 0.5 * (1.0 + (2.0 * PI * (u + t * flow)).sin());

    let mut polylines: Vec<Polyline> = Vec::new();
    let mut stroke = |inset: f64, width: f64, a: f64, from: f64, len: f64| {
        if a <= 0.005 || width <= 0.0 {
            return;
        }
        // Inner strokes follow the rim's corner (a true offset is `corner -
        // inset`), but never go square: past the corner they keep 30% of it.
        let radius = (l.corner - inset).max(l.corner * 0.3);
        let (pts, along) = rounded_rect(&l, inset, radius, step);
        let (points, hues): (Vec<Point>, Vec<f64>) = if len >= 1.0 {
            (pts, along.iter().map(|u| hue_at(*u)).collect())
        } else {
            // An open segment: leave out the loop's closing duplicate and
            // order by distance from `from`, so a segment that crosses the
            // top centre runs on instead of jumping back to its start.
            let n = pts.len() - 1;
            let mut seg: Vec<(f64, Point, f64)> = pts
                .into_iter()
                .zip(along)
                .take(n)
                .map(|(p, u)| ((u - from).rem_euclid(1.0), p, u))
                .filter(|(d, _, _)| *d <= len)
                .collect();
            seg.sort_by(|a, b| a.0.total_cmp(&b.0));
            seg.into_iter().map(|(_, p, u)| (p, hue_at(u))).unzip()
        };
        if points.len() < 2 {
            return;
        }
        let mean = hues.iter().sum::<f64>() / hues.len() as f64;
        polylines.push(Polyline {
            points,
            white,
            a,
            w: width,
            saturation,
            hue: mean,
            hues: if saturation > 0.0 { hues } else { Vec::new() },
        });
    };

    // Inner fade first (underneath), then the rim, then the shimmer.
    for (k, rel) in FADE.iter().rev() {
        stroke(w * (0.5 + k), w, base_a * rel, 0.0, 1.0);
    }
    stroke(w * 0.5, w, base_a, 0.0, 1.0);
    if shimmer > 0.0 {
        let from = (t / SHIMMER_PERIOD).rem_euclid(1.0);
        stroke(
            w * 0.5,
            w,
            base_a + (PEAK_ALPHA - base_a) * shimmer,
            from,
            SHIMMER_LEN,
        );
    }

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    const SIZE: f64 = 64.0;

    fn o(pairs: &[(&str, f64)]) -> ModeOpts {
        let mut x = opts(pairs);
        x.entry("idleOpacity".into()).or_insert(0.5);
        x
    }

    /// The rim itself: the widest stroke that runs the whole loop, drawn last
    /// before any shimmer.
    fn rim(f: &OrbFrame) -> &Polyline {
        f.polylines
            .iter()
            .rfind(|p| p.points.first() == p.points.last())
            .unwrap()
    }

    #[test]
    fn the_outer_edge_sits_on_the_box_at_every_aspect() {
        for aspect in [0.46, 1.0, 2.0, 5.0] {
            for level in [0.0, 1.0] {
                let f = frame_rim(
                    SIZE,
                    0.3,
                    &o(&[("aspect", aspect), ("audioLevel", level), ("shimmer", 1.0)]),
                );
                let (bw, bh) = (SIZE * aspect, SIZE);
                for p in &f.polylines {
                    for q in &p.points {
                        let (lo_x, hi_x) = (q.x - p.w / 2.0, q.x + p.w / 2.0);
                        let (lo_y, hi_y) = (q.y - p.w / 2.0, q.y + p.w / 2.0);
                        assert!(
                            lo_x >= -1e-6
                                && hi_x <= bw + 1e-6
                                && lo_y >= -1e-6
                                && hi_y <= bh + 1e-6
                        );
                    }
                }
                // The rim touches the box: its top-centre point's outer edge is at y = 0.
                let r = rim(&f);
                assert!((r.points[0].y - r.w / 2.0).abs() < 1e-6, "aspect {aspect}");
                assert!((r.points[0].x - bw / 2.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn the_voice_grows_the_rim_inward_and_brightens_it() {
        let quiet = frame_rim(SIZE, 0.0, &o(&[("audioLevel", 0.0)]));
        let loud = frame_rim(SIZE, 0.0, &o(&[("audioLevel", 1.0)]));
        let (q, l) = (rim(&quiet), rim(&loud));
        assert!(l.w > q.w && l.a > q.a);
        // Outer edge unchanged (top centre): centre y - w/2 == 0 for both.
        assert!(
            (q.points[0].y - q.w / 2.0).abs() < 1e-9 && (l.points[0].y - l.w / 2.0).abs() < 1e-9
        );
    }

    #[test]
    fn invisible_at_rest_when_idle_opacity_is_zero() {
        let f = frame_rim(
            SIZE,
            0.0,
            &opts(&[("idleOpacity", 0.0), ("audioLevel", 0.0)]),
        );
        assert!(f.polylines.is_empty());
        let f = frame_rim(
            SIZE,
            0.0,
            &opts(&[("idleOpacity", 0.0), ("audioLevel", 0.4)]),
        );
        assert!(!f.polylines.is_empty(), "the voice brings it up");
    }

    #[test]
    fn colour_varies_round_the_frame_and_flows() {
        let a = frame_rim(SIZE, 0.0, &o(&[]));
        let hues = &rim(&a).hues;
        assert_eq!(hues.len(), rim(&a).points.len());
        let (lo, hi) = hues
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        assert!(hi - lo > 60.0, "hueSpread 80 is visible: {lo}..{hi}");
        let b = frame_rim(SIZE, 1.0, &o(&[]));
        assert!((rim(&b).hues[0] - hues[0]).abs() > 1.0, "it flows with t");
        let grey = frame_rim(SIZE, 0.0, &o(&[("saturation", 0.0)]));
        assert!(rim(&grey).hues.is_empty(), "grey ink: no per-vertex hues");
    }

    #[test]
    fn shimmer_runs_round_the_frame() {
        let seg = |t: f64| {
            let f = frame_rim(SIZE, t, &o(&[("shimmer", 1.0)]));
            let s = f.polylines.last().unwrap().clone();
            assert!(
                s.points.first() != s.points.last(),
                "the last stroke is the open shimmer segment"
            );
            s.points[s.points.len() / 2].clone()
        };
        // A segment that crosses the top centre (starts at 95% of the loop)
        // is one continuous run: no jump between neighbouring points.
        let f = frame_rim(SIZE, SHIMMER_PERIOD * 0.95, &o(&[("shimmer", 1.0)]));
        let s = f.polylines.last().unwrap();
        let max_gap = s
            .points
            .windows(2)
            .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
            .fold(0.0, f64::max);
        // Neighbours sit one sampling step apart (0.06 of the side, 3.84 at 64);
        // a jump back to the segment's start would be half the loop.
        assert!(
            max_gap < 2.0 * 0.06 * SIZE,
            "the wrapping segment jumps by {max_gap}"
        );
        let (a, b) = (seg(0.0), seg(SHIMMER_PERIOD / 4.0));
        assert!(a.y < 10.0, "starts along the top: {a:?}");
        assert!(b.x > 50.0, "a quarter later down the right side: {b:?}");
    }

    #[test]
    fn corners_follow_corner_radius() {
        let l = layout(SIZE, &opts(&[("cornerRadius", 0.25)]));
        let (pts, _) = rounded_rect(&l, 0.0, l.corner, 0.5);
        // The box corner itself is cut off by a 16-unit radius.
        let nearest = pts
            .iter()
            .map(|p| (p.x.powi(2) + p.y.powi(2)).sqrt())
            .fold(f64::MAX, f64::min);
        let expect = (2.0f64.sqrt() - 1.0) * 16.0;
        assert!((nearest - expect).abs() < 0.3, "{nearest} vs {expect}");
        let (sq, _) = rounded_rect(&l, 0.0, 0.0, 0.5);
        let nearest_sq = sq
            .iter()
            .map(|p| (p.x.powi(2) + p.y.powi(2)).sqrt())
            .fold(f64::MAX, f64::min);
        assert!(nearest_sq < 0.5, "radius 0 reaches the corner");
    }
}
