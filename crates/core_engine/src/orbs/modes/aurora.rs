//! Aurora: a colored nebula cloud on a sphere, perturbed by real 3D gradient
//! noise. NOT a port -- there is no `thinking-orbs` upstream equivalent and
//! therefore no golden vector to prove it against (see
//! `docs/effects-research.md`'s summary judgment on why that's the honest
//! tradeoff here). It exists to prove out, end-to-end through every
//! platform binding, the two additive engine capabilities added alongside
//! it: `core::perlin3` (real gradient noise, vs. every ported mode's
//! hash-based `vnoise`) and `Dot`'s `saturation`/`hue` fields (every ported
//! mode leaves these at `0.0` via `..Default::default()`, so this mode is
//! the only thing in the engine that currently sets them to something
//! nonzero). Correctness here rests on the unit test below plus `core.rs`'s
//! noise tests, not a frozen vector file -- treat any preset numbers as a
//! first pass, the same caveat the feature status carries for the
//! Studio's knob ranges.

use crate::orbs::core::{
    fib_dir, finalize_frame, perlin3, radius_scale, Dot, LatticeSample, OrbFrame, Proj,
};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// This mode's own camera (yaw multiplier, tilt) -- pulled out to a
/// constant so `orbs::modes::transition` can look it up without
/// duplicating it, and so it can't silently drift from what `frame_aurora`
/// itself uses below.
pub(crate) const CAMERA: (f64, f64) = (0.1, 0.35);

/// Per-point data for index `i` of `node_n`, exactly as `frame_aurora`
/// computes it -- including this mode's own camera-dependent depth
/// shading (see `LatticeSample`'s doc comment for why that's still done
/// here rather than deferred to the transition's shared camera).
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

    let node_r = get(o, "nodeSize", 1.1);
    let noise_scale = get(o, "surfaceScale", 1.4);
    let flow_speed = get(o, "surfaceSpeed", 0.25);
    let hue_spread = get(o, "hueSpread", 140.0);
    let hue_offset = get(o, "hueOffset", 0.0);
    let hue_speed = get(o, "hueSpeed", 12.0);
    let saturation = get(o, "saturation", 0.55);
    // 1 = the depth-shaded tone (front 0.75, back 0.35); 0 = one tone (0.55)
    // for every dot. Only the tone: size and alpha keep the depth.
    let depth_tone = get(o, "depthTone", 1.0).clamp(0.0, 1.0);

    let (nx, ny, nz) = fib_dir(i as f64, node_n as f64);

    // Real gradient noise flowing over time along the sphere's surface --
    // the thing this mode exists to prove out, vs. every ported mode's
    // `vnoise`. `n` is roughly in [-1, 1] (see `perlin3`'s doc comment).
    let n = perlin3(
        nx * noise_scale + t * flow_speed,
        ny * noise_scale,
        nz * noise_scale + t * flow_speed * 0.7,
    );

    let bulge = 1.0 + 0.18 * n;
    let (_px, _py, z) = pt.project(nx * r * bulge, ny * r * bulge, nz * r * bulge);
    let depth = (z / r + 1.0) / 2.0;
    // `shaded + (mid - shaded) * 0` is `shaded` exactly, so 1 changes no pixel.
    let shaded = 0.35 + 0.4 * depth;
    let white = shaded + (0.55 - shaded) * (1.0 - depth_tone);
    let hue = (hue_spread * (0.5 + 0.5 * n) + hue_offset + t * hue_speed).rem_euclid(360.0);

    LatticeSample {
        dir: (nx, ny, nz),
        radius_frac: bulge,
        dot_r: node_r * rs * (0.6 + 0.6 * depth),
        white,
        alpha: 0.55 + 0.45 * depth,
        saturation,
        hue,
    }
}

pub fn frame_aurora(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * CAMERA.0, CAMERA.1, cx, cy, 1.0);
    let node_n = get(o, "nodeCount", 260.0) as i64;

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
            saturation: s.saturation,
            hue: s.hue,
        });
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    /// Not a golden-vector check (there is none to check against) -- proves
    /// the pipeline this mode was added to demonstrate actually threads
    /// through: real dots come out, and at least one carries nonzero color,
    /// which no ported mode's output ever does.
    #[test]
    fn produces_colored_dots() {
        let o = opts(&[("nodeCount", 40.0), ("rMin", 0.3)]);
        let frame = frame_aurora(64.0, 1.2, &o);
        assert!(!frame.dots.is_empty(), "expected dots, got none");
        assert!(
            frame.dots.iter().any(|d| d.saturation > 0.0),
            "expected at least one colored dot"
        );
        for d in &frame.dots {
            assert!((0.0..360.0).contains(&d.hue), "hue {} out of range", d.hue);
        }
    }

    #[test]
    fn depth_tone_flattens_only_the_tone() {
        let base = opts(&[("nodeCount", 40.0), ("rMin", 0.3)]);
        let mut one = base.clone();
        one.insert("depthTone".into(), 1.0);
        let mut flat = base.clone();
        flat.insert("depthTone".into(), 0.0);
        let (b, o, f) = (
            frame_aurora(64.0, 1.2, &base),
            frame_aurora(64.0, 1.2, &one),
            frame_aurora(64.0, 1.2, &flat),
        );
        // 1 is today's look, bit for bit.
        for (x, y) in b.dots.iter().zip(&o.dots) {
            assert_eq!(x.white.to_bits(), y.white.to_bits());
        }
        // 0: one tone everywhere; the depth still shows in size and alpha.
        assert!(f.dots.iter().all(|d| (d.white - 0.55).abs() < 1e-12));
        assert_eq!(f.dots.len(), b.dots.len());
        let (rmin, rmax) = f
            .dots
            .iter()
            .fold((f64::MAX, 0.0f64), |(lo, hi), d| (lo.min(d.r), hi.max(d.r)));
        assert!(rmax > rmin * 1.2, "sizes should still vary with depth");
    }
}
