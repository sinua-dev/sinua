//! Signal / Halo (1.15, a light volume): a bright core and a cloud of light around it. As
//! `accuracy` rises the cloud tightens onto the core; a thin shell of light keeps travelling
//! from the core out to the edge. The locating halo's successor: how sure a position is.
//!
//! Inputs, none tied to the voice:
//! - `accuracy` (0..1, default 1): how tight the cloud is (1 = on the core, 0 = wide).
//! - `level` (0..1): how brightly the core burns. A bound voice feeds `audioLevel` instead;
//!   with neither it burns at a quiet default.
//! - `churn` (0..1): restless work: the cloud trembles.
//!
//! Reduced motion (`effectReduced`) holds the camera, the noise and the travelling shell still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const CLOUD: usize = 700;
const CORE: usize = 160;

pub fn frame_halo(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let acc = get(o, "accuracy", 1.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.2);
    let budget = volume::budget(volume::view_px(o, size));
    let n = CLOUD.min(budget * 3 / 4).max(60);
    let core = CORE.min(budget / 5).max(12);
    let cam = Camera::new(t * 0.15, 0.35);
    let mut light = Light::new(cam, size, size, volume::px_per_unit(o, size), n + core);
    let radius = 0.25 + (1.0 - acc) * 0.45;
    // The shell leaves the core and reaches the edge once every 1.4 s.
    let shell = (t * 0.7).rem_euclid(1.0);
    for i in 0..n as i64 {
        // A gaussian-ish cloud: the sum of three uniforms per axis.
        let g = |axis: i64| {
            volume::hash3(i, axis, 1) + volume::hash3(i, axis, 2) + volume::hash3(i, axis, 3) - 1.5
        };
        let (x, y, z) = (g(4), g(5), g(6));
        let rr = (x * x + y * y + z * z).sqrt() * radius / 1.2;
        let s = (-((rr / radius - shell).powi(2)) / 0.01).exp();
        let tremble = 0.05 * churn;
        let f = i as f64 * 0.37;
        light.emit(
            x * radius * 0.8 + tremble * volume::noise(f, t * 2.0, 1.0),
            y * radius * 0.8 + tremble * volume::noise(f, t * 2.0, 2.0),
            z * radius * 0.8 + tremble * volume::noise(f, t * 2.0, 3.0),
            (0.2 + 0.6 * s) * (0.4 + 1.2 * lv) * (1.0 + 1.2 * churn),
            Tone::Ramp(0.7),
            0.8,
        );
    }
    for (x, y, z) in volume::sphere(core) {
        light.emit(
            x * 0.06,
            y * 0.06,
            z * 0.06,
            0.4 + 0.5 * lv,
            Tone::Ramp(0.0),
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

    fn spread(f: &OrbFrame) -> f64 {
        let n = f.dots.len() as f64;
        let (cx, cy) = (
            f.dots.iter().map(|d| d.x).sum::<f64>() / n,
            f.dots.iter().map(|d| d.y).sum::<f64>() / n,
        );
        f.dots
            .iter()
            .map(|d| ((d.x - cx).powi(2) + (d.y - cy).powi(2)).sqrt())
            .sum::<f64>()
            / n
    }

    fn glow(f: &OrbFrame) -> f64 {
        f.dots.iter().map(|d| d.a * d.r * d.r).sum()
    }

    #[test]
    fn the_cloud_tightens_as_accuracy_rises() {
        let at = |a: f64| frame_halo(200.0, 1.0, &opts(&[("accuracy", a)]));
        assert!(spread(&at(0.1)) > spread(&at(0.5)) && spread(&at(0.5)) > spread(&at(1.0)));
    }

    #[test]
    fn it_lives_alone_and_a_value_burns_the_core() {
        let quiet = frame_halo(200.0, 1.0, &ModeOpts::new());
        assert_ne!(quiet, frame_halo(200.0, 1.4, &ModeOpts::new()));
        let value = frame_halo(200.0, 1.0, &opts(&[("level", 1.0)]));
        let voice = frame_halo(200.0, 1.0, &opts(&[("audioLevel", 1.0)]));
        assert!(glow(&value) > glow(&quiet) * 1.1);
        assert_eq!(value, voice);
        assert_ne!(
            quiet,
            frame_halo(200.0, 1.0, &opts(&[("churn", 1.0)])),
            "work makes it tremble"
        );
    }

    #[test]
    fn small_views_draw_fewer_points_and_reduced_motion_holds_still() {
        assert!(frame_halo(48.0, 1.0, &ModeOpts::new()).dots.len() <= 150);
        let r = opts(&[("effectReduced", 1.0), ("accuracy", 0.4)]);
        assert_eq!(frame_halo(200.0, 0.3, &r), frame_halo(200.0, 2.9, &r));
    }
}
