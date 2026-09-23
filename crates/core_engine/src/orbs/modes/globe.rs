//! Globe: lat/long field, a scan meridian sweeps -- the "searching" state.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/lattice.ts` (`frameGlobe`).

use crate::orbs::core::{angle_delta, finalize_frame, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_globe(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let spin = 0.5;
    let cx = size / 2.0;
    let cy = size / 2.0;
    let radius = (size / 2.0) * 0.82;
    let tilt = 0.4 + 0.06 * (t * 0.35).sin();
    let pt = Proj::new(t * spin, tilt, cx, cy, radius);
    // scan sweeps relative to the spin; scanMul scales that relative rate
    let scan_mul = get(o, "scanMul", 1.0);
    let scan = t * (spin + (1.7 - spin) * scan_mul);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));
    let dim_base = get(o, "dimBase", 1.0);

    let lat_rings = get(o, "latRings", 17.0) as i64;
    let lon_density = get(o, "lonDensity", 44.0);
    let r_base = get(o, "rBase", 0.6);
    let r_depth = get(o, "rDepth", 1.7);
    let r_boost = get(o, "rBoost", 1.0);
    let ink_far = get(o, "inkFar", 0.62);
    let ink_span = get(o, "inkSpan", 0.54);

    let mut dots: Vec<Dot> = Vec::new();
    for li in 0..=lat_rings {
        let lat = -PI / 2.0 + (li as f64 / lat_rings as f64) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let lon_count = ((cos_lat.abs() * lon_density).round() as i64).max(1);
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let (px, py, z) = pt.project(cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin());
            let depth = (z + 1.0) / 2.0;
            // the scan: a moving meridian read as a size ripple, not a shine
            let d = angle_delta(lon + t * spin, scan);
            let boost = (-(d * d) / 0.18).exp() * z.max(0.0);
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: (r_base + r_depth * depth + r_boost * boost) * rs,
                white: ink_far - ink_span * depth,
                // dimBase < 1 fades un-scanned dots so the meridian reads clearly
                a: dim_base + (1.0 - dim_base) * boost.min(1.0),
                ..Default::default()
            });
        }
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
