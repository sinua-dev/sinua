//! Signal / Waveform: a continuous oscilloscope-style trace -- the second
//! style of the `signal` family (see `signal/mod.rs`'s header for why this
//! is a sibling family to `orbs`, not a mode inside it). Built after `bar`
//! (discrete EQ columns) specifically because discrete bars don't read as
//! "a live waveform" -- this is a signed curve crossing a centerline,
//! matching the sine-wave/jagged-trace family of real audio visualizer
//! icons (as opposed to `bar`'s bar-chart family).
//!
//! Each trace is one `Polyline` (`crate::primitives::Polyline`): the first
//! version emitted ~48 consecutive `Line` segments instead, and every
//! renderer stroked each one as its own butt-capped path, which left a
//! wedge gap on the outside of every bend and a double-painted notch on
//! the inside -- a visibly segmented "caterpillar" at any point count.
//! That renderer-level seam is why `Polyline` exists at all; see its doc
//! comment for the paint contract (one stroke, round caps, round joins).
//!
//! **Everything here is a sum of smooth sines, on purpose.** The curve is
//! analytic, so smoothness needs no spline or Bezier -- just enough samples
//! (`pointCount`, one `sin` each). What *does* produce visible corners is a
//! kink in the math itself, and one shipped briefly: an earlier `speaking`
//! used the audio spectrum as the trace's *spatial shape* (band magnitude
//! at `|x|`, mirrored about the center). `|x|` has a cusp at zero, so every
//! frame had a sharp V at the center, and a spectrum-shaped envelope reads
//! as a single spiky bump rather than a wave. No renderer smoothing can
//! hide a real corner; the fix was to keep the shape smooth and let audio
//! drive *amplitude* instead (see `speaking` below).
//!
//! Several other things are deliberate, for the record:
//! - **Layers.** `layerCount` traces are emitted back-to-front: faint,
//!   lower-amplitude companions with their own spatial frequencies and
//!   phases first, the main trace last and strongest. Overlapping
//!   translucent traces are what make the Siri-style waveform read as
//!   "rich" rather than "a line" -- one trace alone reads as a plot.
//! - **Edge envelope** is Siri's attenuation `(K / (K + u^4))^K` (K = 4,
//!   `u = 1.7 * x_norm`), not the semicircle `sqrt(1 - x^2)` the first
//!   version used: the semicircle has a vertical tangent at the ends, so
//!   the trace dropped abruptly at the edges. This bell is ~1 across the
//!   middle third and fades to ~0 at the margins, in every state --
//!   including `Speaking`, which previously had no envelope at all.
//! - **Stroke width is proportional to `size`** (`lineWidth`), not an
//!   absolute pixel value -- an absolute 1.6 was a hairline at 64 and 8% of
//!   the canvas at 20.
//!
//! Two independent things drive each trace, same split as `bar.rs`:
//! - `Speaking`: real per-band audio (`audioBand0..N`, via the shared
//!   `audio_band()`) sets each layer's *amplitude*: the bands are split into
//!   `layerCount` contiguous groups, low to high, and layer `k`'s drive is a
//!   blend of the overall level (so all layers move together) and its own
//!   group's level (so bass and treble visibly pull different layers). The
//!   shape under that amplitude is a two-term traveling sine per layer, plus
//!   a small share of the synthetic shimmer below so a pause between words
//!   reads as "present," not dead-flat (`bar.rs`'s floor trick, applied to
//!   motion instead of height).
//! - Every other state: a synthetic idle motion (2 offset sine terms plus
//!   `perlin3` jitter) -- the same "summed sines, center-weighted" recipe
//!   that showed up independently in both LiveKit's and ElevenLabs'
//!   shipped idle/processing states (see `docs/effects-research.md`).
//!   Amplitude/speed/frequency scale up through `Idle < Connecting <
//!   Listening < Thinking` so the trace visibly gets "busier" the closer
//!   the session is to actually producing audio. No golden vector -- same
//!   tradeoff as every additive `orbs`/`signal` mode.

use crate::primitives::{audio_band, finalize_frame, perlin3, ModeOpts, OrbFrame, Point, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Mirrors the TS `AgentState` union (the Web Studio's `audio/types.ts`) --
/// identical encoding to `signal::modes::bar::VoiceState`, kept as its own
/// copy rather than shared, since the two modes' state-driven behavior
/// (sequencer highlighting vs. amplitude/speed curves) has nothing else in
/// common to factor out.
#[derive(Clone, Copy, PartialEq)]
enum VoiceState {
    Idle,
    Connecting,
    Listening,
    Thinking,
    Speaking,
}

fn voice_state(o: &ModeOpts) -> VoiceState {
    match get(o, "voiceStateCode", 0.0) as i64 {
        1 => VoiceState::Connecting,
        2 => VoiceState::Listening,
        3 => VoiceState::Thinking,
        4 => VoiceState::Speaking,
        _ => VoiceState::Idle,
    }
}

/// `(amplitude fraction, speed, spatial frequency)` for the synthetic
/// motion -- increasing across `Idle < Connecting < Listening < Thinking`
/// so the trace reads as progressively "busier" the closer the session is
/// to actually producing audio. `Speaking`'s entry is the *shimmer floor*
/// that runs underneath the real-audio term (see `frame_waveform`), not a
/// standalone look.
fn synthetic_params(state: VoiceState) -> (f64, f64, f64) {
    match state {
        VoiceState::Idle => (0.15, 0.5, 1.0),
        VoiceState::Connecting => (0.28, 1.0, 1.4),
        VoiceState::Listening => (0.35, 0.9, 1.2),
        VoiceState::Thinking => (0.5, 2.0, 2.0),
        VoiceState::Speaking => (0.10, 0.9, 1.2),
    }
}

/// Siri-style edge attenuation: `(K / (K + u^4))^K` with `u = 1.7 * x_norm`
/// (`x_norm` in `-1..1`). ~1 across the middle third, ~0.6 halfway out,
/// ~0.01 at the margins -- a bell with soft tails, unlike a semicircle's
/// vertical drop at the ends. (Siri's own reference uses `u = 2 * x`; the
/// slightly wider bell here lets the motion use more of a narrow strip.)
fn envelope(x_norm: f64) -> f64 {
    const K: f64 = 4.0;
    let u = x_norm * 1.7;
    (K / (K + u.powi(4))).powf(K)
}

/// Mean of the supplied bands over `range` (clamped to what exists), `0`
/// when there are none.
fn band_mean(o: &ModeOpts, band_count: usize, range: std::ops::Range<usize>) -> f64 {
    let end = range.end.min(band_count);
    if band_count == 0 || range.start >= end {
        return 0.0;
    }
    let sum: f64 = (range.start..end)
        .map(|i| audio_band(o, i).unwrap_or(0.0).clamp(0.0, 1.0))
        .sum();
    sum / (end - range.start) as f64
}

/// How hard real audio drives layer `k` of `layerCount` (`0..1`): 60% overall
/// level (every layer breathes with the voice as a whole) + 40% the mean of
/// this layer's own contiguous band group, low bands to the main trace
/// (`k = 0`), higher bands to the companions -- so bass and sibilance
/// visibly pull different layers instead of all layers being one scaled
/// copy of each other. With fewer bands than layers, groups collapse to
/// single bands and the last layers share the top one.
fn layer_drive(o: &ModeOpts, band_count: usize, layers: usize, k: usize) -> f64 {
    if band_count == 0 {
        return 0.0;
    }
    let overall = band_mean(o, band_count, 0..band_count);
    let group = (band_count as f64 / layers as f64).max(1.0);
    let start = ((k as f64 * group) as usize).min(band_count - 1);
    let end = (((k + 1) as f64 * group).ceil() as usize).clamp(start + 1, band_count);
    0.6 * overall + 0.4 * band_mean(o, band_count, start..end)
}

pub fn frame_waveform(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let point_count = get(o, "pointCount", 96.0).max(2.0) as usize;
    let layers = get(o, "layerCount", 3.0).clamp(1.0, 6.0) as usize;
    let line_w = size * get(o, "lineWidth", 0.035).max(0.005);
    let max_amp = size * get(o, "amplitude", 0.26).max(0.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);

    let margin = size * 0.08;
    let usable_w = (size - margin * 2.0).max(1.0);
    let centerline_y = size * 0.5;

    let state = voice_state(o);
    let speaking = state == VoiceState::Speaking;
    let audio_band_count = get(o, "audioBandCount", 0.0) as usize;
    let (syn_amp, syn_speed, syn_freq) = synthetic_params(state);
    let (white, base_alpha) = if speaking { (0.08, 0.95) } else { (0.45, 0.7) };

    let mut polylines: Vec<Polyline> = Vec::with_capacity(layers);
    // Back to front: the faintest companion first, the main trace (k = 0)
    // last, so it lands on top.
    for k in (0..layers).rev() {
        let kf = k as f64;
        let ki = k as i32;
        // Each companion is the same curve family with its own phase and a
        // slightly different spatial frequency, so the traces cross and
        // overlap rather than sit parallel.
        let phase = kf * 1.1;
        let amp_k = 0.72f64.powi(ki);
        let alpha_k = if k == 0 {
            base_alpha
        } else {
            base_alpha * 0.38 * 0.8f64.powi(ki - 1)
        };
        let w_k = if k == 0 { line_w } else { line_w * 0.8 };
        // A slight per-layer hue drift only shows when `saturation > 0`
        // (grayscale ink ignores `hue` entirely).
        let hue_k = (hue + kf * 14.0).rem_euclid(360.0);
        let drive = if speaking {
            layer_drive(o, audio_band_count, layers, k)
        } else {
            0.0
        };

        let mut points: Vec<Point> = Vec::with_capacity(point_count);
        for i in 0..point_count {
            let x01 = i as f64 / (point_count - 1) as f64;
            let x_norm = x01 * 2.0 - 1.0; // -1..1, centered
            let x = margin + usable_w * x01;
            let env = envelope(x_norm);

            let s1 = (t * syn_speed * 1.5 + x_norm * syn_freq * 3.0 + phase).sin() * 0.5;
            let s2 = (t * syn_speed * 0.8 - x_norm * syn_freq * 2.0 - phase * 0.6).sin() * 0.3;
            let jitter = perlin3(x_norm * 2.0 + t * 0.3, t * 0.2 + kf * 0.37, 0.0) * 0.2;
            let shimmer = syn_amp * (s1 + s2 + jitter);

            let signal = if speaking {
                // Two traveling sines per layer: ~1.5 and ~1 crests across
                // the visible bell, moving in opposite directions, so the
                // trace rolls and folds instead of scrolling. Only the
                // amplitude (`drive`) comes from audio -- the shape stays
                // smooth by construction.
                let c1 = (x_norm * (9.0 + 1.3 * kf) + t * 2.6 + phase).sin() * 0.6;
                let c2 = (x_norm * (5.5 - 0.8 * kf) - t * 1.9 + phase * 0.7).sin() * 0.4;
                drive * (c1 + c2) + shimmer
            } else {
                shimmer
            };

            points.push(Point {
                x,
                y: centerline_y + env * max_amp * amp_k * signal,
            });
        }

        polylines.push(Polyline {
            points,
            white,
            a: alpha_k,
            w: w_k,
            saturation,
            hue: hue_k,
            hues: Vec::new(),
        });
    }

    // No dots or lines at all -- pure stroked paths. `rMin` is irrelevant
    // with no dots to clamp; passed through for uniformity with every
    // other mode's `finalize_frame` call.
    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    const SIZE: f64 = 64.0;
    const CENTER: f64 = SIZE / 2.0;

    fn base_opts() -> ModeOpts {
        opts(&[("pointCount", 24.0), ("layerCount", 3.0), ("rMin", 0.3)])
    }

    /// The main trace is the last polyline (drawn on top).
    fn main_trace(frame: &OrbFrame) -> &Polyline {
        frame.polylines.last().expect("at least one trace")
    }

    fn trace_energy(p: &Polyline) -> f64 {
        p.points.iter().map(|pt| (pt.y - CENTER).abs()).sum::<f64>()
    }

    /// Total absolute vertical displacement of one polyline (by index into
    /// the frame's draw order), summed over several `t` samples -- a noisy
    /// synthetic signal can coincide at any single instant, so energy is
    /// compared as a *typical* value.
    fn energy_of(o: &ModeOpts, index: usize, samples: usize, dt: f64) -> f64 {
        (0..samples)
            .map(|i| trace_energy(&frame_waveform(SIZE, i as f64 * dt, o).polylines[index]))
            .sum::<f64>()
    }

    fn energy(o: &ModeOpts, samples: usize, dt: f64) -> f64 {
        let last = get(o, "layerCount", 3.0) as usize - 1;
        energy_of(o, last, samples, dt)
    }

    #[test]
    fn produces_one_continuous_trace_per_layer_for_every_state() {
        for code in 0..5 {
            let mut o = base_opts();
            o.insert("voiceStateCode".to_string(), code as f64);
            let frame = frame_waveform(SIZE, 1.23, &o);
            assert_eq!(
                frame.polylines.len(),
                3,
                "voiceStateCode={code} should emit one polyline per layer"
            );
            assert!(
                frame.dots.is_empty() && frame.lines.is_empty(),
                "waveform is pure polylines, expected no dots/lines"
            );
            for p in &frame.polylines {
                assert_eq!(
                    p.points.len(),
                    24,
                    "every layer samples the full point count"
                );
                for w in p.points.windows(2) {
                    assert!(
                        w[1].x > w[0].x,
                        "a trace must advance left to right, never fold back"
                    );
                }
            }
        }
    }

    #[test]
    fn main_trace_draws_last_and_strongest() {
        let frame = frame_waveform(SIZE, 0.8, &base_opts());
        let main = main_trace(&frame);
        for companion in &frame.polylines[..frame.polylines.len() - 1] {
            assert!(
                companion.a < main.a,
                "companion layers must be fainter than the main trace"
            );
            assert!(
                companion.w < main.w,
                "companion layers must be thinner than the main trace"
            );
        }
    }

    #[test]
    fn stroke_width_scales_with_size() {
        let at_64 = main_trace(&frame_waveform(64.0, 0.0, &base_opts())).w;
        let at_32 = main_trace(&frame_waveform(32.0, 0.0, &base_opts())).w;
        assert!(
            (at_64 - at_32 * 2.0).abs() < 1e-9,
            "width should be proportional to size: {at_64} vs {at_32}"
        );
    }

    #[test]
    fn idle_trace_is_not_flat() {
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 0.0);
        let frame = frame_waveform(SIZE, 2.5, &o);
        let mid = &main_trace(&frame).points[12];
        assert!(
            (mid.y - CENTER).abs() > 0.01,
            "expected idle to visibly move off the flat centerline, not freeze"
        );
    }

    #[test]
    fn edges_taper_toward_the_centerline() {
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 3.0); // thinking: the most energetic synthetic state
        o.insert("pointCount".to_string(), 40.0);
        let (mut edge, mut center) = (0.0, 0.0);
        for i in 0..20 {
            let frame = frame_waveform(SIZE, i as f64 * 0.31, &o);
            let pts = &main_trace(&frame).points;
            // outer 10% of samples on each side vs. the middle 20%
            edge += pts[..4]
                .iter()
                .chain(pts[36..].iter())
                .map(|p| (p.y - CENTER).abs())
                .sum::<f64>()
                / 8.0;
            center += pts[16..24]
                .iter()
                .map(|p| (p.y - CENTER).abs())
                .sum::<f64>()
                / 8.0;
        }
        assert!(
            edge < center * 0.1,
            "edges should be nearly flat relative to the center: edge={edge} center={center}"
        );
    }

    #[test]
    fn thinking_has_more_energy_than_idle() {
        let sample = |code: f64| -> f64 {
            let mut o = base_opts();
            o.insert("voiceStateCode".to_string(), code);
            energy(&o, 20, 0.37)
        };
        assert!(
            sample(3.0) > sample(0.0),
            "expected thinking to be visibly more energetic than idle"
        );
    }

    #[test]
    fn speaking_responds_to_real_band_magnitude() {
        let mut loud = base_opts();
        loud.insert("voiceStateCode".to_string(), 4.0);
        loud.insert("audioBandCount".to_string(), 3.0);
        for i in 0..3 {
            loud.insert(format!("audioBand{i}"), 1.0);
        }
        let mut quiet = loud.clone();
        for i in 0..3 {
            quiet.insert(format!("audioBand{i}"), 0.0);
        }
        assert!(
            energy(&loud, 10, 0.13) > energy(&quiet, 10, 0.13),
            "loud bands should move the trace more than silence"
        );
        assert!(
            energy(&quiet, 10, 0.13) > 0.0,
            "silence must keep a visible shimmer floor, not a dead-flat line"
        );
    }

    #[test]
    fn speaking_layers_follow_their_own_band_group() {
        // 2 layers, 4 bands: bands 0-1 belong to the main trace (drawn
        // last), bands 2-3 to the companion (drawn first). A loud low group
        // should move the main trace more than a loud high group does, and
        // vice versa for the companion.
        let with_bands = |vals: [f64; 4]| -> ModeOpts {
            let mut o = base_opts();
            o.insert("voiceStateCode".to_string(), 4.0);
            o.insert("layerCount".to_string(), 2.0);
            o.insert("audioBandCount".to_string(), 4.0);
            for (i, v) in vals.iter().enumerate() {
                o.insert(format!("audioBand{i}"), *v);
            }
            o
        };
        let low_loud = with_bands([1.0, 1.0, 0.0, 0.0]);
        let high_loud = with_bands([0.0, 0.0, 1.0, 1.0]);
        // index 1 = main trace (last of 2), index 0 = companion
        assert!(
            energy_of(&low_loud, 1, 12, 0.17) > energy_of(&high_loud, 1, 12, 0.17),
            "low bands should drive the main trace harder than high bands"
        );
        assert!(
            energy_of(&high_loud, 0, 12, 0.17) > energy_of(&low_loud, 0, 12, 0.17),
            "high bands should drive the companion harder than low bands"
        );
    }

    #[test]
    fn speaking_trace_has_no_corner_at_the_center() {
        // Regression for the `|x|` cusp: the turn angle between consecutive
        // segments around the center sample must stay small -- a kink shows
        // up as one angle far larger than its neighbors.
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 4.0);
        o.insert("layerCount".to_string(), 1.0);
        o.insert("pointCount".to_string(), 97.0);
        o.insert("audioBandCount".to_string(), 8.0);
        for (i, v) in [0.85, 0.7, 0.55, 0.4, 0.3, 0.2, 0.15, 0.1]
            .iter()
            .enumerate()
        {
            o.insert(format!("audioBand{i}"), *v);
        }
        for step in 0..10 {
            let frame = frame_waveform(SIZE, step as f64 * 0.23, &o);
            let pts = &main_trace(&frame).points;
            let angle_at = |i: usize| -> f64 {
                let a = (pts[i].y - pts[i - 1].y).atan2(pts[i].x - pts[i - 1].x);
                let b = (pts[i + 1].y - pts[i].y).atan2(pts[i + 1].x - pts[i].x);
                (b - a).abs()
            };
            let center = angle_at(48);
            let neighbors = (angle_at(44) + angle_at(46) + angle_at(50) + angle_at(52)) / 4.0;
            assert!(
                center < 0.35 && center < neighbors * 3.0 + 0.05,
                "t-step {step}: center turn {center:.3} rad vs. neighbors {neighbors:.3} -- a cusp"
            );
        }
    }
}
