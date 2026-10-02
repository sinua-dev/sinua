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

/// The tap hop (design note 15): a crouch, a hop of `HOP_LIFT` design units and
/// a squash on landing, with a smile; for the first two thirds the eyes glance
/// toward the tap (`tapX` / `tapY`, -1..1 from the view's centre) and come back.
/// The voice state goes on underneath: the mouth, the ears and the lids stay
/// its own, and nothing lasts past the hop. Reduced motion: only the smile.
fn hop(pose: &mut Pose, o: &ModeOpts, age: f64, reduced: bool) {
    let Some(dur) = crate::effects::duration_of(crate::effects::HOP) else {
        return;
    };
    if !(0.0..dur).contains(&age) {
        return;
    }
    let u = age / dur;
    let arc = ((u - 0.1) / 0.75).clamp(0.0, 1.0);
    let lift = (std::f64::consts::PI * arc).sin();
    pose.eyes.smile = pose.eyes.smile.max(0.7 * (std::f64::consts::PI * u).sin());
    if reduced {
        return;
    }
    pose.bob -= HOP_LIFT * lift;
    pose.squash += if u < 0.1 {
        0.05 * u / 0.1
    } else if u > 0.85 {
        0.07 * (1.0 - (u - 0.85) / 0.15)
    } else {
        0.0
    };
    if let (Some(&x), Some(&y)) = (o.get("tapX"), o.get("tapY")) {
        let g = if u < 0.67 {
            (std::f64::consts::PI * u / 0.67).sin()
        } else {
            0.0
        };
        pose.eyes.gx += (x.clamp(-1.0, 1.0) * 9.0 - pose.eyes.gx) * g;
        pose.eyes.gy += (y.clamp(-1.0, 1.0) * 5.0 - pose.eyes.gy) * g;
    }
}

/// The expressions (design note 16), as the opts that weigh them (0..1).
pub const EXPRESSIONS: [(&str, &str); 5] = [
    ("happy", "expressionHappy"),
    ("surprised", "expressionSurprised"),
    ("thoughtful", "expressionThoughtful"),
    ("sad", "expressionSad"),
    ("sleepy", "expressionSleepy"),
];

/// The expressions' weights in `o`, in [`EXPRESSIONS`] order; scaled down
/// together when they add up to more than 1.
fn expression_weights(o: &ModeOpts) -> [f64; 5] {
    let mut w = EXPRESSIONS.map(|(_, k)| get(o, k, 0.0).clamp(0.0, 1.0));
    let sum: f64 = w.iter().sum();
    if sum > 1.0 {
        w.iter_mut().for_each(|x| *x /= sum);
    }
    w
}

/// An expression owns the eyes' shape (design note 16): each weight pulls the
/// voice state's eyes toward its own. The gaze, the turn and the blinks stay
/// the voice state's.
fn express_eyes(e: &mut Eyes, w: [f64; 5]) {
    let (w0, h0, r0) = (e.w, e.h, e.r);
    let mut to = |w: f64, f: &dyn Fn(&mut Eyes)| {
        if w > 0.0 {
            let mut t = *e;
            f(&mut t);
            e.smile += (t.smile - e.smile) * w;
            e.lid += (t.lid - e.lid) * w;
            e.tilt += (t.tilt - e.tilt) * w;
            e.asym += (t.asym - e.asym) * w;
            e.w += (t.w - e.w) * w;
            e.h += (t.h - e.h) * w;
            e.r += (t.r - e.r) * w;
        }
    };
    to(w[0], &|t| t.smile = 0.8);
    to(w[1], &|t| {
        (t.w, t.h, t.r) = (w0 * 1.2, h0 * 1.3, r0 + 3.0);
        (t.lid, t.tilt, t.smile) = (0.0, 0.0, 0.0);
    });
    to(w[2], &|t| (t.lid, t.asym, t.tilt) = (0.25, 0.17, 0.2));
    to(w[3], &|t| {
        (t.lid, t.tilt, t.h, t.smile) = (0.3, 0.6, h0 * 0.85, 0.0);
    });
    to(w[4], &|t| (t.lid, t.tilt, t.smile) = (0.62, 0.1, 0.0));
}

/// How high the tap hop lifts the body, in design units (of 200).
const HOP_LIFT: f64 = 9.0;

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
    let ex = expression_weights(o);
    express_eyes(&mut eyes, ex);
    // Living motion: glances, blinks, breathing.
    // `still` (design note 25): a thumbnail's pose: no glance, no blink.
    let still = get(o, "still", 0.0) >= 0.5;
    let look = get(o, "look", 1.0).clamp(0.0, 2.0);
    if look > 0.0 && !reduced && !still {
        let (gx, gy) = glance(t, seed);
        eyes.gx += gx * look;
        eyes.gy += gy * look;
    }
    // A loadout change swaps the eye style while the eyes are shut (`wearBlink`).
    let mut shut = if still { 0.0 } else { blink(t, seed) }.max(get(o, "wearBlink", 0.0));
    // The turn blink: a new state that asks for it blinks once at its start.
    let turn = get(o, "turnBlink", 0.0).clamp(0.0, 1.0);
    if let Some(&age) = o.get("stateAge") {
        shut = shut.max(turn * bump(age, TURN_BLINK_S));
    }
    // Sleepy breathes slowly and deep.
    let breath = get(o, "breath", 0.0).max(ex[4]);
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
    let rest = (1.0 - talk - dots).clamp(0.0, 1.0);
    // The expression's resting mouth takes the rest share; talk and dots stay the voice's.
    let (mo, mf) = (rest * ex[1], rest * ex[3]);
    let smile = rest - mo - mf;
    let line_level = voice.clamp(0.0, 1.0);
    let mouth = if get(o, "mouth", 1.0) < 0.5 {
        Mouth::None
    } else if mo > 0.0 || mf > 0.0 {
        Mouth::Blend {
            smile,
            dots,
            talk,
            level: line_level,
            phase: t,
            o: mo,
            frown: mf,
        }
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
            o: 0.0,
            frown: 0.0,
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
        if code == crate::effects::HOP {
            hop(&mut pose, o, age, reduced);
            return pose;
        }
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
    fn expressions_own_the_eyes_and_the_resting_mouth_not_the_voice() {
        let p = |kv: &[(&str, f64)]| pose(&opts(kv), 1.0);
        let plain = p(&[("look", 0.0)]);
        assert!(p(&[("look", 0.0), ("expressionHappy", 1.0)]).eyes.smile > 0.79);
        let s = p(&[("look", 0.0), ("expressionSurprised", 1.0)]);
        assert!(s.eyes.h > plain.eyes.h * 1.29 && s.eyes.lid == 0.0);
        assert!(
            matches!(s.mouth, Mouth::Blend { o, .. } if (o - 1.0).abs() < 1e-12),
            "an O at rest"
        );
        let sad = p(&[("look", 0.0), ("expressionSad", 1.0)]);
        assert!((sad.eyes.tilt - 0.6).abs() < 1e-12);
        assert!(matches!(sad.mouth, Mouth::Blend { frown, .. } if (frown - 1.0).abs() < 1e-12));
        assert!((p(&[("look", 0.0), ("expressionSleepy", 1.0)]).eyes.lid - 0.62).abs() < 1e-12);
        let t = p(&[("look", 0.0), ("expressionThoughtful", 1.0)]);
        assert!((t.eyes.asym - 0.17).abs() < 1e-12);
        // Half weight: half way.
        let half = p(&[("look", 0.0), ("expressionSleepy", 0.5)]);
        assert!((half.eyes.lid - 0.31).abs() < 1e-12);
        // Speaking: the voice's mouth stays; the expression keeps only its share of the rest.
        let talk = [
            ("look", 0.0),
            ("mouthTalk", 1.0),
            ("audioLevel", 0.7),
            ("mouthGain", 1.0),
        ];
        let mut sad_talk = talk.to_vec();
        sad_talk.push(("expressionSad", 1.0));
        assert_eq!(
            format!("{:?}", p(&sad_talk).mouth),
            format!("{:?}", p(&talk).mouth)
        );
        assert!(
            (p(&sad_talk).eyes.tilt - 0.6).abs() < 1e-12,
            "and the eyes are still sad"
        );
        // The gaze stays the voice state's.
        let gaze = [("look", 0.0), ("gazeX", -8.0), ("expressionHappy", 1.0)];
        assert_eq!(p(&gaze).eyes.gx, -8.0);
        // No expression: the old pose, exactly.
        assert_eq!(
            format!("{:?}", p(&[("look", 0.0), ("expressionHappy", 0.0)])),
            format!("{plain:?}")
        );
    }

    #[test]
    fn the_tap_hop_lifts_lands_and_leaves_the_voice_state_alone() {
        let speaking = [
            ("mouthTalk", 1.0),
            ("audioLevel", 0.6),
            ("mouthGain", 1.0),
            ("earGain", 0.0),
            ("look", 0.0),
        ];
        let at = |age: f64, extra: &[(&str, f64)]| {
            let mut kv: Vec<(&str, f64)> = speaking.to_vec();
            kv.extend_from_slice(&[("effectCode", 4.0), ("effectAge", age)]);
            kv.extend_from_slice(extra);
            pose(&opts(&kv), 1.0)
        };
        let plain = pose(&opts(&speaking), 1.0);
        // Up in the middle, down at both ends; the voice's mouth goes on.
        assert!(at(0.3, &[]).bob < plain.bob - 8.0);
        assert!((at(0.0, &[]).bob - plain.bob).abs() < 1e-9);
        assert!(at(0.03, &[]).squash > plain.squash, "a crouch first");
        assert!(at(0.55, &[]).squash > plain.squash, "a squash on landing");
        assert_eq!(
            format!("{:?}", at(0.3, &[]).mouth),
            format!("{:?}", plain.mouth)
        );
        assert!(at(0.3, &[]).eyes.smile > 0.5);
        // Over: nothing lasts.
        assert_eq!(format!("{:?}", at(0.6, &[])), format!("{plain:?}"));
        // The glance: toward the tap, then back.
        assert!(at(0.2, &[("tapX", 1.0), ("tapY", -1.0)]).eyes.gx > 6.0);
        assert!(at(0.2, &[("tapX", 1.0), ("tapY", -1.0)]).eyes.gy < -3.0);
        assert!((at(0.5, &[("tapX", 1.0), ("tapY", 0.0)]).eyes.gx - plain.eyes.gx).abs() < 1e-9);
        // Reduced motion: only the smile.
        let r = at(0.3, &[("effectReduced", 1.0), ("tapX", 1.0), ("tapY", 0.0)]);
        assert!((r.bob - plain.bob).abs() < 1e-9 && (r.eyes.gx - plain.eyes.gx).abs() < 1e-9);
        assert!(r.eyes.smile > 0.5);
    }

    #[test]
    fn other_families_draw_nothing_for_the_hop() {
        let mut o: ModeOpts = opts(&[("effectCode", 4.0), ("effectAge", 0.3)]);
        let hopped = crate::frame_with_overrides("breathing".into(), 64, 1.0, o.clone()).unwrap();
        o.clear();
        let plain = crate::frame_with_overrides("breathing".into(), 64, 1.0, o).unwrap();
        assert_eq!(format!("{hopped:?}"), format!("{plain:?}"));
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
