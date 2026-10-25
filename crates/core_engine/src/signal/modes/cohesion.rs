//! Signal / Cohesion (1.15, a light volume): a sphere of light held together by `quality`.
//! As quality falls the points lose their place one by one (each at its own threshold), drift
//! apart and warm to amber; as it recovers they gather back. The pulse's successor: how well
//! something is holding (a connection, a sync, a signal).
//!
//! Inputs, none tied to the voice:
//! - `quality` (0..1, default 1): how whole the sphere is.
//! - `level` (0..1): how brightly it burns. A bound voice feeds `audioLevel` instead; with
//!   neither it glows low.
//! - `churn` (0..1): restless work (trying again): a tremor through every point, a brighter
//!   sphere.
//!
//! Reduced motion (`effectReduced`) holds the turn and the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const POINTS: usize = 450;
const RADIUS: f64 = 0.62;

pub fn frame_cohesion(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let quality = get(o, "quality", 1.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let budget = volume::budget(volume::view_px(o, size));
    let n = POINTS.min(budget * 3 / 4).max(60);
    let cam = Camera::new(t * 0.25, 0.25);
    let mut light = Light::new(cam, size, size, volume::px_per_unit(o, size), n);
    let burn = 0.6 + 1.0 * lv;
    for (i, (px, py, pz)) in volume::sphere(n).into_iter().enumerate() {
        let k = i as i64;
        // Each point lets go at its own quality, over a short stretch.
        let loose = ((1.0 - quality - 0.7 * volume::hash3(k, 7, 3)) / 0.3).clamp(0.0, 1.0);
        let spread = loose * 0.45 + churn * 0.04;
        let ft = t * 0.9;
        let dx = volume::noise(px * 2.0, py * 2.0, ft) * spread;
        let dy = volume::noise(py * 2.0, pz * 2.0, ft + 4.0) * spread;
        let dz = volume::noise(pz * 2.0, px * 2.0, ft + 8.0) * spread;
        let tone = if loose > 0.45 {
            Tone::Warm
        } else {
            Tone::Ramp(0.15)
        };
        let b = (0.18 + 0.3 * (1.0 - 0.5 * loose)) * burn * (1.0 + 0.25 * churn);
        light.emit(
            px * RADIUS + dx,
            py * RADIUS + dy,
            pz * RADIUS + dz,
            b,
            tone,
            0.9,
        );
    }
    light.finish(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn radius_spread(f: &OrbFrame) -> f64 {
        let n = f.dots.len() as f64;
        let (cx, cy) = (
            f.dots.iter().map(|d| d.x).sum::<f64>() / n,
            f.dots.iter().map(|d| d.y).sum::<f64>() / n,
        );
        let r: Vec<f64> = f
            .dots
            .iter()
            .map(|d| ((d.x - cx).powi(2) + (d.y - cy).powi(2)).sqrt())
            .collect();
        let m = r.iter().sum::<f64>() / n;
        (r.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n).sqrt()
    }

    #[test]
    fn it_holds_when_whole_and_scatters_and_warms_as_quality_falls() {
        let whole = frame_cohesion(200.0, 1.0, &opts(&[("quality", 1.0)]));
        let half = frame_cohesion(200.0, 1.0, &opts(&[("quality", 0.5)]));
        let gone = frame_cohesion(200.0, 1.0, &opts(&[("quality", 0.0)]));
        assert!(radius_spread(&whole) < radius_spread(&half));
        assert!(radius_spread(&half) < radius_spread(&gone));
        let warm = |f: &OrbFrame| f.dots.iter().filter(|d| d.hue < 60.0).count();
        assert_eq!(warm(&whole), 0);
        assert!(warm(&half) > 0 && warm(&gone) > warm(&half));
    }

    #[test]
    fn it_is_whole_and_alive_with_no_input() {
        let f = frame_cohesion(200.0, 1.0, &ModeOpts::new());
        assert_eq!(f, frame_cohesion(200.0, 1.0, &opts(&[("quality", 1.0)])));
        assert_ne!(f, frame_cohesion(200.0, 1.7, &ModeOpts::new()), "it turns");
    }

    #[test]
    fn a_value_and_work_burn_brighter() {
        let glow = |f: &OrbFrame| f.dots.iter().map(|d| d.a * d.r * d.r).sum::<f64>();
        let quiet = frame_cohesion(200.0, 1.0, &ModeOpts::new());
        assert!(glow(&frame_cohesion(200.0, 1.0, &opts(&[("level", 0.9)]))) > glow(&quiet) * 1.1);
        assert!(glow(&frame_cohesion(200.0, 1.0, &opts(&[("churn", 1.0)]))) > glow(&quiet) * 1.1);
    }

    #[test]
    fn small_views_draw_fewer_points_and_reduced_motion_holds_still() {
        assert!(frame_cohesion(48.0, 1.0, &ModeOpts::new()).dots.len() <= 150);
        let r = opts(&[("effectReduced", 1.0), ("quality", 0.4)]);
        assert_eq!(
            frame_cohesion(200.0, 0.3, &r),
            frame_cohesion(200.0, 2.9, &r)
        );
    }
}
