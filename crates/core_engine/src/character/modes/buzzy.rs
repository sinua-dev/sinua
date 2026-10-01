//! Character / BUZZY: a small space-hero assistant (`buzzy`).
//!
//! Design (design-07, the user's pick from the prototype sheets,
//! sinua-studio/docs/agents/families/assets/prototype-buzz-*.png): a
//! rounded-square indigo helmet (a square reads dependable) with an amber
//! crest and chevron ear pods (triangles read energetic), a glass bubble
//! visor over a face screen, and a "voice core" in the chest. Inspired by
//! the space-ranger archetype, deliberately none of Buzz Lightyear's marks:
//! no wings, no white-green-purple suit, no human face.
//!
//! Voice: the mouth is a voice line whose swing follows the level (amplitude
//! only, no visemes), the chest bars follow it too, and while listening
//! sound arcs light up at the ear pods with the user's level.
//!
//! Drawn entirely as fills, in order (see `character/geom.rs` for why), in a
//! 200-unit design box scaled to `size`. The frame is `Fixed`: the screen
//! stays dark and the eyes stay cyan in both themes. `hue` turns the shell
//! (and its line and screen tints); amber and cyan stay.
//!
//! Sizes: 64 draws everything; 32 drops the visor; 20 keeps the helmet,
//! screen, pods, eyes and mouth with heavier lines.

use std::f64::consts::PI;

use crate::character::face::{self, Face, CELEBRATE_INK};
use crate::character::geom::{self, blurred, hsl, linear, outline, pt, radial, solid, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// The shell's own hue; `hue` rotates the shell family by `hue - SHELL_HUE`.
pub const SHELL_HUE: f64 = 232.0;

const SHELL: Hsl = hsl(231.9, 0.566, 0.461);
const SHELL_L: Hsl = hsl(231.3, 1.0, 0.718);
const SHELL_D: Hsl = hsl(234.1, 0.631, 0.255);
const LINE: Hsl = hsl(234.1, 0.662, 0.151);
const SCREEN: Hsl = hsl(230.5, 0.61, 0.061);
const AMBER: Hsl = hsl(36.1, 1.0, 0.616);
const CYAN: Hsl = hsl(185.0, 1.0, 0.716);
const GLASS: Hsl = hsl(203.0, 1.0, 0.873);
const GLASS_EDGE: Hsl = hsl(200.0, 1.0, 0.91);
const WHITE: Hsl = hsl(0.0, 0.0, 1.0);
/// What the face turns on: a flattened sphere a little larger than the
/// helmet, so the face turns without wrapping round its edge.
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 92.0),
    r: 64.0,
    depth: 0.55,
    cylinder: false,
};

pub fn frame_buzzy(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let pose = rig::pose(o, t);
    let tier = kit::tier(size);
    let (small, full) = (tier.small, tier.full);
    let accessories = get(o, "accessories", 1.0) >= 0.5;
    let dh = get(o, "hue", SHELL_HUE) - SHELL_HUE;
    let (shell, shell_l, shell_d, line, screen) = (
        SHELL.rotate(dh),
        SHELL_L.rotate(dh),
        SHELL_D.rotate(dh),
        LINE.rotate(dh),
        SCREEN.rotate(dh),
    );
    // Heavier lines when the whole character is 20 px.
    let lw = tier.line;
    // The head turn (0 unless `turn` is set): the face rides a sphere, the
    // light and the parts shift with it, the torso follows at 40 %.
    let tn = turn::angles(o, t, tier);
    let (sy, cy) = (tn.yaw.sin(), tn.yaw.cos());
    let core_dx = 30.0 * (0.4 * tn.yaw).sin();

    let scale = Xf::scale(size / 200.0, size / 200.0);
    let body = Xf::translate(pose.shake, pose.lean)
        .then(Xf::translate(-100.0, -184.0 + pose.bob))
        .then(Xf::scale(1.0 + pose.squash, 1.0 - pose.squash))
        .then(Xf::rotate(pose.tilt))
        .then(Xf::translate(100.0, 184.0))
        .then(scale);

    let mut ground: Vec<Fill> = Vec::new();
    let mut fills: Vec<Fill> = Vec::new();

    // Ground shadow (stays on the floor: no body transform).
    ground.push(kit::ground_shadow(100.0, 184.0, 46.0, 6.0));

    // Torso.
    let torso = geom::join(&[
        geom::cubic(
            (62.0, 186.0),
            (62.0, 158.0),
            (74.0, 146.0),
            (100.0, 146.0),
            16,
        ),
        geom::cubic(
            (100.0, 146.0),
            (126.0, 146.0),
            (138.0, 158.0),
            (138.0, 186.0),
            16,
        ),
    ]);
    fills.push(linear(
        torso.clone(),
        (0.0, 146.0),
        (0.0, 186.0),
        &[(0.0, shell_l), (0.35, shell), (1.0, shell_d)],
    ));
    fills.push(outline(&torso, 3.5 * lw, line, 1.0));

    // Chest voice core: a dark disc, an amber ring, bars with the voice.
    let core_on = 0.3 + 0.6 * pose.drive;
    let core = geom::ellipse(100.0 + core_dx, 168.0, 11.0, 11.0, 0.0, 32);
    fills.push(solid(core.clone(), screen, 1.0));
    fills.push(outline(&core, 2.5 * lw, AMBER, 0.5 + 0.5 * core_on));
    if !small {
        for (i, b) in [0.5, 0.85, 1.0, 0.85, 0.5].iter().enumerate() {
            let idle = 0.1 + 0.05 * (t * 2.0 + i as f64).sin();
            let h = 2.0 + b * pose.drive.max(idle) * 12.0;
            let bar = geom::round_rect(
                100.0 - 8.0 + i as f64 * 4.0 - 1.2 + core_dx,
                168.0 - h / 2.0,
                2.4,
                h,
                1.2,
                2.0,
            );
            let bar = geom::clip_convex(&bar, &core);
            if bar.len() >= 3 {
                fills.push(solid(bar, CYAN, 0.55 + 0.45 * core_on));
            }
        }
    }

    // Ear pods, their chevrons, and the listening arcs. Turned, the pods
    // swing round the head: the far one narrows, the near one widens.
    let pod_at = |s: f64| {
        let w = 16.0 * (1.0 - 0.5 * s * sy);
        let x = 100.0 + s * 62.0 * cy;
        (
            x - (100.0 + s * 62.0),
            geom::round_rect(x - w / 2.0, 76.0, w, 34.0, 7.0, 3.0),
        )
    };
    for s in [-1.0, 1.0] {
        let (dx, pod) = pod_at(s);
        fills.push(solid(pod.clone(), shell_d, 1.0));
        fills.push(outline(&pod, 3.0 * lw, line, 1.0));
        if !small && accessories {
            let chev = [
                pt(100.0 + s * 64.0 + dx, 87.0),
                pt(100.0 + s * 60.0 + dx, 93.0),
                pt(100.0 + s * 64.0 + dx, 99.0),
            ];
            fills.push(solid(geom::stroke(&chev, 3.0), AMBER, 1.0));
            // Turned, the far pod's arcs go behind the helmet with it.
            let far = if s == -tn.near_side() {
                1.0 - tn.fade_in()
            } else {
                1.0
            };
            for k in 0..2 {
                let a = (pose.ears * (pose.drive * 1.6 - k as f64 * 0.5)).min(1.0) * far;
                if a <= 0.02 {
                    continue;
                }
                let (a0, a1) = if s > 0.0 {
                    (-0.7, 0.7)
                } else {
                    (PI - 0.7, PI + 0.7)
                };
                let arc = geom::arc(
                    100.0 + s * 70.0 + dx,
                    93.0,
                    9.0 + k as f64 * 7.0,
                    a0,
                    a1,
                    12,
                );
                let stroke = geom::stroke(&arc, 2.6);
                fills.push(blurred(stroke.clone(), CYAN, 0.5 * a, 2.0));
                fills.push(solid(stroke, CYAN, a));
            }
        }
    }

    // Helmet: a rounded square, lit from the top left, shaded bottom right.
    let head = geom::superellipse(100.0, 92.0, 56.0, 52.0, 4.2, 96);
    if !tn.is_zero() {
        // The side band: the helmet's back edge, showing on the side it
        // turns away from (and below when it looks up).
        if let Some((band, rim)) = turn::Turn::side_band(&head, (-16.0 * sy, 16.0 * tn.pitch.sin()))
        {
            fills.push(solid(band, shell_d, 1.0));
            fills.push(solid(geom::stroke(&rim, 3.5 * lw), line, 1.0));
        }
    }
    // The light stays put while the helmet turns under it.
    let (lx, ly) = (-14.0 * sy, 10.0 * tn.pitch.sin());
    fills.push(radial(
        head.clone(),
        (78.0 + lx, 60.0 + ly),
        78.0,
        &[(0.0, shell_l), (0.55, shell), (1.0, shell_d)],
    ));
    let shade = geom::clip_convex(
        &geom::ellipse(126.0 + lx, 138.0 + ly, 60.0, 34.0, -0.4, 48),
        &head,
    );
    if shade.len() >= 3 {
        fills.push(solid(shade, shell_d, 0.35));
    }
    fills.push(outline(&head, 3.5 * lw, line, 1.0));
    // The near pod comes round in front of the helmet, fading in as it
    // turns so nothing jumps at the switch.
    let near = tn.near_side();
    let front = tn.fade_in();
    if front > 0.0 {
        let (dx, pod) = pod_at(near);
        fills.push(solid(pod.clone(), shell_d, front));
        fills.push(outline(&pod, 3.0 * lw, line, front));
        if !small && accessories {
            let s = near;
            let chev = [
                pt(100.0 + s * 64.0 + dx, 87.0),
                pt(100.0 + s * 60.0 + dx, 93.0),
                pt(100.0 + s * 64.0 + dx, 99.0),
            ];
            fills.push(solid(geom::stroke(&chev, 3.0), AMBER, front));
        }
    }
    if !small && accessories {
        let crest = geom::join(&[
            geom::quad((86.0, 44.0), (100.0, 22.0), (114.0, 44.0), 12),
            geom::quad((114.0, 44.0), (100.0, 38.0), (86.0, 44.0), 8),
        ]);
        let crest = if tn.is_zero() {
            crest
        } else {
            Xf::translate(22.0 * sy, 0.0).map(&crest)
        };
        fills.push(solid(crest.clone(), AMBER, 1.0));
        fills.push(outline(&crest, 2.5, line, 1.0));
    }

    // Face screen, the face on it (clipped to it), its rim. Everything from
    // here to the visor rides the face sphere when the head turns.
    let face_from = fills.len();
    let scr = geom::round_rect(58.0, 62.0, 84.0, 62.0, 22.0, 2.0);
    fills.push(solid(scr.clone(), screen, 1.0));
    let ink = kit::effect_ink(&pose, CYAN);
    let f = Face {
        cx: 100.0,
        cy: 90.0,
        scale: if small { 1.08 } else { 0.92 },
        ink,
        glow: if small { 2.0 } else { 4.0 },
        clip: Some(&scr),
    };
    fills.extend(face::eye_fills(&f, &pose.eyes));
    fills.extend(face::mouth_fills(
        &f,
        &pose.eyes,
        pose.mouth,
        if small { 11.0 } else { 18.0 },
    ));
    fills.push(outline(&scr, 2.5 * lw, line, 1.0));

    // Glass bubble visor with a slowly drifting reflection (64 only).
    if full {
        let visor = geom::ellipse(100.0, 88.0, 50.0, 44.0, 0.0, 64);
        fills.push(solid(visor.clone(), GLASS, 0.16));
        fills.push(outline(&visor, 1.6, GLASS_EDGE, 0.55));
        let sw = 0.15 + 0.1 * (t * 0.5).sin();
        let glint = geom::arc(
            100.0,
            88.0,
            40.0,
            PI * (1.12 + sw * 0.5),
            PI * (1.32 + sw * 0.5),
            12,
        );
        let glint = geom::clip_convex(&geom::stroke(&glint, 4.0), &visor);
        if glint.len() >= 3 {
            fills.push(solid(glint, WHITE, 0.55));
        }
    }
    if !tn.is_zero() {
        let turned: Vec<Fill> = fills
            .drain(face_from..)
            .map(|f| tn.map_fill(f, &FACE))
            .collect();
        fills.extend(turned);
    }

    // Celebrate: the shared ring of sparkles, in Buzzy's colours.
    fills.extend(kit::celebrate_burst(
        &pose,
        o,
        tier,
        (100.0, 96.0),
        80.0,
        [CELEBRATE_INK, AMBER, CYAN],
    ));

    let mut out: Vec<Fill> = ground
        .into_iter()
        .map(|f| geom::transform(f, &scale))
        .collect();
    out.extend(fills.into_iter().map(|f| geom::transform(f, &body)));
    kit::finish(out, o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{ColorMode, Point};

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

    #[test]
    fn stays_in_its_box_at_every_size_and_pose() {
        for size in [20.0, 32.0, 64.0] {
            for o in [
                opts(&[]),
                opts(&[
                    ("lean", 4.0),
                    ("tilt", -0.05),
                    ("earGain", 1.0),
                    ("audioLevel", 1.0),
                ]),
                opts(&[("tilt", 0.07), ("gazeX", -9.0), ("gazeY", -8.0)]),
                opts(&[
                    ("squashGain", 0.045),
                    ("bounceGain", 3.0),
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("audioLevel", 1.0),
                ]),
                opts(&[("effectCode", 3.0), ("effectAge", 1.2)]),
            ] {
                let f = frame_buzzy(size, 1.3, &o);
                let (x0, y0, x1, y1) = bounds(&f);
                // Blur halos may spill a little; the geometry itself stays inside.
                assert!(
                    x0 > -0.02 * size && y0 > -0.02 * size && x1 < 1.02 * size && y1 < 1.02 * size,
                    "{size}: {x0} {y0} {x1} {y1}"
                );
            }
        }
    }

    #[test]
    fn is_a_fixed_colour_frame_of_fills_only() {
        let f = frame_buzzy(64.0, 0.0, &opts(&[]));
        assert_eq!(f.color_mode, ColorMode::Fixed);
        assert!(f.dots.is_empty() && f.lines.is_empty() && f.polylines.is_empty());
        assert!(f.fills.len() > 20);
    }

    #[test]
    fn detail_falls_away_at_small_sizes() {
        let n = |size| frame_buzzy(size, 0.0, &opts(&[])).fills.len();
        assert!(n(64.0) > n(32.0), "the visor goes at 32");
        assert!(n(32.0) > n(20.0), "crest, chevrons and bars go at 20");
    }

    #[test]
    fn hue_turns_the_shell_but_not_the_eyes() {
        let base = frame_buzzy(64.0, 0.0, &opts(&[("look", 0.0)]));
        let green = frame_buzzy(64.0, 0.0, &opts(&[("look", 0.0), ("hue", 132.0)]));
        let hues = |f: &OrbFrame| f.fills.iter().map(|x| x.hue).collect::<Vec<_>>();
        let (a, b) = (hues(&base), hues(&green));
        assert!(a
            .iter()
            .zip(&b)
            .any(|(x, y)| (x - 231.9).abs() < 1e-9 && (y - 131.9).abs() < 1e-9));
        assert!(
            a.iter()
                .zip(&b)
                .any(|(x, y)| (x - 185.0).abs() < 1e-9 && (y - 185.0).abs() < 1e-9),
            "cyan eyes stay"
        );
    }

    #[test]
    fn the_listening_arcs_need_the_ears_and_a_voice() {
        let n = |o: ModeOpts| frame_buzzy(64.0, 0.0, &o).fills.len();
        let quiet = n(opts(&[("earGain", 1.0), ("audioLevel", 0.0)]));
        let loud = n(opts(&[("earGain", 1.0), ("audioLevel", 0.9)]));
        let no_ears = n(opts(&[("earGain", 0.0), ("audioLevel", 0.9)]));
        assert!(
            loud > quiet && no_ears == quiet,
            "quiet {quiet} loud {loud} no_ears {no_ears}"
        );
        let bare = n(opts(&[
            ("earGain", 1.0),
            ("audioLevel", 0.9),
            ("accessories", 0.0),
        ]));
        assert!(
            bare < quiet,
            "accessories 0 drops the chevrons, crest and arcs"
        );
    }

    /// Review helper, not a check: `SINUA_CHAR_SCENES=<in.json>
    /// SINUA_CHAR_DUMP=<out.json> cargo test -p core_engine --lib dump_scenes --
    /// --ignored` renders `[{ "label", "size", "t", "opts" }]` through the real
    /// pipeline (`frame_with_overrides`, so the post-processes run) and writes
    /// the frames as JSON for the web painter.
    #[test]
    #[ignore]
    fn dump_scenes() {
        let (Ok(inp), Ok(out)) = (
            std::env::var("SINUA_CHAR_SCENES"),
            std::env::var("SINUA_CHAR_DUMP"),
        ) else {
            panic!("set SINUA_CHAR_SCENES and SINUA_CHAR_DUMP");
        };
        let scenes: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(inp).unwrap()).unwrap();
        let pts = |p: &[Point]| {
            serde_json::Value::Array(
                p.iter()
                    .map(|q| serde_json::json!({ "x": q.x, "y": q.y }))
                    .collect(),
            )
        };
        let mut frames = Vec::new();
        for sc in scenes.as_array().unwrap() {
            let o: std::collections::HashMap<String, f64> = sc["opts"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_f64().unwrap()))
                .collect();
            let f = crate::frame_with_overrides(
                sc["pattern"].as_str().unwrap_or("buzzy").into(),
                sc["size"].as_u64().unwrap() as u32,
                sc["t"].as_f64().unwrap(),
                o,
            )
            .unwrap();
            let fills: Vec<serde_json::Value> = f
                .fills
                .iter()
                .map(|x| {
                    serde_json::json!({
                        "points": pts(&x.points),
                        "holes": x.holes.iter().map(|h| pts(h)).collect::<Vec<_>>(),
                        "white": x.white, "a": x.a, "saturation": x.saturation, "hue": x.hue,
                        "blur": x.blur, "blend": x.blend,
                        "gradient": x.gradient.as_ref().map(|g| serde_json::json!({
                            "kind": g.kind, "x0": g.x0, "y0": g.y0, "x1": g.x1, "y1": g.y1, "r": g.r,
                            "stops": g.stops.iter().map(|s| serde_json::json!({
                                "offset": s.offset, "white": s.white, "a": s.a, "saturation": s.saturation, "hue": s.hue
                            })).collect::<Vec<_>>()
                        })),
                    })
                })
                .collect();
            frames.push(serde_json::json!({
                "label": sc["label"], "sub": sc["sub"], "size": sc["size"], "row": sc["row"],
                "frame": { "dots": [], "lines": [], "polylines": [], "colorMode": if f.color_mode == ColorMode::Fixed { "fixed" } else { "ink" }, "fills": fills, "effects": [] }
            }));
        }
        std::fs::write(out, serde_json::to_string(&frames).unwrap()).unwrap();
    }

    #[test]
    fn muted_keeps_the_colours_and_fades_a_little() {
        let base =
            crate::frame_with_overrides("buzzy".into(), 64, 0.3, opts(&[("look", 0.0)])).unwrap();
        let muted = crate::frame_with_overrides(
            "buzzy".into(),
            64,
            0.3,
            opts(&[("look", 0.0), ("muted", 1.0)]),
        )
        .unwrap();
        let shell = |f: &OrbFrame| {
            f.fills
                .iter()
                .find(|x| (x.hue - 231.9).abs() < 1e-9 && (x.white - 0.461).abs() < 1e-9)
                .map(|x| (x.saturation, x.a))
        };
        let (s0, a0) = shell(&base).unwrap();
        let (s1, a1) = shell(&muted).unwrap();
        assert_eq!(s0, s1, "no grey-out");
        assert!((a1 - a0 * 0.7).abs() < 1e-9);
    }

    #[test]
    fn colour_and_gradient_overrides_leave_a_character_alone() {
        let o = opts(&[("look", 0.0)]);
        let base = crate::frame_with_overrides("buzzy".into(), 64, 0.3, o.clone()).unwrap();
        let mut tinted = o.clone();
        tinted.extend(opts(&[
            ("colorMix", 1.0),
            ("colorHue", 0.0),
            ("colorSaturation", 1.0),
            ("gradientStrength", 1.0),
        ]));
        assert_eq!(
            crate::frame_with_overrides("buzzy".into(), 64, 0.3, tinted).unwrap(),
            base
        );
    }

    #[test]
    fn a_bound_voice_does_not_swell_the_whole_body() {
        let o = opts(&[("look", 0.0), ("audioLevel", 0.9)]);
        let base = crate::frame_with_overrides("buzzy".into(), 64, 0.3, o.clone()).unwrap();
        let mut swell = o;
        swell.insert("audioStrength".into(), 0.5);
        assert_eq!(
            crate::frame_with_overrides("buzzy".into(), 64, 0.3, swell).unwrap(),
            base
        );
    }

    #[test]
    fn deterministic() {
        let o = opts(&[
            ("seed", 3.0),
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.6),
        ]);
        assert_eq!(frame_buzzy(64.0, 2.7, &o), frame_buzzy(64.0, 2.7, &o));
    }

    /// Every corner of the head turn: yaw and pitch at their limits and between.
    fn turns() -> Vec<ModeOpts> {
        let mut v = Vec::new();
        for y in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            for p in [-1.0, 0.0, 1.0] {
                v.push(opts(&[
                    ("turn", 1.0),
                    ("turnYaw", y),
                    ("turnPitch", p),
                    ("earGain", 1.0),
                    ("audioLevel", 0.8),
                ]));
            }
        }
        v
    }

    #[test]
    fn the_turned_face_stays_on_the_helmet() {
        let scr = geom::round_rect(58.0, 62.0, 84.0, 62.0, 22.0, 2.0);
        let visor = geom::ellipse(100.0, 88.0, 50.0, 44.0, 0.0, 64);
        for o in turns() {
            let tn = turn::angles(&o, 0.0, kit::tier(64.0));
            // The screen stays inside the helmet; the glass visor may reach
            // under the helmet's rim line (half its 3.5 width), never past it.
            let inside = |p: &Point, grow: f64| {
                let q = tn.map(&FACE, p);
                let e = ((q.x - 100.0) / (56.0 + grow)).abs().powf(4.2)
                    + ((q.y - 92.0) / (52.0 + grow)).abs().powf(4.2);
                assert!(e < 1.0, "{:?} at yaw {} pitch {}", q, tn.yaw, tn.pitch);
            };
            scr.iter().for_each(|p| inside(p, 0.0));
            visor.iter().for_each(|p| inside(p, 1.75));
        }
    }

    #[test]
    fn turned_it_stays_in_its_box_and_light() {
        for size in [32u32, 64] {
            for o in turns() {
                let f = frame_buzzy(size as f64, 1.3, &o);
                let (x0, y0, x1, y1) = bounds(&f);
                let s = size as f64;
                assert!(
                    x0 >= 0.0 && y0 >= 0.0 && x1 <= s && y1 <= s,
                    "{size}: {:?}",
                    (x0, y0, x1, y1)
                );
                let c = crate::cost::frame_cost(&f, size);
                let base = crate::cost::frame_cost(
                    &frame_buzzy(
                        size as f64,
                        1.3,
                        &opts(&[("earGain", 1.0), ("audioLevel", 0.8)]),
                    ),
                    size,
                );
                assert_eq!(c.class, "light", "{size}: turned {c:?}\nstill {base:?}");
            }
        }
    }

    /// What the turn costs: engine time per frame and the paint-cost proxy,
    /// off vs on. `cargo test --release -p core_engine --lib bench_turn -- --ignored --nocapture`
    #[test]
    #[ignore = "a measurement, not a check"]
    fn bench_turn() {
        let states: [(&str, &[(&str, f64)]); 4] = [
            ("idle", &[("breath", 1.0), ("look", 1.0)]),
            ("listening", &[("earGain", 1.0), ("audioLevel", 0.7), ("lean", 4.0)]),
            ("thinking", &[("mouthDots", 1.0), ("gazeX", -8.0), ("gazeY", -4.0)]),
            ("speaking", &[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.8)]),
        ];
        let turn_of = |st: &str| -> Vec<(&'static str, f64)> {
            match st {
                "idle" => vec![("turn", 1.0), ("turnWander", 1.0)],
                "listening" => vec![("turn", 1.0), ("turnPitch", 0.35)],
                "thinking" => vec![("turn", 1.0), ("turnYaw", -0.7), ("turnPitch", 0.8)],
                _ => vec![("turn", 1.0), ("turnNod", 1.0), ("turnWander", 0.25)],
            }
        };
        const N: usize = 20_000;
        for size in [32.0, 64.0] {
            for (st, kv) in states {
                let off = opts(kv);
                let mut on_kv = kv.to_vec();
                on_kv.extend(turn_of(st));
                let on = opts(&on_kv);
                let time = |o: &ModeOpts| {
                    let t0 = std::time::Instant::now();
                    let mut n = 0usize;
                    for i in 0..N {
                        n += frame_buzzy(size, 0.5 + i as f64 * 0.016, o).fills.len();
                    }
                    (t0.elapsed().as_secs_f64() * 1e6 / N as f64, n)
                };
                let ((a, _), (b, _)) = (time(&off), time(&on));
                let (ca, cb) = (
                    crate::cost::frame_cost(&frame_buzzy(size, 7.3, &off), size as u32),
                    crate::cost::frame_cost(&frame_buzzy(size, 7.3, &on), size as u32),
                );
                let pts = |o: &ModeOpts| -> usize {
                    frame_buzzy(size, 7.3, o).fills.iter().map(|f| f.points.len()).sum()
                };
                println!(
                    "{size:>3} {st:<10} engine {a:6.2} -> {b:6.2} us/frame | fills {} -> {} | fill points {} -> {} | coverage {:.3} -> {:.3} | {} -> {}",
                    ca.fills, cb.fills, pts(&off), pts(&on), ca.coverage, cb.coverage, ca.class, cb.class
                );
            }
        }
    }

    #[test]
    fn a_turn_moves_the_face_and_not_at_20_px() {
        let still = frame_buzzy(64.0, 1.0, &opts(&[]));
        let turned = frame_buzzy(64.0, 1.0, &opts(&[("turn", 1.0), ("turnYaw", 1.0)]));
        assert_ne!(still, turned);
        let still20 = frame_buzzy(20.0, 1.0, &opts(&[]));
        let turned20 = frame_buzzy(20.0, 1.0, &opts(&[("turn", 1.0), ("turnYaw", 1.0)]));
        assert_eq!(still20, turned20);
    }
}
