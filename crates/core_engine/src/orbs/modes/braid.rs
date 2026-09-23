//! Braid: three strands plait around the sphere -- the "weaving" state.
//! Each strand runs pole to pole on a helix, and a radial breathing term
//! makes them trade places, reading as the over/under of a plait.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/braid.ts`.

use crate::orbs::core::{fib_dir, finalize_frame, frac, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_braid(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.76;
    let pt = Proj::new(t * 0.4, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let mut dots: Vec<Dot> = Vec::new();
    let ghost_n = get(o, "ghostN", 150.0) as usize;
    for i in 0..ghost_n {
        let d = fib_dir(i as f64, ghost_n as f64);
        let (px, py, z) = pt.project(d.0 * r, d.1 * r, d.2 * r);
        let depth = (z / r + 1.0) / 2.0;
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: 0.8 * rs,
            white: 0.78,
            a: 0.1 + 0.22 * depth,
            ..Default::default()
        });
    }

    let strand_n = get(o, "strandN", 52.0) as usize;
    let turns = get(o, "turns", 3.0);
    let r_base = get(o, "rBase", 1.2);
    let r_depth = get(o, "rDepth", 1.8);
    for s in 0..3 {
        let phase = (s as f64 / 3.0) * 2.0 * PI;
        for i in 0..strand_n {
            // u walks pole to pole; the frac() drift slides the whole strand along
            let u = (frac(i as f64 / strand_n as f64 + t * 0.045) * 2.0 - 1.0) * 0.96;
            let surf = (1.0 - u * u).max(0.0).sqrt();
            let end_fade = ((1.0 - u.abs()) / 0.1).min(1.0);
            let a = u * PI * turns + phase;
            // radial breathing: strands trade places -- the over/under of a plait
            let weave = 1.0 + 0.075 * (u * PI * turns * 2.0 + phase * 2.0 + t * 0.8).sin();
            let rr = surf * r * weave;
            let (px, py, zr) = pt.project(a.cos() * rr, u * r * weave, a.sin() * rr);
            let depth = (zr / r + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z: zr,
                r: (r_base + r_depth * depth) * rs,
                white: 0.55 - 0.45 * depth,
                a: end_fade * (0.45 + 0.55 * depth),
                ..Default::default()
            });
        }
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
