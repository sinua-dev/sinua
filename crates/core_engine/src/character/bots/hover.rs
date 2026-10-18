//! WHIRR, the hover bot (1.15): an egg-shaped robot floating over its
//! shadow, a screen face on a dark visor, an antenna with a glowing tip, floating hands.
//!
//! Its own rig: it always hovers (a slow bob the shadow answers), the antenna and the hands
//! follow the bob a little late (springs), the eyes blink and glance on the visor. The
//! four voice states, from its profile weights:
//! - listening: leans in with a head tilt, eyes wide, the antenna tip lit and slow-pulsing;
//! - thinking: looks up and away, eyes narrowed, the antenna tip pulsing fast;
//! - speaking: the mouth and the belt glow follow the voice, the eyes smile, the bob lifts.
//!
//! At `t = 0` with no options the parts sit exactly where the drawing has them.

use std::f64::consts::TAU;
use std::sync::OnceLock;

use crate::character::bots::{about, follow, get, load, spring, Bot};
use crate::character::geom::Xf;
use crate::character::{kit, rig};
use crate::primitives::{ModeOpts, OrbFrame};

const DATA: &str = include_str!("../../../../../spec/characters/bots/whirr.json");
static BOT: OnceLock<Bot> = OnceLock::new();

/// The hover's period (s) and height (design units).
const HOVER_HZ: f64 = 0.42;
const HOVER: f64 = 3.2;

/// Part indices, looked up once.
struct Ids {
    shadow: usize,
    body: usize,
    belt: usize,
    glow: usize,
    antenna: usize,
    tip: usize,
    visor: usize,
    eye_l: usize,
    eye_r: usize,
    mouth: usize,
    arm_l: usize,
    hand_l: usize,
    arm_r: usize,
    hand_r: usize,
}

fn ids(b: &Bot) -> &'static Ids {
    static IDS: OnceLock<Ids> = OnceLock::new();
    IDS.get_or_init(|| Ids {
        shadow: b.part("floorShadow"),
        body: b.part("body"),
        belt: b.part("belt"),
        glow: b.part("underGlow"),
        antenna: b.part("antenna"),
        tip: b.part("antennaTip"),
        visor: b.part("visor"),
        eye_l: b.part("eyeL"),
        eye_r: b.part("eyeR"),
        mouth: b.part("mouth"),
        arm_l: b.part("armL"),
        hand_l: b.part("handL"),
        arm_r: b.part("armR"),
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
    // Motion that runs on its own stops under reduced motion; the state poses stay.
    let m = if reduced { 0.0 } else { 1.0 };

    let piv = |i: usize| b.parts[i].pivot;
    let mut local = vec![Xf::ID; b.parts.len()];
    let mut alpha = vec![1.0; b.parts.len()];

    // The hover: a sine bob (0 at t = 0), deeper while talking, plus the shared rig's
    // breath and startle hop, and a lift on the voice.
    let wave = (TAU * HOVER_HZ * t).sin() * m;
    let lift = HOVER * (1.0 + 0.3 * talk) * wave;
    // A new state: a small lift that settles (overshoot), the antenna and hands a beat
    // behind it (follow-through). `stateAge` is the view's time since the change.
    let age = o.get("stateAge").copied().unwrap_or(f64::INFINITY);
    let settle = spring(age, 1.6, 0.28) * m;
    let settle_late = spring(age - 0.08, 1.6, 0.28) * m;
    let dy = -lift + pose.bob - 2.5 * level * talk - 3.0 * settle;
    let tilt = pose.tilt - 0.07 * listen + 0.05 * think;
    local[p.body] = about(
        piv(p.body),
        tilt,
        (1.0, 1.0),
        (pose.shake + 2.0 * listen, dy + 2.0 * listen),
    );

    // The shadow stays on the floor: smaller and fainter as the body rises.
    let rise = (-dy / (HOVER * 1.6)).clamp(-1.0, 1.0);
    local[p.shadow] = about(
        piv(p.shadow),
        0.0,
        (1.0 - 0.08 * rise, 1.0 - 0.08 * rise),
        (pose.shake, 0.0),
    );
    alpha[p.shadow] = 1.0 - 0.25 * rise;

    // The glow under it breathes with the hover and swells with the voice.
    let g = 1.0 + 0.05 * wave + 0.3 * level * talk;
    local[p.glow] = about(piv(p.glow), 0.0, (g, g), (0.0, 0.0));
    // The belt light: a little brighter band on the voice.
    local[p.belt] = about(
        piv(p.belt),
        0.0,
        (1.0, 1.0 + 0.25 * level * talk),
        (0.0, 0.0),
    );

    // The antenna follows the hover late (a spring), and leans while thinking.
    let sway = follow(t, HOVER_HZ, 1.6, 0.35) * m;
    local[p.antenna] = about(
        piv(p.antenna),
        0.07 * sway + 0.12 * think - 0.05 * listen + 0.12 * settle_late,
        (1.0, 1.0),
        (0.0, 0.0),
    );
    // The tip: lit and slow-pulsing to listen, fast-pulsing to think.
    let slow = 0.5 + 0.5 * (TAU * 1.1 * t).sin() * m;
    let fast = 0.5 + 0.5 * (TAU * 2.6 * t).sin() * m;
    let tip = 1.0 + listen * (0.3 + 0.15 * slow) + think * (0.1 + 0.3 * fast);
    local[p.tip] = about(piv(p.tip), 0.0, (tip, tip), (0.0, 0.0));

    // Eyes on the visor: gaze, blink (about each eye's centre), wide to listen, narrowed
    // to think, a smile squash while talking. The pivots are the eye centres.
    // Thinking looks up and away on its own (the profile's gaze adds to it).
    let (gx, gy) = (
        (pose.eyes.gx * 0.45 - 5.0 * think).clamp(-7.0, 7.0),
        (pose.eyes.gy * 0.45 - 4.5 * think).clamp(-6.0, 4.0),
    );
    let wide = 1.0 + 0.14 * listen;
    let shut = pose.eyes.open
        * (1.0 - 0.5 * pose.eyes.lid)
        * (1.0 - 0.12 * think)
        * (1.0 - 0.15 * pose.eyes.smile);
    for e in [p.eye_l, p.eye_r] {
        local[e] = about(piv(e), 0.0, (wide, wide * shut), (gx, gy));
    }
    // The visor shifts a touch with the gaze (the face turns toward it).
    local[p.visor] = about(piv(p.visor), 0.0, (1.0, 1.0), (gx * 0.25, gy * 0.2));

    // The mouth opens with the voice (talking), and shrinks to a small line while thinking.
    let open = talk * level;
    let mx = 1.0 + 0.15 * open - 0.35 * think;
    let my = 1.0 + 1.8 * open - 0.3 * think;
    local[p.mouth] = about(piv(p.mouth), 0.0, (mx, my), (gx * 0.6, gy * 0.6));

    // Hands float on their own springs behind the body; the raised one waves a little.
    let lag_l = follow(t, HOVER_HZ, 1.2, 0.3) * m;
    let lag_r = follow(t, HOVER_HZ, 0.9, 0.45) * m;
    local[p.arm_l] = about(
        piv(p.arm_l),
        0.05 * lag_l - 0.15 * listen,
        (1.0, 1.0),
        (0.0, 1.2 * lag_l + 2.0 * settle_late),
    );
    let wave_l = (TAU * 0.7 * t).sin() * m * 0.08;
    local[p.hand_l] = about(piv(p.hand_l), wave_l + 0.2 * think, (1.0, 1.0), (0.0, 0.0));
    local[p.arm_r] = about(
        piv(p.arm_r),
        -0.05 * lag_r + 0.1 * listen - 0.25 * think,
        (1.0, 1.0),
        (0.0, 1.2 * lag_r + 2.0 * settle_late),
    );
    local[p.hand_r] = about(piv(p.hand_r), 0.06 * lag_r, (1.0, 1.0), (0.0, 0.0));

    // At 20 px the highlights and the shadow are noise: the silhouette, eyes and mouth stay.
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
        assert_eq!(b.parts.len(), 14);
        let f = frame(200.0, 0.0, &ModeOpts::new());
        assert_eq!(f.fills.len(), 40);
        // Gradients kept: linear and elliptical, some with 4-5 stops.
        let kinds: Vec<u8> = f
            .fills
            .iter()
            .filter_map(|x| x.gradient.as_ref().map(|g| g.kind))
            .collect();
        assert!(kinds.contains(&0) && kinds.contains(&2));
        assert!(f
            .fills
            .iter()
            .any(|x| x.gradient.as_ref().is_some_and(|g| g.stops.len() == 5)));
    }

    #[test]
    fn at_rest_the_parts_sit_where_the_drawing_has_them() {
        // t = 0, no options: every part's transform is the view's scale only.
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
    fn large_views_get_finer_curves() {
        let n = |s: f64| {
            frame(s, 0.0, &ModeOpts::new())
                .fills
                .iter()
                .map(|f| f.points.len())
                .sum::<usize>()
        };
        assert!(n(512.0) > 2 * n(64.0), "{} vs {}", n(512.0), n(64.0));
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
    fn it_hovers_and_the_shadow_answers() {
        let body_top = |t: f64| {
            frame(200.0, t, &ModeOpts::new()).fills[5]
                .points
                .iter()
                .map(|p| p.y)
                .fold(f64::MAX, f64::min)
        };
        // A quarter period up, three quarters down.
        let q = 1.0 / HOVER_HZ / 4.0;
        assert!(body_top(q) < body_top(0.0) - 2.5);
        assert!(body_top(3.0 * q) > body_top(0.0) + 2.5);
        // Same t, same frame.
        assert_eq!(
            frame(64.0, 2.7, &ModeOpts::new()),
            frame(64.0, 2.7, &ModeOpts::new())
        );
    }

    #[test]
    fn reduced_motion_holds_still_but_keeps_the_state() {
        let r = opts(&[("effectReduced", 1.0), ("look", 0.0)]);
        assert_eq!(frame(200.0, 0.0, &r).fills, frame(200.0, 0.3, &r).fills);
        let mut think = r.clone();
        think.insert("mouthDots".into(), 1.0);
        assert_ne!(frame(200.0, 0.9, &r).fills, frame(200.0, 0.9, &think).fills);
    }

    /// Writes frames for the visual check (`SINUA_BOT_DUMP=<file>`).
    #[test]
    #[ignore]
    fn dump_frames() {
        let Ok(path) = std::env::var("SINUA_BOT_DUMP") else {
            return;
        };
        crate::character::bots::dump(&path, frame);
    }
}
