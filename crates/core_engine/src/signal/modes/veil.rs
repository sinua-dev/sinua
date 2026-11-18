//! Signal / Veil (1.15, a light volume): a sheet of light with depth. Layers of points,
//! one behind the other, each curling on its own envelope; seen in perspective it reads as
//! a surface, not a few lines. The waveform's successor.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how high the sheet swells. A bound voice feeds `audioLevel` instead
//!   (listening: the user's, speaking: the agent's); with neither it breathes on its own.
//! - `churn` (0..1): restless work (thinking, processing): the sheet keeps moving without
//!   a level.
//! - `layerCount` (3..12, default 9): sheets front to back.
//!
//! Reduced motion (`effectReduced`) holds the camera and the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

pub fn frame_veil(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let level = volume::value(o, "level");
    let layers = get(o, "layerCount", 9.0).round().clamp(3.0, 12.0) as usize;
    let along = volume::fit(70, layers * 70, volume::budget(volume::view_px(o, size)), 2);
    let layers = volume::fit(
        layers,
        layers * 70,
        volume::budget(volume::view_px(o, size)),
        2,
    )
    .max(3);
    // A fixed angle (no sway), zoomed out a little so the sheet's near edge stays inside the view.
    let cam = Camera {
        zoom: 0.88,
        ..Camera::new(-0.5, 0.35)
    };
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        layers * along,
    );
    // The swell: the level when there is one, a quiet breath otherwise; work adds its own.
    let amp = match level {
        Some(v) => 0.1 + v * 0.6,
        None => 0.06,
    } + churn * 0.15;
    for v in 0..layers {
        let fv = v as f64 / (layers - 1) as f64;
        let z = -0.45 + fv * 0.88;
        for u in 0..along {
            let x = -1.1 + u as f64 / (along - 1) as f64 * 2.2;
            let envelope = 1.0 / (1.0 + (x * 1.3).powi(4));
            let wave = (x * 6.0 + t * 3.1 + fv * 4.0).sin()
                + 1.2 * volume::noise(x * 2.0, t * 1.3, fv * 3.2);
            let y = amp * envelope * wave;
            light.emit(x, y, z, 0.25 + y.abs() * 2.4, Tone::Ramp(fv), 0.9);
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

    fn spread(f: &OrbFrame) -> f64 {
        let (lo, hi) = f
            .dots
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), d| (a.min(d.y), b.max(d.y)));
        hi - lo
    }

    #[test]
    fn it_lives_without_input_and_swells_with_a_value_or_a_voice() {
        let quiet = frame_veil(200.0, 1.0, &ModeOpts::new());
        assert!(!quiet.dots.is_empty());
        assert_ne!(
            quiet,
            frame_veil(200.0, 1.7, &ModeOpts::new()),
            "it moves on its own"
        );
        let value = frame_veil(200.0, 1.0, &opts(&[("level", 0.8)]));
        let voice = frame_veil(200.0, 1.0, &opts(&[("audioLevel", 0.8)]));
        assert!(spread(&value) > spread(&quiet) * 2.0);
        // A value and a voice at the same level draw the same sheet.
        assert_eq!(value, voice);
        // Work keeps it moving higher with no value.
        assert!(spread(&frame_veil(200.0, 1.0, &opts(&[("churn", 1.0)]))) > spread(&quiet));
    }

    #[test]
    fn small_views_draw_fewer_points() {
        let n = |s: f64| frame_veil(s, 1.0, &opts(&[("level", 0.5)])).dots.len();
        assert!(n(48.0) <= 160, "{}", n(48.0));
        assert!(n(512.0) > 500);
    }

    #[test]
    fn reduced_motion_holds_still() {
        let r = opts(&[("effectReduced", 1.0), ("level", 0.4)]);
        assert_eq!(frame_veil(200.0, 0.3, &r), frame_veil(200.0, 2.9, &r));
    }
}
