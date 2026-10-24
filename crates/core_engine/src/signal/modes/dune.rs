//! Signal / Dune (1.15, a light volume): a dune of light. The recording's loudness is the
//! dune's height along it; the part already played glows, the rest is dim sand, and the
//! position is a plane of white light through the volume. The playback's successor.
//!
//! Inputs, the playback's contract, none tied to the voice:
//! - `envelope0..envelope{N-1}` (0..1, N ≤ 64): the dune's profile (a clip's loudness, or
//!   any series). Without one a fixed dune is drawn.
//! - `progress` (0..1): how much is done: a clip's position, or a long task's progress (the
//!   bright part is what's finished).
//! - `churn` (0..1): restless work: the sand shimmers.
//! - a bound voice: while it listens the sand ripples inward with the user's level, while it
//!   speaks outward with the agent's.
//! - `aspect` (1..8): a wide box (the view passes its ratio); the dune stretches with it.
//!   Past 2.2 (a voice message's strip) the camera turns to a side view and the dune fills
//!   the box, so [`seek_progress`] can map a touch to a position.
//!
//! With no input the playhead sweeps slowly across and back, so it still lives. Reduced motion holds
//! the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const MAX_ENVELOPE: usize = 64;
/// A strip's margin each side, in box heights (the old bar row's).
const MARGIN: f64 = 0.06;

/// How the dune sits in its box.
struct Shape {
    cam: Camera,
    /// Half-length along x.
    wide: f64,
    /// The base line (y), where the seek maps a touch, and the tallest sand above it.
    base: f64,
    height: f64,
    /// The unplayed sand's light, and the playhead plane's lowest point and step.
    dim: f64,
    plane: f64,
    step: f64,
}

/// The shape for a box `aspect` wide. Up to 2.2 it is the square one (a turned dune);
/// from 2.2 to 3 it eases into a strip (a voice message): a flatter side view that fills
/// the box, taller sand so the envelope reads at 40 px, and brighter unplayed sand.
fn shape(aspect: f64) -> Shape {
    let a = aspect.clamp(1.0, 8.0);
    let s = ((a - 2.2) / 0.8).clamp(0.0, 1.0);
    let s = s * s * (3.0 - 2.0 * s);
    let mix = |sq: f64, strip: f64| sq + (strip - sq) * s;
    let yaw = mix(-0.35, 0.0);
    let square = a.min(2.2) * 0.84;
    // Light's scale is 0.42 box heights per unit; fill to the margin, less the nearest
    // sand's perspective (the tallest, nearest points come out about 12 % larger).
    let fill = (a / 2.0 - MARGIN) / 0.42 / yaw.cos() / 1.12;
    Shape {
        cam: Camera::new(yaw, mix(0.55, 0.2)),
        wide: mix(square, fill),
        base: mix(-0.1, -0.9),
        height: mix(0.55, 1.75),
        dim: mix(0.13, 0.3),
        plane: mix(-0.15, -0.95),
        step: mix(0.04, 0.083),
    }
}

/// The position under a touch: `x` is the touch's distance from the box's left edge
/// divided by the box's height, for a box `aspect` wide. It inverts the dune's own base
/// line (the drawn projection), so a finger on a spot of the dune lands on that spot's
/// position; outside the dune it clamps to 0 or 1.
pub fn seek_progress(aspect: f64, x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    const SIZE: f64 = 64.0;
    let a = aspect.clamp(1.0, 8.0);
    let sh = shape(a);
    let l = Light::new(sh.cam, SIZE * a, SIZE, 1.0, 0);
    let screen = |f: f64| l.project((-1.0 + f * 2.0) * sh.wide, sh.base, 0.0).0 / SIZE;
    let (lo, hi) = (screen(0.0), screen(1.0));
    if x <= lo {
        return 0.0;
    }
    if x >= hi {
        return 1.0;
    }
    // The projection is monotonic along the line: bisect.
    let (mut a0, mut a1) = (0.0, 1.0);
    for _ in 0..40 {
        let m = (a0 + a1) / 2.0;
        if screen(m) < x {
            a0 = m;
        } else {
            a1 = m;
        }
    }
    (a0 + a1) / 2.0
}

/// The fixed dune drawn without an envelope (deterministic, the preview's look).
fn fixed(i: usize) -> f64 {
    let h = volume::noise(i as f64 * 0.37, 1.7, 3.1) * 0.5 + 0.5;
    let gap = if volume::noise(i as f64 * 0.9, 9.2, 0.4) > 0.75 {
        0.15
    } else {
        1.0
    };
    (0.15 + 0.85 * (i as f64 * 0.37 + h * 2.0).sin().abs() * gap).clamp(0.0, 1.0)
}

pub fn frame_dune(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let tn = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let aspect = get(o, "aspect", 1.0).clamp(1.0, 8.0);
    let env: Vec<f64> = (0..MAX_ENVELOPE)
        .map_while(|i| o.get(format!("envelope{i}").as_str()).copied())
        .map(|v| v.clamp(0.0, 1.0))
        .collect();
    // No progress: a slow sweep across and back, so the shape lives on its own. It never
    // wraps: a playhead that jumped from the end back to the start would be a cut at any
    // session time (the transition contract measures it).
    let head = match o.get("progress") {
        Some(p) => p.clamp(0.0, 1.0),
        None if still => 0.4,
        None => 0.5 - 0.5 * (t * 0.12 * std::f64::consts::PI / 1.15).cos(),
    };
    let along_full = 90;
    let stack_full = 8;
    let budget = volume::budget(volume::view_px(o, size) * aspect.sqrt());
    let along = volume::fit(along_full, along_full * stack_full, budget, 2);
    let stack = volume::fit(stack_full, along_full * stack_full, budget, 2).max(3);
    let sh = shape(aspect);
    let (cam, wide) = (sh.cam, sh.wide);
    let mut light = Light::new(
        cam,
        size * aspect,
        size,
        volume::px_per_unit(o, size),
        along * stack + 24,
    );
    // The voice states: a listening or speaking voice ripples the sand (inward to listen,
    // outward to speak); thinking makes it shimmer harder (`churn`).
    let code = o.get("voiceStateCode").map(|c| c.round() as i64);
    let ripple_dir = match code {
        Some(2) => -1.0,
        _ => 1.0,
    };
    let ripple = if volume::voice_audio(o) && code.is_some() {
        o.get("audioLevel").copied().unwrap_or(0.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for i in 0..along {
        let fi = i as f64 / (along - 1) as f64;
        let x = (-1.0 + fi * 2.0) * wide;
        let a = if env.is_empty() {
            fixed((fi * 89.0).round() as usize)
        } else {
            env[((fi * env.len() as f64) as usize).min(env.len() - 1)]
        } * sh.height;
        let played = fi < head;
        for k in 0..stack {
            let fk = k as f64 / (stack - 1) as f64;
            let wave = ripple * 0.12 * fk * (fi * 26.0 - ripple_dir * tn * 7.0).sin();
            let y = fk * a + sh.base + wave;
            let z = volume::noise(fi * 89.0 * 0.3, k as f64, tn * 0.4) * 0.12
                + if k % 2 == 1 { 0.06 } else { -0.06 };
            let shimmer = churn * 0.4 * (0.5 + 0.5 * volume::noise(fi * 20.0, fk * 4.0, tn * 2.5));
            let (b, tone) = if played {
                (
                    0.55 + 0.5 * fk + shimmer * 0.6 + wave.abs() * 2.0,
                    Tone::Ramp(0.0),
                )
            } else {
                (sh.dim + shimmer + wave.abs() * 2.0, Tone::Ramp(0.85))
            };
            light.emit(x, y, z, b, tone, 0.9);
        }
    }
    // The playhead: a plane of white light through the dune.
    let px = (-1.0 + head.min(1.0) * 2.0) * wide;
    for k in 0..24 {
        let y = sh.plane + k as f64 * sh.step;
        light.emit(
            px,
            y,
            volume::noise(k as f64, tn * 2.0, 0.0) * 0.15,
            0.7,
            Tone::White,
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

    fn bright(f: &OrbFrame) -> usize {
        f.dots
            .iter()
            .filter(|d| d.a > 0.35 && d.saturation < 1.0)
            .count()
    }

    #[test]
    fn progress_lights_what_is_done() {
        let a = frame_dune(200.0, 1.0, &opts(&[("progress", 0.2)]));
        let b = frame_dune(200.0, 1.0, &opts(&[("progress", 0.8)]));
        assert!(bright(&b) > bright(&a) * 2);
    }

    #[test]
    fn an_envelope_shapes_the_dune() {
        let mut flat = opts(&[("progress", 1.0)]);
        let mut tall = opts(&[("progress", 1.0)]);
        for i in 0..32 {
            flat.insert(format!("envelope{i}"), 0.1);
            tall.insert(format!("envelope{i}"), 1.0);
        }
        // The sand's top (the playhead's white plane left out).
        let top = |f: &OrbFrame| {
            f.dots
                .iter()
                .filter(|d| d.white < 0.85)
                .map(|d| d.y)
                .fold(f64::MAX, f64::min)
        };
        assert!(top(&frame_dune(200.0, 1.0, &tall)) < top(&frame_dune(200.0, 1.0, &flat)) - 20.0);
    }

    #[test]
    fn it_lives_without_input() {
        assert_ne!(
            frame_dune(200.0, 1.0, &ModeOpts::new()),
            frame_dune(200.0, 4.0, &ModeOpts::new())
        );
    }

    #[test]
    fn a_strip_fills_its_box_and_a_touch_lands_where_it_points() {
        // A voice message: 220 × 40 → aspect 5.5, engine at 64.
        let o = opts(&[("aspect", 5.5), ("progress", 0.3)]);
        let f = frame_dune(64.0, 1.0, &o);
        let (lo, hi) = f
            .dots
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), d| (a.min(d.x), b.max(d.x)));
        assert!(lo < 64.0 * 0.5 && hi > 64.0 * 5.0, "{lo}..{hi}");
        // The playhead (white) sits where the seek says progress 0.3 is.
        let head: Vec<f64> = f
            .dots
            .iter()
            .filter(|d| d.white > 0.85)
            .map(|d| d.x)
            .collect();
        let mid = head.iter().sum::<f64>() / head.len() as f64 / 64.0;
        assert!(
            (seek_progress(5.5, mid) - 0.3).abs() < 0.02,
            "{}",
            seek_progress(5.5, mid)
        );
        assert_eq!(seek_progress(5.5, 0.0), 0.0);
        assert_eq!(seek_progress(5.5, 5.5), 1.0);
        assert_eq!(seek_progress(5.5, f64::NAN), 0.0);
        // Monotonic across the strip.
        let mut last = -1.0;
        for i in 0..=55 {
            let p = seek_progress(5.5, i as f64 * 0.1);
            assert!(p >= last);
            last = p;
        }
    }

    #[test]
    fn a_given_progress_holds_the_playhead_still() {
        let o = opts(&[("aspect", 5.5), ("progress", 0.5)]);
        let x = |t: f64| {
            let f = frame_dune(64.0, t, &o);
            let h: Vec<f64> = f
                .dots
                .iter()
                .filter(|d| d.white > 0.85)
                .map(|d| d.x)
                .collect();
            h.iter().sum::<f64>() / h.len() as f64
        };
        assert!((x(1.0) - x(9.0)).abs() < 0.5);
    }

    #[test]
    fn a_wide_box_stretches_it() {
        let w = frame_dune(100.0, 1.0, &opts(&[("aspect", 4.0), ("progress", 0.5)]));
        let x_max = w.dots.iter().map(|d| d.x).fold(f64::MIN, f64::max);
        assert!(x_max > 250.0, "{x_max}");
    }
}
