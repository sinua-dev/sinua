//! Signal / Ripple (1.15, a light volume): shells of light spreading from a warm core. Each
//! shell crinkles with the noise and fades as it grows; the next one is a little early or
//! late. The ping's successor: something happened, look here.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how brightly the shells burn. A bound voice feeds `audioLevel` instead;
//!   with neither they burn at a quiet default.
//! - `churn` (0..1): restless work: shells follow each other faster.
//!
//! A shell's start is a fixed, hash-jittered beat (closed-form in `t`, no queue). Reduced
//! motion (`effectReduced`) holds the camera and the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const SHELL: usize = 300;
const CORE: usize = 160;
/// Seconds a shell lives.
const LIFE: f64 = 2.2;

pub fn frame_ripple(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let budget = volume::budget(volume::view_px(o, size));
    let per = SHELL.min(budget / 3).max(24);
    let core = CORE.min(budget / 6).max(12);
    // A fixed beat: its pace never depends on the inputs (a state change must not reshuffle
    // which shells are alive). Work adds a second train, half a beat behind, fading in.
    let beat = 1.6;
    let cam = Camera::new(t * 0.2, 0.3);
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        per * 5 + core,
    );
    let alive = (LIFE / beat).ceil() as i64 + 1;
    let shell = volume::sphere(per);
    let second = churn * churn * (3.0 - 2.0 * churn);
    for (train, weight) in [(0_i64, 1.0), (1, second)] {
        if weight < 0.01 {
            continue;
        }
        let newest = ((t - train as f64 * beat / 2.0) / beat).floor() as i64;
        for k in (newest - alive..=newest).filter(|k| *k >= 0) {
            // Each shell starts a little early or late (up to a third of a beat).
            let start = k as f64 * beat
                + train as f64 * beat / 2.0
                + (volume::hash3(k, 5, 2 + train) - 0.5) * beat / 3.0;
            let age = t - start;
            if !(0.0..LIFE).contains(&age) {
                continue;
            }
            let r = 0.2 + age * 0.5;
            let fade = 1.0 - age / LIFE;
            for (px, py, pz) in &shell {
                let n = 1.0 + 0.12 * volume::noise(px * 3.0, py * 3.0, t + age);
                light.emit(
                    px * r * n,
                    py * r * n,
                    pz * r * n,
                    weight * fade * (0.08 + 1.0 * lv),
                    Tone::Warm,
                    0.8,
                );
            }
        }
    }
    for (x, y, z) in volume::sphere(core) {
        light.emit(x * 0.1, y * 0.1, z * 0.1, 0.35 + 0.35 * lv, Tone::Warm, 0.8);
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
    fn it_pings_alone_and_burns_brighter_with_a_value() {
        let quiet = frame_ripple(200.0, 3.0, &ModeOpts::new());
        assert!(quiet.dots.len() > 160, "a core and at least one shell");
        assert_ne!(quiet, frame_ripple(200.0, 3.7, &ModeOpts::new()));
        let value = frame_ripple(200.0, 3.0, &opts(&[("level", 1.0)]));
        let voice = frame_ripple(200.0, 3.0, &opts(&[("audioLevel", 1.0)]));
        assert!(glow(&value) > glow(&quiet) * 1.1);
        assert_eq!(value, voice);
    }

    #[test]
    fn work_sends_more_shells() {
        let shells = |c: f64| {
            let f = frame_ripple(512.0, 6.0, &opts(&[("churn", c)]));
            f.dots.len()
        };
        assert!(shells(1.0) > shells(0.0));
    }

    #[test]
    fn a_shell_spreads_as_it_ages() {
        // Right after the first beat the shells are small; later they are wide.
        let extent = |t: f64| {
            let f = frame_ripple(512.0, t, &ModeOpts::new());
            f.dots
                .iter()
                .map(|d| (d.x - 256.0 / 4.0).abs())
                .fold(0.0, f64::max)
        };
        assert!(extent(0.1) > 0.0);
        assert!(extent(1.9) > extent(0.1));
    }

    #[test]
    fn small_views_draw_fewer_points_and_reduced_motion_holds_still() {
        assert!(frame_ripple(48.0, 3.0, &ModeOpts::new()).dots.len() <= 170);
        let r = opts(&[("effectReduced", 1.0)]);
        assert_eq!(frame_ripple(200.0, 0.3, &r), frame_ripple(200.0, 2.9, &r));
    }
}
