//! Character / CHIRP: a small songbird (`chirp`).
//!
//! Design (design-07, the user's pick from the cast sheet,
//! sinua-studio/docs/agents/families/assets/prototype-cast-*.png): a round
//! coral bird with a cream breast, a teal crest and wings, after the cartoon
//! birds of Pixar's "For the Birds". Deliberately not an owl (that is
//! Duolingo's) and no angry brows.
//!
//! Voice -- birds are voice-native:
//! - the **beak is the mouth**: the lower half opens with the level while it
//!   speaks, and notes float out of it;
//! - listening is the cartoon bird's **head tilt**, crest up (higher as the user
//!   speaks);
//! - thinking drops the crest and lights three thought dots in turn;
//! - speaking flutters the wings with the voice (`flutterGain`).
//!
//! Each reads the pose's continuous numbers (`tilt`, `earGain`, the mouth
//! weights), so a state change slides the head, the crest, the beak and the
//! notes instead of switching them. Fills only; `Fixed` colour; `hue` turns the
//! coral body. The rest (shadow, effect colours, celebrate, mute) is the shared
//! kit (`character/kit.rs`).

use crate::character::face::{self, Face, CELEBRATE_INK};
use crate::character::geom::{self, hsl, outline, pt, radial, solid, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::effects;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// The body's own hue; `hue` rotates the coral by `hue - BODY_HUE`.
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

/// The head turns about the feet, where a bird tilts from.
const PIVOT: (f64, f64) = (100.0, 170.0);
/// The body is the head: the face (eyes, beak) and the breast ride this
/// sphere when it turns; the wings ride the full-depth one.
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 110.0),
    r: 58.0,
    depth: 0.8,
    cylinder: false,
};
const BALL: turn::Surface = turn::Surface {
    depth: 1.0,
    ..FACE
};

/// One note ♪ at `(x, y)`: a head and a flagged stem.
fn note(x: f64, y: f64, s: f64) -> Vec<Vec<Point>> {
    vec![
        geom::ellipse(x, y, 4.5 * s, 3.4 * s, -0.4, 16),
        geom::stroke(
            &[pt(x + 4.0 * s, y - s), pt(x + 4.0 * s, y - 15.0 * s)],
            2.2 * s,
        ),
        geom::stroke(
            &geom::quad(
                (x + 4.0 * s, y - 15.0 * s),
                (x + 9.0 * s, y - 12.0 * s),
                (x + 10.0 * s, y - 8.0 * s),
                8,
            ),
            2.2 * s,
        ),
    ]
}

pub fn frame_chirp(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let pose = rig::pose(o, t);
    let tier = kit::tier(size);
    let accessories = get(o, "accessories", 1.0) >= 0.5;
    let mouth_off = get(o, "mouth", 1.0) < 0.5;
    let dh = get(o, "hue", BODY_HUE) - BODY_HUE;
    let (body_c, body_l, body_d) = (BODY.rotate(dh), BODY_L.rotate(dh), BODY_D.rotate(dh));
    let (rest, dots, talk, level, phase) = face::mouth_weights(pose.mouth);
    let lw = tier.line;
    // The head turn (0 = the flat bird).
    let tn = turn::angles(o, t, tier);
    let face_xf = |f: Fill| {
        if tn.is_zero() {
            f
        } else {
            tn.map_fill(f, &FACE)
        }
    };

    let scale = Xf::scale(size / 200.0, size / 200.0);
    let whole = Xf::translate(pose.shake, 0.0).then(scale);
    // The bird: tilts from its feet (listening's head tilt), hops (`bob`), leans in.
    let bird = Xf::translate(-PIVOT.0, -PIVOT.1)
        .then(Xf::rotate(pose.tilt))
        .then(Xf::translate(PIVOT.0, PIVOT.1 + pose.bob + pose.lean * 0.5))
        .then(whole);

    let mut out: Vec<Fill> = vec![geom::transform(
        kit::ground_shadow(100.0, 184.0, 40.0, 5.2),
        &scale,
    )];
    // Feet (they stay on the ground).
    for s in [-1.0, 1.0] {
        let x = 100.0 + s * 14.0;
        for leg in [
            vec![pt(x, 164.0), pt(x, 180.0)],
            vec![pt(x, 180.0), pt(x + s * 6.0, 182.0)],
            vec![pt(x, 180.0), pt(x - s * 6.0, 182.0)],
        ] {
            out.push(geom::transform(
                solid(geom::stroke(&leg, 3.5 * lw), BEAK_D, 1.0),
                &whole,
            ));
        }
    }

    let mut b: Vec<Fill> = Vec::new();
    // The crest: three curling feathers, up while listening (higher with the
    // user's voice), perked while speaking, drooping while thinking.
    if !tier.small || accessories {
        let lift = pose.ears * (0.3 + pose.drive * 0.5) + talk * (0.2 + level * 0.4) - dots * 0.4;
        for (i, dx) in [-9.0, 0.0, 9.0].into_iter().enumerate() {
            let mid = if i == 1 { 1.0 } else { 0.0 };
            let feather = geom::quad(
                (100.0 + dx * 0.5, 56.0),
                (100.0 + dx * 1.4, 36.0 - lift * 10.0 - 6.0 * mid),
                (
                    100.0 + dx * 2.2 + (i as f64 - 1.0) * 4.0,
                    34.0 - lift * 12.0 - 8.0 * mid,
                ),
                10,
            );
            let feather = if tn.is_zero() {
                feather
            } else {
                Xf::translate(20.0 * tn.yaw.sin(), 0.0).map(&feather)
            };
            b.push(solid(geom::stroke(&feather, 5.0 * lw), TEAL, 1.0));
        }
    }
    // Wings: flutter with the voice while speaking. Turned, they swing round
    // the body: the near one comes forward and grows a little, the far one
    // reaches the rim and thins, seen edge on (no draw-order switch, so no
    // see-through wing on the way round).
    let flutter = (t * 14.0).sin() * get(o, "flutterGain", 0.0) * pose.drive;
    let wing_at = |s: f64| {
        let (mut cx, mut rx) = (100.0 + s * 54.0, 11.0);
        if !tn.is_zero() {
            cx = tn.map(&BALL, &pt(cx, 118.0)).x;
            rx *= if s == tn.near_side() {
                1.0 + 0.25 * tn.fade_in()
            } else {
                1.0 - 0.6 * tn.fade_in()
            };
        }
        geom::ellipse(cx, 118.0, rx, 22.0, s * (0.5 + flutter), 24)
    };
    // The body, with its breast clipped to it.
    let body = geom::ellipse(100.0, 110.0, 58.0, 56.0, 0.0, 64);
    // The light stays put while the body turns under it.
    let (lx, ly) = (-14.0 * tn.yaw.sin(), 10.0 * tn.pitch.sin());
    b.push(radial(
        body.clone(),
        (80.0 + lx, 80.0 + ly),
        80.0,
        &[(0.0, body_l), (0.55, body_c), (1.0, body_d)],
    ));
    let breast = geom::ellipse(100.0, 146.0, 40.0, 32.0, 0.0, 40);
    let breast = if tn.is_zero() {
        breast
    } else {
        tn.map_points(&FACE, &breast)
    };
    let breast = geom::clip_convex(&breast, &body);
    if breast.len() >= 3 {
        b.push(solid(breast, BELLY, 1.0));
    }
    b.push(outline(&body, 3.5 * lw, LINE, 1.0));
    for s in [-1.0, 1.0] {
        let wing = wing_at(s);
        b.push(solid(wing.clone(), TEAL, 1.0));
        b.push(outline(&wing, 3.0 * lw, LINE, 1.0));
    }
    // Eyes.
    let ink = kit::effect_ink(&pose, EYES);
    let f = Face {
        cx: 100.0,
        cy: 96.0,
        scale: if tier.small { 0.95 } else { 0.82 },
        ink,
        glow: 0.0,
        clip: None,
    };
    b.extend(face::eye_fills(&f, &pose.eyes).into_iter().map(face_xf));
    // The beak is the mouth: the lower half opens with the voice.
    let open = if mouth_off {
        0.0
    } else if matches!(pose.effect, effects::SUCCESS | effects::CELEBRATE) {
        0.45
    } else {
        talk * level + rest * 0.05
    };
    let lower = vec![
        pt(90.0, 118.0),
        pt(110.0, 118.0),
        pt(100.0, 122.0 + open * 14.0),
    ];
    b.push(face_xf(solid(lower.clone(), BEAK_D, 1.0)));
    b.push(face_xf(outline(&lower, 2.5 * lw, LINE, 1.0)));
    let upper = geom::join(&[
        geom::quad((89.0, 118.0), (100.0, 108.0), (111.0, 118.0), 10),
        vec![pt(100.0, 127.0 - open * 2.0)],
    ]);
    b.push(face_xf(solid(upper.clone(), BEAK, 1.0)));
    b.push(face_xf(outline(&upper, 2.5 * lw, LINE, 1.0)));
    out.extend(b.into_iter().map(|f| geom::transform(f, &bird)));

    // Notes while speaking; thought dots while thinking (in the world, not tilted).
    if accessories && !tier.small && pose.effect == 0 {
        let s = if tier.full { 1.0 } else { 0.8 };
        for i in 0..3 {
            let u = (t * 0.8 + i as f64 / 3.0).rem_euclid(1.0);
            let a = talk * (1.0 - u) * (0.3 + level).min(1.0);
            if a > 0.02 {
                for part in note(146.0 + u * 26.0, 70.0 - u * 40.0 + i as f64 * 6.0, s) {
                    out.push(geom::transform(solid(part, TEAL, a), &whole));
                }
            }
            let on = (1.0 - ((phase * 2.2).rem_euclid(3.0) - i as f64).abs()).max(0.0);
            let d = dots * (0.3 + 0.7 * on);
            if d > 0.02 {
                let c = geom::ellipse(
                    146.0 + i as f64 * 10.0,
                    44.0 - i as f64 * 6.0,
                    (3.5 + i as f64) * s,
                    (3.5 + i as f64) * s,
                    0.0,
                    16,
                );
                out.push(geom::transform(solid(c, TEAL, d), &whole));
            }
        }
    }
    out.extend(
        kit::celebrate_burst(
            &pose,
            o,
            tier,
            (100.0, 104.0),
            82.0,
            [CELEBRATE_INK, BEAK, TEAL],
        )
        .into_iter()
        .map(|f| geom::transform(f, &scale)),
    );
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
