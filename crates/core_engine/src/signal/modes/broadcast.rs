//! Signal / Broadcast (1.15, a light volume): two fans of light opening to the left and right
//! of a white core, a front after a front. Louder, the fronts come faster and burn brighter;
//! quiet, they are few and faint. The broadcast ring's successor: sending, speaking out.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how strongly the fronts burn and how often they leave. A bound voice
//!   feeds `audioLevel` instead; with neither a slow swell stands in so it lives.
//! - `churn` (0..1): restless work: a steady stream of fronts with no level (waiting on a
//!   reply).
//!
//! Fronts leave on a fixed, hash-jittered beat (closed-form in `t`, no queue), and all those
//! alive burn at the level of now. Reduced motion (`effectReduced`) holds the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const CORE: usize = 160;
/// Seconds a front lives.
const LIFE: f64 = 1.6;
const COLS: usize = 10;
const ROWS: usize = 7;

pub fn frame_broadcast(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.14 + 0.08 * (t * 0.5).sin());
    let drive = lv.max(0.45 * churn);
    let budget = volume::budget(volume::view_px(o, size));
    let cells = COLS * ROWS;
    // A fixed beat (a state change must not reshuffle which fronts are alive); a louder
    // level or work fades in a second train half a beat behind, so it reads busier.
    let beat = 0.9;
    let cam = Camera::new(0.0, 0.2);
    let core = CORE.min(budget / 6).max(12);
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        cells * 2 * 6 + core,
    );
    let alive = (LIFE / beat).ceil() as i64 + 1;
    // Small views keep fewer points per front.
    let rows = volume::fit(ROWS, cells * 2 * 3, budget, 2).max(3);
    let strength = 0.3 + 0.7 * drive;
    let second = drive * drive * (3.0 - 2.0 * drive);
    for (train, weight) in [(0_i64, 1.0), (1, second)] {
        if weight < 0.01 {
            continue;
        }
        let newest = ((t - train as f64 * beat / 2.0) / beat).floor() as i64;
        for k in (newest - alive..=newest).filter(|k| *k >= 0) {
            let start = k as f64 * beat
                + train as f64 * beat / 2.0
                + (volume::hash3(k, 8, 4 + train) - 0.5) * beat / 3.0;
            let age = t - start;
            if !(0.0..LIFE).contains(&age) {
                continue;
            }
            let r = 0.18 + age * 0.7;
            let fade = (1.0 - age / LIFE) * strength * weight;
            for side in [-1.0, 1.0] {
                for row in 0..rows {
                    for col in 0..COLS {
                        let u = col as f64 / (COLS - 1) as f64 - 0.5;
                        let v = row as f64 / (rows - 1) as f64 - 0.5;
                        let (ang, el) = (u * 1.1, v * 0.9);
                        light.emit(
                            side * ang.cos() * el.cos() * r,
                            el.sin() * r * 0.9,
                            ang.sin() * r * 0.6,
                            fade * 0.8,
                            Tone::Ramp(if side < 0.0 { 0.9 } else { 0.1 }),
                            0.9,
                        );
                    }
                }
            }
        }
    }
    for (x, y, z) in volume::sphere(core) {
        light.emit(
            x * 0.08,
            y * 0.08,
            z * 0.08,
            0.35 + 0.3 * drive,
            Tone::White,
            0.7,
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
    fn it_broadcasts_alone_and_louder_is_brighter_and_busier() {
        let quiet = frame_broadcast(200.0, 3.0, &opts(&[("level", 0.0)]));
        let loud = frame_broadcast(200.0, 3.0, &opts(&[("level", 1.0)]));
        assert!(!quiet.dots.is_empty());
        assert!(glow(&loud) > glow(&quiet) * 1.5);
        assert!(loud.dots.len() > quiet.dots.len(), "more fronts alive");
        assert_ne!(
            frame_broadcast(200.0, 3.0, &ModeOpts::new()),
            frame_broadcast(200.0, 3.6, &ModeOpts::new())
        );
        let voice = frame_broadcast(200.0, 3.0, &opts(&[("audioLevel", 1.0)]));
        assert_eq!(loud, voice);
    }

    #[test]
    fn work_keeps_a_stream_going_with_no_level() {
        // Over a few seconds, so it doesn't hang on which fronts happen to be alive.
        let over = |c: f64| -> f64 {
            (0..12)
                .map(|i| {
                    let o = opts(&[("level", 0.0), ("churn", c)]);
                    glow(&frame_broadcast(200.0, 3.0 + f64::from(i) * 0.3, &o))
                })
                .sum()
        };
        assert!(over(1.0) > over(0.0) * 1.2, "{} {}", over(1.0), over(0.0));
    }

    #[test]
    fn small_views_draw_fewer_points_and_reduced_motion_holds_still() {
        assert!(
            frame_broadcast(48.0, 3.0, &opts(&[("level", 1.0)]))
                .dots
                .len()
                <= 400
        );
        let r = opts(&[("effectReduced", 1.0), ("level", 0.5)]);
        assert_eq!(
            frame_broadcast(200.0, 0.3, &r),
            frame_broadcast(200.0, 2.9, &r)
        );
    }
}
