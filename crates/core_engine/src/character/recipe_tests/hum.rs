//! HUM's tests from before recipes (they were in `character/modes/hum.rs`),
//! now held against its recipe (`spec/characters/hum.json`): the same claims,
//! the recipe's frame. The constants are the recipe's own values, for the checks.
#![allow(dead_code, unused_imports)]

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, hsl, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn frame_hum(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("hum", size, t, o).expect("a built-in recipe")
}

pub const BODY_HUE: f64 = 9.0;
const BODY: Hsl = hsl(9.2, 0.759, 0.563);
const BODY_L: Hsl = hsl(14.1, 1.0, 0.7);
const BODY_D: Hsl = hsl(7.4, 0.688, 0.39);
const BRASS: Hsl = hsl(39.1, 0.667, 0.553);
const BRASS_D: Hsl = hsl(38.9, 0.695, 0.361);
const GRILLE: Hsl = hsl(12.5, 0.4, 0.118);
const GLOW: Hsl = hsl(39.7, 1.0, 0.739);
const LINE: Hsl = hsl(11.7, 0.657, 0.137);
const TALLY_RED: Hsl = hsl(0.0, 1.0, 0.616);
const TALLY_AMBER: Hsl = hsl(36.1, 1.0, 0.616);
const TALLY_OFF: Hsl = hsl(8.6, 0.452, 0.243);
const WHITE: Hsl = hsl(0.0, 0.0, 1.0);
const PIVOT: (f64, f64) = (100.0, 94.0);
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 93.0),
    r: 44.0,
    depth: 1.0,
    cylinder: true,
};
const SLOTS: usize = 9;

/// The old signature, over the grille part's own function.
fn slot_light(i: usize, t: f64, m: Mouth, off: bool) -> f64 {
    crate::character::parts::mic::slot_light(i, SLOTS, t, m, off)
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

    /// Fills in a colour (by hue), with their alpha.
    fn alpha_of(f: &OrbFrame, c: Hsl) -> f64 {
        f.fills
            .iter()
            .filter(|x| (x.hue - c.h).abs() < 1e-9 && (x.white - c.l).abs() < 1e-9 && x.blur == 0.0)
            .map(|x| x.a)
            .fold(0.0, f64::max)
    }

    #[test]
    fn stays_in_its_box_at_every_size_and_pose() {
        for size in [20.0, 32.0, 64.0] {
            for o in [
                opts(&[]),
                opts(&[("tilt", -0.13), ("earGain", 1.0), ("audioLevel", 1.0)]),
                opts(&[("tilt", 0.1), ("mouthDots", 1.0)]),
                opts(&[
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("swayGain", 0.05),
                    ("bounceGain", 5.0),
                    ("audioLevel", 1.0),
                ]),
                opts(&[("effectCode", 3.0), ("effectAge", 1.2)]),
            ] {
                let (x0, y0, x1, y1) = bounds(&frame_hum(size, 1.3, &o));
                assert!(
                    x0 > -0.02 * size && y0 > -0.02 * size && x1 < 1.02 * size && y1 < 1.02 * size,
                    "{size}: {x0} {y0} {x1} {y1}"
                );
            }
        }
    }

    #[test]
    fn is_a_fixed_colour_frame_of_fills_only_with_detail_by_size() {
        let f = frame_hum(64.0, 0.0, &opts(&[]));
        assert_eq!(f.color_mode, ColorMode::Fixed);
        assert!(f.dots.is_empty() && f.lines.is_empty() && f.polylines.is_empty());
        let n = |size| frame_hum(size, 0.0, &opts(&[])).fills.len();
        assert!(n(64.0) > n(32.0) && n(32.0) > n(20.0));
    }

    #[test]
    fn the_tally_is_red_while_listening_and_amber_while_thinking() {
        let listening = frame_hum(64.0, 0.1, &opts(&[("earGain", 1.0)]));
        assert!(alpha_of(&listening, TALLY_RED) > 0.9);
        // Thinking blinks: somewhere in a blink cycle it is fully amber.
        let lit = (0..20)
            .map(|i| {
                alpha_of(
                    &frame_hum(64.0, i as f64 / 30.0, &opts(&[("mouthDots", 1.0)])),
                    TALLY_AMBER,
                )
            })
            .fold(0.0, f64::max);
        assert!(lit > 0.9);
        let idle = frame_hum(64.0, 0.1, &opts(&[]));
        assert_eq!(alpha_of(&idle, TALLY_RED), 0.0);
        let bare = frame_hum(64.0, 0.1, &opts(&[("earGain", 1.0), ("accessories", 0.0)]));
        assert_eq!(
            alpha_of(&bare, TALLY_RED),
            0.0,
            "accessories 0: no tally light"
        );
    }

    #[test]
    fn the_slots_follow_the_voice_and_rest_in_silence() {
        let lit = |o: ModeOpts| {
            (0..SLOTS)
                .map(|i| slot_light(i, 0.3, rig::pose(&o, 0.3).mouth, false))
                .sum::<f64>()
        };
        let loud = lit(opts(&[
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.9),
        ]));
        let quiet = lit(opts(&[
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.0),
        ]));
        let rest = lit(opts(&[]));
        assert!(loud > 3.0 && quiet < 1.0 && rest < 1.0);
        // Mouth off: they stay at rest whatever the voice.
        let off = (0..SLOTS)
            .map(|i| slot_light(i, 0.3, Mouth::Wave(1.0, 0.0), true))
            .sum::<f64>();
        assert!(off < 1.0);
        let scan: Vec<f64> = (0..SLOTS)
            .map(|i| slot_light(i, 0.3, Mouth::Dots(0.3), false))
            .collect();
        // The scanner moves smoothly, so it can sit between two slots: at most two
        // are lit, one of them brightly, and the rest stay dim.
        assert!(scan.iter().cloned().fold(0.0, f64::max) > 0.5);
        assert!(scan.iter().filter(|v| **v > 0.3).count() <= 2);
    }

    #[test]
    fn a_state_change_slides_the_slots_from_the_scanner_to_the_voice() {
        let blend = Mouth::Blend {
            smile: 0.0,
            dots: 0.5,
            talk: 0.5,
            level: 0.8,
            phase: 0.3,
            o: 0.0,
            frown: 0.0,
        };
        let mid: Vec<f64> = (0..SLOTS)
            .map(|i| slot_light(i, 0.3, blend, false))
            .collect();
        let scan: Vec<f64> = (0..SLOTS)
            .map(|i| slot_light(i, 0.3, Mouth::Dots(0.3), false))
            .collect();
        let talk: Vec<f64> = (0..SLOTS)
            .map(|i| slot_light(i, 0.3, Mouth::Wave(0.8, 0.0), false))
            .collect();
        for i in 0..SLOTS {
            let (lo, hi) = (scan[i].min(talk[i]), scan[i].max(talk[i]));
            assert!(
                mid[i] >= lo - 1e-9 && mid[i] <= hi + 1e-9,
                "slot {i} lies between the two ends"
            );
        }
    }

    #[test]
    fn hue_turns_the_capsule_only() {
        let base = frame_hum(64.0, 0.0, &opts(&[("look", 0.0)]));
        let blue = frame_hum(64.0, 0.0, &opts(&[("look", 0.0), ("hue", 209.0)]));
        let hues = |f: &OrbFrame| f.fills.iter().map(|x| x.hue).collect::<Vec<_>>();
        let (a, b) = (hues(&base), hues(&blue));
        assert!(a
            .iter()
            .zip(&b)
            .any(|(x, y)| (x - 9.2).abs() < 1e-9 && (y - 209.2).abs() < 1e-9));
        assert!(
            a.iter()
                .zip(&b)
                .any(|(x, y)| (x - 39.1).abs() < 1e-9 && (y - 39.1).abs() < 1e-9),
            "brass stays"
        );
    }

    #[test]
    fn generic_overrides_leave_it_alone_and_muted_fades() {
        let o = opts(&[("look", 0.0), ("audioLevel", 0.8)]);
        let base = crate::frame_with_overrides("hum".into(), 64, 0.3, o.clone()).unwrap();
        let mut other = o.clone();
        other.extend(opts(&[
            ("colorMix", 1.0),
            ("colorSaturation", 1.0),
            ("gradientStrength", 1.0),
            ("audioStrength", 0.5),
        ]));
        assert_eq!(
            crate::frame_with_overrides("hum".into(), 64, 0.3, other).unwrap(),
            base
        );
        let mut muted = o;
        muted.insert("muted".into(), 1.0);
        let m = crate::frame_with_overrides("hum".into(), 64, 0.3, muted).unwrap();
        assert!(
            (alpha_of(&m, BRASS) - 0.7).abs() < 1e-9,
            "a 30 % fade, colours kept"
        );
    }

    #[test]
    fn deterministic() {
        let o = opts(&[
            ("seed", 3.0),
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.6),
            ("swayGain", 0.05),
        ]);
        assert_eq!(frame_hum(64.0, 2.7, &o), frame_hum(64.0, 2.7, &o));
    }
}
