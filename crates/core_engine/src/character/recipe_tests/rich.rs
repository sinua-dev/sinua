//! The richer look (design note 22, FX Spec 1.13): elliptical gradients, stop
//! alphas, `shade` and `rim` layers, overlay bodies and grain, on the two
//! showcase examples (`spec/examples/rich-bean.fxspec.json`, `rich-buzzy`).
//! Under low power, and with the `grain` / `shading` options off, they shed.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::geom::{self, hsl, Xf};
use crate::fx_spec::{resolve_full, FxSpecResolved};
use crate::primitives::{Fill, OrbFrame};

const BEAN: &str = include_str!("../../../../../spec/examples/rich-bean.fxspec.json");
const BUZZY: &str = include_str!("../../../../../spec/examples/rich-buzzy.fxspec.json");

fn resolve(json: &str, low_power: bool) -> FxSpecResolved {
    resolve_full(json, Some("idle"), &HashMap::new(), low_power)
}

fn render(json: &str, low_power: bool, extra: &[(&str, f64)], size: u32) -> OrbFrame {
    let r = resolve(json, low_power);
    assert!(r.ok, "{:?}", r.diagnostics);
    let mut o = r.overrides.clone();
    for (k, v) in extra {
        o.insert(k.to_string(), *v);
    }
    crate::frame_with_overrides(r.state.clone(), size, 1.3, o).expect("a frame")
}

fn grain(f: &OrbFrame) -> Vec<&Fill> {
    f.fills.iter().filter(|x| x.blend == 2).collect()
}

fn elliptical(f: &OrbFrame) -> usize {
    f.fills
        .iter()
        .filter(|x| x.gradient.as_ref().is_some_and(|g| g.kind == 2))
        .count()
}

/// Soft masses: gradients that fade out to transparent (`shade`, the rim).
fn soft(f: &OrbFrame) -> usize {
    f.fills
        .iter()
        .filter(|x| {
            x.gradient
                .as_ref()
                .is_some_and(|g| g.stops.last().unwrap().a == 0.0)
        })
        .count()
}

fn with_params(json: &str, params: Value) -> String {
    let mut v: Value = serde_json::from_str(json).unwrap();
    for (k, x) in params.as_object().unwrap() {
        v["params"][k] = x.clone();
    }
    v.to_string()
}

#[test]
fn an_elliptical_gradient_carries_its_axis_in_x1_y1_and_maps_exactly() {
    let c = hsl(0.0, 0.0, 0.5);
    let f = geom::elliptical(
        geom::ellipse(0.0, 0.0, 10.0, 10.0, 0.0, 8),
        (10.0, 20.0),
        (30.0, 12.0),
        0.0,
        &[(0.0, c), (1.0, c)],
    );
    let g = f.gradient.clone().unwrap();
    assert_eq!(
        (g.kind, g.x0, g.y0, g.x1, g.y1, g.r),
        (2, 10.0, 20.0, 40.0, 20.0, 12.0)
    );
    // A quarter turn about the origin: the axis turns with it.
    let t = geom::transform(f, &Xf::rotate(std::f64::consts::FRAC_PI_2));
    let g = t.gradient.unwrap();
    assert!(
        (g.x0 + 20.0).abs() < 1e-9 && (g.y0 - 10.0).abs() < 1e-9,
        "{g:?}"
    );
    assert!(
        (g.x1 + 20.0).abs() < 1e-9 && (g.y1 - 40.0).abs() < 1e-9,
        "{g:?}"
    );
    assert!((g.r - 12.0).abs() < 1e-9);
}

#[test]
fn the_showcases_draw_soft_layers_rims_and_grain() {
    for (name, json) in [("bean", BEAN), ("buzzy", BUZZY)] {
        let r = resolve(json, false);
        assert!(
            r.ok && r.diagnostics.is_empty(),
            "{name}: {:?}",
            r.diagnostics
        );
        let f = render(json, false, &[], 64);
        assert!(elliptical(&f) >= 3, "{name}: {} elliptical", elliptical(&f));
        assert!(!grain(&f).is_empty(), "{name}: no grain");
        // A rim: a fill with a hole, lit by a linear gradient that fades out.
        let rim = f.fills.iter().any(|x| {
            !x.holes.is_empty()
                && x.gradient.as_ref().is_some_and(|g| {
                    g.kind == 0 && g.stops.len() == 2 && g.stops[0].a == 1.0 && g.stops[1].a == 0.0
                })
        });
        assert!(rim, "{name}: no rim");
        // Soft masses fade to transparent.
        let soft = f.fills.iter().any(|x| {
            x.gradient
                .as_ref()
                .is_some_and(|g| g.kind == 2 && g.stops.last().unwrap().a == 0.0)
        });
        assert!(soft, "{name}: no soft mass");
    }
}

#[test]
fn grain_sits_inside_each_body_under_its_outline_and_never_on_the_face() {
    let f = render(BEAN, false, &[], 200);
    let g = grain(&f);
    assert_eq!(g.len(), 1, "one body, one grain fill");
    assert!((g[0].a - 0.08).abs() < 1e-9);
    // The body's own outline (the next fill) and the face come after it.
    let at = f.fills.iter().position(|x| x.blend == 2).unwrap();
    assert!(at + 1 < f.fills.len());
    // BUZZY: the two overlay bodies carry it (helmet and torso).
    assert_eq!(grain(&render(BUZZY, false, &[], 200)).len(), 2);
}

#[test]
fn an_overlay_body_draws_no_fill_of_its_own() {
    let r: Value = serde_json::from_str(BUZZY).unwrap();
    let helmet = r["recipe"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["part"] == "body" && p["light"] == "none")
        .cloned()
        .unwrap();
    let mut alone = r.clone();
    let mut overlay = helmet.clone();
    overlay["inner"] = json!([]);
    overlay["outline"]["width"] = json!(0);
    alone["recipe"]["parts"] = json!([overlay]);
    alone["recipe"]["grain"] = json!({ "strength": 0 });
    let f = render(&alone.to_string(), false, &[], 64);
    assert!(f.fills.is_empty(), "{:?}", f.fills.len());
}

#[test]
fn the_options_turn_grain_and_shading_off() {
    let all = render(BEAN, false, &[], 64);
    let no_grain = render(&with_params(BEAN, json!({ "grain": 0 })), false, &[], 64);
    assert!(grain(&no_grain).is_empty());
    assert_eq!(no_grain.fills.len(), all.fills.len() - 1);
    let no_shading = render(&with_params(BEAN, json!({ "shading": 0 })), false, &[], 64);
    assert_eq!(soft(&no_shading), 0, "no soft masses, no rim");
    // Stronger grain from the option.
    let strong = render(&with_params(BEAN, json!({ "grain": 0.2 })), false, &[], 64);
    assert!((grain(&strong)[0].a - 0.2).abs() < 1e-9);
}

#[test]
fn grain_and_rims_are_left_out_at_20_px() {
    let f = render(BEAN, false, &[], 20);
    assert!(grain(&f).is_empty());
    // A rim: a fill with a hole and a fading light (an outline ring has a hole too).
    let rims = f
        .fills
        .iter()
        .filter(|x| {
            !x.holes.is_empty()
                && x.gradient
                    .as_ref()
                    .is_some_and(|g| g.stops.last().unwrap().a == 0.0)
        })
        .count();
    assert_eq!(rims, 0, "no rim at 20 px");
}

#[test]
fn low_power_sheds_grain_and_soft_layers_of_a_rich_character_only() {
    for json in [BEAN, BUZZY] {
        let r = resolve(json, true);
        assert!(r.ok);
        assert_eq!(r.overrides.get("grain"), Some(&0.0));
        assert_eq!(r.overrides.get("shading"), Some(&0.0));
        assert!(r.disabled_materials.contains(&"grain".to_string()));
        assert!(r.disabled_materials.contains(&"shading".to_string()));
        let f = render(json, true, &[], 64);
        assert!(grain(&f).is_empty() && soft(&f) == 0);
        // Back to "light" under low power.
        assert_eq!(crate::cost::frame_cost(&f, 64).class, "light");
    }
    // A plain character's low-power resolve is untouched.
    let plain = r##"{ "fxSpec": "1.13", "object": "character", "pattern": "bean" }"##;
    let r = resolve(plain, true);
    assert!(!r.overrides.contains_key("grain") && r.disabled_materials.is_empty());
}

#[test]
fn the_showcases_are_medium_at_most_and_never_heavy() {
    for json in [BEAN, BUZZY] {
        for size in [32u32, 64] {
            let c = crate::cost::frame_cost(&render(json, false, &[], size), size);
            assert_ne!(c.class, "heavy", "{c:?}");
            assert!(c.coverage < 2.0, "{c:?}");
        }
    }
}

#[test]
fn the_built_ins_draw_no_grain_and_no_elliptical_gradient() {
    for id in ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"] {
        let f = crate::character::recipe::frame(id, 64.0, 1.3, &Default::default()).unwrap();
        assert!(grain(&f).is_empty() && elliptical(&f) == 0, "{id}");
    }
}

#[test]
fn a_radial_light_takes_three_or_five_numbers() {
    let mut v: Value = serde_json::from_str(BEAN).unwrap();
    let body = v["recipe"]["parts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["part"] == "body")
        .unwrap();
    body["light"]["radial"] = json!([74, 62, 150, 128]);
    let r = resolve(&v.to_string(), false);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.path.ends_with("/light/radial")),
        "{:?}",
        r.diagnostics
    );
}

/// The showcases' frames, frozen (rounded so libm's last bits on another OS don't show).
#[test]
fn the_showcases_draw_as_frozen() {
    let digest = |f: &OrbFrame| {
        let mut s = String::new();
        for x in &f.fills {
            s += &format!(
                "F{:.3},{:.3},{:.3},{:.3},{};",
                x.hue, x.saturation, x.white, x.a, x.blend
            );
            if let Some(g) = &x.gradient {
                s += &format!(
                    "G{},{:.2},{:.2},{:.2},{:.2},{:.2};",
                    g.kind, g.x0, g.y0, g.x1, g.y1, g.r
                );
                for st in &g.stops {
                    s += &format!("{:.3},{:.3};", st.offset, st.a);
                }
            }
            for p in x.points.iter().chain(x.holes.iter().flatten()) {
                s += &format!("{:.2},{:.2} ", p.x, p.y);
            }
        }
        s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let got: Vec<String> = [BEAN, BUZZY]
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
    if std::env::var("RICH_FROZEN_PRINT").is_ok() {
        println!("{got:?}");
    }
    assert_eq!(
        got, FROZEN,
        "if the change is deliberate: RICH_FROZEN_PRINT=1 … -- --nocapture"
    );
}

const FROZEN: [&str; 4] = [
    "26e42776e5da24f5",
    "1a2cef035fda4b06",
    "47a933678456771f",
    "30512ef29066019b",
];
