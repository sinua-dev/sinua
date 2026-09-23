//! Signal / Bar: a linear, non-spherical, dockable audio-EQ bar graph --
//! the first style of the `signal` family (see `signal/mod.rs`'s header
//! for why this is a sibling family to `orbs`, not a mode inside it).
//! Bars in a row, not points on a sphere -- `orbs::core::Proj`/`fib_dir`
//! aren't used at all here, only the family-agnostic `Polyline`/
//! `finalize_frame`.
//!
//! Each bar is one two-point `Polyline` (see `crate::primitives::Polyline`
//! for the paint contract): a vertical stroke whose round caps make it a
//! pill, **centered on the strip's midline** and growing both ways, the
//! way LiveKit's/Siri's bar visualizers do -- not rising from a baseline
//! like a bar chart. Before `Polyline` existed this was a butt-capped
//! `Line` shaft plus a `Dot` cap on top (round at the top only, flat at
//! the bottom, baseline-anchored), and before that a stack of small `Dot`s
//! whose gaps read as scattered dots rather than a bar -- see
//! `docs/signal.md` for that history.
//!
//! Two independent signals drive the same bars, a technique confirmed in
//! both LiveKit's and ElevenLabs' shipped `BarVisualizer` components (read
//! directly from their source, not guessed): bar *height* comes from real
//! per-band audio volume (`audioBand0..N` opts, same indexed-key encoding
//! `orbs::modes::spectrum` uses, via the shared `crate::primitives::audio_band`)
//! while `speaking`; a state-driven "highlight sequencer" (driven by
//! `voiceStateCode` -- see `VoiceState` below) cycles which bars are lit at
//! a tempo tied to the voice session's lifecycle phase for every other
//! state. This is what keeps the bars visibly alive during phases with no
//! audio yet (`thinking`, `connecting`) instead of freezing or reading as
//! broken. No golden vector -- same tradeoff as every additive `orbs` mode.

use crate::primitives::{audio_band, finalize_frame, ModeOpts, OrbFrame, Point, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Mirrors the TS `AgentState` union (the Web Studio's `audio/types.ts`),
/// encoded as a number since a flat `HashMap<String, f64>` opts map can't
/// carry a string -- same reasoning as `audio_band`'s indexed keys.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum VoiceState {
    Idle,
    Connecting,
    Listening,
    Thinking,
    Speaking,
}

pub(crate) fn voice_state(o: &ModeOpts) -> VoiceState {
    match get(o, "voiceStateCode", 0.0) as i64 {
        1 => VoiceState::Connecting,
        2 => VoiceState::Listening,
        3 => VoiceState::Thinking,
        4 => VoiceState::Speaking,
        _ => VoiceState::Idle,
    }
}

/// Which bars are "lit" (highlighted) at time `t`, for the 4 non-`Speaking`
/// states -- ported straight from the cadences LiveKit's `BarVisualizer`
/// ships (`sequencerIntervals`): listening's center bar blinks every 500ms,
/// thinking chases back and forth every 150ms/step, connecting/idle sweep
/// inward from both ends over a 2000ms/barCount-per-step cadence. `Idle`
/// reuses the same sweep as `Connecting` but slower (no real distinction
/// upstream either -- LiveKit's own map has no separate "idle" entry).
pub(crate) fn highlighted(state: VoiceState, t: f64, i: i64, bar_count: i64) -> bool {
    match state {
        VoiceState::Speaking => true,
        VoiceState::Listening => {
            let center = bar_count / 2;
            let blink_on = (t / 0.5) as i64 % 2 == 0;
            i == center && blink_on
        }
        VoiceState::Thinking => {
            let step_ms = 150.0;
            let span = (bar_count - 1).max(1);
            let period = (span * 2).max(1);
            let phase = ((t * 1000.0 / step_ms) as i64).rem_euclid(period);
            let idx = if phase <= span { phase } else { period - phase };
            i == idx
        }
        VoiceState::Connecting | VoiceState::Idle => {
            let cadence = if state == VoiceState::Idle {
                4000.0
            } else {
                2000.0
            };
            let step_ms = cadence / bar_count.max(1) as f64;
            let half = bar_count / 2;
            let sweep = ((t * 1000.0 / step_ms) as i64).rem_euclid(half + 1);
            i == sweep || i == bar_count - 1 - sweep
        }
    }
}

/// `Speaking`'s real-audio level (0..1) for bar/column `i` of `count`:
/// center-out, mirrored band layout -- the lowest band on the middle bar,
/// the highest at both ends -- same reasoning as `waveform`'s symmetric
/// layout. A left-to-right layout is always lopsided with a real voice
/// spectrum (band energy falls off with frequency, so the left half
/// towered over the right); LiveKit's BarVisualizer does lay bands out
/// left-to-right, this deliberately doesn't. `0` when no bands are
/// supplied. Shared with `matrix`, so both styles map bands identically.
pub(crate) fn band_level(o: &ModeOpts, i: i64, count: i64) -> f64 {
    let audio_band_count = get(o, "audioBandCount", 0.0) as usize;
    if audio_band_count == 0 {
        return 0.0;
    }
    let dist01 = ((i as f64 + 0.5) / count as f64 * 2.0 - 1.0).abs();
    let band_idx = (dist01 * (audio_band_count as f64 - 1.0)).round() as usize;
    audio_band(o, band_idx.min(audio_band_count - 1))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

pub fn frame_bar(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let bar_count = get(o, "barCount", 9.0).max(1.0) as i64;
    let bar_width_frac = get(o, "barWidth", 0.55);
    let min_height_frac = get(o, "minHeight", 0.20).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);

    let margin = size * 0.1;
    let usable_w = (size - margin * 2.0).max(1.0);
    let slot_w = usable_w / bar_count as f64;
    let center_y = size * 0.5;
    let max_bar_h = size * 0.6;
    // Proportional floor (not an absolute 1.5px, which at size 20 was
    // nearly the whole slot) so bars keep their gaps at every size.
    let bar_w = (slot_w * bar_width_frac).max(size * 0.02);

    let state = voice_state(o);

    let mut polylines: Vec<Polyline> = Vec::with_capacity(bar_count as usize);

    for i in 0..bar_count {
        let lit = highlighted(state, t, i, bar_count);
        // Never fully flat -- same floor trick as `orbs::modes::spectrum`.
        // `Speaking`'s height comes from real audio; every other state has
        // no audio to show yet, so the sequencer (`lit`) has to carry
        // height too, not just color -- a color-only pulse on an
        // unchanging bar height doesn't read as "something is happening."
        let height_frac = if state == VoiceState::Speaking {
            // Center-out, mirrored band layout -- see `band_level`.
            let raw = band_level(o, i, bar_count);
            min_height_frac + (1.0 - min_height_frac) * raw
        } else if lit {
            0.65
        } else {
            min_height_frac
        };

        let cx = margin + slot_w * (i as f64 + 0.5);
        let bar_h = max_bar_h * height_frac;
        // The round caps add `bar_w / 2` at each end, so the stroked
        // segment is shortened by `bar_w` to keep the *visible* pill
        // exactly `bar_h` tall -- `minHeight` means visible height,
        // not shaft length. Shorter than the caps alone collapses to a
        // zero-length stroke, which round caps render as a dot of
        // diameter `bar_w` (see `Polyline`'s doc comment).
        let half_shaft = ((bar_h - bar_w) * 0.5).max(0.0);
        let (white, alpha) = if lit { (0.08, 0.95) } else { (0.72, 0.32) };

        polylines.push(Polyline {
            points: vec![
                Point {
                    x: cx,
                    y: center_y - half_shaft,
                },
                Point {
                    x: cx,
                    y: center_y + half_shaft,
                },
            ],
            white,
            a: alpha,
            w: bar_w,
            saturation: if lit { saturation } else { 0.0 },
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

    fn base_opts() -> ModeOpts {
        opts(&[("barCount", 9.0), ("rMin", 0.3)])
    }

    /// A bar's visible height: its stroked length plus the two round caps
    /// (`w / 2` each) -- see `frame_bar`.
    fn bar_height(frame: &OrbFrame, index: usize) -> f64 {
        let p = &frame.polylines[index];
        (p.points[1].y - p.points[0].y).abs() + p.w
    }

    #[test]
    fn every_bar_always_renders_one_centered_pill() {
        for code in 0..5 {
            let mut o = base_opts();
            o.insert("voiceStateCode".to_string(), code as f64);
            let frame = frame_bar(64.0, 1.23, &o);
            assert_eq!(
                frame.polylines.len(),
                9,
                "voiceStateCode={code} should render every bar"
            );
            assert!(
                frame.dots.is_empty() && frame.lines.is_empty(),
                "bars are pure polylines now"
            );
            for i in 0..9 {
                let p = &frame.polylines[i];
                assert_eq!(p.points.len(), 2, "a bar is a two-point vertical stroke");
                assert_eq!(p.points[0].x, p.points[1].x, "a bar is vertical");
                let mid = (p.points[0].y + p.points[1].y) * 0.5;
                assert!(
                    (mid - 32.0).abs() < 1e-9,
                    "voiceStateCode={code} bar {i} should be centered on the strip"
                );
                assert!(
                    bar_height(&frame, i) > 0.0,
                    "voiceStateCode={code} bar {i} collapsed to zero height"
                );
            }
        }
    }

    #[test]
    fn bar_width_scales_with_size() {
        let at_64 = frame_bar(64.0, 0.0, &base_opts()).polylines[0].w;
        let at_20 = frame_bar(20.0, 0.0, &base_opts()).polylines[0].w;
        assert!(
            at_64 > at_20,
            "bars must not hit an absolute-pixel floor at small sizes"
        );
        assert!(
            at_20 < 20.0 * 0.8 / 9.0,
            "a bar must stay narrower than its slot at size 20"
        );
    }

    #[test]
    fn listening_blinks_only_the_center_bar_on_a_fixed_cadence() {
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 2.0);
        // 500ms cadence -- t=0.0 is "on", t=0.5 is "off", deterministically.
        let on = frame_bar(64.0, 0.0, &o);
        let off = frame_bar(64.0, 0.5, &o);
        let center = 4; // bar_count/2 for barCount=9
        assert!(
            bar_height(&on, center) > bar_height(&off, center),
            "expected the center bar taller while blinking on"
        );
        for i in (0..9).filter(|&i| i != center) {
            assert!(
                (bar_height(&on, i) - bar_height(&off, i)).abs() < 1e-9,
                "only the center bar should blink"
            );
        }
    }

    #[test]
    fn speaking_uses_real_band_data_when_supplied() {
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
        let loud_frame = frame_bar(64.0, 0.0, &loud);
        let quiet_frame = frame_bar(64.0, 0.0, &quiet);
        assert!(bar_height(&loud_frame, 0) > bar_height(&quiet_frame, 0));
        assert!(
            bar_height(&quiet_frame, 0) > 0.0,
            "silent bands must still render a floor-height bar"
        );
    }

    #[test]
    fn speaking_bars_mirror_about_the_center_with_the_lowest_band_in_the_middle() {
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 4.0);
        o.insert("audioBandCount".to_string(), 4.0);
        o.insert("audioBand0".to_string(), 1.0);
        for i in 1..4 {
            o.insert(format!("audioBand{i}"), 0.0);
        }
        let frame = frame_bar(64.0, 0.0, &o);
        for i in 0..4 {
            assert!(
                (bar_height(&frame, i) - bar_height(&frame, 8 - i)).abs() < 1e-9,
                "bar {i} should mirror bar {}",
                8 - i
            );
        }
        assert!(
            bar_height(&frame, 4) > bar_height(&frame, 0),
            "the loud lowest band belongs on the center bar"
        );
    }

    #[test]
    fn lit_bars_carry_the_requested_color_and_unlit_ones_stay_grey() {
        let mut o = base_opts();
        o.insert("voiceStateCode".to_string(), 2.0); // listening: center lit at t=0, others unlit
        o.insert("saturation".to_string(), 0.8);
        o.insert("hue".to_string(), 120.0);
        let frame = frame_bar(64.0, 0.0, &o);
        assert!((frame.polylines[4].saturation - 0.8).abs() < 1e-9);
        assert_eq!(frame.polylines[0].saturation, 0.0);
        assert!((frame.polylines[4].hue - 120.0).abs() < 1e-9);
    }
}
