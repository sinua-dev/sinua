//! CHIRP's tests from before recipes (they were in `character/modes/chirp.rs`),
//! now held against its recipe (`spec/characters/chirp.json`): the same claims,
//! the recipe's frame. The constants are the recipe's own values, for the checks.
#![allow(dead_code, unused_imports)]

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, hsl, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn frame_chirp(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("chirp", size, t, o).expect("a built-in recipe")
}

pub const BODY_HUE: f64 = 12.0;
const BODY: Hsl = hsl(11.9, 1.0, 0.675);
const BODY_L: Hsl = hsl(14.6, 1.0, 0.767);
const BODY_D: Hsl = hsl(11.6, 0.691, 0.518);
const BELLY: Hsl = hsl(27.8, 1.0, 0.92);
const TEAL: Hsl = hsl(174.8, 0.707, 0.416);
const BEAK: Hsl = hsl(41.4, 1.0, 0.614);
const BEAK_D: Hsl = hsl(39.3, 1.0, 0.425);
const LINE: Hsl = hsl(11.4, 0.643, 0.176);
const EYES: Hsl = hsl(10.0, 0.273, 0.129);
const PIVOT: (f64, f64) = (100.0, 170.0);
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 110.0),
    r: 58.0,
    depth: 0.8,
    cylinder: false,
};
const BALL: turn::Surface = turn::Surface { depth: 1.0, ..FACE };

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

    /// The lowest point of the lower beak (its tip): lower = more open.
    fn beak_tip(f: &OrbFrame) -> f64 {
        f.fills
            .iter()
            .filter(|x| x.points.len() == 3 && x.hue == BEAK_D.h)
            .flat_map(|x| x.points.iter().map(|p| p.y))
            .fold(f64::MIN, f64::max)
    }

    /// The top of the crest (smaller y = higher).
    fn crest_top(f: &OrbFrame) -> f64 {
        f.fills
            .iter()
            .filter(|x| x.hue == TEAL.h && x.points.len() > 30)
            .flat_map(|x| x.points.iter().map(|p| p.y))
            .fold(f64::MAX, f64::min)
    }

    fn teal_count(f: &OrbFrame, n: usize) -> usize {
        f.fills
            .iter()
            .filter(|x| x.hue == TEAL.h && x.points.len() == n)
            .count()
    }

    #[test]
    fn stays_in_its_box_at_every_size_and_pose() {
        for size in [20.0, 32.0, 64.0] {
            for o in [
                opts(&[]),
                opts(&[("tilt", -0.2), ("earGain", 1.0), ("audioLevel", 1.0)]),
                opts(&[("mouthDots", 1.0)]),
                opts(&[
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("flutterGain", 0.3),
                    ("bounceGain", 4.0),
                    ("audioLevel", 1.0),
                ]),
                opts(&[("effectCode", 3.0), ("effectAge", 1.2)]),
            ] {
                for t in [0.0, 0.7, 1.3] {
                    let (x0, y0, x1, y1) = bounds(&frame_chirp(size, t, &o));
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
        let f = frame_chirp(64.0, 0.0, &opts(&[]));
        assert_eq!(f.color_mode, ColorMode::Fixed);
        assert!(f.dots.is_empty() && f.lines.is_empty() && f.polylines.is_empty());
        let talking = opts(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.8)]);
        assert!(
            frame_chirp(64.0, 0.3, &talking).fills.len()
                > frame_chirp(20.0, 0.3, &talking).fills.len()
        );
    }

    #[test]
    fn the_beak_opens_with_the_voice_and_closes_in_silence() {
        let at = |lvl: f64| {
            beak_tip(&frame_chirp(
                64.0,
                0.3,
                &opts(&[
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("audioLevel", lvl),
                    ("look", 0.0),
                ]),
            ))
        };
        assert!(at(0.9) > at(0.0) + 3.0);
        let off = beak_tip(&frame_chirp(
            64.0,
            0.3,
            &opts(&[
                ("mouth", 0.0),
                ("mouthTalk", 1.0),
                ("mouthGain", 1.0),
                ("audioLevel", 0.9),
            ]),
        ));
        assert!((off - at(0.0)).abs() < 0.5);
    }

    #[test]
    fn the_crest_rises_to_listen_and_droops_to_think() {
        let rest = crest_top(&frame_chirp(64.0, 0.3, &opts(&[])));
        let listen = crest_top(&frame_chirp(
            64.0,
            0.3,
            &opts(&[("earGain", 1.0), ("audioLevel", 1.0)]),
        ));
        let think = crest_top(&frame_chirp(64.0, 0.3, &opts(&[("mouthDots", 1.0)])));
        assert!(listen < rest && rest < think, "{listen} {rest} {think}");
    }

    #[test]
    fn notes_only_while_speaking_and_thought_dots_only_while_thinking() {
        let talking = frame_chirp(
            64.0,
            0.3,
            &opts(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.8)]),
        );
        let thinking = frame_chirp(64.0, 0.3, &opts(&[("mouthDots", 1.0)]));
        let idle = frame_chirp(64.0, 0.3, &opts(&[]));
        assert!(teal_count(&talking, 16) >= 2, "note heads");
        assert_eq!(teal_count(&idle, 16), 0);
        assert!(teal_count(&thinking, 16) >= 2, "thought dots");
        let bare = frame_chirp(
            64.0,
            0.3,
            &opts(&[("mouthDots", 1.0), ("accessories", 0.0)]),
        );
        assert_eq!(teal_count(&bare, 16), 0, "accessories 0: no dots, no notes");
    }

    #[test]
    fn hue_turns_the_body_only() {
        let base = frame_chirp(64.0, 0.0, &opts(&[("look", 0.0)]));
        let blue = frame_chirp(64.0, 0.0, &opts(&[("look", 0.0), ("hue", 212.0)]));
        let hues = |f: &OrbFrame| f.fills.iter().map(|x| x.hue).collect::<Vec<_>>();
        let (a, b) = (hues(&base), hues(&blue));
        assert!(a
            .iter()
            .zip(&b)
            .any(|(x, y)| (x - BODY.h).abs() < 1e-9 && (y - (BODY.h + 200.0)).abs() < 1e-9));
        assert!(
            a.iter()
                .zip(&b)
                .any(|(x, y)| (x - TEAL.h).abs() < 1e-9 && (y - TEAL.h).abs() < 1e-9),
            "teal stays"
        );
    }

    #[test]
    fn generic_overrides_leave_it_alone_and_muted_fades() {
        let o = opts(&[("look", 0.0), ("audioLevel", 0.8)]);
        let base = crate::frame_with_overrides("chirp".into(), 64, 0.3, o.clone()).unwrap();
        let mut other = o.clone();
        other.extend(opts(&[
            ("colorMix", 1.0),
            ("colorSaturation", 1.0),
            ("gradientStrength", 1.0),
            ("audioStrength", 0.5),
        ]));
        assert_eq!(
            crate::frame_with_overrides("chirp".into(), 64, 0.3, other).unwrap(),
            base
        );
        let mut muted = o;
        muted.insert("muted".into(), 1.0);
        let m = crate::frame_with_overrides("chirp".into(), 64, 0.3, muted).unwrap();
        let belly = |f: &OrbFrame| {
            f.fills
                .iter()
                .find(|x| x.hue == BELLY.h)
                .map(|x| x.a)
                .unwrap()
        };
        assert!((belly(&m) - belly(&base) * 0.7).abs() < 1e-9);
    }

    #[test]
    fn deterministic() {
        let o = opts(&[
            ("seed", 3.0),
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.6),
            ("flutterGain", 0.3),
        ]);
        assert_eq!(frame_chirp(64.0, 2.7, &o), frame_chirp(64.0, 2.7, &o));
    }
}
