//! Signal / Comet (1.15, a light volume): a stream of light orbiting a faint core. The
//! head leads, the tail thins out behind it and breathes with the noise, so no two laps are
//! the same. The spinner's successor: waiting with no end in sight.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how bright and how long the stream burns. A bound voice feeds
//!   `audioLevel` instead (listening: the user's, speaking: the agent's); with neither it
//!   glows low and keeps circling.
//! - `churn` (0..1): restless work (thinking, processing): a longer, brighter tail. Its pace
//!   is the view's `speed` (phase-continuous), not a key.
//!
//! Reduced motion (`effectReduced`) holds the stream and the camera still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const STREAM: usize = 260;
const CORE: usize = 160;
const ORBIT: f64 = 0.75;

pub fn frame_comet(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let budget = volume::budget(volume::view_px(o, size));
    let n = STREAM.min(budget * 2 / 3).max(40);
    let core = CORE.min(budget / 4).max(12);
    let cam = Camera::new(0.3, 0.45);
    let mut light = Light::new(cam, size, size, volume::px_per_unit(o, size), n + core);
    // The head, and a tail that breathes between short and long.
    let head = t * 2.1 + 0.6 * volume::noise(t * 0.3, 0.0, 0.0);
    let breath = 0.5 + 0.5 * (t * 1.1 + volume::noise(t * 0.2, 1.0, 0.0) * 2.0).sin();
    let tail = (1.3 + 1.9 * breath) * (0.75 + 0.5 * lv + 0.4 * churn);
    let burn = 0.35 + 0.9 * lv + 0.2 * churn;
    for i in 0..n {
        let f = i as f64 / (n - 1) as f64;
        let a = head - f * tail;
        let wob = 0.22 * volume::noise(a * 0.6, t * 0.2, 3.0);
        let x = a.cos() * ORBIT;
        let z = a.sin() * ORBIT;
        let y = 0.28 * (a * 2.0 + wob * 3.0).sin();
        let j = 0.05 * f;
        let i = i as f64;
        light.emit(
            x + volume::noise(i, t, 1.0) * j,
            y + volume::noise(i, t, 2.0) * j,
            z + volume::noise(i, t, 3.0) * j,
            (1.0 - f) * burn,
            Tone::Ramp(f),
            1.2 - f * 0.6,
        );
    }
    for (x, y, z) in volume::sphere(core) {
        light.emit(
            x * 0.22,
            y * 0.22,
            z * 0.22,
            0.12 + 0.1 * lv,
            Tone::Ramp(0.5),
            0.6,
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

    fn glow(f: &OrbFrame) -> f64 {
        f.dots.iter().map(|d| d.a * d.r * d.r).sum()
    }

    #[test]
    fn it_circles_alone_and_burns_brighter_with_a_value_or_work() {
        let quiet = frame_comet(200.0, 1.0, &ModeOpts::new());
        assert!(!quiet.dots.is_empty());
        assert_ne!(
            quiet,
            frame_comet(200.0, 1.7, &ModeOpts::new()),
            "it moves on its own"
        );
        let value = frame_comet(200.0, 1.0, &opts(&[("level", 0.9)]));
        let voice = frame_comet(200.0, 1.0, &opts(&[("audioLevel", 0.9)]));
        assert!(glow(&value) > glow(&quiet) * 1.2);
        assert_eq!(value, voice, "a value and a voice at the same level agree");
        assert!(glow(&frame_comet(200.0, 1.0, &opts(&[("churn", 1.0)]))) > glow(&quiet) * 1.1);
    }

    #[test]
    fn small_views_draw_fewer_points() {
        assert!(frame_comet(48.0, 1.0, &ModeOpts::new()).dots.len() <= 150);
        assert!(frame_comet(512.0, 1.0, &ModeOpts::new()).dots.len() > 350);
    }

    #[test]
    fn reduced_motion_holds_still() {
        let r = opts(&[("effectReduced", 1.0), ("level", 0.4)]);
        assert_eq!(frame_comet(200.0, 0.3, &r), frame_comet(200.0, 2.9, &r));
    }
}
