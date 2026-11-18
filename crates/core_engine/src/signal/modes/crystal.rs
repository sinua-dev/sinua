//! Signal / Crystal (1.15, a light volume): a crystal of light filled from below. Each
//! column is a band; its cells light from the bottom up to the column's value, and the unlit
//! cells stay as dust, so the volume reads even when it's empty. The meter's successor.
//!
//! Inputs, none tied to the voice:
//! - `level` (0..1): one value for the whole crystal (a battery, a quota, a goal), with a
//!   little life across the columns.
//! - bands (`audioBand0..` + `audioBandCount`): a value per column (a bound voice, or any
//!   app's several values), resampled to the columns.
//! - `churn` (0..1): restless work: cells light and fade on their own.
//! - `columnCount` (4..24, default 16).
//!
//! A bound voice's bands count only while it listens or speaks ([`volume::bands`]). With no
//! input it idles low and drifts. Reduced motion holds the camera and the noise still.

use crate::primitives::{ModeOpts, OrbFrame};
use crate::volume::{self, Camera, Light, Tone};

fn get(o: &ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

const ROWS: usize = 11;
const DEPTH: usize = 3;

pub fn frame_crystal(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let still = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    let t = if still { 0.0 } else { t };
    let churn = get(o, "churn", 0.0).clamp(0.0, 1.0);
    let cols_full = get(o, "columnCount", 16.0).round().clamp(4.0, 24.0) as usize;
    let total = cols_full * ROWS * DEPTH;
    let budget = volume::budget(volume::view_px(o, size));
    // Small views drop depth first, then resolution.
    let depth = if total > budget { 1 } else { DEPTH };
    let cols = volume::fit(cols_full, cols_full * ROWS * depth, budget, 2).max(4);
    let rows = volume::fit(ROWS, cols_full * ROWS * depth, budget, 2).max(4);
    let bands = volume::bands(o);
    let level = o.get("level").map(|v| v.clamp(0.0, 1.0));
    // A fixed angle (no sway), slightly from the side so the layers' depth shows; zoomed so the
    // columns stay inside the view.
    let cam = Camera {
        zoom: 1.05,
        ..Camera::new(0.25, 0.3)
    };
    let mut light = Light::new(
        cam,
        size,
        size,
        volume::px_per_unit(o, size),
        cols * rows * depth,
    );
    let span = 0.12 * 15.0;
    for i in 0..cols {
        let fi = i as f64 / (cols - 1) as f64;
        let fill = if let Some(v) = level {
            v * (0.9 + 0.2 * volume::noise(fi * 3.0, t * 0.45, 4.0).abs())
        } else if !bands.is_empty() {
            bands[((fi * bands.len() as f64) as usize).min(bands.len() - 1)].clamp(0.0, 1.0)
        } else {
            // At rest: a low, slowly drifting fill, so the crystal is there and alive.
            0.2 + 0.14 * volume::noise(fi * 3.5, t * 0.5, 0.0)
        } + churn * 0.4 * (0.5 + 0.5 * volume::noise(fi * 6.4, 0.0, t * 1.5));
        for j in 0..rows {
            let fj = j as f64 / (rows - 1) as f64;
            for k in 0..depth {
                let x = -span / 2.0 + fi * span;
                let y = -0.6 + fj * 1.2;
                let z = if depth == 1 {
                    0.0
                } else {
                    -0.15 + k as f64 * 0.15
                };
                let on = fj < fill * 1.1;
                let (b, tone, s) = if on {
                    (0.5 + fj * 0.6, Tone::Ramp(fj), 1.0)
                } else {
                    (
                        0.05 + 0.04 * volume::noise(i as f64, j as f64, t),
                        Tone::Ramp(0.9),
                        0.6,
                    )
                };
                light.emit(x, y, z, b, tone, s);
            }
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

    fn lit(f: &OrbFrame) -> usize {
        f.dots.iter().filter(|d| d.a > 0.3).count()
    }

    #[test]
    fn a_value_fills_it_from_below() {
        let empty = frame_crystal(200.0, 1.0, &opts(&[("level", 0.0)]));
        let half = frame_crystal(200.0, 1.0, &opts(&[("level", 0.5)]));
        let full = frame_crystal(200.0, 1.0, &opts(&[("level", 1.0)]));
        assert!(lit(&empty) < lit(&half) && lit(&half) < lit(&full));
        // The unlit cells stay as dust: the volume is there even when empty.
        assert!(empty.dots.len() > 300);
    }

    #[test]
    fn bands_fill_their_own_columns() {
        let mut o = opts(&[("audioBandCount", 4.0)]);
        for (i, v) in [1.0, 0.0, 1.0, 0.0].iter().enumerate() {
            o.insert(format!("audioBand{i}"), *v);
        }
        let f = frame_crystal(200.0, 1.0, &o);
        assert!(lit(&f) > 100);
        assert_ne!(f, frame_crystal(200.0, 1.0, &opts(&[("level", 0.5)])));
    }

    #[test]
    fn it_idles_alone_and_works_with_churn() {
        let idle = frame_crystal(200.0, 1.0, &ModeOpts::new());
        assert!(!idle.dots.is_empty());
        assert!(lit(&frame_crystal(200.0, 1.0, &opts(&[("churn", 1.0)]))) > lit(&idle));
    }

    #[test]
    fn small_views_draw_fewer_points() {
        assert!(frame_crystal(48.0, 1.0, &ModeOpts::new()).dots.len() <= 160);
    }
}
