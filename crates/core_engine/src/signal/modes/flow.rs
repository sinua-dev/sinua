//! Signal / Flow (1.15, a light volume): a front of light sweeping across a sheet of points.
//! The front has a sharp leading edge and a trail that fades behind it, lifts the sheet into a
//! crest as it passes, and a fainter second front follows; ahead of the edge the sheet waits as
//! dust. The pace wanders with the noise. The shimmer's successor: something is being made.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): how strongly the fronts light the sheet. A bound voice feeds `audioLevel`
//!   instead; with neither it glows low and keeps sweeping.
//! - `churn` (0..1): restless work (thinking, processing): a stronger second front and a
//!   shimmer across the whole sheet. Its pace is the view's `speed`, not a key.
//!
//! The fronts' positions are closed-form in `t` (no state), and wrap off the sheet. Reduced
//! motion (`effectReduced`) holds the fronts and the camera still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const POINTS: usize = 800;
/// Seconds the second front trails the first.
const LAG: f64 = 1.6;
/// A front crosses `-SPAN..SPAN` (past the sheet's edges on both sides) in `1 / PACE` s.
const SPAN: f64 = 1.6;
const PACE: f64 = 0.2;
/// How far behind the edge the trail has faded to a third.
const TRAIL: f64 = 0.3;

/// Where a front is at `t`: a steady sweep that the noise speeds up and slows down.
fn front(t: f64) -> f64 {
    let phase = t * PACE + 0.06 * volume::noise(t * 0.4, 0.0, 5.0);
    -SPAN + 2.0 * SPAN * phase.rem_euclid(1.0)
}

/// How lit a point at `x` is by a front at `f`: nothing ahead of the edge but a hair of light,
/// the brightest line at the edge itself, then a trail that fades behind it.
fn lit(x: f64, f: f64) -> f64 {
    let d = x - f;
    let edge = (-(d / 0.045).powi(2)).exp();
    let trail = if d <= 0.0 { (d / TRAIL).exp() } else { 0.0 };
    0.55 * edge + 0.7 * trail
}

/// How far the front lifts the sheet around itself: a crest.
fn crest(x: f64, f: f64) -> f64 {
    (-((x - f) / 0.12).powi(2)).exp()
}

pub fn frame_flow(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let lv = volume::value(o, "level").unwrap_or(0.25);
    let budget = volume::budget(volume::view_px(o, size));
    let n = POINTS.min(budget).max(60);
    // A jittered grid, about 2.4 times as many columns (along the front's way) as rows.
    let cols = ((n as f64 * 2.4).sqrt().round() as usize).max(8);
    let rows = (n / cols).max(3);
    let cam = Camera::new(-0.45, 0.5);
    let mut light = Light::new(cam, size, size, volume::px_per_unit(o, size), cols * rows);
    let (f1, f2) = (front(t), front(t - LAG));
    let amp = 0.55 + 0.9 * lv;
    let second = 0.45 + 0.4 * churn;
    for r in 0..rows as i64 {
        for c in 0..cols as i64 {
            let jx = (volume::hash3(c, r, 1) - 0.5) * 0.7;
            let jz = (volume::hash3(c, r, 2) - 0.5) * 0.7;
            let x = -1.1 + 2.2 * (c as f64 + 0.5 + jx) / cols as f64;
            let z = -0.45 + 0.9 * (r as f64 + 0.5 + jz) / rows as f64;
            let g = lit(x, f1) + second * lit(x, f2);
            let y = 0.04 * volume::noise(x * 3.0, z * 3.0, t * 0.8)
                + 0.14 * crest(x, f1)
                + 0.07 * second * crest(x, f2);
            let shimmer =
                churn * 0.25 * (0.5 + 0.5 * volume::noise(x * 3.0 + 9.0, z * 5.0, t * 1.5));
            light.emit(
                x,
                y,
                z,
                0.09 + g * amp + shimmer,
                Tone::Ramp((x + 1.1) / 2.2),
                0.8 + g * 0.5,
            );
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
    fn the_front_has_a_sharp_edge_and_a_fading_trail() {
        let f = 0.0;
        // Ahead of the edge: next to nothing. At the edge: the brightest. Behind: fading.
        assert!(lit(0.15, f) < 0.05 * lit(0.0, f));
        assert!(lit(0.0, f) > lit(-0.2, f));
        let behind: Vec<f64> = (1..8).map(|k| lit(-0.15 * f64::from(k), f)).collect();
        assert!(behind.windows(2).all(|w| w[0] > w[1]), "{behind:?}");
        assert!(
            lit(-0.9, f) < 0.1 * lit(0.0, f),
            "the trail is gone well behind"
        );
    }

    #[test]
    fn the_front_lifts_the_sheet_into_a_crest() {
        assert!(crest(0.0, 0.0) > 0.99 && crest(0.6, 0.0) < 0.01);
    }

    #[test]
    fn it_flows_alone_and_lights_more_with_a_value_or_work() {
        let quiet = frame_flow(200.0, 1.0, &ModeOpts::new());
        assert!(quiet.dots.len() > 300, "a sheet, not a smear");
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
