//! Cosmetics (design note 21, FX Spec 1.13): the party hat from
//! `spec/examples/party-hat.fxspec.json` on every character. It sits on the
//! `headTop` slot and moves with it, zooms the character out to fit, is left
//! out at 20 px, takes `palette`, and a cosmetic that doesn't fit warns.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::recipe::{Recipe, RECIPES};
use crate::fx_spec::{resolve_full, FxSpecResolved};
use crate::primitives::{Fill, OrbFrame};

const EXAMPLE: &str = include_str!("../../../../../spec/examples/party-hat.fxspec.json");
const LATTE: &str = include_str!("../../../../../spec/examples/remix-latte.fxspec.json");
const CHARACTERS: [&str; 7] = ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"];

fn example() -> Value {
    serde_json::from_str(EXAMPLE).unwrap()
}

fn hat() -> Value {
    example()["cosmetics"][0].clone()
}

/// The example, on `id`, with `edit` applied.
fn spec(id: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut v = example();
    v["pattern"] = json!(id);
    edit(&mut v);
    v.to_string()
}

fn resolve(json: &str, state: Option<&str>) -> FxSpecResolved {
    resolve_full(json, state, &HashMap::new(), false)
}

fn render(json: &str, state: &str, extra: &[(&str, f64)], size: u32) -> OrbFrame {
    let r = resolve(json, Some(state));
    assert!(r.ok, "{:?}", r.diagnostics);
    let mut o = r.overrides.clone();
    for (k, v) in extra {
        o.insert(k.to_string(), *v);
    }
    crate::frame_with_overrides(r.state.clone(), size, 1.3, o).expect("a frame")
}

/// What the cosmetics drew: they come after the character's own fills, so the
/// frame's last fills beyond the plain character's count (no celebrate burst
/// in these poses).
fn worn(json: &str, state: &str, extra: &[(&str, f64)], size: u32) -> Vec<Fill> {
    let mut plain: Value = serde_json::from_str(json).unwrap();
    let o = plain.as_object_mut().unwrap();
    o.remove("cosmetics");
    o.remove("palette");
    let n = render(&plain.to_string(), state, extra, size).fills.len();
    let f = render(json, state, extra, size);
    f.fills[n.min(f.fills.len())..].to_vec()
}

/// The mean of the fills' points.
fn centre(fills: &[Fill]) -> (f64, f64) {
    let pts: Vec<_> = fills.iter().flat_map(|x| &x.points).collect();
    assert!(!pts.is_empty(), "nothing worn");
    let n = pts.len() as f64;
    (
        pts.iter().map(|p| p.x).sum::<f64>() / n,
        pts.iter().map(|p| p.y).sum::<f64>() / n,
    )
}

fn diags(r: &FxSpecResolved, severity: &str) -> Vec<(String, String)> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| (d.path.clone(), d.message.clone()))
        .collect()
}

/// A plain body part in local units, outlined in `colour`.
fn blob(shape: Value, colour: &str) -> Value {
    json!({ "part": "body", "shape": shape,
        "light": { "radial": [0, 0, 20], "stops": [[0, colour], [1, colour]] },
        "outline": { "width": 2, "color": colour } })
}

#[test]
fn every_character_wears_the_hat_in_every_state() {
    for id in CHARACTERS {
        let json = spec(id, |_| {});
        let r = resolve(&json, None);
        assert!(
            r.ok && r.diagnostics.is_empty(),
            "{id}: {:?}",
            r.diagnostics
        );
        assert!(r.state.starts_with("recipe:"), "{id}: {}", r.state);
        for state in ["idle", "listening", "thinking", "speaking"] {
            let n = worn(&json, state, &[], 64).len();
            assert!(n >= 4, "{id} {state}: {n} fills");
        }
    }
}

#[test]
fn the_hat_sits_on_the_head_inside_the_box_even_on_the_hop() {
    for id in CHARACTERS {
        let json = spec(id, |_| {});
        for extra in [vec![], vec![("effectCode", 4.0), ("effectAge", 0.18)]] {
            let hat = worn(&json, "idle", &extra, 200);
            let top = hat
                .iter()
                .flat_map(|x| &x.points)
                .map(|p| p.y)
                .fold(f64::MAX, f64::min);
            assert!(top >= 0.0, "{id} {extra:?}: the hat is cut off at {top}");
            assert!(centre(&hat).1 < 80.0, "{id}: {:?}", centre(&hat));
        }
    }
}

#[test]
fn the_hat_follows_the_head_tilt_and_hop() {
    for id in CHARACTERS {
        let json = spec(id, |_| {});
        let at = |extra: &[(&str, f64)]| centre(&worn(&json, "idle", extra, 200));
        let rest = at(&[("look", 0.0)]);
        let tilted = at(&[("look", 0.0), ("tilt", 0.3)]);
        assert!((tilted.0 - rest.0).abs() > 2.0, "{id}: {rest:?} {tilted:?}");
        let hopped = at(&[("look", 0.0), ("effectCode", 4.0), ("effectAge", 0.18)]);
        assert!(hopped.1 < rest.1 - 2.0, "{id}: {rest:?} {hopped:?}");
    }
}

#[test]
fn glasses_on_the_face_slot_turn_with_the_head() {
    let glasses = json!({ "id": "specs", "slot": "face",
        "palette": { "frame": [330, 0.5, 0.2] },
        "parts": [blob(json!({ "roundRect": [-30, -8, 60, 16, 6, 2] }), "frame")] });
    for id in CHARACTERS {
        let json = spec(id, |v| v["cosmetics"] = json!([glasses.clone()]));
        let at = |extra: &[(&str, f64)]| centre(&worn(&json, "idle", extra, 200));
        let rest = at(&[("look", 0.0)]);
        let turned = at(&[("look", 0.0), ("turnYaw", 0.8)]);
        assert!(turned.0 - rest.0 > 2.0, "{id}: {rest:?} {turned:?}");
    }
}

#[test]
fn a_head_top_cosmetic_zooms_the_character_out_to_fit_and_others_dont() {
    let zoom = |id: &str, c: Value| {
        let text = RECIPES.iter().find(|(n, _)| *n == id).unwrap().1;
        let mut r: Value = serde_json::from_str(text).unwrap();
        r["cosmetics"] = json!([c]);
        Recipe::parse(&r.to_string()).unwrap().zoom
    };
    // CHIRP has room above its head; HUM has the least.
    assert_eq!(zoom("chirp", hat()), 1.0);
    let hum = zoom("hum", hat());
    assert!(hum > 0.8 && hum < 0.9, "{hum}");
    let badge = json!({ "id": "badge", "slot": "chest",
        "parts": [blob(json!({ "ellipse": [0, 0, 8, 8, 0, 16] }), "celebrate")] });
    assert_eq!(zoom("hum", badge), 1.0);
}

#[test]
fn cosmetics_are_left_out_at_20_px_unless_accessories_is_on() {
    for id in CHARACTERS {
        let json = spec(id, |_| {});
        let off = [("accessories", 0.0)];
        assert!(worn(&json, "idle", &off, 20).is_empty(), "{id}");
        assert!(!worn(&json, "idle", &off, 32).is_empty(), "{id}");
        assert!(
            !worn(&json, "idle", &[("accessories", 1.0)], 20).is_empty(),
            "{id}"
        );
    }
}

#[test]
fn palette_repaints_a_cosmetic_by_its_prefixed_name() {
    let json = spec("bean", |v| {
        v["palette"] = json!({ "party-hat.felt": "#2E8B57" })
    });
    let r = resolve(&json, None);
    assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let green = worn(&json, "idle", &[], 64).iter().any(|x| {
        x.gradient
            .as_ref()
            .is_some_and(|g| (g.stops[0].hue - 146.0).abs() < 2.0)
    });
    assert!(green, "the felt is sea green");
}

#[test]
fn a_cosmetic_part_may_use_the_characters_own_colours() {
    let scarf = json!({ "id": "scarf", "slot": "chest",
        "parts": [blob(json!({ "roundRect": [-20, -10, 40, 10, 4, 2] }), "mug")] });
    let r = resolve(&spec("cuppa", |v| v["cosmetics"] = json!([scarf])), None);
    assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn a_cosmetic_that_does_not_fit_warns_and_is_not_drawn() {
    // Not in `fits`: nothing is worn, so the built-in draws as it is.
    let fits = spec("bean", |v| v["cosmetics"][0]["fits"] = json!(["cuppa"]));
    let r = resolve(&fits, None);
    assert!(r.ok);
    assert_eq!(r.state, "bean");
    let w = diags(&r, "warning");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, "/cosmetics/0/fits");
    assert!(
        w[0].1.contains("`party-hat` isn't made for `bean`"),
        "{w:?}"
    );
    // No such slot (Bean has no neck).
    let neck = spec("bean", |v| v["cosmetics"][0]["slot"] = json!("neck"));
    let r = resolve(&neck, None);
    assert!(r.ok);
    let w = diags(&r, "warning");
    assert_eq!(w[0].0, "/cosmetics/0/slot");
    assert!(w[0].1.contains("`bean` has no `neck` slot"), "{w:?}");
    // One fits, one doesn't: the one that fits is worn.
    let both = spec("bean", |v| {
        let mut neck = hat();
        neck["id"] = json!("neck-hat");
        neck["slot"] = json!("neck");
        v["cosmetics"] = json!([hat(), neck]);
    });
    let r = resolve(&both, None);
    assert!(r.state.starts_with("recipe:"));
    assert_eq!(diags(&r, "warning").len(), 1);
}

#[test]
fn a_fit_nudges_one_character_only() {
    let look = [("look", 0.0)];
    let plain = centre(&worn(&spec("cuppa", |_| {}), "idle", &look, 200));
    let nudged =
        spec(
            "cuppa",
            |v| {
                v["cosmetics"][0]["fit"] =
                    json!({ "cuppa": { "at": [10, 0] }, "bean": { "at": [-30, 0] } })
            },
        );
    let moved = centre(&worn(&nudged, "idle", &look, 200));
    // 10 local units at CUPPA's slot scale (0.95) and zoom: about 9 px at 200 px.
    assert!((moved.0 - plain.0 - 9.0).abs() < 2.5, "{plain:?} {moved:?}");
}

#[test]
fn broken_cosmetics_are_errors_with_their_pointer() {
    type Edit = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(Edit, &str)> = vec![
        (
            Box::new(|v| v["cosmetics"][0]["parts"][0]["part"] = json!("wings")),
            "/cosmetics/0/parts/0/part",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["parts"][0]["space"] = json!("body")),
            "/cosmetics/0/parts/0/space",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["parts"][0]["inner"][0]["surface"] = json!("face")),
            "/cosmetics/0/parts/0/inner/0/surface",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["parts"][0]["outline"]["color"] = json!("lime")),
            "/cosmetics/0/parts/0/outline/color",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["palette"]["felt"] = json!("pink")),
            "/cosmetics/0/palette/felt",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["hat"] = json!(1)),
            "/cosmetics/0/hat",
        ),
        (
            Box::new(|v| v["cosmetics"] = json!([hat(), hat()])),
            "/cosmetics/1/id",
        ),
        (
            Box::new(|v| v["cosmetics"][0]["id"] = json!("Party Hat")),
            "/cosmetics/0/id",
        ),
    ];
    for (edit, at) in cases {
        let r = resolve(&spec("bean", |v| edit(v)), None);
        let e = diags(&r, "error");
        assert!(!r.ok && e.iter().any(|(p, _)| p == at), "{at}: {e:?}");
    }
}

#[test]
fn cosmetics_need_a_character_and_fx_spec_1_13() {
    let orb = resolve(&spec("bean", |v| v["object"] = json!("orb")), None);
    let e = diags(&orb, "error");
    assert!(e.iter().any(|(p, _)| p == "/cosmetics"), "{e:?}");
    let old = resolve(&spec("bean", |v| v["fxSpec"] = json!("1.12")), None);
    let e = diags(&old, "error");
    assert!(e.iter().any(|(p, _)| p == "/cosmetics"), "{e:?}");
}

#[test]
fn a_files_own_recipe_wears_them_too_and_errors_point_into_cosmetics() {
    let with = |c: Option<Value>| {
        let mut v: Value = serde_json::from_str(LATTE).unwrap();
        v["fxSpec"] = json!("1.13");
        if let Some(c) = c {
            v["cosmetics"] = json!([c]);
        }
        let r = resolve(&v.to_string(), None);
        let f = r
            .ok
            .then(|| crate::frame_with_overrides(r.state.clone(), 64, 1.3, r.overrides.clone()));
        (r, f.flatten())
    };
    let (r, f) = with(Some(hat()));
    assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let (_, plain) = with(None);
    assert!(f.unwrap().fills.len() >= plain.unwrap().fills.len() + 4);
    let mut broken = hat();
    broken["parts"][0]["part"] = json!("beak");
    let e = diags(&with(Some(broken)).0, "error");
    assert!(
        e.iter().any(|(p, _)| p == "/cosmetics/0/parts/0/part"),
        "{e:?}"
    );
}

#[test]
fn every_character_with_the_hat_stays_light() {
    let poses: [(&str, Vec<(&str, f64)>); 7] = [
        ("idle", vec![]),
        ("listening", vec![("audioLevel", 0.7)]),
        ("thinking", vec![]),
        ("speaking", vec![("audioLevel", 0.8)]),
        ("idle", vec![("turnYaw", -0.6), ("turnPitch", 0.2)]),
        ("idle", vec![("effectCode", 4.0), ("effectAge", 0.18)]),
        ("idle", vec![("effectCode", 3.0), ("effectAge", 0.5)]),
    ];
    for id in CHARACTERS {
        let json = spec(id, |_| {});
        for size in [32u32, 64] {
            for (state, extra) in &poses {
                let c = crate::cost::frame_cost(&render(&json, state, extra, size), size);
                assert_eq!(c.class, "light", "{id} {size} {state} {extra:?}: {c:?}");
            }
        }
    }
}

/// The hat's frames, frozen: a change to how cosmetics draw shows here by name.
/// (The golden file holds built-in states only; a cosmetic is a registered
/// recipe, and the 1.13 lock holds how the example resolves.)
#[test]
fn the_hat_draws_as_frozen() {
    // Rounded to 0.01 px and 0.001 in colour, so libm's last bits on another
    // OS don't show.
    let digest = |f: &OrbFrame| {
        let mut s = String::new();
        for x in &f.fills {
            s += &format!(
                "F{:.3},{:.3},{:.3},{:.3};",
                x.hue, x.saturation, x.white, x.a
            );
            for p in x.points.iter().chain(x.holes.iter().flatten()) {
                s += &format!("{:.2},{:.2} ", p.x, p.y);
            }
        }
        for d in &f.dots {
            s += &format!("D{:.2},{:.2},{:.2};", d.x, d.y, d.r);
        }
        for p in &f.polylines {
            s += &format!("P{};", p.points.len());
        }
        s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let got: Vec<(String, String)> = CHARACTERS
        .iter()
        .flat_map(|id| {
            let json = spec(id, |_| {});
            ["idle", "speaking"].map(|state| {
                let r = resolve(&json, Some(state));
                let mut o = r.overrides.clone();
                o.insert("audioLevel".into(), 0.6);
                let f = crate::frame_with_overrides(r.state.clone(), 64, 0.6, o).unwrap();
                (format!("{id}-{state}"), format!("{:016x}", digest(&f)))
            })
        })
        .collect();
    if std::env::var("COSMETIC_FROZEN_PRINT").is_ok() {
        for (k, d) in &got {
            println!("    (\"{k}\", \"{d}\"),");
        }
    }
    assert_eq!(got.len(), FROZEN.len());
    for ((k, d), (fk, fd)) in got.iter().zip(FROZEN) {
        assert_eq!(
            (k.as_str(), d.as_str()),
            (fk, fd),
            "if the change is deliberate: COSMETIC_FROZEN_PRINT=1 … -- --nocapture"
        );
    }
}

const FROZEN: [(&str, &str); 14] = [
    ("buzzy-idle", "9924e54279e4ad7a"),
    ("buzzy-speaking", "0c6f44354e9a5f6b"),
    ("hum-idle", "1e3b5f7652807379"),
    ("hum-speaking", "711c371945454946"),
    ("wisp-idle", "75607caf1c86022d"),
    ("wisp-speaking", "6c34150855abc136"),
    ("chirp-idle", "2d796e3f24bf7e5c"),
    ("chirp-speaking", "b37493c5db0cac8f"),
    ("cuppa-idle", "032a714403f5b94c"),
    ("cuppa-speaking", "59553dd4aa13760b"),
    ("bean-idle", "7d035d16b966526a"),
    ("bean-speaking", "16eb27dfe81d0896"),
    ("beep-idle", "438295e5b4abbe70"),
    ("beep-speaking", "d9bb2a55776c0528"),
];
