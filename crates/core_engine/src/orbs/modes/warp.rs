//! Warp: a depth-motion starfield tunnel for the `initializing` state. NOT a
//! port -- see `docs/effects-research.md`'s "Warp starfield" proposal.
//! Every ported mode keeps dots on (or just off) a fixed-radius shell,
//! moving tangentially; nothing moves a dot's *depth* toward the viewer
//! over its lifetime. This is the one mode that does -- dots spawn near the
//! center and streak outward to the silhouette edge, then recycle. Reads
//! as "spinning up" (a session's cold start), distinct in kind, not just
//! in geometry. No golden vector -- same tradeoff as every other mode in
//! this file's siblings (`aurora.rs`, `webflow.rs`, etc.).
//!
//! Stateless-`t` note: `frame(state, size, t)` has no invocation memory, so
//! each star's position-in-its-lifecycle is derived purely from `t` and a
//! per-star hash offset (`hash_d(i, ...)`) rather than tracked across
//! calls -- the same technique `sonar.rs` uses for its repeating pulse.

use crate::orbs::core::{
    fib_dir, finalize_frame, frac, hash_d, lerp, radius_scale, Dot, Line, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_warp(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.05, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let star_n = get(o, "starCount", 120.0) as i64;
    let period = get(o, "period", 2.2).max(0.05);
    let warp_speed = get(o, "warpSpeed", 1.0);
    let trail = get(o, "decay", 0.06).clamp(0.0, 0.5);

    let mut dots: Vec<Dot> = Vec::with_capacity(star_n.max(0) as usize);
    let mut lines: Vec<Line> = Vec::new();

    for i in 0..star_n {
        let i_f = i as f64;
        // A fixed radial direction per star (doesn't change across its
        // lifecycle -- it travels straight out along its own ray) and a
        // stable per-star phase offset so stars don't all warp in sync.
        let (dx, dy, dz) = fib_dir(i_f, star_n as f64);
        let phase = hash_d(i_f * 3.7 + 11.0, 0.0);
        let life = frac((t * warp_speed) / period + phase);

        let radius_here = r * life;
        let (px, py, z) = pt.project(dx * radius_here, dy * radius_here, dz * radius_here);
        let dot_r = lerp(0.15, 1.6, life) * rs;
        let alpha = life;

        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: dot_r,
            white: 0.15,
            a: alpha,
            ..Default::default()
        });

        // A short trailing streak behind the head, fading toward the
        // center -- what makes this read as "warp speed" instead of just
        // dots crawling outward.
        let life_prev = (life - trail).max(0.0);
        if life_prev < life {
            let radius_prev = r * life_prev;
            let (px0, py0, _z0) = pt.project(dx * radius_prev, dy * radius_prev, dz * radius_prev);
            lines.push(Line {
                x1: px0,
                y1: py0,
                x2: px,
                y2: py,
                white: 0.2,
                a: alpha * 0.55,
                w: (0.4 + 0.6 * life) * rs,
                saturation: 0.0,
                hue: 0.0,
            });
        }
    }

    finalize_frame(dots, lines, get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    #[test]
    fn stars_recycle_and_stay_within_the_silhouette() {
        let o = opts(&[("starCount", 40.0), ("period", 1.0), ("rMin", 0.3)]);
        for i in 0..20 {
            let frame = frame_warp(64.0, i as f64 * 0.13, &o);
            assert!(!frame.dots.is_empty());
            for d in &frame.dots {
                let dist = (d.x - 32.0).hypot(d.y - 32.0);
                assert!(
                    dist <= 32.0 * 0.82 + 1e-6,
                    "dot escaped the silhouette radius: {dist}"
                );
            }
        }
    }
}
