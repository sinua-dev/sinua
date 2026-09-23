//! Eclipse: a day/night terminator sweep driven by an actual **percentage**,
//! for the `progressing` state. NOT a port -- see
//! `docs/effects-research.md`'s "Eclipse terminator" proposal, the one
//! entry there flagged as functionally new, not just visually new: every
//! other mode (ported or additive) is indeterminate ("something is
//! happening"); this is the first determinate ("N% done") state, for a
//! model download, a long tool call with a known duration, or an upload.
//! Dots on the dark side are dimmed, not removed, per the proposal. No
//! golden vector -- same tradeoff as this file's siblings.
//!
//! `progress` (`0..1`) is threaded through `opts` instead of being derived
//! from `t` -- a caller drives it via `frame_with_overrides`'s overrides
//! map (the same mechanism the Studio's sliders already use), exactly the
//! way `docs/engine.md` describes that entry point as "the correct one for
//! any future tune-this-at-runtime use case." `t` still drives a slow idle
//! spin so the sphere doesn't look frozen while progress itself sits still
//! between updates.

use crate::orbs::core::{
    fib_dir, finalize_frame, radius_scale, Dot, LatticeSample, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// This mode's own camera (yaw multiplier, tilt) -- see `aurora::CAMERA`'s
/// doc comment for why this is pulled into a constant.
pub(crate) const CAMERA: (f64, f64) = (0.05, 0.3);

/// Per-point data for index `i` of `node_n`, exactly as `frame_eclipse`
/// computes it -- see `LatticeSample`'s doc comment.
pub(crate) fn lattice_sample(
    i: i64,
    node_n: i64,
    t: f64,
    o: &ModeOpts,
    size: f64,
) -> LatticeSample {
    let r = (size / 2.0) * 0.82;
    let rs = radius_scale(size, get(o, "rsPow", 0.6));
    let pt = Proj::new(t * CAMERA.0, CAMERA.1, size / 2.0, size / 2.0, 1.0);

    let node_r = get(o, "nodeSize", 0.9);
    let progress = get(o, "progress", 0.5).clamp(0.0, 1.0);
    let boundary = 1.0 - 2.0 * progress;

    let (dx, dy, dz) = fib_dir(i as f64, node_n as f64);
    let sweep = dx;
    let lit = sweep > boundary;

    let (_px, _py, z) = pt.project(dx * r, dy * r, dz * r);
    let depth = (z / r + 1.0) / 2.0;

    let edge_dist = (sweep - boundary).abs();
    let edge_boost = (1.0 - (edge_dist * 6.0).min(1.0)).max(0.0);

    let (white, alpha) = if lit {
        (0.55 - 0.35 * depth - 0.15 * edge_boost, 0.9)
    } else {
        (0.08, 0.28 + 0.25 * edge_boost)
    };

    LatticeSample {
        dir: (dx, dy, dz),
        radius_frac: 1.0,
        dot_r: node_r * rs * (1.0 + 0.5 * edge_boost),
        white,
        alpha,
        saturation: 0.0,
        hue: 0.0,
    }
}

pub fn frame_eclipse(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * CAMERA.0, CAMERA.1, cx, cy, 1.0);
    let node_n = get(o, "nodeCount", 260.0) as i64;

    // `sweep` (a dot's position along the terminator axis, -1..1) compared
    // against `boundary`: at progress=0 the boundary sits past +1 (nothing
    // can be lit), at progress=1 it sits past -1 (everything is lit) -- see
    // `lattice_sample`, which computes this per point.
    let mut dots: Vec<Dot> = Vec::with_capacity(node_n.max(0) as usize);
    for i in 0..node_n {
        let s = lattice_sample(i, node_n, t, o, size);
        let (px, py, z) = pt.project(
            s.dir.0 * r * s.radius_frac,
            s.dir.1 * r * s.radius_frac,
            s.dir.2 * r * s.radius_frac,
        );
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: s.dot_r,
            white: s.white,
            a: s.alpha,
            ..Default::default()
        });
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    fn lit_fraction(frame: &crate::OrbFrame) -> f64 {
        let lit = frame.dots.iter().filter(|d| d.a > 0.5).count();
        lit as f64 / frame.dots.len() as f64
    }

    #[test]
    fn progress_controls_how_much_of_the_sphere_is_lit() {
        let base = &[("nodeCount", 400.0), ("rMin", 0.3)];

        let mut dark = opts(base);
        dark.insert("progress".to_string(), 0.0);
        let dark_frame = frame_eclipse(64.0, 0.0, &dark);
        assert!(
            lit_fraction(&dark_frame) < 0.05,
            "expected almost nothing lit at progress=0"
        );

        let mut full = opts(base);
        full.insert("progress".to_string(), 1.0);
        let full_frame = frame_eclipse(64.0, 0.0, &full);
        assert!(
            lit_fraction(&full_frame) > 0.95,
            "expected almost everything lit at progress=1"
        );

        let mut half = opts(base);
        half.insert("progress".to_string(), 0.5);
        let half_frame = frame_eclipse(64.0, 0.0, &half);
        let frac = lit_fraction(&half_frame);
        assert!(
            (0.3..0.7).contains(&frac),
            "expected roughly half lit at progress=0.5, got {frac}"
        );
    }
}
