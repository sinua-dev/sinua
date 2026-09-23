//! Spectrum: a discrete audio-EQ ring for the `speaking` state. NOT a port
//! -- see `docs/effects-research.md`'s "Spectrum bars" proposal, which this
//! implements: fixed angular columns whose height jumps to an independent
//! value on a fixed cadence (a classic circular-equalizer look), unlike
//! `wave`'s smooth continuous rolling waveform. `listening` already covers
//! receiving input; nothing covered "the agent is talking right now" before
//! this. No golden vector -- same tradeoff as `aurora`/`webflow`, see their
//! header comments.

use crate::orbs::core::{audio_band, finalize_frame, hash_d, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_spectrum(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.15, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let bars = get(o, "barCount", 24.0) as i64;
    let bar_dots = get(o, "barDotCount", 6.0) as i64;
    let jump_rate = get(o, "jumpSpeed", 4.0);
    let dot_r = get(o, "dotSize", 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.6);

    // Quantized time step, not continuous `t` -- this is what makes bars
    // jump percussively to a new height instead of drifting smoothly the
    // way every noise-driven mode does. Only used for the fallback
    // (no-audio-connected) pattern below.
    let step = (t * jump_rate).floor();

    // If a caller (e.g. the Studio's VoiceSource pipeline) has supplied
    // real audio band data, use it instead of the synthetic hash-driven
    // pattern -- `audioBandCount` is the caller's explicit signal that real
    // data is present, not just `audioBand0`'s absence/presence, since `0`
    // is itself a valid band reading (silence).
    let audio_band_count = get(o, "audioBandCount", 0.0) as usize;

    let mut dots: Vec<Dot> = Vec::with_capacity((bars * bar_dots) as usize);
    for i in 0..bars {
        let i_f = i as f64;
        let ang = i_f / bars as f64 * 2.0 * PI;
        let raw = if audio_band_count > 0 {
            let band_idx = ((i_f / bars as f64) * audio_band_count as f64) as usize;
            audio_band(o, band_idx.min(audio_band_count - 1))
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        } else {
            hash_d(i_f * 7.31 + 1.0, step)
        };
        // Never fully zero -- a silent band still reads as "present," not
        // "broken" (the same floor trick real bar-visualizer
        // implementations use, confirmed independently in both LiveKit's
        // and ElevenLabs' shipped code -- see docs/effects-research.md).
        let bar_h = 0.15 + 0.85 * raw;
        let (dx, dy) = (ang.cos(), ang.sin());

        let n = bar_dots.max(1);
        for k in 0..n {
            let frac_along = k as f64 / (n as f64 - 1.0).max(1.0);
            if frac_along > bar_h {
                continue;
            }
            let radius_here = r * (0.35 + 0.65 * frac_along);
            let (px, py, z) = pt.project(dx * radius_here, dy * radius_here, 0.0);
            let depth = (z / r + 1.0) / 2.0;
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: dot_r * rs * (1.0 - 0.3 * frac_along),
                white: 0.25 + 0.35 * depth,
                a: 0.55 + 0.45 * depth,
                saturation,
                hue,
            });
        }
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    #[test]
    fn bars_change_height_between_steps() {
        let o = opts(&[
            ("barCount", 12.0),
            ("barDotCount", 6.0),
            ("jumpSpeed", 4.0),
            ("rMin", 0.3),
        ]);
        let a = frame_spectrum(64.0, 0.0, &o);
        let b = frame_spectrum(64.0, 0.5, &o); // one full jumpSpeed step later
        assert_ne!(
            a.dots.len(),
            b.dots.len(),
            "expected a different total dot count after a discrete jump"
        );
    }

    #[test]
    fn within_one_step_height_is_stable() {
        let o = opts(&[
            ("barCount", 12.0),
            ("barDotCount", 6.0),
            ("jumpSpeed", 4.0),
            ("rMin", 0.3),
        ]);
        let a = frame_spectrum(64.0, 0.01, &o);
        let b = frame_spectrum(64.0, 0.20, &o); // still within the same 0.25s step
        assert_eq!(
            a.dots.len(),
            b.dots.len(),
            "expected the same bar heights within one step"
        );
    }

    #[test]
    fn real_audio_bands_override_the_synthetic_pattern() {
        let mut silent = opts(&[
            ("barCount", 4.0),
            ("barDotCount", 6.0),
            ("rMin", 0.3),
            ("audioBandCount", 4.0),
        ]);
        for i in 0..4 {
            silent.insert(format!("audioBand{i}"), 0.0);
        }
        let mut loud = silent.clone();
        for i in 0..4 {
            loud.insert(format!("audioBand{i}"), 1.0);
        }

        let quiet_frame = frame_spectrum(64.0, 1.0, &silent);
        let loud_frame = frame_spectrum(64.0, 1.0, &loud);
        assert!(
            loud_frame.dots.len() > quiet_frame.dots.len(),
            "loud bands ({}) should produce taller bars than silent ones ({})",
            loud_frame.dots.len(),
            quiet_frame.dots.len()
        );

        // Still never fully collapses to nothing even at 0 -- the floor trick.
        assert!(
            !quiet_frame.dots.is_empty(),
            "a silent band should still render a small bar, not nothing"
        );

        // t changing must NOT change the result when driven by real audio
        // (unlike the synthetic path, which jumps on a `jumpSpeed` timer).
        let quiet_later = frame_spectrum(64.0, 5.0, &silent);
        assert_eq!(quiet_frame.dots.len(), quiet_later.dots.len());
    }
}
