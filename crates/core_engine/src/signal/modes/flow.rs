//! Signal / Flow (1.15, a light volume): a front of light sweeping through a slab of
//! scattered points, a fainter second front trailing it. Points light as the front passes and
//! settle back to dust; the pace wanders with the noise. The shimmer's successor: something is
//! being made.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how strongly the fronts light the slab. A bound voice feeds `audioLevel`
//!   instead; with neither it glows low and keeps sweeping.
//! - `churn` (0..1): restless work (thinking, processing): a stronger second front and a
//!   shimmer across the whole slab. Its pace is the view's `speed`, not a key.
//!
//! The fronts' positions are closed-form in `t` (no state), and wrap off-screen. Reduced
//! motion (`effectReduced`) holds the fronts and the camera still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const POINTS: usize = 800;
/// Seconds the second front trails the first.
const LAG: f64 = 1.6;
/// A front crosses `-SPAN..SPAN` (past the slab's edges on both sides) in `1 / PACE` s.
const SPAN: f64 = 1.6;
const PACE: f64 = 0.2;

/// Where a front is at `t`: a steady sweep that the noise speeds up and slows down.
fn front(t: f64) -> f64 {
    let phase = t * PACE + 0.06 * volume::noise(t * 0.4, 0.0, 5.0);
    -SPAN + 2.0 * SPAN * phase.rem_euclid(1.0)
}

pub fn frame_flow(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let budget = volume::budget(volume::view_px(o, size));
    let n = POINTS.min(budget).max(60);
    let cam = Camera::new(-0.45, 0.5);
    let mut light = Light::new(cam, size, size, volume::px_per_unit(o, size), n);
    let (f1, f2) = (front(t), front(t - LAG));
    let amp = 0.55 + 0.9 * lv;
    let second = 0.45 + 0.4 * churn;
    for i in 0..n as i64 {
        let x = -1.1 + 2.2 * volume::hash3(i, 0, 1);
        let y = (volume::hash3(i, 1, 1) - 0.5) * 0.22;
        let z = (volume::hash3(i, 2, 1) - 0.5) * 0.6;
        let g = (-((x - f1).powi(2)) / 0.02).exp() + second * (-((x - f2).powi(2)) / 0.03).exp();
        let yy = y + 0.05 * volume::noise(x * 3.0, z * 3.0, t * 0.8);
        let shimmer = churn * 0.25 * (0.5 + 0.5 * volume::noise(x * 3.0 + 9.0, y * 20.0, t * 1.5));
        light.emit(
            x,
            yy,
            z,
            0.07 + g * amp + shimmer,
            Tone::Ramp((x + 1.1) / 2.2),
            0.8 + g * 0.6,
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
    fn the_fronts_sweep_and_wrap_off_screen() {
        // Continuous: no jump, including across the wrap (which happens past the slab).
        let mut prev = front(0.0);
        for i in 1..4000 {
            let f = front(f64::from(i) * 0.005);
            if f < prev {
                assert!(prev > SPAN - 0.1 && f < -SPAN + 0.1, "wrap at the edges");
            } else {
                assert!(f - prev < 0.05, "{i}");
            }
            prev = f;
        }
    }

    #[test]
    fn it_flows_alone_and_lights_more_with_a_value_or_work() {
        let quiet = frame_flow(200.0, 1.0, &ModeOpts::new());
        assert!(quiet.dots.len() > 300);
        assert_ne!(quiet, frame_flow(200.0, 1.7, &ModeOpts::new()));
        let value = frame_flow(200.0, 1.0, &opts(&[("level", 0.9)]));
        let voice = frame_flow(200.0, 1.0, &opts(&[("audioLevel", 0.9)]));
        assert!(glow(&value) > glow(&quiet) * 1.1);
        assert_eq!(value, voice);
        assert!(glow(&frame_flow(200.0, 1.0, &opts(&[("churn", 1.0)]))) > glow(&quiet) * 1.1);
    }

    #[test]
    fn small_views_draw_fewer_points() {
        assert!(frame_flow(48.0, 1.0, &ModeOpts::new()).dots.len() <= 150);
    }

    #[test]
    fn reduced_motion_holds_still() {
        let r = opts(&[("effectReduced", 1.0)]);
        assert_eq!(frame_flow(200.0, 0.3, &r), frame_flow(200.0, 2.9, &r));
    }
}
