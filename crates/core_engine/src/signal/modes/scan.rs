//! Signal / Scan (1.15, a light volume): a tilted disc of light and a beam sweeping it.
//! The beam lights what it passes and the light fades behind it; found points flash white as
//! the beam reaches them and burn down slowly. The radar's successor: looking for something.
//!
//! Inputs, none tied to the voice:
//! - `blips` (0..12, default 7): how many points are found.
//! - `level` (0..1): how bright the beam burns. A bound voice feeds `audioLevel` instead; with
//!   neither it glows low.
//! - `churn` (0..1): restless work (still searching): a longer trail behind the beam.
//!   Its pace is the view's `speed` (phase-continuous), not a key.
//!
//! The beam and the blips' glow are closed-form in `t` (no state). Reduced motion
//! (`effectReduced`) holds the beam and the camera still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const DISC: usize = 900;
const TAU: f64 = std::f64::consts::TAU;
/// The beam's angular speed (rad/s) at the view's speed 1.
const BEAM: f64 = 1.4;
/// Seconds a found point takes to burn down.
const BURN: f64 = 2.2;
const STACK: usize = 6;

pub fn frame_scan(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let blips = get(o, "blips", 7.0).round().clamp(0.0, 12.0) as usize;
    let budget = volume::budget(volume::view_px(o, size));
    let n = DISC.min(budget * 4 / 5).max(60);
    let cam = Camera {
        dy: 0.05,
        ..Camera::new(0.2, 0.95)
    };
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        n + blips * STACK,
    );
    let beam = t * BEAM;
    let trail = 0.5 * (1.0 + 0.8 * churn);
    let burn = 0.5 + 0.8 * lv + 0.2 * churn;
    for (dx, dz) in volume::disc(n) {
        let a = dz.atan2(dx);
        let behind = (beam - a).rem_euclid(TAU);
        let g = (-behind / trail).exp();
        let r = dx.hypot(dz);
        light.emit(
            dx * 0.95,
            0.02 * volume::noise(dx * 4.0, dz * 4.0, t),
            dz * 0.95,
            0.1 + g * burn,
            Tone::Ramp(0.2 + 0.6 * r),
            0.7 + g * 0.5,
        );
    }
    for i in 0..blips as i64 {
        let r = volume::hash3(i, 3, 9).sqrt() * 0.9;
        let a = volume::hash3(i, 4, 9) * TAU;
        let behind = (beam - a).rem_euclid(TAU);
        // The glow is the time since the beam passed: full at the flash, then burning down.
        let b = (-behind / (BEAM * BURN)).exp();
        let (x, z) = (a.cos() * r, a.sin() * r);
        for k in 0..STACK {
            light.emit(x, 0.03 + k as f64 * 0.012, z, b * 0.4, Tone::White, 0.9);
        }
    }
    light.finish(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn glow(f: &OrbFrame) -> f64 {
        f.dots.iter().map(|d| d.a * d.r * d.r).sum()
    }

    #[test]
    fn the_beam_sweeps_alone_and_burns_brighter_with_a_value_or_work() {
        let quiet = frame_scan(200.0, 1.0, &ModeOpts::new());
        assert!(quiet.dots.len() > 400);
        assert_ne!(quiet, frame_scan(200.0, 1.7, &ModeOpts::new()));
        let value = frame_scan(200.0, 1.0, &opts(&[("level", 0.9)]));
        let voice = frame_scan(200.0, 1.0, &opts(&[("audioLevel", 0.9)]));
        assert!(glow(&value) > glow(&quiet) * 1.1);
        assert_eq!(value, voice);
        assert!(glow(&frame_scan(200.0, 1.0, &opts(&[("churn", 1.0)]))) > glow(&quiet) * 1.1);
    }

    #[test]
    fn blips_are_extra_points_that_flash_when_the_beam_reaches_them() {
        let none = frame_scan(200.0, 1.0, &opts(&[("blips", 0.0)]));
        let some = frame_scan(200.0, 1.0, &opts(&[("blips", 12.0)]));
        assert!(some.dots.len() - none.dots.len() <= 12 * STACK);
        assert!(some.dots.len() > none.dots.len());
        // A blip is white: it just flashed at the beam (a found point at that angle).
        let white = |f: &OrbFrame| f.dots.iter().filter(|d| d.white > 0.85).count();
        assert!(white(&some) > white(&none));
    }

    #[test]
    fn small_views_draw_fewer_points() {
        assert!(frame_scan(48.0, 1.0, &ModeOpts::new()).dots.len() <= 170);
    }

    #[test]
    fn reduced_motion_holds_still() {
        let r = opts(&[("effectReduced", 1.0)]);
        assert_eq!(frame_scan(200.0, 0.3, &r), frame_scan(200.0, 2.9, &r));
    }
}
