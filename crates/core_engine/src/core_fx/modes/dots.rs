//! Core / Dots: the typing indicator (`typing`) -- three dots bouncing in
//! sequence, the chat-UI cue for "a reply is being produced". The pattern,
//! as every shipped CSS version of it (iMessage/Messenger clones on
//! CodePen, CodeFronts) states it: **identical animation with offset start
//! times** -- one `translateY` bounce, `ease-in-out`, and per-dot delays of
//! 0 / 0.2 / 0.4s. Each dot rises for the first half of its period and
//! rests on the baseline for the second, brightening slightly at the top so
//! the wave reads on a low-contrast surface too.

use crate::primitives::{cubic_bezier, finalize_frame, Dot, ModeOpts, OrbFrame};
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Height of dot `k`'s bounce (`0..1`) at time `t`: a half-sine over the
/// first half of the period (eased), rest for the second half, delayed by
/// `k * delay`.
pub fn bounce(t: f64, k: usize, period: f64, delay: f64) -> f64 {
    let p = ((t - k as f64 * delay) / period).rem_euclid(1.0);
    if p < 0.5 {
        (PI * cubic_bezier(0.42, 0.0, 0.58, 1.0, p / 0.5)).sin()
    } else {
        0.0
    }
}

pub fn frame_dots(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let count = get(o, "dotCount", 3.0).clamp(1.0, 6.0) as usize;
    let period = get(o, "period", 1.2).max(0.05);
    let delay = get(o, "delay", 0.2).max(0.0);
    let dot_r = size * get(o, "dotSize", 0.06).clamp(0.01, 0.3);
    let spacing = size * get(o, "spacing", 0.22).clamp(0.02, 0.5);
    let amp = size * get(o, "bounceAmplitude", 0.12).clamp(0.0, 0.5);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);

    let mut dots: Vec<Dot> = Vec::with_capacity(count);
    for k in 0..count {
        let h = bounce(t, k, period, delay);
        dots.push(Dot {
            x: cx + (k as f64 - (count as f64 - 1.0) * 0.5) * spacing,
            y: cy - amp * h,
            z: 0.0,
            r: dot_r,
            white: 0.15,
            a: 0.6 + 0.35 * h,
            saturation,
            hue,
        });
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    fn base() -> ModeOpts {
        opts(&[("period", 1.2), ("delay", 0.2), ("rMin", 0.3)])
    }

    #[test]
    fn three_dots_on_one_baseline_with_even_spacing() {
        let f = frame_dots(64.0, 10.0, &base()); // t deep in the cycle, arbitrary
        assert_eq!(f.dots.len(), 3);
        // sort by x since finalize_frame z-sorts (all z = 0, stable) -- order is preserved
        let xs: Vec<f64> = f.dots.iter().map(|d| d.x).collect();
        assert!((xs[1] - xs[0] - (xs[2] - xs[1])).abs() < 1e-9);
        assert!((xs[1] - 32.0).abs() < 1e-9, "centered");
    }

    #[test]
    fn each_dot_peaks_one_delay_after_the_previous() {
        // dot 0 peaks at a quarter period; dot 1 at that plus `delay`, etc.
        let peak0 = 0.3;
        assert!((bounce(peak0, 0, 1.2, 0.2) - 1.0).abs() < 1e-9);
        assert!((bounce(peak0 + 0.2, 1, 1.2, 0.2) - 1.0).abs() < 1e-9);
        assert!((bounce(peak0 + 0.4, 2, 1.2, 0.2) - 1.0).abs() < 1e-9);
        assert!(
            bounce(peak0 + 0.2, 1, 1.2, 0.2) > bounce(peak0, 1, 1.2, 0.2),
            "dot 1 is still on its way up when dot 0 peaks"
        );
    }

    #[test]
    fn dots_rise_by_the_bounce_amplitude_and_rest_on_the_baseline() {
        let peak = frame_dots(64.0, 0.3, &base());
        assert!(
            (peak.dots[0].y - (32.0 - 64.0 * 0.12)).abs() < 1e-9,
            "dot 0 at the top of its bounce"
        );
        assert!(
            peak.dots[0].a > peak.dots[2].a,
            "the rising dot is brighter than a resting one"
        );
        // second half of dot 0's period: resting
        let rest = frame_dots(64.0, 0.9, &base());
        assert!((rest.dots[0].y - 32.0).abs() < 1e-9);
        assert_eq!(frame_dots(64.0, 0.9, &base()), rest, "deterministic");
    }
}
