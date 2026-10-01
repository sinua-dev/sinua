//! Character / HUM: a vintage studio microphone that hosts the show (`hum`).
//!
//! Design (design-07, the user's pick from the cast sheet,
//! sinua-studio/docs/agents/families/assets/prototype-cast-*.png): a red
//! capsule on a brass yoke and stand, from the talking-object tradition
//! (Luxo Jr.'s lamp, 1930s rubber-hose cartoons). Its face sits on the grille
//! band; the **grille is its mouth**.
//!
//! Voice:
//! - the grille slots light with the level while speaking (amplitude only);
//! - thinking runs one light back and forth along them;
//! - a **tally light** on top is red while it listens ("on air" for the
//!   user), blinks amber while it thinks, and is off otherwise;
//! - the capsule tips toward you on its yoke while listening and sways on it
//!   while speaking.
//!
//! Every one of these reads the pose's continuous numbers (`earGain`, the
//! mouth weights, `tilt`, `swayGain`), so a state change slides the tilt and
//! fades the tally and the slots instead of switching them.
//!
//! Drawn entirely as fills, in order (see `character/geom.rs`), in a 200-unit
//! design box scaled to `size`. `Fixed` colour; `hue` turns the capsule's red.
//! Sizes: 64 draws everything; 32 drops the chrome glints and the grille's edge
//! lines; 20 keeps the stand, capsule, eyes, slots and tally, with heavier lines.

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, blurred, hsl, linear, outline, pt, solid, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// The capsule's own hue; `hue` rotates the capsule by `hue - BODY_HUE`.
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

/// The capsule turns about this point, where the yoke holds it.
const PIVOT: (f64, f64) = (100.0, 94.0);
/// What the face turns on: the capsule is an upright cylinder, so turning
/// slides the eyes and the slots round it and bunches them at the edge; the
/// silhouette and the band's rings stay as they are.
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 93.0),
    r: 44.0,
    depth: 1.0,
    cylinder: true,
};
/// Grille slots (the mouth).
const SLOTS: usize = 9;

/// How lit slot `i` is (0..1): resting glow, the thinking scanner and the voice,
/// each at its weight.
fn slot_light(i: usize, t: f64, m: Mouth, off: bool) -> f64 {
    let rest_glow = 0.05 + 0.03 * (t * 2.0 + i as f64).sin();
    if off {
        return rest_glow;
    }
    let (rest, scan, talk, level, phase) = face::mouth_weights(m);
    let u = i as f64 / (SLOTS - 1) as f64;
    // Thinking: one light sweeping to and fro (a smooth ping-pong).
    let pos = ((phase * 2.4).sin() * 0.5 + 0.5) * (SLOTS - 1) as f64;
    let scanner = (1.0 - (i as f64 - pos).abs() / 1.3).max(0.0);
    // Speaking: the level, highest in the middle, with a little flicker.
    let env = (u * PI).sin().powf(0.8);
    let voice = (level * env * (1.2 + 0.5 * (i as f64 * 2.3 + t * 11.0).sin())).clamp(0.0, 1.0);
    (rest * rest_glow + scan * (0.1 + 0.9 * scanner) + talk * voice.max(rest_glow)).clamp(0.0, 1.0)
}

pub fn frame_hum(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let pose = rig::pose(o, t);
    let tier = kit::tier(size);
    // The head turn: the capsule turns on its yoke (0 = the flat character).
    let tn = turn::angles(o, t, tier);
    let (small, full) = (tier.small, tier.full);
    let accessories = get(o, "accessories", 1.0) >= 0.5;
    let mouth_off = get(o, "mouth", 1.0) < 0.5;
    let dh = get(o, "hue", BODY_HUE) - BODY_HUE;
    let (body, body_l, body_d) = (BODY.rotate(dh), BODY_L.rotate(dh), BODY_D.rotate(dh));
    let lw = tier.line;

    let scale = Xf::scale(size / 200.0, size / 200.0);
    // The whole microphone: the error shake and the bob.
    let whole = Xf::translate(pose.shake, 0.0).then(scale);
    // The capsule on its pivot: tilt (listening leans in, thinking tips back),
    // the speaking sway, a small squash with the voice, and the lean.
    let sway = get(o, "swayGain", 0.0) * (t * 9.0).sin() * (0.4 + pose.drive);
    let capsule = Xf::translate(-PIVOT.0, -PIVOT.1)
        .then(Xf::scale(1.0 + pose.squash, 1.0 - pose.squash))
        .then(Xf::rotate(pose.tilt + sway))
        .then(Xf::translate(PIVOT.0, PIVOT.1 + pose.bob + pose.lean * 0.5))
        .then(whole);
    let yoke_xf = Xf::translate(0.0, pose.bob * 0.3).then(whole);

    let mut out: Vec<Fill> = Vec::new();
    let put = |out: &mut Vec<Fill>, f: Fill, xf: &Xf| out.push(geom::transform(f, xf));

    // Floor shadow and the stand.
    put(
        &mut out,
        kit::ground_shadow(100.0, 186.0, 40.0, 5.2),
        &scale,
    );
    put(
        &mut out,
        solid(
            geom::ellipse(100.0, 181.0, 34.0, 7.0, 0.0, 40),
            BRASS_D,
            1.0,
        ),
        &whole,
    );
    let base = geom::ellipse(100.0, 178.0, 34.0, 7.0, 0.0, 40);
    put(&mut out, solid(base.clone(), BRASS, 1.0), &whole);
    put(&mut out, outline(&base, 2.5 * lw, LINE, 1.0), &whole);
    let stem = geom::round_rect(96.0, 148.0, 8.0, 30.0, 4.0, 3.0);
    put(&mut out, solid(stem.clone(), BRASS, 1.0), &whole);
    put(&mut out, outline(&stem, 2.5 * lw, LINE, 1.0), &whole);

    // The yoke: a U holding the capsule at its sides (drawn under the capsule).
    let yoke = geom::join(&[
        vec![pt(48.0, 94.0), pt(48.0, 128.0)],
        geom::quad((48.0, 128.0), (48.0, 150.0), (100.0, 150.0), 12),
        geom::quad((100.0, 150.0), (152.0, 150.0), (152.0, 128.0), 12),
        vec![pt(152.0, 94.0)],
    ]);
    put(
        &mut out,
        solid(geom::stroke(&yoke, 9.0 * lw), LINE, 1.0),
        &yoke_xf,
    );
    put(
        &mut out,
        solid(geom::stroke(&yoke, 5.0 * lw), BRASS, 1.0),
        &yoke_xf,
    );

    // The capsule and its grille band (the face).
    let cap = geom::round_rect(56.0, 34.0, 88.0, 118.0, 44.0, 3.0);
    let mut c: Vec<Fill> = Vec::new();
    c.push(linear(
        cap.clone(),
        (56.0, 0.0),
        (144.0, 0.0),
        &[(0.0, body_l), (0.45, body), (1.0, body_d)],
    ));
    let band = geom::clip_convex(&geom::round_rect(40.0, 66.0, 120.0, 60.0, 0.0, 4.0), &cap);
    c.push(solid(band.clone(), GRILLE, 1.0));
    if !small {
        for y in [64.0, 125.0] {
            let edge = geom::clip_convex(&geom::round_rect(40.0, y, 120.0, 3.0, 0.0, 4.0), &cap);
            if edge.len() >= 3 {
                c.push(solid(edge, WHITE, 0.18));
            }
        }
    }
    // The slots: the mouth.
    let ink = kit::effect_ink(&pose, GLOW);
    for i in 0..SLOTS {
        let on = slot_light(i, t, pose.mouth, mouth_off);
        let (w, h) = if small {
            (6.0, 4.0 + on * 10.0)
        } else {
            (4.2, 3.0 + on * 8.0)
        };
        let x = 66.0 + i as f64 * 7.6 - (w - 4.2) / 2.0;
        let slot = geom::round_rect(x, 112.0 - 1.5 - on * 4.0, w, h, w / 2.0, 2.0);
        let slot = if tn.is_zero() {
            slot
        } else {
            tn.map_points(&FACE, &slot)
        };
        let slot = geom::clip_convex(&slot, &band);
        if slot.len() < 3 {
            continue;
        }
        if on > 0.15 && !small {
            c.push(blurred(slot.clone(), ink, 0.6 * on, 3.0));
        }
        c.push(solid(slot, ink, 0.12 + 0.88 * on));
    }
    // Chrome glints, clipped to the capsule.
    if !small && full {
        for (y, h) in [(40.0, 22.0), (132.0, 12.0)] {
            let g = geom::clip_convex(&geom::round_rect(68.0, y, 10.0, h, 5.0, 2.0), &cap);
            if g.len() >= 3 {
                c.push(solid(g, WHITE, 0.35));
            }
        }
    }
    // The eyes, on the band.
    let f = Face {
        cx: 100.0,
        cy: 90.0,
        scale: if small { 0.95 } else { 0.82 },
        ink,
        glow: if small { 2.0 } else { 4.5 },
        clip: Some(&band),
    };
    let eyes = face::eye_fills(&f, &pose.eyes);
    if tn.is_zero() {
        c.extend(eyes);
    } else {
        c.extend(eyes.into_iter().map(|e| tn.map_fill(e, &FACE)));
    }
    c.push(outline(&cap, 3.5 * lw, LINE, 1.0));

    // The tally light: red while listening, amber blinking while thinking.
    let tally = geom::ellipse(100.0, 28.0, 6.0, 6.0, 0.0, 24);
    c.push(solid(tally.clone(), TALLY_OFF, 1.0));
    if accessories {
        let (_, scan, _, _, _) = face::mouth_weights(pose.mouth);
        let blink = 0.5 + 0.5 * (t * TAU * 1.5).sin();
        for (col, a) in [(TALLY_RED, pose.ears), (TALLY_AMBER, scan * blink)] {
            if a > 0.02 {
                if !small {
                    c.push(blurred(tally.clone(), col, 0.7 * a, 4.0));
                }
                c.push(solid(tally.clone(), col, a));
            }
        }
    }
    c.push(outline(&tally, 2.5 * lw, LINE, 1.0));

    // Celebrate: the shared ring of sparkles, in Hum's colours.
    c.extend(kit::celebrate_burst(
        &pose,
        o,
        tier,
        (100.0, 92.0),
        78.0,
        [CELEBRATE_INK, TALLY_AMBER, GLOW],
    ));
    out.extend(c.into_iter().map(|f| geom::transform(f, &capsule)));

    kit::finish(out, o)
}

#[cfg(test)]
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
