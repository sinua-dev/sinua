//! Wave: a waveform rolls through the rings -- the "listening" state.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/lattice.ts` (`frameWave`).

use crate::orbs::core::{finalize_frame, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_wave(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    // 0.76 base x 1.15 -- the undulation pulls the sphere inward, so wave
    // reads ~15% smaller than the other lattice modes; scaled up to match them
    let r = (size / 2.0) * 0.874;
    let pt = Proj::new(t * 0.18, 0.38, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let rings = get(o, "rings", 15.0) as i64;
    let lon_density = get(o, "lonDensity", 40.0);
    let r_base = get(o, "rBase", 0.6);
    let r_depth = get(o, "rDepth", 1.7);

    let mut dots: Vec<Dot> = Vec::new();
    for ri in 0..=rings {
        let lat = -PI / 2.0 + (ri as f64 / rings as f64) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        // two waves, different tempi -- organic, never quite repeating
        let w =
            0.62 * (t * 2.1 - ri as f64 * 0.52).sin() + 0.38 * (t * 1.27 + ri as f64 * 0.83).sin();
        let rr = r * (0.88 + 0.105 * w);
        let lon_count = ((cos_lat.abs() * lon_density).round() as i64).max(1);
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let (px, py, z) = pt.project(
                cos_lat * lon.cos() * rr,
                sin_lat * rr,
                cos_lat * lon.sin() * rr,
            );
            let depth = (z / r + 1.0) / 2.0;
            let crest = w.max(0.0);
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: (r_base + r_depth * depth) * (1.0 + 0.4 * crest) * rs,
                white: 0.66 - 0.56 * depth - 0.1 * crest,
                a: 1.0,
                ..Default::default()
            });
        }
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
