//! WISP's tests from before recipes (they were in `character/modes/wisp.rs`),
//! now held against its recipe (`spec/characters/wisp.json`): the same claims,
//! the recipe's frame. The constants are the recipe's own values, for the checks.
#![allow(dead_code, unused_imports)]

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, hsl, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn frame_wisp(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("wisp", size, t, o).expect("a built-in recipe")
}

pub const BODY_HUE: f64 = 253.0;
const VIOLET: Hsl = hsl(252.7, 1.0, 0.712);
const BLUE: Hsl = hsl(227.9, 1.0, 0.718);
const TEAL: Hsl = hsl(174.8, 0.662, 0.524);
const LINE: Hsl = hsl(249.7, 0.523, 0.255);
const EYES: Hsl = hsl(247.2, 0.569, 0.2);
const GOLD: Hsl = hsl(46.0, 1.0, 0.739);
const ICE: Hsl = hsl(187.0, 1.0, 0.875);
const WHITE: Hsl = hsl(0.0, 0.0, 1.0);
const HEAD: (f64, f64, f64) = (100.0, 82.0, 46.0);
const FACE: turn::Surface = turn::Surface {
    c: (HEAD.0, HEAD.1),
    r: 52.0,
    depth: 0.55,
    cylinder: false,
};
const TAIL_LAG: f64 = 0.25;
const TAIL_SWING: f64 = 14.0;
const SPARKLES: usize = 5;

/// The old signature, over the sparkles part's own function.
fn sparkle(i: usize, t: f64, float: f64, level: f64, w: [f64; 4]) -> (f64, f64, f64, f64) {
    crate::character::parts::spirit::sparkle(SPARKLES, (HEAD.0, HEAD.1), i, t, float, level, w)
}

mod tests {
    use super::*;
    use crate::primitives::ColorMode;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn bounds(f: &OrbFrame) -> (f64, f64, f64, f64) {
        f.fills
            .iter()
            .flat_map(|x| x.points.iter())
            .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| {
                (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y))
            })
    }

    fn sparkles_of(f: &OrbFrame) -> Vec<&Fill> {
        f.fills
            .iter()
            .filter(|x| x.points.len() == 8 && (x.hue == GOLD.h || x.hue == ICE.h))
            .collect()
    }

    #[test]
    fn stays_in_its_box_at_every_size_and_pose() {
        for size in [20.0, 32.0, 64.0] {
            for o in [
                opts(&[]),
                opts(&[
                    ("tilt", -0.12),
                    ("earGain", 1.0),
                    ("audioLevel", 1.0),
                    ("curlGain", 0.35),
                ]),
                opts(&[("mouthDots", 1.0), ("curlGain", 1.4)]),
                opts(&[
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("squashGain", 0.07),
                    ("bounceGain", 3.0),
                    ("audioLevel", 1.0),
                ]),
                opts(&[("effectCode", 3.0), ("effectAge", 1.2)]),
            ] {
                for t in [0.0, 0.7, 1.3, 2.9] {
                    let (x0, y0, x1, y1) = bounds(&frame_wisp(size, t, &o));
                    assert!(
                        x0 > -0.02 * size
                            && y0 > -0.02 * size
                            && x1 < 1.02 * size
                            && y1 < 1.02 * size,
                        "{size} {t}: {x0} {y0} {x1} {y1}"
                    );
                }
            }
        }
    }

    #[test]
    fn is_a_fixed_colour_frame_of_fills_only_with_detail_by_size() {
        let f = frame_wisp(64.0, 0.0, &opts(&[]));
        assert_eq!(f.color_mode, ColorMode::Fixed);
        assert!(f.dots.is_empty() && f.lines.is_empty() && f.polylines.is_empty());
        let n = |size| frame_wisp(size, 0.0, &opts(&[])).fills.len();
        assert!(n(64.0) > n(32.0) && n(32.0) > n(20.0));
    }

    #[test]
    fn sparkles_gather_orbit_and_stream_by_state() {
        let head = |x: f64, y: f64| ((x - 100.0).powi(2) + (y - 90.0).powi(2)).sqrt();
        let mean_dist = |o: ModeOpts| {
            let (sum, n) = (0..SPARKLES).fold((0.0, 0.0), |(s, n), i| {
                let w = rig::pose(&o, 1.0);
                let (_, dots, talk, _, _) = face::mouth_weights(w.mouth);
                let wv = [(1.0 - w.ears - dots - talk).max(0.0), w.ears, dots, talk];
                let (x, y, _, _) = sparkle(i, 1.0, 0.0, w.drive, wv);
                (s + head(x, y), n + 1.0)
            });
            sum / n
        };
        let loud = mean_dist(opts(&[("earGain", 1.0), ("audioLevel", 1.0)]));
        let quiet = mean_dist(opts(&[("earGain", 1.0), ("audioLevel", 0.0)]));
        assert!(
            loud < quiet,
            "they gather closer as the user speaks: {loud} vs {quiet}"
        );
        // Thinking: they orbit above the face.
        let o = opts(&[("mouthDots", 1.0)]);
        for i in 0..SPARKLES {
            let (_, y, _, _) = sparkle(i, 1.0, 0.0, 0.0, [0.0, 0.0, 1.0, 0.0]);
            assert!(y < 50.0, "orbiting the crown, above the eyes (y {y})");
        }
        assert!(sparkles_of(&frame_wisp(64.0, 1.0, &o)).len() == SPARKLES);
        // accessories 0: none; 20 px: none.
        assert!(sparkles_of(&frame_wisp(64.0, 1.0, &opts(&[("accessories", 0.0)]))).is_empty());
        assert!(sparkles_of(&frame_wisp(20.0, 1.0, &opts(&[]))).is_empty());
    }

    #[test]
    fn a_state_change_glides_the_sparkles_round_the_head_not_across_the_face() {
        // The face: the eyes and the mouth (design units, the head at rest).
        let over_face = |(x, y, _, _): (f64, f64, f64, f64)| {
            (70.0..130.0).contains(&x) && (62.0..114.0).contains(&y)
        };
        for t in [0.3, 1.0, 2.2] {
            for i in 0..SPARKLES {
                for (from, to) in [(0, 1), (1, 2), (2, 3), (0, 3)] {
                    for k in 0..=10 {
                        let u = k as f64 / 10.0;
                        let mut w = [0.0; 4];
                        w[from] = 1.0 - u;
                        w[to] += u;
                        let p = sparkle(i, t, 0.0, 0.6, w);
                        assert!(
                            !over_face(p),
                            "sparkle {i} at {u} of {from}->{to} is over the face: {p:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_mouth_opens_with_the_voice() {
        let oval_height = |lvl: f64| {
            let f = frame_wisp(
                64.0,
                0.3,
                &opts(&[
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("audioLevel", lvl),
                    ("look", 0.0),
                ]),
            );
            let o = f
                .fills
                .iter()
                .rfind(|x| x.points.len() == 24 && x.hue == EYES.h)
                .unwrap();
            let ys: Vec<f64> = o.points.iter().map(|p| p.y).collect();
            ys.iter().cloned().fold(f64::MIN, f64::max)
                - ys.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(oval_height(0.9) > oval_height(0.1) * 2.0);
    }

    #[test]
    fn hue_turns_the_body_but_not_the_sparkles() {
        let base = frame_wisp(64.0, 1.0, &opts(&[("look", 0.0)]));
        let green = frame_wisp(64.0, 1.0, &opts(&[("look", 0.0), ("hue", 133.0)]));
        let hues = |f: &OrbFrame| f.fills.iter().map(|x| x.hue).collect::<Vec<_>>();
        let (a, b) = (hues(&base), hues(&green));
        assert!(
            a.iter()
                .zip(&b)
                .any(|(x, y)| (x - 249.7).abs() < 1e-9 && (y - 129.7).abs() < 1e-9),
            "the line turns"
        );
        assert_eq!(sparkles_of(&base).len(), sparkles_of(&green).len());
        assert!(sparkles_of(&green)
            .iter()
            .all(|s| s.hue == GOLD.h || s.hue == ICE.h));
    }

    #[test]
    fn generic_overrides_leave_it_alone_and_muted_fades() {
        let o = opts(&[("look", 0.0), ("audioLevel", 0.8)]);
        let base = crate::frame_with_overrides("wisp".into(), 64, 0.3, o.clone()).unwrap();
        let mut other = o.clone();
        other.extend(opts(&[
            ("colorMix", 1.0),
            ("colorSaturation", 1.0),
            ("gradientStrength", 1.0),
            ("audioStrength", 0.5),
        ]));
        assert_eq!(
            crate::frame_with_overrides("wisp".into(), 64, 0.3, other).unwrap(),
            base
        );
        let mut muted = o;
        muted.insert("muted".into(), 1.0);
        let m = crate::frame_with_overrides("wisp".into(), 64, 0.3, muted).unwrap();
        let line = |f: &OrbFrame| {
            f.fills
                .iter()
                .find(|x| x.hue == LINE.h)
                .map(|x| x.a)
                .unwrap()
        };
        assert!((line(&m) - line(&base) * 0.7).abs() < 1e-9);
    }

    #[test]
    fn deterministic() {
        let o = opts(&[
            ("seed", 3.0),
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.6),
        ]);
        assert_eq!(frame_wisp(64.0, 2.7, &o), frame_wisp(64.0, 2.7, &o));
    }
}
