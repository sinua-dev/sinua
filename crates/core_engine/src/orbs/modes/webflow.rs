//! Webflow: a deliberate, near-identical twin of `web.rs` (`connecting`),
//! for the `drifting` state -- exists ONLY to let the two noise functions
//! be compared side by side with everything else held constant (same node
//! count, same edge threshold, same signals, same paint values). It is a
//! copy, not a shared helper, on purpose: `web.rs` is a `thinking-orbs` port
//! and must stay a faithful transcription (see its own header comment and
//! `docs/engine.md`'s provenance section) -- it cannot be refactored to
//! parameterize the noise function without risking that. Like `aurora.rs`,
//! this mode is NOT a port and has no golden vector.
//!
//! The one change from `frame_web`: node wander uses `core::perlin3` (real
//! gradient noise) instead of three independent `core::vnoise` calls. Every
//! other line is identical to `web.rs` on purpose -- see
//! `docs/effects-research.md` and `docs/engine.md#additive-capabilities-beyond-the-port`
//! for why.

use crate::orbs::core::{
    fib_dir, finalize_frame, frac, hash_d, lerp, perlin3, radius_scale, Dot, Line, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_webflow(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.8 * get(o, "spread", 1.0);
    let pt = Proj::new(t * 0.12, 0.32, cx, cy, r);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let node_n = get(o, "nodeN", 30.0) as usize;
    let thr = get(o, "thr", 0.72);
    let node_r = get(o, "nodeR", 1.4);
    let node_r_depth = get(o, "nodeRDepth", 1.8);

    // nodes: fib lattice + slow noise wander, renormalised to the surface --
    // identical to `web.rs` except this line, which is the entire point of
    // this file: real gradient noise instead of three independent vnoise
    // channels. The fixed per-axis offsets (0.3/0.7/1.1 on the z input)
    // decorrelate the three axes the same way `web.rs`'s +9.0/+27.0/+55.0
    // seeds do for vnoise. perlin3's output is already ~[-1, 1], so no
    // `(- 0.5) * 2.0` remap is needed the way vnoise's [0, 1) range needs.
    let mut nodes: Vec<(f64, f64, f64)> = Vec::with_capacity(node_n);
    for i in 0..node_n {
        let i_f = i as f64;
        let d = fib_dir(i_f, node_n as f64);
        let x = d.0 + 0.3 * perlin3(i_f * 0.31 + 9.0, t * 0.24, 0.3);
        let y = d.1 + 0.3 * perlin3(i_f * 0.53 + 27.0, t * 0.21, 0.7);
        let z = d.2 + 0.3 * perlin3(i_f * 0.77 + 55.0, t * 0.27, 1.1);
        let l = (x * x + y * y + z * z).sqrt();
        nodes.push((x / l, y / l, z / l));
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut dots: Vec<Dot> = Vec::new();
    let line_w = get(o, "lineW", 0.8);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::modes::web::frame_web;
    use crate::orbs::profiles::opts;

    /// Proves the noise swap actually changes the result -- if this ever
    /// passes with identical output, the swap silently stopped doing
    /// anything (e.g. a copy-paste that didn't actually change the call).
    #[test]
    fn differs_from_connecting_at_the_same_params() {
        let o = opts(&[
            ("nodeN", 30.0),
            ("thr", 0.72),
            ("signals", 5.0),
            ("nodeR", 1.4),
            ("nodeRDepth", 1.8),
            ("lineW", 0.8),
            ("rMin", 0.3),
        ]);
        let connecting = frame_web(64.0, 3.0, &o);
        let drifting = frame_webflow(64.0, 3.0, &o);
        assert_eq!(connecting.dots.len(), drifting.dots.len());
        let any_different = connecting
            .dots
            .iter()
            .zip(drifting.dots.iter())
            .any(|(a, b)| (a.x - b.x).abs() > 1e-9 || (a.y - b.y).abs() > 1e-9);
        assert!(
            any_different,
            "expected the noise swap to change node positions"
        );
    }
}
