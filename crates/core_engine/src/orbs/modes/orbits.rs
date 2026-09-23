//! Orbits: particles on tilted orbits -- the "working" state. No nucleus
//! (the tuned preset runs coreless): just ghost paths and the particles
//! doing the work.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/orbits.ts`.

use crate::orbs::core::{finalize_frame, hash_d, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_orbits(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.12, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let orbit_n = get(o, "orbitN", 12.0) as i64;
    let ghost_n = get(o, "ghostN", 40.0) as i64;
    let particles = get(o, "particles", 3.0) as i64;
    let ghost_r = get(o, "ghostR", 0.9);
    let ghost_a = get(o, "ghostA", 0.5);
    let part_r = get(o, "partR", 1.2);
    let part_r_depth = get(o, "partRDepth", 1.6);

    let mut dots: Vec<Dot> = Vec::with_capacity(((ghost_n + particles) * orbit_n.max(0)) as usize);

    // orbits: each a tilted circle -- a ghost path + running particles
    for orb in 0..orbit_n {
        let orb_f = orb as f64;
        let h1 = hash_d(orb_f, 1.7);
        let h2 = hash_d(orb_f, 5.2);
        let h3 = hash_d(orb_f, 8.9);
        let ro = r * (0.45 + 0.52 * h1);
        let th = h1 * 2.0 * PI;
        let phi = (2.0 * h2 - 1.0).acos();
        // orbit plane basis (u, v perpendicular to normal n)
        let nx = phi.sin() * th.cos();
        let ny = phi.cos();
        let nz = phi.sin() * th.sin();
        let mut ux = -ny;
        let mut uy = nx;
        let uz = 0.0_f64;
        let ul = (ux * ux + uy * uy).sqrt().max(1e-6);
        ux /= ul;
        uy /= ul;
        let vx = ny * uz - nz * uy;
        let vy = nz * ux - nx * uz;
        let vz = nx * uy - ny * ux;
        let speed = (0.25 + 0.55 * h3) * if h3 > 0.5 { 1.0 } else { -1.0 };

        // ghost path
        for k in 0..ghost_n {
            let a = (k as f64 / ghost_n as f64) * 2.0 * PI;
            let (px, py, z) = pt.project(
                (ux * a.cos() + vx * a.sin()) * ro,
                (uy * a.cos() + vy * a.sin()) * ro,
                (uz * a.cos() + vz * a.sin()) * ro,
            );
            let depth = (z / ro + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: ghost_r * rs,
                white: 0.72,
                a: ghost_a * (0.4 + 0.6 * depth),
                ..Default::default()
            });
        }
        // the particles doing the work
        for m in 0..particles {
            let a = t * speed + (m as f64 / particles as f64) * 2.0 * PI + h2 * 6.0;
            let (px, py, z) = pt.project(
                (ux * a.cos() + vx * a.sin()) * ro,
                (uy * a.cos() + vy * a.sin()) * ro,
                (uz * a.cos() + vz * a.sin()) * ro,
            );
            let depth = (z / ro + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: (part_r + part_r_depth * depth) * rs,
                white: 0.3 - 0.22 * depth,
                a: 1.0,
                ..Default::default()
            });
        }
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
