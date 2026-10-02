//! BUZZY's tests from before recipes (they were in `character/modes/buzzy.rs`),
//! now held against its recipe (`spec/characters/buzzy.json`): the same claims,
//! the recipe's frame. The constants are the recipe's own values, for the checks.
#![allow(dead_code, unused_imports)]

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth, CELEBRATE_INK};
use crate::character::geom::{self, hsl, Hsl, Xf};
use crate::character::kit;
use crate::character::rig;
use crate::character::turn;
use crate::primitives::{Fill, ModeOpts, OrbFrame, Point};

fn frame_buzzy(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("buzzy", size, t, o).expect("a built-in recipe")
}

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
const FACE: turn::Surface = turn::Surface {
    c: (100.0, 92.0),
    r: 64.0,
    depth: 0.55,
    cylinder: false,
};

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
            (
                "listening",
                &[("earGain", 1.0), ("audioLevel", 0.7), ("lean", 4.0)],
            ),
            (
                "thinking",
                &[("mouthDots", 1.0), ("gazeX", -8.0), ("gazeY", -4.0)],
            ),
            (
                "speaking",
                &[("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.8)],
            ),
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
                    frame_buzzy(size, 7.3, o)
                        .fills
                        .iter()
                        .map(|f| f.points.len())
                        .sum()
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
