//! The character rig: the pose every character is drawn in, from the opts
//! and the clock. Pure: the same opts and `t` give the same pose on every
//! platform.
//!
//! Two layers:
//! - **Designed values** (`eyeW`, `gazeX`, `lean`, …) come from the
//!   voice-state profile per state (`spec/voice-state-profile.json`,
//!   `patterns.<character>`), or from a spec's `params`. They are plain
//!   numbers, so a state change interpolates them (`transition.rs`,
//!   `params`): the eyes *slide* up when thinking starts.
//! - **Living motion** is added here from `t`, `seed` and the runtime
//!   inputs: blinks on a seeded schedule, glances, breathing, the blink at
//!   the end of the user's turn (`stateAge`), the startle on a barge-in
//!   (`interruptAge`), the mute squint (`muted`) and the effect
//!   expressions (`effectCode` / `effectAge`).

use super::face::{EyeKind, Eyes, Mouth};
use crate::primitives::{hash_d, ModeOpts};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Seconds per blink slot; one blink lands somewhere in each.
const BLINK_SLOT: f64 = 4.5;
/// A blink closes and opens over this long.
const BLINK_S: f64 = 0.16;
/// The turn blink runs over the first this-long of a new state.
const TURN_BLINK_S: f64 = 0.18;
/// Seconds per glance slot while `look` > 0.
const LOOK_SLOT: f64 = 5.0;
/// How long a glance holds (with a quick move in and out).
const LOOK_HOLD: f64 = 1.3;
/// The barge-in startle's length (the interrupt flash's default duration).
const STARTLE_S: f64 = 0.45;

/// The pose a character is drawn in this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eyes: Eyes,
    pub mouth: Mouth,
    /// Whole-body lean toward the viewer (design units, down = toward).
    pub lean: f64,
    /// Whole-body tilt (radians, + = clockwise).
    pub tilt: f64,
    /// Squash-stretch about the feet (+ = wider and shorter).
    pub squash: f64,
    /// Vertical offset (design units, - = up).
    pub bob: f64,
    /// Horizontal offset (the error shake).
    pub shake: f64,
    /// The voice drive for accessories (ear arcs, chest bars), 0..1.
    pub drive: f64,
    /// How strongly the listening accessories (ear arcs) show, 0..1.
    pub ears: f64,
    /// A one-shot effect's code (0 = none) and its progress 0..1.
    pub effect: u32,
    pub effect_u: f64,
}

/// 0..1..0 over `len` seconds starting at 0 (a smooth bump), 0 outside.
fn bump(x: f64, len: f64) -> f64 {
    if !(0.0..len).contains(&x) {
        return 0.0;
    }
    (std::f64::consts::PI * x / len).sin()
}

/// How closed the eyes are from the seeded blink schedule at time `t`.
pub fn blink(t: f64, seed: f64) -> f64 {
    let slot = (t / BLINK_SLOT).floor();
    let at = slot * BLINK_SLOT + 0.4 + hash_d(seed + 17.0, slot) * (BLINK_SLOT - 1.0);
    bump(t - at, BLINK_S)
}

/// The glance offset `(gx, gy)` at time `t` (design units, before `look`).
pub fn glance(t: f64, seed: f64) -> (f64, f64) {
    let slot = (t / LOOK_SLOT).floor();
    let start = slot * LOOK_SLOT + 0.5 + hash_d(seed + 3.0, slot) * (LOOK_SLOT - LOOK_HOLD - 1.0);
    let x = t - start;
    if !(0.0..LOOK_HOLD).contains(&x) {
        return (0.0, 0.0);
    }
    // Move in over 0.15 s, hold, move out over the last 0.15 s.
    let k = (x / 0.15)
        .min(1.0)
        .min((LOOK_HOLD - x) / 0.15)
        .clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    let a = hash_d(seed + 9.0, slot) * std::f64::consts::TAU;
    (a.cos() * 7.0 * k, a.sin() * 3.5 * k)
}

pub fn pose(o: &ModeOpts, t: f64) -> Pose {
    let seed = get(o, "seed", 0.0);
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0);
    let mut eyes = Eyes {
        w: get(o, "eyeW", 20.0),
        h: get(o, "eyeH", 26.0),
        r: get(o, "eyeR", 8.0),
        lid: get(o, "lid", 0.0).clamp(0.0, 0.9),
        tilt: get(o, "eyeTilt", 0.0).clamp(-1.0, 1.0),
        smile: get(o, "eyeSmile", 0.0).clamp(0.0, 1.0),
        asym: get(o, "eyeAsym", 0.0).clamp(0.0, 0.6),
        open: 1.0,
        gx: get(o, "gazeX", 0.0),
        gy: get(o, "gazeY", 0.0),
        gap: 1.0,
        kind: EyeKind::Shape,
    };
    let reduced = o.get("effectReduced").is_some_and(|v| *v >= 0.5);
    // Living motion: glances, blinks, breathing.
    let look = get(o, "look", 1.0).clamp(0.0, 2.0);
    if look > 0.0 && !reduced {
        let (gx, gy) = glance(t, seed);
        eyes.gx += gx * look;
        eyes.gy += gy * look;
    }
    let mut shut = blink(t, seed);
    // The turn blink: a new state that asks for it blinks once at its start.
    let turn = get(o, "turnBlink", 0.0).clamp(0.0, 1.0);
    if let Some(&age) = o.get("stateAge") {
        shut = shut.max(turn * bump(age, TURN_BLINK_S));
    }
    let breath = get(o, "breath", 0.0);
    let mut bob = -(t * 1.6).sin().abs() * 1.5 * breath;
    // Mute / connection lost: a heavy-lidded rest.
    let muted = get(o, "muted", 0.0).clamp(0.0, 1.0);
    eyes.lid = eyes.lid.max(0.45 * muted);
    // Barge-in: eyes wide, a small hop back.
    if let Some(&age) = o.get("interruptAge") {
        if (0.0..STARTLE_S).contains(&age) {
            let startle = (1.0 - age / STARTLE_S).powi(2);
            shut = 0.0;
            eyes.w *= 1.0 + 0.25 * startle;
            eyes.h *= 1.0 + 0.3 * startle;
            eyes.lid *= 1.0 - startle;
            eyes.gap = 1.0 + 0.06 * startle;
            bob -= 4.0 * startle;
        }
    }
    eyes.open = 1.0 - shut * 0.94;
    // The mouth: `mouth` 0 turns it off; otherwise the pose's weights pick.
    let voice = level * get(o, "mouthGain", 0.0);
    // The shapes are weights, so a state change fades one mouth into the next
    // (the pose interpolates `mouthTalk` / `mouthDots` like every other number).
    let talk = get(o, "mouthTalk", 0.0).clamp(0.0, 1.0);
    let dots = get(o, "mouthDots", 0.0).clamp(0.0, 1.0);
    let smile = (1.0 - talk - dots).clamp(0.0, 1.0);
    let line_level = voice.clamp(0.0, 1.0);
    let mouth = if get(o, "mouth", 1.0) < 0.5 {
        Mouth::None
    } else if dots >= 1.0 && talk == 0.0 {
        Mouth::Dots(t)
    } else if talk >= 1.0 && dots == 0.0 {
        Mouth::Wave(line_level, if reduced { 0.0 } else { t })
    } else if smile >= 1.0 {
        Mouth::Smile
    } else {
        Mouth::Blend {
            smile,
            dots,
            talk,
            level: line_level,
            phase: t,
        }
    };
    let mut pose = Pose {
        eyes,
        mouth,
        lean: get(o, "lean", 0.0),
        tilt: get(o, "tilt", 0.0),
        squash: level * get(o, "squashGain", 0.0),
        bob: bob - level * get(o, "bounceGain", 0.0),
        shake: 0.0,
        drive: level,
        ears: get(o, "earGain", 0.0).clamp(0.0, 1.0),
        effect: 0,
        effect_u: 0.0,
    };
    // One-shot effects become expressions. `crate::effects` draws the generic
    // ring / tick / burst for other families; a character's face *is* the effect.
    if let (Some(&age), Some(&code)) = (o.get("effectAge"), o.get("effectCode")) {
        let code = code as u32;
        if let Some(dur) = crate::effects::duration_of(code) {
            if (0.0..dur).contains(&age) {
                pose.effect = code;
                pose.effect_u = age / dur;
                pose.eyes.open = 1.0;
                pose.eyes.lid = 0.0;
                pose.eyes.tilt = 0.0;
                pose.eyes.asym = 0.0;
                match code {
                    crate::effects::SUCCESS => {
                        pose.eyes.smile = 0.9;
                        pose.mouth = Mouth::Grin;
                    }
                    crate::effects::ERROR => {
                        pose.eyes.kind = EyeKind::Cross;
                        pose.mouth = Mouth::Wave(0.5, if reduced { 0.0 } else { t });
                        if !reduced {
                            // Three decaying swings, like the generic error shake.
                            let u = pose.effect_u;
                            pose.shake = (u * 6.0 * std::f64::consts::PI).sin() * 4.0 * (1.0 - u);
                        }
                    }
                    crate::effects::CELEBRATE => {
                        pose.eyes.kind = EyeKind::Star;
                        pose.mouth = Mouth::Grin;
                    }
                    _ => {}
                }
            }
        }
    }
    pose
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn blinks_happen_about_once_a_slot_and_depend_on_the_seed() {
        let count = |seed: f64| {
            let mut n = 0;
            let mut was = false;
            for i in 0..(45 * 100) {
                let b = blink(i as f64 / 100.0, seed) > 0.5;
                if b && !was {
                    n += 1;
                }
                was = b;
            }
            n
        };
        assert_eq!(count(0.0), 10); // 45 s / 4.5 s
        let times = |seed: f64| {
            (0..900)
                .filter(|i| blink(*i as f64 / 20.0, seed) > 0.5)
                .collect::<Vec<_>>()
        };
        assert_ne!(
            times(0.0),
            times(5.0),
            "two characters must not blink in sync"
        );
    }

    #[test]
    fn the_turn_blink_closes_the_eyes_at_the_start_of_a_state() {
        let o = opts(&[("turnBlink", 1.0), ("stateAge", 0.09), ("look", 0.0)]);
        // t chosen between scheduled blinks.
        let p = pose(&o, 0.1);
        assert!(p.eyes.open < 0.2);
        let later = pose(
            &opts(&[("turnBlink", 1.0), ("stateAge", 0.5), ("look", 0.0)]),
            0.1,
        );
        assert_eq!(later.eyes.open, 1.0);
    }

    #[test]
    fn the_mouth_follows_the_voice_only_while_talking() {
        let talk = opts(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.8)]);
        assert_eq!(pose(&talk, 0.0).mouth, Mouth::Wave(0.8, 0.0));
        let silent = opts(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.0)]);
        assert_eq!(pose(&silent, 0.0).mouth, Mouth::Wave(0.0, 0.0));
        assert_eq!(pose(&opts(&[("audioLevel", 0.8)]), 0.0).mouth, Mouth::Smile);
        let off = opts(&[("mouth", 0.0), ("mouthTalk", 1.0), ("audioLevel", 0.8)]);
        assert_eq!(pose(&off, 0.0).mouth, Mouth::None);
        assert!(matches!(
            pose(&opts(&[("mouthDots", 1.0)]), 0.0).mouth,
            Mouth::Dots(_)
        ));
    }

    #[test]
    fn a_state_change_fades_one_mouth_into_the_next() {
        // Halfway from thinking (dots) to speaking (voice line): both, at half weight.
        let mid = pose(
            &opts(&[
                ("mouthDots", 0.5),
                ("mouthTalk", 0.5),
                ("mouthGain", 1.0),
                ("audioLevel", 0.6),
            ]),
            1.0,
        );
        match mid.mouth {
            Mouth::Blend {
                smile,
                dots,
                talk,
                level,
                ..
            } => {
                assert_eq!((smile, dots, talk), (0.0, 0.5, 0.5));
                assert!((level - 0.6).abs() < 1e-12);
            }
            m => panic!("expected a blend, got {m:?}"),
        }
        // A quarter of the way from idle (smile) to thinking (dots).
        let q = pose(&opts(&[("mouthDots", 0.25)]), 1.0);
        assert!(
            matches!(q.mouth, Mouth::Blend { smile, dots, .. } if (smile - 0.75).abs() < 1e-12 && dots == 0.25)
        );
        // The ends are the plain shapes (so a settled state draws exactly as before).
        assert!(matches!(
            pose(&opts(&[("mouthDots", 1.0)]), 1.0).mouth,
            Mouth::Dots(_)
        ));
        assert_eq!(pose(&opts(&[]), 1.0).mouth, Mouth::Smile);
    }

    #[test]
    fn a_barge_in_opens_the_eyes_wide_and_fades() {
        let base = pose(&opts(&[("look", 0.0)]), 0.1);
        let hit = pose(&opts(&[("interruptAge", 0.0), ("look", 0.0)]), 0.1);
        assert!(hit.eyes.h > base.eyes.h * 1.25 && hit.bob < base.bob);
        let over = pose(&opts(&[("interruptAge", 0.6), ("look", 0.0)]), 0.1);
        assert_eq!(over.eyes.h, base.eyes.h);
    }

    #[test]
    fn effects_become_expressions() {
        let s = pose(&opts(&[("effectCode", 1.0), ("effectAge", 0.2)]), 0.0);
        assert!(s.eyes.smile > 0.8 && s.mouth == Mouth::Grin);
        let e = pose(&opts(&[("effectCode", 2.0), ("effectAge", 0.1)]), 0.0);
        assert_eq!(e.eyes.kind, EyeKind::Cross);
        assert!(e.shake.abs() > 0.0);
        let reduced = pose(
            &opts(&[
                ("effectCode", 2.0),
                ("effectAge", 0.1),
                ("effectReduced", 1.0),
            ]),
            0.0,
        );
        assert_eq!(reduced.shake, 0.0);
        let c = pose(&opts(&[("effectCode", 3.0), ("effectAge", 0.5)]), 0.0);
        assert_eq!(c.eyes.kind, EyeKind::Star);
        let done = pose(&opts(&[("effectCode", 3.0), ("effectAge", 5.0)]), 0.0);
        assert_eq!(done.effect, 0);
    }

    #[test]
    fn muted_squints() {
        let p = pose(&opts(&[("muted", 1.0), ("look", 0.0)]), 0.1);
        assert!((p.eyes.lid - 0.45).abs() < 1e-9);
    }

    #[test]
    fn glances_stay_small_and_return_to_centre() {
        for i in 0..2000 {
            let (x, y) = glance(i as f64 / 40.0, 1.0);
            assert!(x.abs() <= 7.0 + 1e-9 && y.abs() <= 3.5 + 1e-9);
        }
        let rest = (0..2000)
            .filter(|i| glance(*i as f64 / 40.0, 1.0) == (0.0, 0.0))
            .count();
        assert!(rest > 1000, "mostly looking at the viewer");
    }
}
