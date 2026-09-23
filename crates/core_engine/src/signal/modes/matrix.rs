//! Signal / Matrix: a dot-grid LED EQ (`metering`) -- the vintage LED
//! spectrum-analyzer look. `columnCount` columns of `ledCount` round LEDs;
//! each column lights `floor(level · ledCount)` of them, the rest stay a dim
//! grid.
//!
//! Transcribed from audioMotion-analyzer's LED mode (README + raw
//! `src/audioMotion-analyzer.js`, fetched):
//! `ledBars`, a lit count of `value * ledCount | 0` (floor), and unlit LEDs
//! painted `LEDS_UNLIT_COLOR = '#7f7f7f22'` -- mid grey at alpha ~0.13.
//! That dim unlit grid is the point: an early `bar` built from a stack of
//! small dots read as *scattered* dots and was rejected; a full grid with
//! only the lit part inked reads as a panel instead (checked on the
//! contact sheet). Not transcribed: audioMotion's peak hold/gravity, which
//! is stateful -- here a caller-owned `peak{i}` (0..1) draws one lit LED,
//! the same stateless contract as `scroll`'s caller-owned history.
//!
//! Levels come from `bar`'s own helpers, so both styles agree: while
//! `Speaking`, `band_level` (the center-out band mapping); otherwise the
//! `voiceStateCode` highlight sequencer (`highlighted`) lifts active
//! columns to 0.65 and leaves the rest at `minLevel`. Audio drives
//! amplitude only (column height), never layout. Every mark is a `Dot`.

use super::bar::{band_level, highlighted, voice_state, VoiceState};
use crate::primitives::{finalize_frame, indexed_opt, Dot, ModeOpts, OrbFrame};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// audioMotion's `LEDS_UNLIT_COLOR` alpha (`0x22 / 255`).
pub const UNLIT_ALPHA: f64 = 0x22 as f64 / 255.0;
const SEQUENCER_LEVEL: f64 = 0.65;

/// Which rows (0 = bottom) of `leds` are lit for `lit` LEDs: the bottom
/// `lit` rows, or with `mirror`, the rows within `lit / 2` of the middle
/// (symmetric by construction).
pub(crate) fn row_lit(row: usize, leds: usize, lit: usize, mirror: bool) -> bool {
    if !mirror {
        return row < lit;
    }
    let c = (leds as f64 - 1.0) * 0.5;
    (row as f64 - c).abs() <= lit as f64 * 0.5
}

pub fn frame_matrix(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cols = get(o, "columnCount", 12.0).round().clamp(1.0, 32.0) as i64;
    let leds = get(o, "ledCount", 8.0).round().clamp(2.0, 24.0) as usize;
    let min_level = get(o, "minLevel", 0.125).clamp(0.0, 1.0);
    let led_size = get(o, "ledSize", 0.7).clamp(0.2, 1.0);
    let mirror = get(o, "mirror", 0.0) >= 0.5;
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);

    // Same box as `bar`: 10% side margins, a 60%-tall strip, centered.
    let margin = size * 0.1;
    let cell_w = (size - margin * 2.0).max(1.0) / cols as f64;
    let box_h = size * 0.6;
    let cell_h = box_h / leds as f64;
    let top = size * 0.5 - box_h * 0.5;
    let r = 0.5 * led_size * cell_w.min(cell_h);

    let state = voice_state(o);
    let mut dots: Vec<Dot> = Vec::with_capacity(cols as usize * leds);
    for i in 0..cols {
        let active = highlighted(state, t, i, cols);
        let level = if state == VoiceState::Speaking {
            min_level + (1.0 - min_level) * band_level(o, i, cols)
        } else if active {
            SEQUENCER_LEVEL
        } else {
            min_level
        };
        let lit = ((level * leds as f64) as usize).min(leds);
        let peak_row = indexed_opt(o, "peak", i as usize)
            .map(|p| ((p.clamp(0.0, 1.0) * leds as f64) as usize).min(leds - 1));
        let x = margin + cell_w * (i as f64 + 0.5);
        for row in 0..leds {
            // Row 0 is the bottom of the strip.
            let y = top + cell_h * (leds - 1 - row) as f64 + cell_h * 0.5;
            let is_peak = match peak_row {
                Some(p) if mirror => {
                    let c = (leds as f64 - 1.0) * 0.5;
                    ((row as f64 - c).abs() - p as f64 * 0.5).abs() < 0.5
                }
                Some(p) => row == p,
                None => false,
            };
            let (white, a, sat) = if row_lit(row, leds, lit, mirror) {
                if active {
                    (0.08, 0.95, saturation)
                } else {
                    // `bar`'s inactive-bar look.
                    (0.72, 0.32, 0.0)
                }
            } else if is_peak {
                (0.08, 0.95, saturation)
            } else {
                (0.5, UNLIT_ALPHA, 0.0)
            };
            dots.push(Dot {
                x,
                y,
                z: 0.0,
                r,
                white,
                a,
                saturation: sat,
                hue,
            });
        }
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    const SIZE: f64 = 64.0;

    fn column(f: &OrbFrame, cols: usize, leds: usize, i: usize) -> Vec<&Dot> {
        // Dots are emitted column-major and `finalize_frame`'s stable z-sort
        // (all z = 0) keeps that order.
        assert_eq!(f.dots.len(), cols * leds);
        f.dots[i * leds..(i + 1) * leds].iter().collect()
    }

    fn lit_count(col: &[&Dot]) -> usize {
        col.iter().filter(|d| d.a > UNLIT_ALPHA + 0.01).count()
    }

    fn speaking(bands: &[f64]) -> ModeOpts {
        let mut o = opts(&[
            ("voiceStateCode", 4.0),
            ("audioBandCount", bands.len() as f64),
        ]);
        for (k, v) in bands.iter().enumerate() {
            o.insert(format!("audioBand{k}"), *v);
        }
        o
    }

    #[test]
    fn a_full_grid_inside_the_strip() {
        let f = frame_matrix(SIZE, 0.0, &opts(&[]));
        assert_eq!(f.dots.len(), 12 * 8, "every LED drawn, lit or not");
        for d in &f.dots {
            assert!(d.x - d.r >= SIZE * 0.1 - 1e-9 && d.x + d.r <= SIZE * 0.9 + 1e-9);
            assert!(d.y - d.r >= SIZE * 0.2 - 1e-9 && d.y + d.r <= SIZE * 0.8 + 1e-9);
        }
    }

    #[test]
    fn lit_count_is_floored_like_audiomotion() {
        let mut o = speaking(&[0.99]);
        o.insert("columnCount".into(), 1.0);
        o.insert("minLevel".into(), 0.0);
        let f = frame_matrix(SIZE, 0.0, &o);
        assert_eq!(lit_count(&column(&f, 1, 8, 0)), 7, "0.99 x 8 -> 7, not 8");
    }

    #[test]
    fn louder_bands_light_taller_columns_with_bars_mapping() {
        let o = speaking(&[1.0, 0.2]);
        let f = frame_matrix(SIZE, 0.0, &o);
        // Center-out: band 0 sits in the middle columns, band 1 at the ends.
        let mid = lit_count(&column(&f, 12, 8, 6));
        let end = lit_count(&column(&f, 12, 8, 0));
        assert!(mid > end);
        let want = ((0.125 + 0.875 * band_level(&o, 6, 12)) * 8.0) as usize;
        assert_eq!(mid, want);
    }

    #[test]
    fn non_speaking_uses_bars_sequencer() {
        // Listening: the center column blinks on at t = 0.
        let o = opts(&[("voiceStateCode", 2.0)]);
        let f = frame_matrix(SIZE, 0.0, &o);
        let state = voice_state(&o);
        for i in 0..12 {
            let col = column(&f, 12, 8, i);
            let active = col.iter().any(|d| d.a > 0.9);
            assert_eq!(active, highlighted(state, 0.0, i as i64, 12));
        }
    }

    #[test]
    fn mirror_is_symmetric_about_the_middle_row() {
        let mut o = speaking(&[0.6]);
        o.insert("columnCount".into(), 1.0);
        o.insert("mirror".into(), 1.0);
        let f = frame_matrix(SIZE, 0.0, &o);
        let col = column(&f, 1, 8, 0);
        let lit: Vec<bool> = col.iter().map(|d| d.a > 0.5).collect();
        let rev: Vec<bool> = lit.iter().rev().copied().collect();
        assert_eq!(lit, rev);
        assert!(
            lit.iter().any(|&b| b) && !lit[0],
            "grows from the middle, not the bottom"
        );
    }

    #[test]
    fn a_caller_peak_lights_one_led_above_the_level() {
        let mut o = speaking(&[0.3]);
        o.insert("columnCount".into(), 1.0);
        o.insert("minLevel".into(), 0.0);
        o.insert("peak0".into(), 0.8);
        let f = frame_matrix(SIZE, 0.0, &o);
        let col = column(&f, 1, 8, 0);
        let lit_rows: Vec<usize> = (0..8).filter(|&r| col[7 - r].a > 0.5).collect();
        // Level 0.3 -> 2 lit (rows 0, 1); peak 0.8 -> row 6.
        assert_eq!(lit_rows.len(), 3);
        assert!(lit_rows.contains(&6));
    }

    #[test]
    fn unlit_leds_are_a_dim_grey_grid_even_when_saturated() {
        let f = frame_matrix(SIZE, 0.0, &opts(&[("saturation", 0.9), ("hue", 120.0)]));
        let unlit: Vec<&Dot> = f.dots.iter().filter(|d| d.a < 0.2).collect();
        assert!(!unlit.is_empty());
        assert!(unlit
            .iter()
            .all(|d| (d.a - UNLIT_ALPHA).abs() < 1e-12 && d.saturation == 0.0));
    }
}
