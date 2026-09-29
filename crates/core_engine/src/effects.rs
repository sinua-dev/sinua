//! One-shot feedback effects (docs/fx-view.md, *One-shot effects*): `success`,
//! `error` and `celebrate`, played once on top of whatever a view shows, then gone.
//!
//! The same contract as the barge-in flash (`primitives::apply_interrupt`): the
//! view keeps the clock and passes runtime keys, and this post-process draws the
//! effect on the finished frame, so it works on every pattern of every family
//! without touching a mode.
//!
//! - `effectCode`: 1 success, 2 error, 3 celebrate (see [`info`]).
//! - `effectAge`: seconds since the trigger. Nothing is drawn without it or
//!   outside `0..duration`, so a frame without these keys is unchanged.
//! - `effectReduced`: 1 = the reduced-motion variant: no shake, no burst, no
//!   moving ring; only the tint pulse (and the tick, in place).

use crate::primitives::{decay_envelope, Dot, ModeOpts, OrbFrame, Point, Polyline, DECAY_QUAD};

/// What a view needs to play an effect: its code, how long to feed the keys,
/// and the words to speak (item 4's announcer; the app's `labels["effect:<name>"]` win).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
pub struct EffectInfo {
    pub code: u32,
    pub duration: f64,
    pub words: String,
}

pub const SUCCESS: u32 = 1;
pub const ERROR: u32 = 2;
pub const CELEBRATE: u32 = 3;

const SUCCESS_S: f64 = 0.9;
const ERROR_S: f64 = 0.5;
const CELEBRATE_S: f64 = 1.4;

/// The effect called `name`, or `None` for an unknown one.
pub fn info(name: &str) -> Option<EffectInfo> {
    let (code, duration, words) = match name {
        "success" => (SUCCESS, SUCCESS_S, "Done"),
        "error" => (ERROR, ERROR_S, "Something went wrong"),
        "celebrate" => (CELEBRATE, CELEBRATE_S, "Well done"),
        _ => return None,
    };
    Some(EffectInfo {
        code,
        duration,
        words: words.to_owned(),
    })
}

fn duration(code: u32) -> Option<f64> {
    match code {
        SUCCESS => Some(SUCCESS_S),
        ERROR => Some(ERROR_S),
        CELEBRATE => Some(CELEBRATE_S),
        _ => None,
    }
}

/// Centroid and radius (mean distance of every point from it) of the frame.
fn extent(frame: &OrbFrame) -> Option<(f64, f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    pts.extend(frame.dots.iter().map(|d| (d.x, d.y)));
    for l in &frame.lines {
        pts.push((l.x1, l.y1));
        pts.push((l.x2, l.y2));
    }
    for p in &frame.polylines {
        pts.extend(p.points.iter().map(|q| (q.x, q.y)));
    }
    for f in &frame.fills {
        pts.extend(f.points.iter().map(|q| (q.x, q.y)));
    }
    if pts.is_empty() {
        return None;
    }
    let n = pts.len() as f64;
    let cx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let cy = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let r = pts
        .iter()
        .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt())
        .sum::<f64>()
        / n;
    Some((cx, cy, r))
}

/// Tint every element toward `hue` by `k` (0..1): the saturation rises to `0.85 k`
/// in `hue` over the first half, and the original colour comes back through grey
/// (never a hue lerp, which would pass through other colours -- the barge-in
/// flash's rule).
fn tint(frame: &mut OrbFrame, hue: f64, k: f64) {
    if k <= 0.0 {
        return;
    }
    let f = |sat: f64, h: f64| -> (f64, f64) {
        if k >= 0.5 {
            (0.85 * ((k - 0.5) / 0.5), hue)
        } else {
            (sat * (1.0 - k / 0.5), h)
        }
    };
    for d in frame.dots.iter_mut() {
        (d.saturation, d.hue) = f(d.saturation, d.hue);
    }
    for l in frame.lines.iter_mut() {
        (l.saturation, l.hue) = f(l.saturation, l.hue);
    }
    for p in frame.polylines.iter_mut() {
        p.map_hue(f);
    }
    for fl in frame.fills.iter_mut() {
        (fl.saturation, fl.hue) = f(fl.saturation, fl.hue);
    }
}

/// Moves every element by `dx`.
fn shift(frame: &mut OrbFrame, dx: f64) {
    for d in frame.dots.iter_mut() {
        d.x += dx;
    }
    for l in frame.lines.iter_mut() {
        l.x1 += dx;
        l.x2 += dx;
    }
    for p in frame.polylines.iter_mut() {
        for q in p.points.iter_mut() {
            q.x += dx;
        }
    }
    for f in frame.fills.iter_mut() {
        for q in f.points.iter_mut() {
            q.x += dx;
        }
        for h in f.holes.iter_mut() {
            for q in h.iter_mut() {
                q.x += dx;
            }
        }
    }
}

fn ring(cx: f64, cy: f64, r: f64, w: f64, a: f64, hue: f64) -> Polyline {
    let n = 72;
    Polyline {
        points: (0..=n)
            .map(|i| {
                let t = i as f64 / n as f64 * std::f64::consts::TAU;
                Point {
                    x: cx + r * t.cos(),
                    y: cy + r * t.sin(),
                }
            })
            .collect(),
        white: 0.42,
        a,
        w,
        saturation: 0.8,
        hue,
        hues: Vec::new(),
    }
}

/// A check mark of half-width `s` around (cx, cy), drawn `u` (0..1) of the way.
fn tick(cx: f64, cy: f64, s: f64, u: f64, w: f64, a: f64) -> Polyline {
    let p0 = (cx - 0.55 * s, cy + 0.02 * s);
    let p1 = (cx - 0.15 * s, cy + 0.42 * s);
    let p2 = (cx + 0.62 * s, cy - 0.45 * s);
    let l1 = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
    let l2 = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
    let mut len = u.clamp(0.0, 1.0) * (l1 + l2);
    let mut points = vec![Point { x: p0.0, y: p0.1 }];
    let seg = |a: (f64, f64), b: (f64, f64), l: f64, part: f64| Point {
        x: a.0 + (b.0 - a.0) * part / l,
        y: a.1 + (b.1 - a.1) * part / l,
    };
    if len <= l1 {
        points.push(seg(p0, p1, l1, len));
    } else {
        points.push(Point { x: p1.0, y: p1.1 });
        len -= l1;
        points.push(seg(p1, p2, l2, len.min(l2)));
    }
    Polyline {
        points,
        white: 0.38,
        a,
        w,
        saturation: 0.85,
        hue: 140.0,
        hues: Vec::new(),
    }
}

/// A small deterministic hash in [0, 1).
fn hash01(i: u32, salt: u32) -> f64 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x as f64 / 4_294_967_296.0
}

/// Draws the running effect (if any) on top of `frame`; see the module docs.
pub fn apply_effect(mut frame: OrbFrame, size: f64, opts: &ModeOpts) -> OrbFrame {
    let Some(&age) = opts.get("effectAge") else {
        return frame;
    };
    let code = opts.get("effectCode").copied().unwrap_or(0.0) as u32;
    let Some(dur) = duration(code) else {
        return frame;
    };
    if !(0.0..dur).contains(&age) {
        return frame;
    }
    let reduced = opts.get("effectReduced").copied().unwrap_or(0.0) >= 0.5;
    let Some((cx, cy, r0)) = extent(&frame) else {
        return frame;
    };
    let r0 = r0.max(size * 0.12);
    let u = age / dur;
    // The tint pulse: a quick rise (first 15 %), then the shared decay.
    let pulse = if u < 0.15 {
        u / 0.15
    } else {
        decay_envelope(age - 0.15 * dur, 0.85 * dur, DECAY_QUAD)
    };
    match code {
        SUCCESS => {
            tint(&mut frame, 140.0, 0.7 * pulse);
            if !reduced {
                // One ring, expanding from the shape and fading.
                let rr = r0 * (1.05 + 0.55 * u);
                frame.polylines.push(ring(
                    cx,
                    cy,
                    rr,
                    size * 0.018 * (1.0 - u),
                    0.8 * (1.0 - u).powi(2),
                    140.0,
                ));
            }
            // The tick: drawn in over the first 0.25 s (at once when reduced), held, then faded.
            let drawn = if reduced { 1.0 } else { (age / 0.25).min(1.0) };
            let fade = if u < 0.6 { 1.0 } else { 1.0 - (u - 0.6) / 0.4 };
            frame
                .polylines
                .push(tick(cx, cy, r0 * 0.55, drawn, size * 0.045, 0.95 * fade));
        }
        ERROR => {
            // Held red: a quick rise (10 %), full until 60 %, then back through grey. The
            // shared pulse only crossed the grey valley for ~50 ms, too short to read.
            let k = if u < 0.1 {
                u / 0.1
            } else if u < 0.6 {
                1.0
            } else {
                1.0 - (u - 0.6) / 0.4
            };
            tint(&mut frame, 8.0, k);
            if !reduced {
                // Three swings, decaying: 10 % of the size, so a 64 px view visibly shakes.
                let dx = size * 0.1 * (u * 3.0 * std::f64::consts::TAU).sin() * (1.0 - u);
                shift(&mut frame, dx);
            }
        }
        CELEBRATE => {
            tint(&mut frame, 48.0, 0.35 * pulse);
            let lift = 1.0 + 0.3 * pulse;
            for d in frame.dots.iter_mut() {
                d.a = (d.a * lift).min(1.0);
            }
            for p in frame.polylines.iter_mut() {
                p.a = (p.a * lift).min(1.0);
            }
            if !reduced {
                // A seeded burst: out fast, easing, fading.
                let ease = 1.0 - (1.0 - u).powi(3);
                for i in 0..24u32 {
                    let ang = (i as f64 + 0.6 * hash01(i, 1)) / 24.0 * std::f64::consts::TAU;
                    let reach = r0 * (1.1 + (0.9 + 0.8 * hash01(i, 2)) * ease);
                    frame.dots.push(Dot {
                        x: cx + reach * ang.cos(),
                        y: cy + reach * ang.sin(),
                        r: size * (0.012 + 0.012 * hash01(i, 3)) * (1.0 - 0.5 * u),
                        white: 0.45,
                        a: 0.9 * (1.0 - u).powi(2),
                        saturation: 0.8,
                        hue: 360.0 * hash01(i, 4),
                        ..Default::default()
                    });
                }
            }
        }
        _ => {}
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> OrbFrame {
        OrbFrame {
            dots: (0..12)
                .map(|i| {
                    let t = i as f64 / 12.0 * std::f64::consts::TAU;
                    Dot {
                        x: 32.0 + 16.0 * t.cos(),
                        y: 32.0 + 16.0 * t.sin(),
                        r: 1.5,
                        white: 0.2,
                        a: 0.8,
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        }
    }

    fn opts(code: u32, age: f64, reduced: bool) -> ModeOpts {
        [
            ("effectCode".to_string(), code as f64),
            ("effectAge".to_string(), age),
            ("effectReduced".to_string(), if reduced { 1.0 } else { 0.0 }),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn nothing_without_the_keys_or_outside_the_window() {
        let f = frame();
        assert_eq!(apply_effect(f.clone(), 64.0, &ModeOpts::new()), f);
        assert_eq!(
            apply_effect(f.clone(), 64.0, &opts(SUCCESS, 0.95, false)),
            f
        );
        assert_eq!(
            apply_effect(f.clone(), 64.0, &opts(SUCCESS, -0.1, false)),
            f
        );
        assert_eq!(apply_effect(f.clone(), 64.0, &opts(9, 0.2, false)), f);
    }

    #[test]
    fn success_draws_a_ring_and_a_tick_and_turns_green() {
        let out = apply_effect(frame(), 64.0, &opts(SUCCESS, 0.2, false));
        assert_eq!(out.polylines.len(), 2, "ring + tick");
        assert!(out.polylines[0].points.len() > 60, "a ring");
        assert!((out.polylines[1].hue - 140.0).abs() < 1e-9);
        let reduced = apply_effect(frame(), 64.0, &opts(SUCCESS, 0.2, true));
        assert_eq!(
            reduced.polylines.len(),
            1,
            "reduced: the tick only, no moving ring"
        );
        assert_eq!(
            reduced.polylines[0].points.len(),
            3,
            "reduced: the whole tick at once"
        );
    }

    #[test]
    fn error_shakes_both_ways_unless_reduced() {
        let base = frame();
        let x0 = base.dots[0].x;
        let at =
            |t: f64, red: bool| apply_effect(frame(), 64.0, &opts(ERROR, t, red)).dots[0].x - x0;
        assert!(at(0.04, false) > 0.0);
        assert!(at(0.12, false) < 0.0);
        assert_eq!(at(0.12, true), 0.0);
        assert!(
            apply_effect(frame(), 64.0, &opts(ERROR, 0.1, true)).dots[0].saturation > 0.0,
            "the tint still plays"
        );
    }

    #[test]
    fn celebrate_bursts_twenty_four_seeded_dots() {
        let a = apply_effect(frame(), 64.0, &opts(CELEBRATE, 0.5, false));
        let b = apply_effect(frame(), 64.0, &opts(CELEBRATE, 0.5, false));
        assert_eq!(a.dots.len(), 12 + 24);
        assert_eq!(a, b, "deterministic");
        assert_eq!(
            apply_effect(frame(), 64.0, &opts(CELEBRATE, 0.5, true))
                .dots
                .len(),
            12
        );
    }

    #[test]
    fn info_names_the_three() {
        assert_eq!(
            info("success").map(|i| (i.code, i.duration)),
            Some((1, 0.9))
        );
        assert_eq!(
            info("celebrate").map(|i| i.words),
            Some("Well done".to_string())
        );
        assert_eq!(info("confetti"), None);
    }
}
