//! Signal / Playback: a recorded voice message's waveform with its playback
//! position (`playing`) -- the chat-bubble control of WhatsApp, iMessage and
//! Telegram. `scroll` shows a *live* recording sliding by; this shows a
//! *finished* one standing still, with the played part in full ink and the
//! rest dimmed.
//!
//! Prior art, the same sources `scroll.rs` read: Telegram Android
//! `SeekBarWaveform.java` (a fixed row of rounded bars; the part before the
//! playhead in the active colour, the rest in the inactive one; the peaks
//! are *resampled* to the bar count the width allows) and wavesurfer.js
//! `renderer.ts` (bars mirrored about the centre line; `progressColor`
//! before the cursor, `waveColor` after).
//!
//! **The app owns the audio.** Sinua decodes nothing: the caller passes the
//! clip's loudness envelope as `envelope0..envelope{N-1}` (`0..1`, N <= 64,
//! the same indexed-key encoding `history`/`audioBand` use; the typed views
//! take an `envelope` array) and the position as `progress` `0..1`. The
//! count is the run of consecutive keys from `envelope0`; views rebuild the
//! key map every frame, so a shorter clip never leaves stale keys behind.
//! Without an envelope (the Studio, the gallery, tests) a fixed synthetic
//! message is drawn -- fixed, not animated, because it stands for a
//! recording, not a live signal.
//!
//! **Wide layout.** A message bubble is several times wider than tall, and
//! engine space is square by default. This mode reads `aspect` (width /
//! height, `1..8`, default 1) and lays itself out in `size * aspect` by
//! `size`; a view that knows the pattern is wide passes its box's ratio
//! (catalog: the pattern's `layout: "box"`). At the default 1 it fits a
//! square like every other pattern.
//!
//! Stateless: a pure function of the opts (`t` is unused -- nothing here
//! moves unless the app moves `progress`).

use crate::primitives::{finalize_frame, hash_d, indexed_opt, ModeOpts, OrbFrame, Point, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Most envelope values a caller may pass (`envelope0..envelope63`).
pub(crate) const MAX_ENVELOPE: usize = 64;
/// Bars in the synthetic stand-in message.
const SYNTH_BARS: usize = 40;
const PLAYED_ALPHA: f64 = 0.9;

/// The caller's envelope (consecutive `envelope{i}` keys), or `None`.
fn envelope(o: &ModeOpts) -> Option<Vec<f64>> {
    let v: Vec<f64> = (0..MAX_ENVELOPE)
        .map_while(|i| indexed_opt(o, "envelope", i))
        .map(|x| {
            if x.is_finite() {
                x.clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect();
    (!v.is_empty()).then_some(v)
}

/// A fixed, speech-like message: an attack, uneven syllables, a tail.
fn synthetic() -> Vec<f64> {
    (0..SYNTH_BARS)
        .map(|i| {
            let u = i as f64 / (SYNTH_BARS - 1) as f64;
            let shape = (u * 9.0).min(1.0) * (1.0 - u.powi(6));
            let h = |j: usize| hash_d(j as f64, 23.0);
            let v = 0.55 * h(i) + 0.3 * h(i + 1) + 0.15 * h(i + 2);
            (0.12 + 0.88 * v * shape).clamp(0.0, 1.0)
        })
        .collect()
}

/// `src` resampled to `m` bars: the max over each bar's span when
/// shrinking (so a loud syllable never disappears), linear when growing.
pub(crate) fn resample(src: &[f64], m: usize) -> Vec<f64> {
    let n = src.len();
    if m == 0 || n == 0 {
        return Vec::new();
    }
    if m == n {
        return src.to_vec();
    }
    if m < n {
        return (0..m)
            .map(|j| {
                let a = j * n / m;
                let b = ((j + 1) * n / m).max(a + 1);
                src[a..b].iter().copied().fold(0.0, f64::max)
            })
            .collect();
    }
    (0..m)
        .map(|j| {
            let x = if m == 1 {
                0.0
            } else {
                j as f64 * (n - 1) as f64 / (m - 1) as f64
            };
            let i = (x.floor() as usize).min(n - 1);
            let k = (i + 1).min(n - 1);
            src[i] + (src[k] - src[i]) * (x - i as f64)
        })
        .collect()
}

/// Where the bars go: the left edge of the row and its width.
pub(crate) fn row(size: f64, aspect: f64) -> (f64, f64) {
    let width = size * aspect;
    let margin = size * 0.06;
    (margin, (width - margin * 2.0).max(1.0))
}

/// The playback position under a touch: `x` is the touch's distance from the
/// box's left edge divided by the box's height (the engine's unit here), for a
/// box `aspect` wide. The same row [`frame_playback`] draws, so a finger on a bar
/// lands on that bar's position; outside the row it clamps to 0 or 1.
pub fn seek_progress(aspect: f64, x: f64) -> f64 {
    // In engine units at the reference size (the row's 1-unit floor is for real frames).
    const SIZE: f64 = 64.0;
    let (left, usable) = row(SIZE, aspect.clamp(1.0, 8.0));
    if !x.is_finite() {
        return 0.0;
    }
    ((x * SIZE - left) / usable).clamp(0.0, 1.0)
}

pub fn frame_playback(size: f64, _t: f64, o: &ModeOpts) -> OrbFrame {
    let aspect = get(o, "aspect", 1.0).clamp(1.0, 8.0);
    let values = envelope(o).unwrap_or_else(synthetic);
    let bar_count = get(o, "barCount", 0.0).round().clamp(0.0, 128.0) as usize;
    let bars = resample(
        &values,
        if bar_count == 0 {
            values.len()
        } else {
            bar_count
        },
    );
    let progress = get(o, "progress", 0.0).clamp(0.0, 1.0);
    let unplayed = get(o, "unplayedOpacity", 0.35).clamp(0.0, 1.0);
    let bar_width_frac = get(o, "barWidth", 0.6).clamp(0.05, 1.0);
    let min_height_frac = get(o, "minHeight", 0.08).clamp(0.0, 1.0);
    let playhead = get(o, "playhead", 0.0) >= 0.5;
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let white = 0.15;

    let (left, usable) = row(size, aspect);
    let slot = usable / bars.len().max(1) as f64;
    let bar_w = (slot * bar_width_frac).max(size * 0.012);
    let center_y = size * 0.5;
    let max_bar_h = size * 0.7;

    let mut polylines: Vec<Polyline> = Vec::with_capacity(bars.len() + 1);
    for (i, v) in bars.iter().enumerate() {
        let x = left + slot * (i as f64 + 0.5);
        let played = (i as f64 + 0.5) / bars.len() as f64 <= progress;
        let bar_h = max_bar_h * (min_height_frac + (1.0 - min_height_frac) * v);
        // `scroll.rs`'s cap accounting: round caps add `bar_w / 2` at each
        // end, so the shaft is shortened by `bar_w` to keep the visible pill
        // `bar_h` tall; shorter than that it is a dot.
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
            a: if played {
                PLAYED_ALPHA
            } else {
                PLAYED_ALPHA * unplayed
            },
            w: bar_w,
            saturation,
            hue,
            hues: Vec::new(),
        });
    }
    if playhead {
        let x = left + usable * progress;
        let half = size * 0.4;
        polylines.push(Polyline {
            points: vec![
                Point {
                    x,
                    y: center_y - half,
                },
                Point {
                    x,
                    y: center_y + half,
                },
            ],
            white,
            a: PLAYED_ALPHA,
            w: size * 0.02,
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

    fn with_envelope(values: &[f64], extra: &[(&str, f64)]) -> ModeOpts {
        let mut o = opts(extra);
        for (i, v) in values.iter().enumerate() {
            o.insert(format!("envelope{i}"), *v);
        }
        o
    }

    fn height(p: &Polyline) -> f64 {
        (p.points[1].y - p.points[0].y).abs() + p.w
    }

    #[test]
    fn one_bar_per_envelope_value_left_to_right() {
        let f = frame_playback(SIZE, 0.0, &with_envelope(&[0.2, 0.9, 0.5], &[]));
        assert_eq!(f.polylines.len(), 3);
        assert!(f
            .polylines
            .windows(2)
            .all(|w| w[0].points[0].x < w[1].points[0].x));
        let h: Vec<f64> = f.polylines.iter().map(height).collect();
        assert!(h[1] > h[2] && h[2] > h[0], "taller for louder: {h:?}");
    }

    #[test]
    fn the_count_stops_at_the_first_missing_key() {
        let mut o = with_envelope(&[0.5, 0.5], &[]);
        o.insert("envelope3".into(), 0.5); // a gap at 2: not part of the clip
        assert_eq!(frame_playback(SIZE, 0.0, &o).polylines.len(), 2);
    }

    #[test]
    fn progress_splits_played_from_unplayed() {
        let o = with_envelope(&[0.5; 10], &[("progress", 0.3), ("unplayedOpacity", 0.4)]);
        let a: Vec<f64> = frame_playback(SIZE, 0.0, &o)
            .polylines
            .iter()
            .map(|p| p.a)
            .collect();
        let played = a
            .iter()
            .filter(|x| (**x - PLAYED_ALPHA).abs() < 1e-12)
            .count();
        assert_eq!(played, 3, "bars centred at 5%, 15%, 25%: {a:?}");
        assert!(a[3..]
            .iter()
            .all(|x| (x - PLAYED_ALPHA * 0.4).abs() < 1e-12));
        let none = frame_playback(SIZE, 0.0, &with_envelope(&[0.5; 10], &[]));
        assert!(
            none.polylines.iter().all(|p| p.a < PLAYED_ALPHA),
            "progress 0: nothing played"
        );
        let all = frame_playback(SIZE, 0.0, &with_envelope(&[0.5; 10], &[("progress", 1.0)]));
        assert!(all
            .polylines
            .iter()
            .all(|p| (p.a - PLAYED_ALPHA).abs() < 1e-12));
    }

    #[test]
    fn resampling_keeps_the_peaks_when_shrinking() {
        let src: Vec<f64> = (0..64).map(|i| if i == 37 { 1.0 } else { 0.1 }).collect();
        let r = resample(&src, 16);
        assert_eq!(r.len(), 16);
        assert_eq!(
            r.iter().copied().fold(0.0, f64::max),
            1.0,
            "the spike survives"
        );
        assert_eq!(resample(&[0.0, 1.0], 5), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(resample(&[0.3, 0.6], 2), vec![0.3, 0.6]);
        let f = frame_playback(SIZE, 0.0, &with_envelope(&src, &[("barCount", 16.0)]));
        assert_eq!(f.polylines.len(), 16);
    }

    #[test]
    fn playhead_sits_at_progress() {
        let o = with_envelope(&[0.5; 8], &[("progress", 0.25), ("playhead", 1.0)]);
        let f = frame_playback(SIZE, 0.0, &o);
        assert_eq!(f.polylines.len(), 9, "8 bars + the playhead, drawn last");
        let (left, usable) = row(SIZE, 1.0);
        let head = f.polylines.last().unwrap();
        assert!((head.points[0].x - (left + usable * 0.25)).abs() < 1e-9);
    }

    #[test]
    fn wide_layout_fills_the_width_and_stays_inside() {
        for aspect in [1.0, 3.0, 5.0, 8.0] {
            let o = with_envelope(
                &[1.0; 30],
                &[("aspect", aspect), ("playhead", 1.0), ("progress", 1.0)],
            );
            let f = frame_playback(SIZE, 0.0, &o);
            let w = SIZE * aspect;
            for p in &f.polylines {
                for q in &p.points {
                    assert!(
                        q.x - p.w / 2.0 >= -1e-9 && q.x + p.w / 2.0 <= w + 1e-9,
                        "x inside at aspect {aspect}"
                    );
                    assert!(
                        q.y - p.w / 2.0 >= -1e-9 && q.y + p.w / 2.0 <= SIZE + 1e-9,
                        "y inside"
                    );
                }
            }
            let last_bar = &f.polylines[29];
            assert!(
                last_bar.points[0].x > w * 0.9,
                "the row spans the width at aspect {aspect}"
            );
        }
    }

    #[test]
    fn seek_lands_on_the_bar_under_the_finger() {
        for aspect in [1.0, 4.0, 5.0] {
            let (left, usable) = row(64.0, aspect);
            let (left, usable) = (left / 64.0, usable / 64.0);
            assert_eq!(seek_progress(aspect, 0.0), 0.0, "left of the row clamps");
            assert_eq!(
                seek_progress(aspect, aspect),
                1.0,
                "right of the row clamps"
            );
            assert!((seek_progress(aspect, left + usable * 0.3) - 0.3).abs() < 1e-12);
            // Bar i of 10 is centred at (i + 0.5) / 10 of the row: a finger at 33%
            // of the row has passed the centres of bars 0, 1 and 2, so they're played.
            let o = opts(&[
                ("aspect", aspect),
                ("progress", seek_progress(aspect, left + usable * 0.33)),
            ]);
            let f = frame_playback(64.0, 0.0, &{
                let mut o = o;
                for i in 0..10 {
                    o.insert(format!("envelope{i}"), 0.5);
                }
                o
            });
            let played = f
                .polylines
                .iter()
                .filter(|p| (p.a - PLAYED_ALPHA).abs() < 1e-12)
                .count();
            assert_eq!(played, 3, "aspect {aspect}");
        }
        assert_eq!(seek_progress(4.0, f64::NAN), 0.0);
    }

    #[test]
    fn synthetic_message_is_fixed_in_time() {
        let a = frame_playback(SIZE, 0.0, &opts(&[]));
        let b = frame_playback(SIZE, 7.3, &opts(&[]));
        assert_eq!(a.polylines.len(), SYNTH_BARS);
        assert_eq!(a, b);
    }
}
