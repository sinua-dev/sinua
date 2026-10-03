//! Eye styles (design note 24, FX Spec 1.13): glossy, pixel and dot eyes keep
//! the shape eye's behaviour (blinks, the gaze, expressions), the `eyeStyle`
//! option beats a recipe's `style`, and the showcases
//! (`spec/examples/glossy-bean.fxspec.json`, `pixel-beep`, `dot-hum`) resolve.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::face::{eye_fills, eye_outline, EyeKind, Eyes, Face};
use crate::character::geom::hsl;
use crate::fx_spec::resolve_full;
use crate::primitives::{Fill, OrbFrame};

const SHOWCASES: [&str; 3] = [
    include_str!("../../../../../spec/examples/glossy-bean.fxspec.json"),
    include_str!("../../../../../spec/examples/pixel-beep.fxspec.json"),
    include_str!("../../../../../spec/examples/dot-hum.fxspec.json"),
];

fn face(style: u8, small: bool) -> Face<'static> {
    Face {
        cx: 100.0,
        cy: 90.0,
        scale: 1.0,
        ink: hsl(185.0, 1.0, 0.7),
        glow: 3.0,
        clip: None,
        style,
        iris: hsl(188.0, 0.75, 0.45),
        sclera: false,
        small,
    }
}

/// The fills' vertical extent.
fn span_y(f: &[Fill]) -> f64 {
    let ys = f.iter().flat_map(|x| x.points.iter().map(|p| p.y));
    let (lo, hi) = ys.fold((f64::MAX, f64::MIN), |(a, b), y| (a.min(y), b.max(y)));
    hi - lo
}

fn centre_x(f: &Fill) -> f64 {
    f.points.iter().map(|p| p.x).sum::<f64>() / f.points.len() as f64
}

/// The right eye's fills (x > the face centre).
fn right(f: Vec<Fill>) -> Vec<Fill> {
    f.into_iter().filter(|x| centre_x(x) > 100.0).collect()
}

#[test]
fn every_style_draws_inside_the_eyes_bounds() {
    for style in 1..=3u8 {
        let e = Eyes::default();
        // The glossy eye is 1.2 × 1.1 the shape eye; the dot is smaller than both.
        let (hw, hh) = (e.w * 0.6 + 0.5, e.h * 0.55 + 0.5);
        for x in eye_fills(&face(style, false), &e) {
            if x.blur > 0.0 {
                continue; // a glow spreads past the eye on purpose
            }
            for p in &x.points {
                let dx = (p.x - 100.0).abs() - 24.0;
                assert!(
                    dx.abs() <= hw && (p.y - 90.0).abs() <= hh,
                    "style {style}: {p:?}"
                );
            }
        }
    }
}

#[test]
fn a_blink_shuts_every_style() {
    for style in 1..=3u8 {
        let open = right(eye_fills(&face(style, false), &Eyes::default()));
        let shut = right(eye_fills(
            &face(style, false),
            &Eyes {
                open: 0.06,
                ..Eyes::default()
            },
        ));
        assert!(!shut.is_empty(), "style {style}: a shut eye still shows");
        let solid = |f: &[Fill]| {
            f.iter()
                .filter(|x| x.blur == 0.0)
                .cloned()
                .collect::<Vec<_>>()
        };
        assert!(
            span_y(&solid(&shut)) < span_y(&solid(&open)) * 0.45,
            "style {style}: {} vs {}",
            span_y(&solid(&shut)),
            span_y(&solid(&open))
        );
    }
}

#[test]
fn the_glossy_iris_follows_the_gaze_and_the_highlights_stay_with_the_light() {
    let at = |gx: f64| {
        let f = right(eye_fills(
            &face(1, false),
            &Eyes {
                gx,
                ..Eyes::default()
            },
        ));
        let iris = f
            .iter()
            .find(|x| x.gradient.is_some())
            .unwrap()
            .gradient
            .clone()
            .unwrap()
            .x0;
        let spot = f
            .iter()
            .find(|x| x.white == 1.0 && x.a > 0.9)
            .map(centre_x)
            .unwrap();
        // Relative to the eye's own centre, which moves with the gaze.
        (iris - (124.0 + gx), spot - (124.0 + gx))
    };
    let (iris0, spot0) = at(0.0);
    let (iris1, spot1) = at(6.0);
    assert!(
        (iris1 - iris0 - 2.4).abs() < 1e-9,
        "the iris moves within the eye"
    );
    assert!(
        spot1 - spot0 < 0.0 && spot1 - spot0 > -1.0,
        "the highlight barely moves"
    );
}

#[test]
fn the_glossy_eye_collapses_to_an_iris_and_one_highlight_at_20_px() {
    let full = right(eye_fills(&face(1, false), &Eyes::default()));
    let small = right(eye_fills(&face(1, true), &Eyes::default()));
    // Lens, iris, one highlight.
    assert_eq!(small.len(), 3, "{small:?}");
    assert!(full.len() > small.len() + 2);
}

#[test]
fn expressions_shape_the_styled_eyes() {
    for style in 1..=3u8 {
        let rest = eye_fills(&face(style, false), &Eyes::default());
        let happy = eye_fills(
            &face(style, false),
            &Eyes {
                smile: 0.8,
                ..Eyes::default()
            },
        );
        assert_ne!(format!("{rest:?}"), format!("{happy:?}"), "style {style}");
    }
}

#[test]
fn the_effect_eyes_ignore_the_style() {
    for kind in [EyeKind::Star, EyeKind::Cross] {
        let e = Eyes {
            kind,
            ..Eyes::default()
        };
        let plain = eye_fills(&face(0, false), &e);
        for style in 1..=3u8 {
            assert_eq!(plain, eye_fills(&face(style, false), &e), "{kind:?}");
        }
    }
    assert!(eye_outline(&Eyes::default(), 1.0).len() > 3);
}

fn frame(id: &str, o: &[(&str, f64)]) -> OrbFrame {
    let o = o.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    crate::character::recipe::frame(id, 64.0, 1.3, &o).unwrap()
}

fn gradients(f: &OrbFrame) -> usize {
    f.fills.iter().filter(|x| x.gradient.is_some()).count()
}

#[test]
fn the_option_draws_a_style_on_any_character_and_auto_keeps_the_recipes() {
    for id in ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"] {
        let plain = frame(id, &[]);
        assert_eq!(plain, frame(id, &[("eyeStyle", 0.0)]), "{id}: auto");
        assert_eq!(plain, frame(id, &[("eyeStyle", 1.0)]), "{id}: shape");
        for style in [2.0, 3.0, 4.0] {
            assert_ne!(plain, frame(id, &[("eyeStyle", style)]), "{id}: {style}");
        }
        // Glossy: an iris per eye.
        assert_eq!(
            gradients(&frame(id, &[("eyeStyle", 2.0)])),
            gradients(&plain) + 2,
            "{id}"
        );
    }
}

/// E4 (design note 27): every character has an `iris` slot behind the `iris` role; it
/// colours the glossy eye and nothing else.
#[test]
fn the_iris_role_colours_the_glossy_eye_on_every_character_and_only_it() {
    for id in ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"] {
        let red = crate::palette_overrides(id.into(), r##"{ "iris": "#FF0000" }"##.into());
        assert!(red.diagnostics.is_empty(), "{id}: {:?}", red.diagnostics);
        assert_eq!(red.overrides.get("palette.iris.h"), Some(&0.0), "{id}");
        let with = |extra: &[(&str, f64)]| {
            let mut o: Vec<(&str, f64)> = red
                .overrides
                .iter()
                .map(|(k, v)| (k.as_str(), *v))
                .collect();
            o.extend_from_slice(extra);
            frame(id, &o)
        };
        let glossy = frame(id, &[("eyeStyle", 2.0)]);
        assert_ne!(glossy, with(&[("eyeStyle", 2.0)]), "{id}: glossy");
        // The shape eye has no iris: the role is skipped silently.
        assert_eq!(frame(id, &[]), with(&[]), "{id}: shape");
    }
}

fn bean_with(eyes: Value) -> String {
    let text = crate::character::recipe::RECIPES
        .iter()
        .find(|(id, _)| *id == "bean")
        .unwrap()
        .1;
    let mut r: Value = serde_json::from_str(text).unwrap();
    let part = r["parts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["part"] == "eyes")
        .unwrap();
    for (k, v) in eyes.as_object().unwrap() {
        part[k] = v.clone();
    }
    r["id"] = json!("styled-bean");
    json!({ "fxSpec": "1.13", "object": "character", "pattern": "styled-bean", "recipe": r })
        .to_string()
}

#[test]
fn a_recipe_sets_its_style_iris_and_sclera_and_the_option_beats_it() {
    let draw = |json: &str, o: &[(&str, f64)]| {
        let r = resolve_full(json, Some("idle"), &HashMap::new(), false);
        assert!(r.ok, "{:?}", r.diagnostics);
        let mut ov = r.overrides.clone();
        ov.extend(o.iter().map(|(k, v)| (k.to_string(), *v)));
        crate::frame_with_overrides(r.state.clone(), 64, 1.3, ov).unwrap()
    };
    let glossy = draw(
        &bean_with(json!({ "style": "glossy", "iris": "aroma", "sclera": true })),
        &[],
    );
    let aroma = crate::character::recipe::recipes()["bean"]
        .colour_named("aroma")
        .unwrap();
    // Two irises (a three-stop radial gradient) in the recipe's colour.
    let irises = glossy
        .fills
        .iter()
        .filter_map(|x| x.gradient.as_ref())
        .filter(|g| g.kind == 1 && g.stops.len() == 3 && (g.stops[1].hue - aroma.h).abs() < 1e-9)
        .count();
    assert_eq!(irises, 2);
    // A white sclera under it.
    assert!(glossy
        .fills
        .iter()
        .any(|x| x.white > 0.9 && x.a == 1.0 && x.saturation < 0.2));
    // `eyeStyle: shape` draws the shape eye again.
    let shape = draw(
        &bean_with(json!({ "style": "glossy" })),
        &[("eyeStyle", 1.0)],
    );
    assert_eq!(gradients(&shape), gradients(&frame("bean", &[])));
}

#[test]
fn an_unknown_style_is_an_error_in_a_recipe_and_in_params() {
    let r = resolve_full(
        &bean_with(json!({ "style": "shiny" })),
        None,
        &HashMap::new(),
        false,
    );
    assert!(!r.ok, "{:?}", r.diagnostics);
    assert!(
        r.diagnostics.iter().any(|d| d.message.contains("glossy")),
        "{:?}",
        r.diagnostics
    );
    let spec = |v: Value| {
        json!({ "fxSpec": "1.13", "object": "character", "pattern": "bean", "params": { "eyeStyle": v } })
            .to_string()
    };
    let r = resolve_full(&spec(json!("glosy")), None, &HashMap::new(), false);
    assert!(
        r.diagnostics.iter().any(|d| d.path == "/params/eyeStyle"),
        "{:?}",
        r.diagnostics
    );
    let r = resolve_full(&spec(json!("pixel")), None, &HashMap::new(), false);
    assert!(
        r.ok && r.overrides["eyeStyle"] == 3.0,
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn the_showcases_resolve_cleanly_and_stay_light() {
    for json in SHOWCASES {
        let r = resolve_full(json, Some("idle"), &HashMap::new(), false);
        assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        for size in [20u32, 32, 64] {
            let f = crate::frame_with_overrides(r.state.clone(), size, 1.3, r.overrides.clone())
                .unwrap();
            assert_eq!(crate::cost::frame_cost(&f, size).class, "light");
        }
    }
}

/// The showcases' frames, frozen (rounded so libm's last bits on another OS don't show).
#[test]
fn the_showcases_draw_as_frozen() {
    let digest = |f: &OrbFrame| {
        let mut s = String::new();
        for x in &f.fills {
            s += &format!(
                "F{:.3},{:.3},{:.3},{:.3},{:.2};",
                x.hue, x.saturation, x.white, x.a, x.blur
            );
            if let Some(g) = &x.gradient {
                s += &format!("G{},{:.2},{:.2},{:.2};", g.kind, g.x0, g.y0, g.r);
            }
            for p in x.points.iter().chain(x.holes.iter().flatten()) {
                s += &format!("{:.2},{:.2} ", p.x, p.y);
            }
        }
        s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let got: Vec<String> = SHOWCASES
        .iter()
        .flat_map(|json| {
            ["idle", "speaking"].map(|state| {
                let r = resolve_full(json, Some(state), &HashMap::new(), false);
                let f = crate::frame_with_overrides(r.state.clone(), 64, 0.6, r.overrides.clone())
                    .unwrap();
                format!("{:016x}", digest(&f))
            })
        })
        .collect();
    if std::env::var("EYES_FROZEN_PRINT").is_ok() {
        println!("{got:?}");
    }
    assert_eq!(
        got, FROZEN,
        "if the change is deliberate: EYES_FROZEN_PRINT=1 … -- --nocapture"
    );
}

const FROZEN: [&str; 6] = [
    "62481a473dbb2e18",
    "12783c983f7bb56f",
    "15c49f714aeddfb1",
    "c6aeeedae6d6862b",
    "078d39be5e0c28f6",
    "c22b06fc2eea6699",
];
