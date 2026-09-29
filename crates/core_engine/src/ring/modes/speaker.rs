//! Ring / Speaker: a voice ring around an avatar (`talking`) -- "who is
//! speaking" in a call grid, an agent list or a chat header. The host view
//! puts the image in the middle; the engine draws only the ring, which sits
//! just outside the avatar's edge and never crosses it.
//!
//! Prior art: Google Meet / Zoom / Discord's speaking indicator (a ring
//! around the tile or avatar that thickens with the voice) and Apple's
//! FaceTime "active speaker" outline. All of them grow the ring *outward*
//! with level and leave the face alone; this does the same.
//!
//! **Audio drives thickness, never the shape** (the signal family's rule,
//! see `signal/mod.rs`): the ring stays a circle, its stroke grows outward
//! by `reach` with `audioLevel`, and its opacity rises from `idleOpacity`.
//! The mode reads `audioLevel` itself -- the generic radial swell
//! (`apply_audio_reactive`) would scale the ring about the centre and pull
//! it over the avatar while listening, so the voice-state profile turns
//! `audioStrength` off for this pattern (`spec/voice-state-profile.json`).
//!
//! The states speak through two knobs the profile sets:
//! - `flow`: thin ripples that travel away from the ring (`+1`, the agent
//!   giving out -- speaking) or in towards it from outside (`-1`, taking in
//!   -- listening); `0` = none. Ripples live outside the ring, so the
//!   avatar stays clear either way.
//! - `shimmer`: a brighter arc that circles the ring (thinking, connecting).
//!
//! Stateless: a pure function of `t` and the opts.

use std::f64::consts::PI;

use crate::primitives::{arc_polyline, finalize_frame, ModeOpts, OrbFrame, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// The outermost the ring or a ripple may reach, as a fraction of `size`
/// (the 0.41 silhouette every ring uses, plus room for round caps / glow).
const OUTER: f64 = 0.47;
/// Seconds a ripple takes to cross its span.
const RIPPLE_PERIOD: f64 = 1.6;
/// Seconds for the shimmer arc to go once round.
const SHIMMER_PERIOD: f64 = 2.4;
/// The shimmer arc's length, in degrees.
const SHIMMER_SWEEP: f64 = 80.0;
const PEAK_ALPHA: f64 = 0.85;

/// The ring's geometry for these opts at this level.
pub(crate) struct Layout {
    pub cx: f64,
    pub cy: f64,
    /// The avatar's edge + gap: nothing is drawn inside this radius.
    pub r_in: f64,
    /// The ring's stroke width now (grows with the level).
    pub w: f64,
    /// Where ripples may go, outside the ring's resting outer edge.
    pub r_max: f64,
}

pub(crate) fn layout(size: f64, o: &ModeOpts) -> Layout {
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0);
    let r_in = size
        * (get(o, "innerRadius", 0.34).clamp(0.1, 0.44)
            + get(o, "avatarGap", 0.012).clamp(0.0, 0.05));
    let r_max = (size * OUTER).max(r_in + 1.0);
    let room = r_max - r_in;
    let thickness = (size * get(o, "thickness", 0.022).clamp(0.004, 0.1)).min(room);
    let reach = (size * get(o, "reach", 0.04).clamp(0.0, 0.12)).min(room - thickness);
    Layout {
        cx: size * 0.5,
        cy: size * 0.5,
        r_in,
        w: thickness + reach * level,
        r_max,
    }
}

pub fn frame_speaker(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let l = layout(size, o);
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0);
    let idle = get(o, "idleOpacity", 0.35).clamp(0.0, 1.0);
    let flow = get(o, "flow", 0.0).clamp(-1.0, 1.0);
    let ripples = get(o, "rippleCount", 2.0).round().clamp(0.0, 4.0) as usize;
    let shimmer = get(o, "shimmer", 0.0).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;

    let ring_r = l.r_in + l.w * 0.5;
    let ring_a = idle + (PEAK_ALPHA - idle) * (level * 1.5).min(1.0);
    let mut polylines: Vec<Polyline> = Vec::new();

    // Ripples first (underneath), outside the ring's outer edge. Each
    // travels its span once per period, faded in and out by sin(pi u) so
    // none pops; phases are spread evenly.
    if flow != 0.0 && ripples > 0 {
        let from = ring_r + l.w * 0.5;
        let span = (l.r_max - from).max(0.0);
        let rw = (l.w * 0.35).max(size * 0.004);
        let strength = flow.abs() * (0.25 + 0.75 * level);
        for k in 0..ripples {
            let u = (t / RIPPLE_PERIOD + k as f64 / ripples as f64).rem_euclid(1.0);
            // Outward: from the ring to the edge. Inward: from the edge to the ring.
            let d = if flow > 0.0 { u } else { 1.0 - u };
            let r = from + span * d;
            let a = 0.5 * strength * (PI * u).sin();
            if a > 0.005 && r - rw * 0.5 > from {
                polylines.push(arc_polyline(
                    l.cx, l.cy, r, 0.0, 360.0, rw, white, a, saturation, hue,
                ));
            }
        }
    }

    // The ring: dimmed under a shimmer so the travelling arc reads.
    let base_a = ring_a * (1.0 - 0.45 * shimmer);
    polylines.push(arc_polyline(
        l.cx, l.cy, ring_r, 0.0, 360.0, l.w, white, base_a, saturation, hue,
    ));
    if shimmer > 0.0 {
        let start = 360.0 * (t / SHIMMER_PERIOD).rem_euclid(1.0);
        polylines.push(arc_polyline(
            l.cx,
            l.cy,
            ring_r,
            start,
            SHIMMER_SWEEP,
            l.w,
            white,
            base_a + (PEAK_ALPHA - base_a) * shimmer,
            saturation,
            hue,
        ));
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

    /// The innermost any stroke reaches: its centreline radius minus half
    /// its width.
    fn inner_edge(f: &OrbFrame) -> f64 {
        f.polylines
            .iter()
            .map(|p| dist(&p.points[0]) - p.w * 0.5)
            .fold(f64::INFINITY, f64::min)
    }

    fn ring(f: &OrbFrame) -> &Polyline {
        // With no shimmer, the ring is drawn last (on top of the ripples).
        f.polylines.last().unwrap()
    }

    #[test]
    fn the_ring_never_crosses_the_avatar_edge() {
        for flow in [-1.0, 0.0, 1.0] {
            for level in [0.0, 0.5, 1.0] {
                for t in [0.0, 0.4, 0.9, 1.3] {
                    let o = opts(&[("flow", flow), ("audioLevel", level), ("shimmer", 1.0)]);
                    let l = layout(SIZE, &o);
                    let f = frame_speaker(SIZE, t, &o);
                    assert!(
                        inner_edge(&f) >= l.r_in - 1e-9,
                        "flow {flow} level {level} t {t}: {} < {}",
                        inner_edge(&f),
                        l.r_in
                    );
                }
            }
        }
    }

    #[test]
    fn everything_stays_inside_the_frame() {
        let o = opts(&[
            ("flow", 1.0),
            ("audioLevel", 1.0),
            ("reach", 0.12),
            ("thickness", 0.1),
        ]);
        for t in [0.0, 0.3, 0.8, 1.5] {
            for p in &frame_speaker(SIZE, t, &o).polylines {
                assert!(dist(&p.points[0]) + p.w * 0.5 <= SIZE * OUTER + 1e-9);
            }
        }
    }

    #[test]
    fn the_voice_thickens_the_ring_outward_and_brightens_it() {
        let quiet = frame_speaker(SIZE, 0.0, &opts(&[("audioLevel", 0.0)]));
        let loud = frame_speaker(SIZE, 0.0, &opts(&[("audioLevel", 1.0)]));
        let (q, l) = (ring(&quiet), ring(&loud));
        assert!(l.w > q.w, "thicker when loud");
        assert!(l.a > q.a, "brighter when loud");
        // Same inner edge: it grows outward only.
        assert!((dist(&q.points[0]) - q.w / 2.0 - (dist(&l.points[0]) - l.w / 2.0)).abs() < 1e-9);
        // Still a closed circle.
        let (a, b) = (&l.points[0], l.points.last().unwrap());
        assert!((a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    }

    #[test]
    fn flow_sends_ripples_out_or_in() {
        let radius_at = |flow: f64, t: f64| {
            let f = frame_speaker(
                SIZE,
                t,
                &opts(&[("flow", flow), ("rippleCount", 1.0), ("audioLevel", 0.5)]),
            );
            assert_eq!(f.polylines.len(), 2, "one ripple + the ring");
            dist(&f.polylines[0].points[0])
        };
        assert!(
            radius_at(1.0, 0.6) > radius_at(1.0, 0.3),
            "speaking ripples move out"
        );
        assert!(
            radius_at(-1.0, 0.6) < radius_at(-1.0, 0.3),
            "listening ripples move in"
        );
        let none = frame_speaker(SIZE, 0.5, &opts(&[("flow", 0.0)]));
        assert_eq!(none.polylines.len(), 1, "no flow, just the ring");
    }

    #[test]
    fn shimmer_circles_the_ring() {
        let o = opts(&[("shimmer", 1.0)]);
        let a = frame_speaker(SIZE, 0.0, &o);
        let b = frame_speaker(SIZE, SHIMMER_PERIOD / 4.0, &o);
        let arc = |f: &OrbFrame| f.polylines.last().unwrap().points[0].clone();
        assert_eq!(a.polylines.len(), 2, "ring + shimmer arc");
        let (pa, pb) = (arc(&a), arc(&b));
        assert!(
            pa.y < 32.0 && (pa.x - 32.0).abs() < 1e-6,
            "starts at 12 o'clock"
        );
        assert!(
            pb.x > 32.0 && (pb.y - 32.0).abs() < 1e-6,
            "a quarter later at 3 o'clock"
        );
        assert!(
            a.polylines[1].a > a.polylines[0].a,
            "the arc is brighter than the ring"
        );
    }

    #[test]
    fn periodic_and_scales_with_size() {
        let o = opts(&[("flow", 1.0), ("shimmer", 1.0), ("audioLevel", 0.4)]);
        // 4.8 s = 3 ripple periods = 2 shimmer turns.
        let (a, b) = (
            frame_speaker(SIZE, 0.7, &o),
            frame_speaker(SIZE, 0.7 + 4.8, &o),
        );
        assert_eq!(a.polylines.len(), b.polylines.len());
        for (pa, pb) in a.polylines.iter().zip(&b.polylines) {
            assert!((pa.a - pb.a).abs() < 1e-9 && (pa.w - pb.w).abs() < 1e-9);
            for (x, y) in pa.points.iter().zip(&pb.points) {
                assert!((x.x - y.x).abs() < 1e-6 && (x.y - y.y).abs() < 1e-6);
            }
        }
        let big = ring(&frame_speaker(64.0, 0.0, &opts(&[]))).w;
        let small = ring(&frame_speaker(32.0, 0.0, &opts(&[]))).w;
        assert!((big / small - 2.0).abs() < 1e-9);
    }
}
