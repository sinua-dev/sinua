//! Catalog packs (design note 26, FX Spec 1.13): Sinua's own pack
//! (`spec/catalog/catalog-1.json`) loads, every item fits every character or says
//! `fits`, files and loadouts name items as `"catalog:<id>"`, the `frame` slot,
//! and the loader's rules (idempotent, namespaces, unload, the cap, warnings).

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::catalog;
use crate::fx_spec::{apply_loadout, resolve_full};

const PACK: &str = include_str!("../../../../../spec/catalog/catalog-1.json");
const CHARACTERS: [&str; 7] = ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"];

fn load() {
    let d = catalog::load(PACK);
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn every_item_fits_every_character_or_says_which() {
    load();
    let pack: Value = serde_json::from_str(PACK).unwrap();
    let mut ids = Vec::new();
    for c in pack["cosmetics"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        assert!(!ids.contains(&id), "{id} twice");
        ids.push(id);
        assert!(c["category"].is_string(), "{id}: no category");
        // A slot is a capability (C1): an item fits every character that has its slot.
        assert!(
            c.get("fits").is_none(),
            "{id}: names characters; use the slot or `requires`"
        );
        for ch in CHARACTERS {
            let slots = &crate::character::recipe::recipes()[ch].slots;
            let has =
                c["slot"] == "frame" || slots.iter().any(|s| s.name == c["slot"].as_str().unwrap());
            let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": ch, "cosmetics": [format!("catalog:{id}")] });
            let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
            assert!(r.ok, "{id} on {ch}: {:?}", r.diagnostics);
            let worn = r.state.starts_with("recipe:");
            assert_eq!(worn, has, "{id} on {ch}: {:?}", r.diagnostics);
            if worn {
                assert!(
                    r.diagnostics.is_empty(),
                    "{id} on {ch}: {:?}",
                    r.diagnostics
                );
                let f = crate::frame_with_overrides(r.state.clone(), 64, 1.0, r.overrides.clone())
                    .unwrap();
                assert_ne!(
                    crate::cost::frame_cost(&f, 64).class,
                    "heavy",
                    "{id} on {ch}"
                );
            }
        }
    }
    assert_eq!(ids.len(), 14);
    assert_eq!(pack["palettes"].as_object().unwrap().len(), 11);
}

#[test]
fn a_catalog_palette_paints_like_a_named_one() {
    load();
    for name in ["berry", "lagoon", "citrus", "lavender", "cocoa", "neon"] {
        let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": "bean", "palette": format!("catalog:{name}") });
        let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
        assert!(
            r.ok && r.diagnostics.is_empty(),
            "{name}: {:?}",
            r.diagnostics
        );
        assert!(
            r.overrides.contains_key("palette.body.w")
                && r.overrides.contains_key("palette.dark.body.w"),
            "{name}"
        );
    }
}

/// Design note 27: the catalog's eye colours, named by a loadout's `iris`, join the
/// palette it picks and colour the glossy eye.
#[test]
fn a_loadout_picks_a_catalog_eye_colour() {
    load();
    let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": "chirp",
        "params": { "eyeStyle": "glossy" } })
    .to_string();
    let plain = resolve_full(&spec, None, &HashMap::new(), false);
    for name in ["brown", "blue", "green", "hazel", "violet"] {
        let l = json!({ "loadout": 1, "palette": "catalog:berry", "iris": format!("catalog:eyes-{name}") });
        let (s, d) = apply_loadout(&spec, &l.to_string());
        assert!(d.is_empty(), "{name}: {d:?}");
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["palette"]["theme"], "catalog:berry", "{name}");
        let r = resolve_full(&s, None, &HashMap::new(), false);
        assert!(
            r.ok && r.diagnostics.is_empty(),
            "{name}: {:?}",
            r.diagnostics
        );
        assert!(r.overrides.contains_key("palette.iris.w"), "{name}");
        let draw = |r: &crate::fx_spec::FxSpecResolved| {
            crate::frame_with_overrides(r.state.clone(), 64, 1.0, r.overrides.clone()).unwrap()
        };
        assert_ne!(draw(&r), draw(&plain), "{name}");
    }
    // A colour palette never carries an eye colour by accident.
    let (_, d) = apply_loadout(
        &spec,
        &json!({ "loadout": 1, "iris": "catalog:berry" }).to_string(),
    );
    assert_eq!(d.len(), 1, "{d:?}");
}

#[test]
fn a_loadout_wears_from_the_catalog_through_the_wardrobe() {
    load();
    let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": "chirp",
        "wardrobe": { "cosmetics": ["catalog:crown", "catalog:scarf", "catalog:halo-ring"] } })
    .to_string();
    let (s, d) = apply_loadout(&spec, &json!({ "loadout": 1, "wear": ["crown", "scarf", "halo-ring"], "palette": "catalog:berry" }).to_string());
    assert!(d.is_empty(), "{d:?}");
    let r = resolve_full(&s, None, &HashMap::new(), false);
    assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let fits: Vec<(String, String)> = crate::fx_spec::cosmetics_for(&spec, "bean")
        .into_iter()
        .map(|d| (d.path, d.severity))
        .collect();
    assert!(
        fits.contains(&("scarf".into(), "no-slot".into())),
        "{fits:?}"
    );
    assert!(
        fits.contains(&("halo-ring".into(), "fits".into())),
        "{fits:?}"
    );
}

#[test]
fn a_frame_sits_behind_the_character_and_round_it() {
    load();
    let draw = |c: Value| {
        let spec =
            json!({ "fxSpec": "1.13", "object": "character", "pattern": "bean", "cosmetics": c });
        let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
        crate::frame_with_overrides(r.state.clone(), 200, 1.0, r.overrides.clone()).unwrap()
    };
    let plain = draw(json!([]));
    let framed = draw(json!(["catalog:halo-ring"]));
    // The ring is drawn first (behind), then the bean exactly as before.
    let n = framed.fills.len() - plain.fills.len();
    assert!(n >= 1);
    assert_eq!(framed.fills[n..], plain.fills[..]);
    let xs: Vec<f64> = framed.fills[0].points.iter().map(|p| p.x).collect();
    let span =
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    assert!(span > 180.0, "round the whole character: {span}");
}

#[test]
fn the_loader_keeps_its_rules() {
    // A brand's own namespace, loaded twice (it replaces itself), then unloaded.
    let brand = |hat: &str| {
        json!({ "catalog": 1, "namespace": "acme-test", "cosmetics": [
        { "id": hat, "slot": "headTop", "parts": [{ "part": "body", "shape": { "ellipse": [0, -10, 20, 10, 0, 20] },
          "light": { "radial": [0, -10, 20], "stops": [[0, "line"], [1, "line"]] }, "outline": { "width": 0, "color": "line" } }],
          "palette": { "line": [0, 0, 0.2] } }] }).to_string()
    };
    assert!(catalog::load(&brand("cap")).is_empty());
    assert!(catalog::load(&brand("cap")).is_empty());
    assert!(catalog::get("acme-test:cap", false).is_some());
    assert!(catalog::load(&brand("visor")).is_empty());
    assert!(
        catalog::get("acme-test:cap", false).is_none(),
        "a reload replaces the namespace"
    );
    assert!(catalog::unload("acme-test"));
    assert!(!catalog::unload("acme-test"));
    // Bad packs load nothing.
    for bad in [
        json!({ "namespace": "x" }),
        json!({ "catalog": 1, "namespace": "Bad Name" }),
        json!([1]),
    ] {
        let d = catalog::load(&bad.to_string());
        assert!(d.iter().any(|x| x.severity == "error"), "{bad}: {d:?}");
    }
    let many: Vec<Value> = (0..catalog::MAX_ITEMS + 1)
        .map(|i| json!({ "id": format!("i{i}"), "slot": "chest", "parts": [] }))
        .collect();
    let d = catalog::load(
        &json!({ "catalog": 1, "namespace": "too-many", "cosmetics": many }).to_string(),
    );
    assert!(d.iter().any(|x| x.severity == "error"), "{d:?}");
    assert!(catalog::get("too-many:i0", false).is_none());
    // A newer format only warns.
    let d = catalog::load(&json!({ "catalog": 2, "namespace": "newer-test" }).to_string());
    assert!(
        d.iter().all(|x| x.severity == "warning") && !d.is_empty(),
        "{d:?}"
    );
    catalog::unload("newer-test");
}

#[test]
fn a_missing_catalog_item_only_warns() {
    let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": "bean",
        "cosmetics": ["nopack:hat"], "palette": "nopack:mint" });
    let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
    assert!(r.ok, "{:?}", r.diagnostics);
    let paths: Vec<&str> = r.diagnostics.iter().map(|d| d.path.as_str()).collect();
    assert!(
        paths.contains(&"/cosmetics/0") && paths.contains(&"/palette/theme"),
        "{paths:?}"
    );
    assert_eq!(r.state, "bean");
}

/// What a frame of `id` wearing the catalog's `item` draws for `o` (still, no glance).
fn worn(id: &str, item: &str, o: &[(&str, f64)]) -> crate::primitives::OrbFrame {
    load();
    let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": id, "cosmetics": [format!("catalog:{item}")] });
    let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
    let mut ov = r.overrides.clone();
    ov.insert("still".into(), 1.0);
    ov.extend(o.iter().map(|(k, v)| (k.to_string(), *v)));
    crate::frame_with_overrides(r.state.clone(), 200, 1.0, ov).unwrap()
}

/// The cosmetic's fills: those after the character's own (it draws on top).
fn item_fills(id: &str, item: &str, o: &[(&str, f64)]) -> Vec<crate::primitives::Fill> {
    let plain = {
        let spec = json!({ "fxSpec": "1.13", "object": "character", "pattern": id });
        let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
        let mut ov = r.overrides.clone();
        ov.insert("still".into(), 1.0);
        ov.extend(o.iter().map(|(k, v)| (k.to_string(), *v)));
        crate::frame_with_overrides(r.state.clone(), 200, 1.0, ov)
            .unwrap()
            .fills
            .len()
    };
    worn(id, item, o).fills[plain..].to_vec()
}

fn mid_x(f: &[crate::primitives::Fill]) -> f64 {
    let xs: Vec<f64> = f
        .iter()
        .flat_map(|x| x.points.iter().map(|p| p.x))
        .collect();
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn width(f: &[crate::primitives::Fill]) -> f64 {
    let xs = f.iter().flat_map(|x| x.points.iter().map(|p| p.x));
    let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    hi - lo
}

#[test]
fn glasses_follow_the_gaze_as_the_eyes_do() {
    for id in ["bean", "buzzy", "chirp"] {
        let at = |gx: f64| mid_x(&item_fills(id, "round-glasses", &[("gazeX", gx)]));
        let moved = at(6.0) - at(0.0);
        assert!(moved > 3.0, "{id}: glasses moved {moved}");
    }
}

#[test]
fn head_and_body_cosmetics_turn_with_the_head() {
    // A badge slides round the body with the patches under it. (A hat on top turns about
    // the head's own vertical axis: it narrows, below, but doesn't slide.)
    for (id, item) in [
        ("beep", "star-badge"),
        ("cuppa", "heart-badge"),
        ("bean", "star-badge"),
    ] {
        let at = |yaw: f64| item_fills(id, item, &[("turnYaw", yaw), ("turn", 1.0)]);
        let (a, b) = (at(0.0), at(0.9));
        assert!(
            (mid_x(&b) - mid_x(&a)).abs() > 1.0,
            "{id} {item}: it slides with the turn"
        );
    }
    // A hat narrows as the head turns.
    let w = |yaw: f64| {
        width(&item_fills(
            "bean",
            "crown",
            &[("turnYaw", yaw), ("turn", 1.0)],
        ))
    };
    assert!(w(0.9) < w(0.0) - 0.5, "{} vs {}", w(0.9), w(0.0));
}

#[test]
fn the_far_headphone_cup_fades_round_the_back() {
    let f = item_fills("chirp", "headphones", &[("turnYaw", 1.0), ("turn", 1.0)]);
    let min_a = f.iter().map(|x| x.a).fold(1.0, f64::min);
    assert!(min_a < 0.5, "a cup going behind fades: {min_a}");
    let rest = item_fills("chirp", "headphones", &[]);
    assert!(
        rest.iter()
            .all(|x| x.a > 0.99 || x.a == 0.0 || x.gradient.is_some()),
        "facing: nothing faded"
    );
}

#[test]
fn a_frame_stays_put_while_the_head_turns() {
    let a = worn("bean", "halo-ring", &[]).fills[0].clone();
    let b = worn("bean", "halo-ring", &[("turnYaw", 0.9), ("turn", 1.0)]).fills[0].clone();
    assert_eq!(a.points, b.points);
}
