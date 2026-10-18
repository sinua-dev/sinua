//! CLANK, the walker bot (1.15): a standing robot with a white shell, a
//! dark visor with big teal eyes, teal ear lights and a chest light, jointed arms (the left
//! one waving, with fingers), legs and boots.
//!
//! Its own rig: it stands and shifts its weight from foot to foot (the torso and head sway
//! over planted legs, the head a little late), the hanging arm swings from the shoulder,
//! the raised hand waves and its fingers follow, the eyes blink and glance. The four voice
//! states, from its profile weights (the character keys: `earGain` listening, `mouthDots`
//! thinking, `mouthTalk` speaking):
//! - listening: the head tilts in, the ear lights pulse, the eyes widen;
//! - thinking: looks up and away, the head lifts, the antenna tip pulses fast;
//! - speaking: the mouth and the chest light follow the voice, the head nods on it.
//!
//! At `t = 0` with no options the parts sit exactly where the drawing has them.

use std::f64::consts::TAU;
use std::sync::OnceLock;

use crate::character::bots::{about, follow, get, load, spring, Bot};
use crate::character::geom::Xf;
use crate::character::{kit, rig};
use crate::primitives::{ModeOpts, OrbFrame};

const DATA: &str = include_str!("../../../../../spec/characters/bots/clank.json");
static BOT: OnceLock<Bot> = OnceLock::new();

/// The weight shift's rate (Hz) and reach (design units).
const SHIFT_HZ: f64 = 0.28;
const SHIFT: f64 = 1.6;
/// The soles' y in the design box.
const FLOOR: f64 = 186.5;

struct Ids {
    shadow: usize,
    leg_l: usize,
    leg_r: usize,
    torso: usize,
    chest: usize,
    head: usize,
    antenna: usize,
    tip: usize,
    visor: usize,
    eye_l: usize,
    eye_r: usize,
    mouth: usize,
    ear_light_l: usize,
    ear_light_r: usize,
    arm_l: usize,
    fore_l: usize,
    hand_l: usize,
    fingers_l: [usize; 3],
    arm_r: usize,
    fore_r: usize,
    hand_r: usize,
}

fn ids(b: &Bot) -> &'static Ids {
    static IDS: OnceLock<Ids> = OnceLock::new();
    IDS.get_or_init(|| Ids {
        shadow: b.part("floorShadow"),
        leg_l: b.part("legL"),
        leg_r: b.part("legR"),
        torso: b.part("torso"),
        chest: b.part("chestLight"),
        head: b.part("head"),
        antenna: b.part("antenna"),
        tip: b.part("antennaTip"),
        visor: b.part("visor"),
        eye_l: b.part("eyeL"),
        eye_r: b.part("eyeR"),
        mouth: b.part("mouth"),
        ear_light_l: b.part("earLightL"),
        ear_light_r: b.part("earLightR"),
        arm_l: b.part("armL"),
        fore_l: b.part("forearmL"),
        hand_l: b.part("handL"),
        fingers_l: [b.part("fingerL1"), b.part("fingerL2"), b.part("fingerL3")],
        arm_r: b.part("armR"),
        fore_r: b.part("forearmR"),
        hand_r: b.part("handR"),
    })
}

pub fn frame(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let b = load(&BOT, DATA);
    let p = ids(b);
    let pose = rig::pose(o, t);
    let reduced = kit::reduced(o);
    let small = kit::tier(size).small;
    let listen = get(o, "earGain", 0.0).clamp(0.0, 1.0);
    let think = get(o, "mouthDots", 0.0).clamp(0.0, 1.0);
    let talk = get(o, "mouthTalk", 0.0).clamp(0.0, 1.0);
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0) * get(o, "mouthGain", 0.0);
    let m = if reduced { 0.0 } else { 1.0 };

    let piv = |i: usize| b.parts[i].pivot;
    let mut local = vec![Xf::ID; b.parts.len()];
    let mut alpha = vec![1.0; b.parts.len()];

    // The weight shift: the torso leans over one hip, then the other (0 at t = 0); the
    // legs stay planted and the loaded one gives a little. Breathing lifts the chest.
    let shift = (TAU * SHIFT_HZ * t).sin() * m;
    let breath = (TAU * 0.25 * t).sin() * m * get(o, "breath", 0.0).clamp(0.0, 1.0);
    let lean = 0.025 * shift + pose.tilt * 0.5;
    local[p.torso] = about(
        piv(p.torso),
        lean,
        (1.0, 1.0 + 0.006 * breath),
        (SHIFT * shift + pose.shake, pose.bob),
    );
    // The loaded leg gives about its sole, so the boots stay on the floor.
    let sole = |i: usize| (piv(i).0, FLOOR);
    local[p.leg_l] = about(
        sole(p.leg_l),
        0.0,
        (1.0, 1.0 - 0.012 * shift.max(0.0)),
        (0.0, 0.0),
    );
    local[p.leg_r] = about(
        sole(p.leg_r),
        0.0,
        (1.0, 1.0 + 0.012 * shift.min(0.0)),
        (0.0, 0.0),
    );
    local[p.shadow] = about(piv(p.shadow), 0.0, (1.0, 1.0), (0.6 * SHIFT * shift, 0.0));

    // The head rides the torso a little late, tilts in to listen, lifts to think, and
    // nods on the voice.
    let late = follow(t, SHIFT_HZ, 1.1, 0.4) * m;
    // A new state: the head bobs and settles (overshoot), the antenna a beat behind it.
    let age = o.get("stateAge").copied().unwrap_or(f64::INFINITY);
    let settle = spring(age, 1.8, 0.3) * m;
    let settle_late = spring(age - 0.08, 1.8, 0.3) * m;
    let nod = 0.05 * level * talk;
    let head_r = -0.02 * late - 0.09 * listen - 0.05 * think + nod + pose.tilt * 0.5;
    local[p.head] = about(
        piv(p.head),
        head_r + 0.035 * settle,
        (1.0, 1.0),
        (0.0, -1.5 * think + 1.0 * listen - 1.2 * settle),
    );

    // Antenna: follows the head late; the tip pulses fast while thinking.
    let sway = follow(t, SHIFT_HZ, 0.8, 0.3) * m;
    local[p.antenna] = about(
        piv(p.antenna),
        0.06 * sway + 0.14 * settle_late,
        (1.0, 1.0),
        (0.0, 0.0),
    );
    let fast = 0.5 + 0.5 * (TAU * 2.6 * t).sin() * m;
    let tip = 1.0 + think * (0.15 + 0.3 * fast);
    local[p.tip] = about(piv(p.tip), 0.0, (tip, tip), (0.0, 0.0));

    // Ear lights pulse to listen.
    let slow = 0.5 + 0.5 * (TAU * 1.2 * t).sin() * m;
    let ear = 1.0 + listen * (0.12 + 0.18 * slow);
    for e in [p.ear_light_l, p.ear_light_r] {
        local[e] = about(piv(e), 0.0, (ear, ear), (0.0, 0.0));
    }

    // Eyes: gaze, blink about each eye's centre, wide to listen, narrowed to think, a
    // smile squash while talking.
    let (gx, gy) = (
        (pose.eyes.gx * 0.4 - 4.0 * think).clamp(-5.0, 5.0),
        (pose.eyes.gy * 0.4 - 4.0 * think).clamp(-5.0, 3.0),
    );
    let wide = 1.0 + 0.1 * listen;
    let shut = pose.eyes.open
        * (1.0 - 0.5 * pose.eyes.lid)
        * (1.0 - 0.12 * think)
        * (1.0 - 0.12 * pose.eyes.smile);
    for e in [p.eye_l, p.eye_r] {
        local[e] = about(piv(e), 0.0, (wide, wide * shut), (gx, gy));
    }
    local[p.visor] = about(piv(p.visor), 0.0, (1.0, 1.0), (gx * 0.2, gy * 0.15));

    // The mouth opens with the voice; a small line while thinking.
    let open = talk * level;
    local[p.mouth] = about(
        piv(p.mouth),
        0.0,
        (
            1.0 + 0.15 * open - 0.3 * think,
            1.0 + 1.6 * open - 0.3 * think,
        ),
        (gx * 0.5, gy * 0.5),
    );
    // The chest light swells and dims with the voice.
    let chest = 1.0 + 0.3 * open;
    local[p.chest] = about(piv(p.chest), 0.0, (chest, chest), (0.0, 0.0));
    alpha[p.chest] = 0.85 + 0.15 * (1.0 - talk) + 0.15 * open;

    // Arms: the hanging one swings from the shoulder against the shift (late); the raised
    // hand waves and its fingers follow. (Arm poses that cross the body wait for gestures:
    // the drawing paints the legs over the hanging hand.)
    let swing = follow(t, SHIFT_HZ, 0.9, 0.35) * m;
    local[p.arm_r] = about(
        piv(p.arm_r),
        -0.05 * swing + 0.1 * listen,
        (1.0, 1.0),
        (0.0, 0.0),
    );
    local[p.fore_r] = about(piv(p.fore_r), 0.03 * swing, (1.0, 1.0), (0.0, 0.0));
    local[p.hand_r] = about(piv(p.hand_r), 0.03 * swing, (1.0, 1.0), (0.0, 0.0));
    let wave = (TAU * 0.7 * t).sin() * m;
    local[p.arm_l] = about(piv(p.arm_l), 0.02 * late, (1.0, 1.0), (0.0, 0.0));
    local[p.fore_l] = about(piv(p.fore_l), 0.04 * wave, (1.0, 1.0), (0.0, 0.0));
    local[p.hand_l] = about(piv(p.hand_l), 0.1 * wave, (1.0, 1.0), (0.0, 0.0));
    let flick = follow(t, 0.7, 2.2, 0.3) * m;
    for (i, f) in p.fingers_l.iter().enumerate() {
        local[*f] = about(
            piv(*f),
            0.06 * flick * (1.0 + 0.3 * i as f64),
            (1.0, 1.0),
            (0.0, 0.0),
        );
    }

    if small {
        alpha[p.shadow] = 0.0;
    }
    // `mouth` 0 draws no mouth.
    if get(o, "mouth", 1.0) < 0.5 {
        alpha[p.mouth] = 0.0;
    }

    let view = Xf::scale(size / 200.0, size / 200.0);
    let world = b.world(&local, view);
    kit::finish(b.draw(&world, &alpha, size), o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn the_data_reads_and_every_part_is_drawn() {
        let b = load(&BOT, DATA);
        assert_eq!(b.parts.len(), 25);
        assert_eq!(frame(200.0, 0.0, &ModeOpts::new()).fills.len(), 102);
    }

    #[test]
    fn at_rest_the_parts_sit_where_the_drawing_has_them() {
        let a = frame(200.0, 0.0, &ModeOpts::new());
        let b = load(&BOT, DATA);
        let world = b.world(&vec![Xf::ID; b.parts.len()], Xf::ID);
        let still = b.draw(&world, &vec![1.0; b.parts.len()], 200.0);
        assert_eq!(a.fills.len(), still.len());
        for (x, y) in a.fills.iter().zip(&still) {
            assert_eq!(x.a, y.a);
            for (p, q) in x.points.iter().zip(&y.points) {
                assert!((p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn the_four_states_differ_and_the_voice_moves_the_mouth() {
        let at = |kv: &[(&str, f64)]| frame(200.0, 1.3, &opts(kv));
        let idle = at(&[("breath", 1.0)]);
        let listen = at(&[("earGain", 1.0)]);
        let think = at(&[("mouthDots", 1.0)]);
        let quiet = at(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.0)]);
        let loud = at(&[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.9)]);
        for (x, y) in [
            (&idle, &listen),
            (&idle, &think),
            (&listen, &think),
            (&idle, &loud),
            (&quiet, &loud),
        ] {
            assert_ne!(x.fills, y.fills);
        }
    }

    #[test]
    fn the_feet_stay_planted_while_the_weight_shifts() {
        // The boots (the last leg shapes) don't move; the torso does.
        let lowest = |f: &OrbFrame| {
            f.fills[82]
                .points
                .iter()
                .map(|p| p.y)
                .fold(f64::MIN, f64::max)
        };
        let torso_x = |f: &OrbFrame| f.fills[26].points.iter().map(|p| p.x).sum::<f64>();
        let (a, b) = (
            frame(200.0, 0.0, &ModeOpts::new()),
            frame(200.0, 1.0 / SHIFT_HZ / 4.0, &ModeOpts::new()),
        );
        assert!((lowest(&a) - lowest(&b)).abs() < 1e-6);
        assert!((torso_x(&a) - torso_x(&b)).abs() > 1.0);
    }

    /// Writes frames for the visual check (`SINUA_BOT_DUMP=<file>`), like the hover bot's.
    #[test]
    #[ignore]
    fn dump_frames() {
        let Ok(path) = std::env::var("SINUA_BOT_DUMP") else {
            return;
        };
        crate::character::bots::dump(&path, frame);
    }
}
