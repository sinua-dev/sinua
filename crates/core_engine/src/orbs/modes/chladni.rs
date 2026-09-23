//! Chladni: nodal standing-wave clustering on a sphere, for the
//! `calibrating` state. NOT a port -- see `docs/effects-research.md`'s
//! "Chladni bloom" proposal. Dots sit at fixed positions (a Fibonacci
//! lattice, `fib_dir`) and a cheap sum-of-cosines standing-wave function
//! (not real spherical harmonics -- the proposal explicitly doesn't need
//! them) brightens antinodes and dims nodal lines; the wave's two mode
//! numbers snap to a new pseudo-random pair on a fixed cadence instead of
//! drifting continuously, which is what makes it read as "settling into
//! resonance" (cymatics) rather than `web`'s organic continuous drift or
//! `globe`'s uniform field. No golden vector -- same tradeoff as this
//! file's siblings.

use crate::orbs::core::{
    fib_dir, finalize_frame, hash_d, radius_scale, Dot, LatticeSample, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// This mode's own camera (yaw multiplier, tilt) -- see `aurora::CAMERA`'s
/// doc comment for why this is pulled into a constant.
pub(crate) const CAMERA: (f64, f64) = (0.1, 0.3);

/// Which low-order mode pair is active during the hold window containing
/// `t`. Pulled out of `frame_chladni` so it's testable independent of the
/// projection's continuous idle spin (`Proj::new(t * 0.1, ...)`), which
/// changes every dot's projected depth -- and therefore alpha -- even
/// while the wave pattern itself is held steady; comparing whole frames
/// across two `t` values in the "same window" would otherwise conflate
/// "the pattern changed" with "the sphere rotated a bit."
fn mode_numbers(t: f64, hold_time: f64) -> (f64, f64) {
    let step = (t / hold_time).floor();
    let m = 2.0 + (hash_d(step, 3.1) * 5.0).floor();
    let n_lon = 2.0 + (hash_d(step, 7.7) * 5.0).floor();
    (m, n_lon)
}

/// Per-point data for index `i` of `node_n`, exactly as `frame_chladni`
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
    let hold_time = get(o, "holdDuration", 2.5).max(0.1);
    let (m, n_lon) = mode_numbers(t, hold_time);

    let (dx, dy, dz) = fib_dir(i as f64, node_n as f64);
    let lat = dy.acos();
    let lon = dz.atan2(dx);
    let wave = (m * lat).cos() * (n_lon * lon).cos();
    let bright = wave.abs();

    let (_px, _py, z) = pt.project(dx * r, dy * r, dz * r);
    let depth = (z / r + 1.0) / 2.0;

    LatticeSample {
        dir: (dx, dy, dz),
        radius_frac: 1.0,
        dot_r: node_r * rs * (0.4 + 0.8 * bright),
        white: 0.6 - 0.45 * bright,
        alpha: (0.12 + 0.88 * bright) * (0.5 + 0.5 * depth),
        saturation: 0.0,
        hue: 0.0,
    }
}

pub fn frame_chladni(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * CAMERA.0, CAMERA.1, cx, cy, 1.0);
    let node_n = get(o, "nodeCount", 260.0) as i64;

    // Snap (not drift) to a new low-order mode pair every `holdDuration`
    // seconds -- the "kaleidoscope-like snap between discrete symmetric
    // states" the proposal calls for.
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

    #[test]
    fn snaps_to_a_new_pattern_after_hold_time() {
        // Two `t` values inside the same 1.0s window must pick the same
        // mode pair; a third, one window later, must (almost certainly)
        // differ. (Not a full frame comparison -- see `mode_numbers`'s doc
        // comment for why: the projection's continuous idle spin changes
        // every dot's depth/alpha regardless of whether the pattern itself
        // held steady.)
        assert_eq!(mode_numbers(0.3, 1.0), mode_numbers(0.6, 1.0));
        assert_ne!(mode_numbers(0.3, 1.0), mode_numbers(1.3, 1.0));
    }

    #[test]
    fn holding_the_pattern_still_produces_a_stable_frame() {
        // Freeze `t` for the projection too (call at the exact same `t`
        // twice) as a sanity check that the mode is otherwise deterministic.
        let o = opts(&[("nodeCount", 200.0), ("holdDuration", 1.0), ("rMin", 0.3)]);
        let a = frame_chladni(64.0, 0.45, &o);
        let b = frame_chladni(64.0, 0.45, &o);
        assert_eq!(a, b);
    }
}
