//! Signal / Scroll: a scrolling history waveform -- the third style of the
//! `signal` family (see `signal/mod.rs`'s header for why this is a sibling
//! family to `orbs`, not a mode inside it). This is the visual language of
//! WhatsApp/iMessage voice-message bars and SoundCloud-style waveforms:
//! a row of thin, round-capped bars mirrored about a centerline, the
//! newest sample entering at the right and the row sliding left, the
//! oldest fading out at the left edge. `bar` (a live EQ) and `waveform`
//! (a live oscilloscope trace) show *now*; this shows the last few
//! seconds.
//!
//! Prior art, read from shipped source (fetched, not recalled -- see
//! fetched 2026-09-17 20:50):
//! - wavesurfer.js `plugins/record.ts` -- the live "scrolling waveform":
//!   one peak (`max |sample|`) per frame appended to a fixed window
//!   (`scrollingWaveformWindow (5s) x FPS (100) = 500` samples) that is
//!   shifted left by one each frame, rendered `normalize = true` to stop
//!   the bars "dancing". That fixed, shifting window IS the ring-buffer
//!   technique.
//! - wavesurfer.js `renderer.ts` -- bars are mirrored about `halfHeight`
//!   when `barAlign` is unset (the default look), drawn with `roundRect`.
//! - Telegram Android `SeekBarWaveform.java` -- 5-bit samples, one bar per
//!   3dp with a 2dp stroke (~2/3 fill), rounded, and new bars *grow in*
//!   via `appearProgress`. Telegram anchors bars to a baseline; we mirror
//!   instead (wavesurfer's default, and consistent with our own `bar`).
//! - react-native-waveform-recorder -- `scroll` mode: metering at 30/s but
//!   visual samples at `samplesPerSecond = 12`, `barWidth 3 / barGap 2`,
//!   `newSampleEntry: 'grow'`.
//!
//! **The ring buffer lives in the caller, not here.** The engine is a pure
//! function of `t` with no memory (see `orbs::modes::sonar`'s header for
//! the same constraint), so the history arrives through opts with the same
//! indexed-key encoding `audioBandN` already uses: `historyCount` = N,
//! `history0..history{N-1}` = amplitudes `0..1`, oldest first, and
//! `historyPhase` = how far (`0..1`) the caller is between its last push
//! and the next one. `historyPhase` slides the whole row left by that
//! fraction of a slot (so the motion is continuous, not a step per push --
//! the caller pushes at ~12/s, far below the render rate) and grows the
//! newest bar in from zero (Telegram's/RN's "appear" entry). With no
//! `history0` in `opts` (the Studio with no voice source; tests), a
//! deterministic synthetic pattern scrolls instead, its amplitude set by
//! `voiceStateCode`, so the style is previewable without audio -- the
//! same fallback `bar`/`waveform` have. No golden vector -- same tradeoff
//! as every additive `orbs`/`signal` mode.

use crate::primitives::{
    finalize_frame, frac, hash_d, indexed_opt, ModeOpts, OrbFrame, Point, Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Synthetic pushes per second when no real history is supplied -- matches
/// react-native-waveform-recorder's `samplesPerSecond` default, and what
/// `SignalStudio` uses for its real buffer, so the two look alike.
const SYNTH_RATE: f64 = 12.0;

/// `(floor, spread)` of the synthetic amplitude per voice state -- the same
/// `Idle < Connecting < Listening < Thinking < Speaking` busyness ramp the
/// other two styles use (`voiceStateCode` `0..4`, see `bar.rs`).
fn synthetic_range(state_code: f64) -> (f64, f64) {
    match state_code as i64 {
        1 => (0.10, 0.20),
        2 => (0.15, 0.30),
        3 => (0.20, 0.35),
        4 => (0.25, 0.65),
        _ => (0.05, 0.15),
    }
}

/// Amplitude of synthetic sample `k` (an absolute push index, so the value
/// a bar carries stays the same as it slides left). Three hashed neighbors
/// are blended so consecutive samples correlate a little, the way a real
/// speech envelope does, instead of reading as white noise.
fn synthetic_sample(k: i64, state_code: f64) -> f64 {
    let (floor, spread) = synthetic_range(state_code);
    let h = |j: i64| hash_d(j as f64, 17.0);
    let v = 0.5 * h(k) + 0.3 * h(k - 1) + 0.2 * h(k + 1);
    (floor + spread * v).clamp(0.0, 1.0)
}

pub fn frame_scroll(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let count = get(o, "historyCount", 40.0).clamp(2.0, 256.0) as usize;
    let bar_width_frac = get(o, "barWidth", 0.6).clamp(0.1, 1.0);
    let min_height_frac = get(o, "minHeight", 0.08).clamp(0.0, 1.0);
    let fade_frac = get(o, "fadeWidth", 0.3).clamp(0.01, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let state_code = get(o, "voiceStateCode", 0.0);

    let margin = size * 0.08;
    let usable_w = (size - margin * 2.0).max(1.0);
    let slot_w = usable_w / count as f64;
    let bar_w = (slot_w * bar_width_frac).max(size * 0.012);
    let center_y = size * 0.5;
    let max_bar_h = size * 0.7;

    let real = indexed_opt(o, "history", 0).is_some();
    let phase = if real {
        get(o, "historyPhase", 0.0).clamp(0.0, 1.0)
    } else {
        frac(t * SYNTH_RATE)
    };
    let newest_k = (t * SYNTH_RATE).floor() as i64;
    let sample = |i: usize| -> f64 {
        if real {
            indexed_opt(o, "history", i).unwrap_or(0.0).clamp(0.0, 1.0)
        } else {
            synthetic_sample(newest_k - (count - 1 - i) as i64, state_code)
        }
    };

    let (white, base_alpha) = (0.15, 0.9);
    let mut polylines: Vec<Polyline> = Vec::with_capacity(count);
    for i in 0..count {
        // Slot `i` for the oldest-first sample `i`; the whole row slides
        // left by `phase` of a slot, so when the caller's next push shifts
        // every index down by one the bar lands exactly where it already
        // is -- no jump at the push boundary.
        let x = margin + slot_w * (i as f64 + 0.5) - phase * slot_w;

        let mut height_frac = min_height_frac + (1.0 - min_height_frac) * sample(i);
        // Left-edge fade: from (almost) nothing at the margin line to full
        // over the leftmost `fade_frac` of the strip, so the oldest bar
        // dissolves as it slides out instead of being clipped. The floor
        // stays above `with_polylines`' cull threshold on purpose: the
        // output is always exactly `count` polylines, oldest first, so
        // `polylines[i]` is sample `i` at every phase -- an invariant a
        // consumer (or a test) can rely on.
        let fade = ((x - margin + slot_w * 0.5) / (usable_w * fade_frac)).clamp(0.03, 1.0);
        let mut alpha = base_alpha * fade;
        // The newest bar grows in over the push interval (Telegram's
        // `appearProgress` / RN's `newSampleEntry: 'grow'`): height from a
        // dot to full, alpha from 30% so it doesn't pop in at full ink.
        if i == count - 1 {
            height_frac *= phase;
            alpha *= 0.3 + 0.7 * phase;
        }

        let bar_h = max_bar_h * height_frac;
        // Same cap accounting as `bar.rs`: round caps add `bar_w / 2` at
        // each end, so the stroke is shortened by `bar_w` to keep the
        // visible pill exactly `bar_h` tall; below that it collapses to a
        // zero-length stroke, which round caps render as a dot.
        let half_shaft = ((bar_h - bar_w) * 0.5).max(0.0);

        polylines.push(Polyline {
            points: vec![
                Point {
                    x,
                    y: center_y - half_shaft,
                },
                Point {
                    x,
                    y: center_y + half_shaft,
                },
            ],
            white,
            a: alpha,
            w: bar_w,
            saturation,
            hue,
            hues: Vec::new(),
        });
    }

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    const SIZE: f64 = 64.0;

    fn with_history(values: &[f64], phase: f64) -> ModeOpts {
        let mut o = opts(&[
            ("historyCount", values.len() as f64),
            ("historyPhase", phase),
            ("rMin", 0.3),
        ]);
        for (i, v) in values.iter().enumerate() {
            o.insert(format!("history{i}"), *v);
        }
        o
    }

    /// Visible height: stroked length plus the two round caps.
    fn bar_height(p: &Polyline) -> f64 {
        (p.points[1].y - p.points[0].y).abs() + p.w
    }

    fn total_height(frame: &OrbFrame) -> f64 {
        frame.polylines.iter().map(bar_height).sum()
    }

    #[test]
    fn renders_one_centered_pill_per_history_sample_oldest_to_newest() {
        let o = with_history(&[0.2; 12], 1.0);
        let frame = frame_scroll(SIZE, 0.0, &o);
        assert_eq!(frame.polylines.len(), 12);
        assert!(frame.dots.is_empty() && frame.lines.is_empty());
        for p in &frame.polylines {
            assert_eq!(p.points.len(), 2);
            assert_eq!(p.points[0].x, p.points[1].x, "a bar is vertical");
            let mid = (p.points[0].y + p.points[1].y) * 0.5;
            assert!(
                (mid - SIZE / 2.0).abs() < 1e-9,
                "bars mirror about the centerline"
            );
        }
        for w in frame.polylines.windows(2) {
            assert!(
                w[1].points[0].x > w[0].points[0].x,
                "oldest at the left, newest at the right"
            );
        }
    }

    #[test]
    fn supplied_history_drives_bar_height() {
        let loud = frame_scroll(SIZE, 0.0, &with_history(&[1.0; 10], 1.0));
        let quiet = frame_scroll(SIZE, 0.0, &with_history(&[0.0; 10], 1.0));
        assert!(total_height(&loud) > total_height(&quiet));
        assert!(
            quiet.polylines.iter().all(|p| bar_height(p) > 0.0),
            "silence keeps a visible floor (a dot per sample), not an empty strip"
        );
        // A single loud newest sample shows up on the rightmost bar only.
        let mut spike = vec![0.0; 10];
        spike[9] = 1.0;
        let frame = frame_scroll(SIZE, 0.0, &with_history(&spike, 1.0));
        let last = frame.polylines.last().unwrap();
        assert!(frame.polylines[..9]
            .iter()
            .all(|p| bar_height(p) < bar_height(last)));
    }

    #[test]
    fn phase_slides_the_row_left_continuously_and_grows_the_newest_bar() {
        let at = |phase: f64| frame_scroll(SIZE, 0.0, &with_history(&[0.5; 8], phase));
        let x0 = at(0.0).polylines[3].points[0].x;
        let x_half = at(0.5).polylines[3].points[0].x;
        let x1 = at(1.0).polylines[3].points[0].x;
        let slot_w = SIZE * 0.84 / 8.0;
        assert!(
            (x0 - x1 - slot_w).abs() < 1e-9,
            "a full phase moves a bar exactly one slot left"
        );
        assert!(
            (x_half - (x0 + x1) * 0.5).abs() < 1e-9,
            "half a phase is halfway -- continuous, not stepped"
        );
        // newest bar grows in with phase
        let h0 = bar_height(at(0.0).polylines.last().unwrap());
        let h1 = bar_height(at(1.0).polylines.last().unwrap());
        assert!(h1 > h0, "the newest bar should grow in from (near) nothing");
    }

    #[test]
    fn oldest_bars_fade_toward_the_left_edge() {
        let frame = frame_scroll(SIZE, 0.0, &with_history(&[0.5; 20], 1.0));
        let a = |i: usize| frame.polylines[i].a;
        assert!(
            a(0) < a(2) && a(2) < a(5),
            "alpha ramps up away from the left edge"
        );
        assert!(
            (a(10) - a(19)).abs() < 1e-9,
            "past the fade zone every bar is at full alpha"
        );
    }

    #[test]
    fn synthetic_pattern_when_no_history_is_supplied() {
        let o = opts(&[
            ("historyCount", 24.0),
            ("voiceStateCode", 4.0),
            ("rMin", 0.3),
        ]);
        let a = frame_scroll(SIZE, 1.234, &o);
        let b = frame_scroll(SIZE, 1.234, &o);
        assert_eq!(a, b, "synthetic history must be deterministic in t");
        assert_eq!(a.polylines.len(), 24);
        let heights: Vec<f64> = a.polylines.iter().map(bar_height).collect();
        let (min, max) = heights
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
        assert!(
            max - min > 1.0,
            "synthetic bars should visibly vary, not sit at one height"
        );
        // Advancing t by exactly one push keeps each bar's value as it
        // slides one slot left (the pattern scrolls, it doesn't reshuffle).
        // The newest bar (index 23) is excluded: it's still growing in,
        // so its height is phase-scaled while its successor's isn't.
        let later = frame_scroll(SIZE, 1.234 + 1.0 / SYNTH_RATE, &o);
        for i in 1..23 {
            assert!(
                (bar_height(&later.polylines[i - 1]) - bar_height(&a.polylines[i])).abs() < 1e-9,
                "bar {i} should have moved one slot left with the same height"
            );
        }
    }

    #[test]
    fn synthetic_speaking_is_busier_than_idle() {
        let energy = |code: f64| -> f64 {
            let o = opts(&[
                ("historyCount", 24.0),
                ("voiceStateCode", code),
                ("rMin", 0.3),
            ]);
            (0..10)
                .map(|i| total_height(&frame_scroll(SIZE, i as f64 * 0.37, &o)))
                .sum::<f64>()
        };
        assert!(energy(4.0) > energy(0.0));
    }
}
