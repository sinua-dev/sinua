//! Beacon / Ping: the notification badge (`notifying`) -- a solid dot with
//! rings that expand outward and fade, the "new message" / "incoming
//! call" / radar-ping cue. Transcribed from Tailwind CSS's `animate-ping`
//! (`75%, 100% { transform: scale(2); opacity: 0 }` over 1s with
//! `cubic-bezier(0, 0, 0.2, 1)`, documented as "useful for things like
//! notification badges"): each ring grows from the dot's radius to
//! `ringReach` and fades to nothing by 75% of the period, then rests
//! until the period restarts. The dot itself is Material's content-less
//! badge -- a small solid circle (`Badge.kt`'s `BadgeTokens.Size`).
//!
//! `ringCount` (default 2) staggers that many rings by `period / rings` so the
//! ripple reads as continuous rather than one-and-done. `once: 1` plays
//! the rings for the first period after `t = 0` only, then leaves just the
//! badge -- the stateless way to express a one-shot event: the caller
//! restarts `t` at the moment it happens. Related but separate: the
//! interrupt flash post-process (`primitives::apply_interrupt`) layers a
//! one-shot over *another* family's frame; this is a standalone beacon.

use crate::primitives::{
    arc_polyline, cubic_bezier, finalize_frame, Dot, ModeOpts, OrbFrame, Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Tailwind's `ping` curve: fast start, slow finish.
fn ping_ease(u: f64) -> f64 {
    cubic_bezier(0.0, 0.0, 0.2, 1.0, u)
}

pub fn frame_ping(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let period = get(o, "period", 1.0).max(0.05);
    let rings = get(o, "ringCount", 2.0).clamp(1.0, 4.0) as usize;
    let once = get(o, "once", 0.0) >= 0.5;
    let dot_r = size * get(o, "dotSize", 0.12).clamp(0.01, 0.4);
    let ring_max = size * get(o, "ringReach", 0.41).clamp(0.05, 0.5);
    let stroke = size * get(o, "ringWidth", 0.03).clamp(0.005, 0.2);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let white = 0.15;

    let mut polylines: Vec<Polyline> = Vec::with_capacity(rings);
    for k in 0..rings {
        // Ring `k` starts `k / rings` of a period after the first.
        let local = t - period * (k as f64 / rings as f64);
        if local < 0.0 || (once && local >= period) {
            continue;
        }
        let u = (local / period).fract();
        // Tailwind: scale 1 -> 2 and opacity 1 -> 0 by 75% of the cycle,
        // then hold (invisible) until the next cycle.
        let e = ping_ease((u / 0.75).min(1.0));
        let r = dot_r + (ring_max - dot_r) * e;
        let alpha = 0.6 * (1.0 - e);
        polylines.push(arc_polyline(
            cx, cy, r, 0.0, 360.0, stroke, white, alpha, saturation, hue,
        ));
    }

    let dot = Dot {
        x: cx,
        y: cy,
        z: 0.0,
        r: dot_r,
        white,
        a: 0.95,
        saturation,
        hue,
    };
    // Invisible rings (alpha < 0.02, i.e. past 75% of their cycle) are
    // culled by `with_polylines` -- the rest of the cycle is a rest.
    finalize_frame(vec![dot], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    fn one_ring() -> ModeOpts {
        opts(&[("ringCount", 1.0), ("period", 1.0), ("rMin", 0.3)])
    }

    fn ring_radius(p: &Polyline) -> f64 {
        let pt = &p.points[0];
        ((pt.x - 32.0).powi(2) + (pt.y - 32.0).powi(2)).sqrt()
    }

    #[test]
    fn the_badge_dot_is_always_there_and_the_ring_grows_and_fades() {
        let start = frame_ping(64.0, 0.0, &one_ring());
        let mid = frame_ping(64.0, 0.375, &one_ring());
        let rest = frame_ping(64.0, 0.8, &one_ring());
        for f in [&start, &mid, &rest] {
            assert_eq!(f.dots.len(), 1, "the badge dot never disappears");
            assert!((f.dots[0].x - 32.0).abs() < 1e-9);
        }
        assert_eq!(start.polylines.len(), 1);
        assert!(
            (ring_radius(&start.polylines[0]) - start.dots[0].r).abs() < 1e-6,
            "a ring is born at the dot's edge"
        );
        assert!((start.polylines[0].a - 0.6).abs() < 1e-9);
        assert!(
            ring_radius(&mid.polylines[0]) > ring_radius(&start.polylines[0]),
            "it expands"
        );
        assert!(mid.polylines[0].a < start.polylines[0].a, "and fades");
        assert!(
            rest.polylines.is_empty(),
            "invisible for the last quarter of the period"
        );
    }

    #[test]
    fn two_rings_are_staggered_by_half_a_period() {
        let o = opts(&[("ringCount", 2.0), ("period", 1.0), ("rMin", 0.3)]);
        let early = frame_ping(64.0, 0.1, &o);
        assert_eq!(
            early.polylines.len(),
            1,
            "the second ring hasn't started yet"
        );
        // Just after the second ring is born: the first is still (barely)
        // visible -- Tailwind's ease-out has it nearly gone by mid-period.
        let later = frame_ping(64.0, 0.52, &o);
        assert_eq!(later.polylines.len(), 2);
        let (r0, r1) = (
            ring_radius(&later.polylines[0]),
            ring_radius(&later.polylines[1]),
        );
        assert!(
            r0 > r1,
            "the first ring is further out than the one half a period behind it"
        );
    }

    #[test]
    fn once_plays_a_single_period_then_rests_on_the_badge() {
        let mut o = one_ring();
        o.insert("once".to_string(), 1.0);
        assert_eq!(frame_ping(64.0, 0.3, &o).polylines.len(), 1);
        let after = frame_ping(64.0, 1.3, &o);
        assert!(
            after.polylines.is_empty(),
            "no second ripple in one-shot mode"
        );
        assert_eq!(after.dots.len(), 1);
        // and the repeating default does ripple again
        assert_eq!(frame_ping(64.0, 1.3, &one_ring()).polylines.len(), 1);
    }
}
