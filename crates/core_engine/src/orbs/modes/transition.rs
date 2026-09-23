//! Real point-by-point morphing between two states, for the one trio of
//! modes where it's actually meaningful: `aurora` (`glowing`), `chladni`
//! (`calibrating`), and `eclipse` (`progressing`) all place point `i` of
//! `nodeCount` at the *exact same* `fib_dir(i, nodeN)` direction -- not a
//! coincidence, `aurora` was the first of the three and the other two were
//! deliberately built on the same lattice. That shared correspondence is
//! what makes a real morph possible: point `i` in one mode IS point `i` in
//! another, so blending them point-by-point is meaningful. No other pair of
//! modes in this engine has that property (different dot counts, different
//! meanings -- see `docs/effects-research.md`), so this only ever supports
//! this specific trio; any other state pair should cross-dissolve two
//! independently-rendered whole frames instead (client-side, no engine
//! support needed for that -- see the Studio's transition panel).
//!
//! Two things worth calling out about how the blend actually works:
//!
//! - **No `slerp` needed.** Since `dir` is the literal same `fib_dir` call
//!   for both endpoints (not independently computed and only coincidentally
//!   similar), `sample_a.dir == sample_b.dir` exactly -- only
//!   `radius_frac` (aurora's noise-driven bulge; chladni/eclipse are always
//!   `1.0`) needs blending, which is a plain scalar `lerp`. If a future
//!   fourth lattice-sharing mode ever perturbed `dir` itself per-point,
//!   *that* would need `slerp` (spherical, not straight-line, interpolation
//!   -- a straight lerp between two points on a sphere cuts through the
//!   interior); not needed for this trio.
//! - **Depth shading uses each mode's own camera; final position uses one
//!   shared, blended camera.** `lattice_sample` (in each of `aurora.rs`/
//!   `chladni.rs`/`eclipse.rs`) still projects internally with that mode's
//!   own `CAMERA` just to compute `white`/`alpha` the same way that mode
//!   always has -- unifying that shading math across three different modes
//!   isn't reasonable. But the dot's actual on-screen position for the
//!   transition frame comes from ONE `Proj` built from the two modes'
//!   `CAMERA`s lerped together, so the sphere doesn't visibly jump cameras
//!   mid-transition.

use crate::orbs::core::{finalize_frame, lerp, lerp_hue, Dot, LatticeSample, OrbFrame, Proj};
use crate::orbs::modes::{aurora, chladni, eclipse};
use crate::orbs::profiles::ModeOpts;

fn sample(
    mode: &str,
    i: i64,
    node_n: i64,
    t: f64,
    o: &ModeOpts,
    size: f64,
) -> Option<LatticeSample> {
    match mode {
        "aurora" => Some(aurora::lattice_sample(i, node_n, t, o, size)),
        "chladni" => Some(chladni::lattice_sample(i, node_n, t, o, size)),
        "eclipse" => Some(eclipse::lattice_sample(i, node_n, t, o, size)),
        _ => None,
    }
}

fn camera_for(mode: &str) -> Option<(f64, f64)> {
    match mode {
        "aurora" => Some(aurora::CAMERA),
        "chladni" => Some(chladni::CAMERA),
        "eclipse" => Some(eclipse::CAMERA),
        _ => None,
    }
}

/// `None` if either `mode` isn't one of the three lattice-sharing modes --
/// the caller (`core_engine::frame_transition`) should fall back to
/// cross-dissolving two independent `frame()` calls in that case.
pub fn frame_transition(
    from_mode: &str,
    to_mode: &str,
    size: f64,
    t: f64,
    blend: f64,
    from_opts: &ModeOpts,
    to_opts: &ModeOpts,
) -> Option<OrbFrame> {
    let blend = blend.clamp(0.0, 1.0);
    let (yaw_mult_a, tilt_a) = camera_for(from_mode)?;
    let (yaw_mult_b, tilt_b) = camera_for(to_mode)?;

    let r = (size / 2.0) * 0.82;
    let yaw = lerp(yaw_mult_a * t, yaw_mult_b * t, blend);
    let tilt = lerp(tilt_a, tilt_b, blend);
    let pt = Proj::new(yaw, tilt, size / 2.0, size / 2.0, 1.0);

    let node_n = *from_opts.get("nodeCount").unwrap_or(&260.0) as i64;

    let mut dots: Vec<Dot> = Vec::with_capacity(node_n.max(0) as usize);
    for i in 0..node_n {
        let sa = sample(from_mode, i, node_n, t, from_opts, size)?;
        let sb = sample(to_mode, i, node_n, t, to_opts, size)?;

        // `sa.dir` and `sb.dir` are identical (see this module's header) --
        // only the radius fraction (aurora's bulge) can differ.
        let radius_frac = lerp(sa.radius_frac, sb.radius_frac, blend);
        let (px, py, z) = pt.project(
            sa.dir.0 * r * radius_frac,
            sa.dir.1 * r * radius_frac,
            sa.dir.2 * r * radius_frac,
        );

        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: lerp(sa.dot_r, sb.dot_r, blend),
            white: lerp(sa.white, sb.white, blend),
            a: lerp(sa.alpha, sb.alpha, blend),
            saturation: lerp(sa.saturation, sb.saturation, blend),
            hue: lerp_hue(sa.hue, sb.hue, blend),
        });
    }

    Some(finalize_frame(dots, vec![], 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    #[test]
    fn unsupported_mode_pair_returns_none() {
        let o = opts(&[("nodeCount", 40.0)]);
        assert!(frame_transition("orbits", "eclipse", 64.0, 1.0, 0.5, &o, &o).is_none());
        assert!(frame_transition("aurora", "morph", 64.0, 1.0, 0.5, &o, &o).is_none());
    }

    #[test]
    fn endpoints_match_the_source_modes() {
        let ao = opts(&[("nodeCount", 40.0), ("rMin", 0.3)]);
        let eo = opts(&[("nodeCount", 40.0), ("rMin", 0.3)]);

        let at_zero = frame_transition("aurora", "eclipse", 64.0, 2.0, 0.0, &ao, &eo).unwrap();
        let aurora_alone = aurora::frame_aurora(64.0, 2.0, &ao);
        assert_eq!(at_zero.dots.len(), aurora_alone.dots.len());
        // blend=0 -> full "from" camera and values, so this should be a
        // near-exact reproduction of frame_aurora's own output (position
        // and color both), modulo z-sort tie-break order.
        let sum_hue = |f: &OrbFrame| f.dots.iter().map(|d| d.hue).sum::<f64>();
        assert!((sum_hue(&at_zero) - sum_hue(&aurora_alone)).abs() < 1e-6);

        let at_one = frame_transition("aurora", "eclipse", 64.0, 2.0, 1.0, &ao, &eo).unwrap();
        let eclipse_alone = eclipse::frame_eclipse(64.0, 2.0, &eo);
        let sum_white = |f: &OrbFrame| f.dots.iter().map(|d| d.white).sum::<f64>();
        assert!((sum_white(&at_one) - sum_white(&eclipse_alone)).abs() < 1e-6);
    }

    #[test]
    fn midway_blend_is_between_the_two_endpoints() {
        let ao = opts(&[("nodeCount", 40.0), ("rMin", 0.3), ("saturation", 0.55)]);
        let eo = opts(&[("nodeCount", 40.0), ("rMin", 0.3)]);
        let mid = frame_transition("aurora", "eclipse", 64.0, 0.5, 0.5, &ao, &eo).unwrap();
        // aurora is colored (saturation 0.55), eclipse is not (0.0) -- the
        // midpoint should be partway between, not equal to either extreme.
        let avg_sat = mid.dots.iter().map(|d| d.saturation).sum::<f64>() / mid.dots.len() as f64;
        assert!(
            (0.1..0.5).contains(&avg_sat),
            "expected a partial saturation, got {avg_sat}"
        );
    }
}
