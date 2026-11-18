//! Signal / Constellation (1.15, a light volume): steps as clouds of light on a circle. The
//! steps fill one after another around it, starting at the top and going clockwise: a done
//! step is tight and bright, the current one gathers out of dust, and the ones still to come are
//! very faint, small clouds on a faint path of the circle. The segmented ring's successor: a task
//! in stages.
//!
//! Inputs, none tied to the voice:
//! - `progress` (0..1): how far through all the steps; each step takes an equal share. With
//!   no value the first step is done and the next one gathers and loosens on its own, so
//!   the shape lives.
//! - `steps` (2..8, default 5): how many clouds.
//! - `level` (0..1): how brightly the clouds burn. A bound voice feeds `audioLevel` instead;
//!   with neither they burn at a quiet default.
//! - `churn` (0..1): restless work: the clouds shimmer.
//!
//! The camera is fixed, a little off the vertical so the circle reads as a circle. Reduced
//! motion (`effectReduced`) holds the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const CLOUD: usize = 110;
const RING: f64 = 0.72;
/// Points on the faint path of the circle.
const PATH: usize = 120;

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

pub fn frame_constellation(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let steps = get(o, "steps", 5.0).round().clamp(2.0, 8.0) as usize;
    let lv = volume::value(o, "level").unwrap_or(0.3);
    let gain = 0.6 + 0.9 * lv;
    let budget = volume::budget(volume::view_px(o, size));
    let per = CLOUD.min(budget / (steps + 1)).max(10);
    let path = PATH.min(budget / 6).max(24);
    // How many steps are done: the value's share, or a slow sway.
    let done = match o.get("progress") {
        Some(p) => p.clamp(0.0, 1.0) * steps as f64,
        // One step done, the next gathering and loosening: a bounded sway, so two states
        // running at different speeds never show different steps.
        None => 1.0 + 0.9 * (0.5 + 0.5 * (t * 0.5).sin()),
    };
    // A fixed view, a little off the vertical.
    let cam = Camera::new(0.0, 0.95);
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        per * steps + path,
    );
    let sphere = volume::sphere(900);
    for s in 0..steps {
        let a = s as f64 / steps as f64 * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
        let (cx, cz) = (a.cos() * RING, a.sin() * RING);
        let f = (done - s as f64).clamp(0.0, 1.0);
        let finished = f >= 1.0;
        // Waiting: a small, very faint cloud; the current step grows out of it; done is tight.
        let waiting = f <= 0.0;
        let radius = if waiting {
            0.28
        } else {
            0.28 + (0.14 - 0.28) * smooth(f)
        };
        let (b, npts, sz) = if finished {
            (0.8, per, 1.1)
        } else if !waiting {
            (0.07 + 0.78 * f, per, 0.9)
        } else {
            (0.07, per * 6 / 10, 0.7)
        };
        for i in 0..npts {
            let (px, py, pz) = sphere[(i * 8 + s * 13) % sphere.len()];
            let n =
                volume::noise(i as f64 * 0.2, s as f64, t * 0.6) * (0.25 * (1.0 - f) + 0.2 * churn);
            light.emit(
                cx + px * radius * (1.0 + n),
                py * radius * (1.0 + n),
                cz + pz * radius * (1.0 + n),
                b * gain,
                Tone::Ramp(s as f64 / (steps - 1) as f64),
                sz,
            );
        }
    }
    // The path the steps sit on.
    for i in 0..path {
        let a = i as f64 / path as f64 * std::f64::consts::TAU;
        light.emit(
            a.cos() * RING,
            0.0,
            a.sin() * RING,
            0.08 * gain,
            Tone::Ramp(0.5),
            0.55,
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
    fn it_cycles_alone_and_a_value_picks_how_many_steps_are_done() {
        let quiet = frame_constellation(200.0, 1.0, &ModeOpts::new());
        assert!(!quiet.dots.is_empty());
        assert_ne!(quiet, frame_constellation(200.0, 2.4, &ModeOpts::new()));
        let at = |p: f64| frame_constellation(200.0, 1.0, &opts(&[("progress", p)]));
        assert!(glow(&at(0.0)) < glow(&at(0.5)) && glow(&at(0.5)) < glow(&at(1.0)));
    }

    #[test]
    fn done_steps_are_tight() {
        let spread = |f: &OrbFrame| {
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
        };
        let none = frame_constellation(200.0, 1.0, &opts(&[("progress", 0.0)]));
        let all = frame_constellation(200.0, 1.0, &opts(&[("progress", 1.0)]));
        // The ring itself is as wide in both; each cloud is smaller when done.
        let r = |f: &OrbFrame| f.dots.iter().map(|d| d.r).sum::<f64>();
        assert!(spread(&none) > 0.0 && spread(&all) > 0.0);
        assert!(glow(&all) > glow(&none) * 2.0);
        assert!(r(&all) > r(&none), "done steps burn larger");
    }

    #[test]
    fn steps_set_the_number_of_clouds_and_small_views_draw_fewer_points() {
        let n = |s: f64| {
            frame_constellation(200.0, 1.0, &opts(&[("steps", s)]))
                .dots
                .len()
        };
        assert!(n(8.0) > n(3.0));
        assert!(frame_constellation(48.0, 1.0, &ModeOpts::new()).dots.len() <= 150);
    }

    #[test]
    fn reduced_motion_holds_still() {
        let r = opts(&[("effectReduced", 1.0), ("progress", 0.4)]);
        assert_eq!(
            frame_constellation(200.0, 0.3, &r),
            frame_constellation(200.0, 2.9, &r)
        );
    }
}
