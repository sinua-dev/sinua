//! Character / WISP: a helpful spirit (`wisp`).
//!
//! Design (design-07, the user's pick from the cast sheet,
//! sinua-studio/docs/agents/families/assets/prototype-cast-*.png): the
//! genie-in-a-lamp idea as a small floating wisp -- a round, glowing head that
//! flows into a curling smoke tail, violet into teal. The review note "it
//! reads a little like a ghost" is answered here: the tail is longer and ends in
//! puffs of smoke that drift off and fade, and the head carries a soft halo.
//!
//! Voice: its **sparkles**. They wander round it at rest, gather in toward it
//! while it listens (closer as the user speaks), orbit its head while it thinks,
//! and stream out of it with the voice while it speaks. Each behaviour is a
//! weight from the pose (`earGain`, the thinking dots, the talking mouth), and
//! a sparkle's place is the weighted mix of the four, so a state change glides
//! them from one pattern into the next. The tail curls tighter while thinking
//! (`curlGain`), and the mouth opens with the level.
//!
//! Drawn entirely as fills (see `character/geom.rs`), in a 200-unit design box
//! scaled to `size`. `Fixed` colour; `hue` turns the body (violet and teal
//! together); the sparkles and the eyes stay. Sizes: 64 draws everything; 32
//! drops the halo and the smoke; 20 keeps the body, eyes and mouth.

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, blurred, hsl, linear, outline, pt, solid, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// The body's own hue; `hue` rotates the body by `hue - BODY_HUE`.
pub const BODY_HUE: f64 = 253.0;

const VIOLET: Hsl = hsl(252.7, 1.0, 0.712);
const BLUE: Hsl = hsl(227.9, 1.0, 0.718);
const TEAL: Hsl = hsl(174.8, 0.662, 0.524);
const LINE: Hsl = hsl(249.7, 0.523, 0.255);
const EYES: Hsl = hsl(247.2, 0.569, 0.2);
const GOLD: Hsl = hsl(46.0, 1.0, 0.739);
const ICE: Hsl = hsl(187.0, 1.0, 0.875);
const WHITE: Hsl = hsl(0.0, 0.0, 1.0);

/// The head: a circle the face sits on (and the highlight is clipped to).
const HEAD: (f64, f64, f64) = (100.0, 82.0, 46.0);
/// What the face turns on: a flattened sphere a little larger than the head.
const FACE: turn::Surface = turn::Surface {
    c: (HEAD.0, HEAD.1),
    r: 52.0,
    depth: 0.55,
    cylinder: false,
};
/// The tail follows the head this late, and swings this far (design units).
const TAIL_LAG: f64 = 0.25;
const TAIL_SWING: f64 = 14.0;
const SPARKLES: usize = 5;

/// The body outline: the round head flowing into a tail that curls by `curl`
/// and sways with `sw`.
fn body_path(curl: f64, sw: f64) -> Vec<Point> {
    geom::join(&[
        geom::cubic((54.0, 86.0), (52.0, 38.0), (148.0, 38.0), (146.0, 86.0), 24),
        geom::cubic(
            (146.0, 86.0),
            (144.0, 122.0),
            (128.0, 142.0),
            (118.0 + sw, 158.0),
            14,
        ),
        geom::cubic(
            (118.0 + sw, 158.0),
            (110.0 + sw, 172.0),
            (120.0 + curl * 22.0 + sw, 186.0),
            (136.0 + curl * 10.0, 176.0 - curl * 14.0),
            14,
        ),
        geom::cubic(
            (136.0 + curl * 10.0, 176.0 - curl * 14.0),
            (120.0 + curl * 8.0, 192.0),
            (88.0 + sw, 182.0),
            (92.0 + sw * 0.5, 154.0),
            14,
        ),
        geom::cubic(
            (92.0 + sw * 0.5, 154.0),
            (70.0, 136.0),
            (55.0, 118.0),
            (54.0, 86.0),
            14,
        ),
    ])
}

/// Where sparkle `i` is and how it looks: `(x, y, radius, alpha)`, the weighted
/// mix of the four behaviours (rest, gather, orbit, stream).
fn sparkle(i: usize, t: f64, float: f64, level: f64, w: [f64; 4]) -> (f64, f64, f64, f64) {
    let n = SPARKLES as f64;
    let k = i as f64;
    // At rest: wandering slowly round it.
    let a0 = k / n * TAU + t * 0.4;
    let rest = (
        100.0 + a0.cos() * 70.0,
        92.0 + a0.sin() * 50.0 + float,
        3.5,
        0.35,
    );
    // Listening: gathered in, closer as the user speaks.
    let a1 = k / n * TAU + 0.3;
    let r1 = 78.0 - level * 16.0;
    let gather = (
        100.0 + a1.cos() * r1,
        90.0 + a1.sin() * r1 * 0.8,
        3.5,
        0.4 + level * 0.6,
    );
    // Thinking: orbiting the crown, like a halo of ideas (above the eyes).
    let a2 = t * 2.5 + k / n * TAU;
    let orbit = (
        100.0 + a2.cos() * 56.0,
        34.0 + a2.sin() * 11.0 + float,
        3.2,
        0.9,
    );
    // Speaking: streaming out from the head's edge with the voice (each fades
    // out before it wraps round to the edge again).
    let u = (t * 0.9 + k / n).rem_euclid(1.0);
    let a3 = -0.9 + k * 0.45;
    let d = 54.0 + u * 34.0;
    let stream = (
        HEAD.0 + a3.cos() * d,
        HEAD.1 + 4.0 + a3.sin() * d * 0.8,
        3.0 + level * 2.0,
        (1.0 - u) * (0.3 + level),
    );
    let total: f64 = w.iter().sum::<f64>().max(1e-9);
    let parts = [rest, gather, orbit, stream];
    // Mix the places **around the head** (angle and distance from its centre),
    // not in x/y: mid-change a sparkle then travels round the head instead of
    // cutting across the face.
    let (cx, cy) = (HEAD.0, HEAD.1 + 4.0);
    let (mut ux, mut uy, mut dist, mut r, mut a) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, wi) in parts.iter().zip(w) {
        let (dx, dy) = (p.0 - cx, p.1 - cy);
        let d = (dx * dx + dy * dy).sqrt().max(1e-9);
        ux += dx / d * wi;
        uy += dy / d * wi;
        dist += d * wi;
        r += p.2 * wi;
        a += p.3 * wi;
    }
    let ang = uy.atan2(ux);
    let dist = dist / total;
    (
        cx + ang.cos() * dist,
        cy + ang.sin() * dist,
        r / total,
        (a / total).min(1.0),
    )
}

fn star(cx: f64, cy: f64, r: f64) -> Vec<Point> {
    (0..8)
        .map(|j| {
            let a = j as f64 / 8.0 * TAU - PI / 2.0;
            let rr = if j % 2 == 0 { r } else { r * 0.3 };
            pt(cx + a.cos() * rr, cy + a.sin() * rr)
        })
        .collect()
}

pub fn frame_wisp(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let pose = rig::pose(o, t);
    let tier = kit::tier(size);
    let accessories = get(o, "accessories", 1.0) >= 0.5;
    let mouth_off = get(o, "mouth", 1.0) < 0.5;
    let dh = get(o, "hue", BODY_HUE) - BODY_HUE;
    let (violet, blue, teal, line) = (
        VIOLET.rotate(dh),
        BLUE.rotate(dh),
        TEAL.rotate(dh),
        LINE.rotate(dh),
    );
    let (rest, dots, talk, level, phase) = face::mouth_weights(pose.mouth);
    // The head turn (0 = the flat spirit); the tail trails behind it.
    let tn = turn::angles(o, t, tier);
    let trail = -TAIL_SWING * turn::angles_lagged(o, t, tier, TAIL_LAG).yaw.sin();
    let face_xf = |f: Fill| {
        if tn.is_zero() {
            f
        } else {
            tn.map_fill(f, &FACE)
        }
    };

    let float = (t * 2.0).sin() * 4.0;
    let scale = Xf::scale(size / 200.0, size / 200.0);
    // The spirit: floats, tilts (listening leans in), swells with the voice.
    let swell = 1.0 + pose.squash;
    let body_xf = Xf::translate(-100.0, -90.0)
        .then(Xf::scale(swell, swell))
        .then(Xf::rotate(pose.tilt))
        .then(Xf::translate(
            100.0 + pose.shake,
            90.0 + float + pose.bob + pose.lean * 0.5,
        ))
        .then(scale);

    let mut out: Vec<Fill> = Vec::new();
    out.push(geom::transform(
        kit::ground_shadow(100.0, 190.0, 26.0 + float, 4.0),
        &scale,
    ));

    let mut b: Vec<Fill> = Vec::new();
    // A soft halo behind the head: a radial fade, not a blur (no blur cost, and
    // Safari draws a blurred gradient sharp).
    if !tier.small {
        let mut halo = geom::radial(
            geom::ellipse(HEAD.0, HEAD.1, 64.0, 64.0, 0.0, 48),
            (HEAD.0, HEAD.1),
            64.0,
            &[(0.0, violet), (0.7, violet), (1.0, violet)],
        );
        if let Some(g) = halo.gradient.as_mut() {
            (g.stops[0].a, g.stops[1].a, g.stops[2].a) = (0.5, 0.3, 0.0);
        }
        b.push(halo);
    }
    // Smoke puffs drifting off the tail tip and fading (the tail's smokiness).
    let curl = get(o, "curlGain", 0.6) + talk * level * 0.4;
    let sw = (t * 2.4).sin() * 6.0;
    if tier.full {
        for k in 0..2 {
            let u = (t * 0.35 + k as f64 * 0.5).rem_euclid(1.0);
            let (x, y) = (
                136.0 + curl * 10.0 + u * 14.0,
                176.0 - curl * 14.0 + u * 10.0,
            );
            b.push(blurred(
                geom::ellipse(x, y, 5.0 + u * 5.0, 4.0 + u * 4.0, 0.0, 20),
                teal,
                0.35 * (1.0 - u),
                3.0,
            ));
        }
    }
    // The body.
    let body = body_path(curl, sw + trail);
    b.push(linear(
        body.clone(),
        (0.0, 40.0),
        (0.0, 190.0),
        &[(0.0, violet), (0.6, blue), (1.0, teal)],
    ));
    let head = geom::ellipse(HEAD.0, HEAD.1, HEAD.2, HEAD.2, 0.0, 48);
    if !tier.small {
        // The light stays put while the head turns under it.
        let (lx, ly) = (-10.0 * tn.yaw.sin(), 8.0 * tn.pitch.sin());
        let shine = geom::clip_convex(
            &geom::ellipse(80.0 + lx, 58.0 + ly, 18.0, 9.0, -0.5, 24),
            &head,
        );
        if shine.len() >= 3 {
            b.push(blurred(shine, WHITE, 0.5, 4.0));
        }
    }
    b.push(outline(&body, 3.0 * tier.line, line, 1.0));

    // The face: dark shape eyes; the mouth is a smile, the thinking dots, or an
    // oval that opens with the voice -- each at its weight.
    let ink = kit::effect_ink(&pose, EYES);
    let f = Face {
        cx: 100.0,
        cy: 84.0,
        scale: if tier.small { 1.0 } else { 0.9 },
        ink,
        glow: 0.0,
        clip: None,
    };
    let face_from = b.len();
    b.extend(face::eye_fills(&f, &pose.eyes));
    if !mouth_off {
        if pose.effect != 0 {
            b.extend(face::mouth_fills(&f, &pose.eyes, pose.mouth, 12.0));
        } else {
            let small_mouth = Mouth::Blend {
                smile: rest,
                dots,
                talk: 0.0,
                level,
                phase,
            };
            b.extend(face::mouth_fills(&f, &pose.eyes, small_mouth, 12.0));
            if talk > 0.0 {
                let s = f.scale;
                let (mx, my) = (
                    f.cx + pose.eyes.gx * 0.5 * s,
                    f.cy + (22.0 + pose.eyes.gy * 0.3) * s,
                );
                let oval =
                    geom::ellipse(mx, my - 2.0 * s, 7.0 * s, (1.5 + level * 7.0) * s, 0.0, 24);
                b.push(solid(oval, ink, talk.min(1.0)));
            }
        }
    }
    if !tn.is_zero() {
        let turned: Vec<Fill> = b.drain(face_from..).map(face_xf).collect();
        b.extend(turned);
    }
    out.extend(b.into_iter().map(|f| geom::transform(f, &body_xf)));

    // The sparkles (outside the body transform: they move round it).
    if accessories && !tier.small {
        let w = [
            (1.0 - pose.ears - dots - talk).max(0.0),
            pose.ears,
            dots,
            talk,
        ];
        let spark_xf = scale;
        for i in 0..SPARKLES {
            let (x, y, r, a) = sparkle(i, t, float, pose.drive, w);
            if a > 0.02 {
                let col = if i % 2 == 1 { GOLD } else { ICE };
                out.push(geom::transform(
                    solid(star(x, y, r * 1.6), col, a),
                    &spark_xf,
                ));
            }
        }
    }
    out.extend(
        kit::celebrate_burst(
            &pose,
            o,
            tier,
            (100.0, 88.0),
            80.0,
            [CELEBRATE_INK, GOLD, ICE],
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
