//! Web: a constellation wires itself -- the "connecting" state. Nodes drift
//! on the sphere under slow value noise; any pair closer than `thr` grows an
//! edge, and bright packets run along randomly re-picked node pairs.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/web.ts`.

use crate::orbs::core::{
    fib_dir, finalize_frame, frac, hash_d, lerp, radius_scale, vnoise, Dot, Line, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_web(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.8 * get(o, "spread", 1.0);
    // note the projector carries the radius as its scale, so node vectors
    // stay unit-length and distances below are in unit-sphere space
    let pt = Proj::new(t * 0.12, 0.32, cx, cy, r);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let node_n = get(o, "nodeN", 30.0) as usize;
    let thr = get(o, "thr", 0.72);
    let node_r = get(o, "nodeR", 1.4);
    let node_r_depth = get(o, "nodeRDepth", 1.8);

    // nodes: fib lattice + slow noise wander, renormalised to the surface
    let mut nodes: Vec<(f64, f64, f64)> = Vec::with_capacity(node_n);
    for i in 0..node_n {
        let i_f = i as f64;
        let d = fib_dir(i_f, node_n as f64);
        let x = d.0 + 0.3 * (vnoise(i_f * 0.31 + 9.0, t * 0.24) - 0.5) * 2.0;
        let y = d.1 + 0.3 * (vnoise(i_f * 0.53 + 27.0, t * 0.21) - 0.5) * 2.0;
        let z = d.2 + 0.3 * (vnoise(i_f * 0.77 + 55.0, t * 0.27) - 0.5) * 2.0;
        let l = (x * x + y * y + z * z).sqrt();
        nodes.push((x / l, y / l, z / l));
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut dots: Vec<Dot> = Vec::new();
    let line_w = get(o, "lineW", 0.8);

    // edges between close neighbours, alpha by proximity + depth
    for i in 0..node_n {
        for j in (i + 1)..node_n {
            let dx = nodes[i].0 - nodes[j].0;
            let dy = nodes[i].1 - nodes[j].1;
            let dz = nodes[i].2 - nodes[j].2;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist >= thr {
                continue;
            }
            let (x1, y1, z1) = pt.project(nodes[i].0, nodes[i].1, nodes[i].2);
            let (x2, y2, z2) = pt.project(nodes[j].0, nodes[j].1, nodes[j].2);
            let depth = ((z1 + z2) / 2.0 + 1.0) / 2.0;
            lines.push(Line {
                x1,
                y1,
                x2,
                y2,
                white: 0.42,
                a: (1.0 - dist / thr) * (0.3 + 0.55 * depth),
                w: (line_w * rs).max(0.6),
                saturation: 0.0,
                hue: 0.0,
            });
        }
    }

    for (i, n) in nodes.iter().enumerate() {
        let (px, py, z) = pt.project(n.0, n.1, n.2);
        let depth = (z + 1.0) / 2.0;
        let pulse = 1.0 + 0.25 * (t * 1.4 + i as f64 * 2.7).sin();
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: (node_r + node_r_depth * depth) * pulse * rs,
            white: 0.55 - 0.45 * depth,
            a: 1.0,
            ..Default::default()
        });
    }

    // signals: bright packets running between paired nodes
    let signals = get(o, "signals", 5.0) as usize;
    for s in 0..signals {
        let s_f = s as f64;
        let seg = (t * 0.55 + s_f * 7.31).floor();
        let a = (hash_d(seg, s_f * 3.1 + 1.7) * node_n as f64).floor() as usize;
        let b = (hash_d(seg, s_f * 5.7 + 4.2) * node_n as f64).floor() as usize;
        if a == b {
            continue;
        }
        let f = frac(t * 0.55 + s_f * 7.31);
        let x = lerp(nodes[a].0, nodes[b].0, f);
        let y = lerp(nodes[a].1, nodes[b].1, f);
        let z = lerp(nodes[a].2, nodes[b].2, f);
        let l = (x * x + y * y + z * z).sqrt().max(1e-6);
        let (px, py, zr) = pt.project(x / l, y / l, z / l);
        let depth = (zr + 1.0) / 2.0;
        dots.push(Dot {
            x: px,
            y: py,
            z: zr,
            r: (node_r * 1.5 + node_r_depth * depth) * rs,
            white: 0.05,
            a: 0.5 + 0.5 * depth,
            ..Default::default()
        });
    }

    finalize_frame(dots, lines, get(o, "rMin", 0.3))
}
