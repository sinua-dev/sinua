//! Loadouts (design note 25, FX Spec 1.13): the wardrobe and a loadout applied
//! to a file (the shared vectors in `spec/loadout-vectors.json`, which every
//! platform also checks), what fits a character, the soft change between two
//! loadouts (pop in and out, the zoom, colours, the eye style in a blink), the
//! still pose for thumbnails, and the thumbnails' own registry place.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::wear::frame_wear;
use crate::fx_spec::{apply_loadout, cosmetics_for, resolve_full};
use crate::primitives::OrbFrame;
use crate::transition::TransitionSide;

const SPEC: &str = include_str!("../../../../../spec/examples/wardrobe-bean.fxspec.json");
const VECTORS: &str = include_str!("../../../../../spec/loadout-vectors.json");

fn applied(loadout: &Value) -> (Value, Vec<String>) {
    let (s, d) = apply_loadout(SPEC, &loadout.to_string());
    let mut paths: Vec<String> = d.iter().map(|x| x.path.clone()).collect();
    paths.sort();
    assert!(d.iter().all(|x| x.severity == "warning"), "{d:?}");
    (serde_json::from_str(&s).unwrap(), paths)
}

#[test]
fn the_shared_vectors_hold() {
    let v: Value = serde_json::from_str(VECTORS).unwrap();
    for c in v["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (spec, warnings) = applied(&c["loadout"]);
        let worn: Vec<&str> = spec["cosmetics"]
            .as_array()
            .map(|a| a.iter().map(|x| x["id"].as_str().unwrap()).collect())
            .unwrap_or_default();
        let want: Vec<&str> = c["wear"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(worn, want, "{name}: worn");
        assert_eq!(
            spec.get("palette").cloned().unwrap_or(Value::Null),
            c["palette"],
            "{name}: palette"
        );
        assert_eq!(
            spec["params"]
                .get("eyeStyle")
                .cloned()
                .unwrap_or(Value::Null),
            c["eyeStyle"],
            "{name}: eyeStyle"
        );
        let want: Vec<String> = serde_json::from_value(c["warnings"].clone()).unwrap();
        assert_eq!(warnings, want, "{name}: warnings");
        // Whatever the loadout said, the file still resolves and draws.
        let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
        assert!(r.ok, "{name}: {:?}", r.diagnostics);
        // A nudge (C2) lands in the cosmetic's `fit` for the drawn character.
        for (id, want) in c
            .get("fit")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let got = spec["cosmetics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == *id)
                .unwrap()["fit"]["bean"]
                .clone();
            for k in ["scale", "angle"] {
                let (g, w) = (got[k].as_f64().unwrap(), want[k].as_f64().unwrap());
                assert!((g - w).abs() < 1e-9, "{name}: {id} {k} {got}");
            }
            for i in 0..2 {
                let (g, w) = (
                    got["at"][i].as_f64().unwrap(),
                    want["at"][i].as_f64().unwrap(),
                );
                assert!((g - w).abs() < 1e-9, "{name}: {id} at {got}");
            }
        }
    }
    for f in v["fits"].as_array().unwrap() {
        let ch = f["character"].as_str().unwrap();
        let own = f.get("spec").map(Value::to_string);
        let got: HashMap<String, String> = cosmetics_for(own.as_deref().unwrap_or(SPEC), ch)
            .into_iter()
            .map(|d| {
                assert_eq!(d.message.is_empty(), d.severity == "fits", "{d:?}");
                (d.path, d.severity)
            })
            .collect();
        let want: HashMap<String, String> = serde_json::from_value(f["expect"].clone()).unwrap();
        assert_eq!(got, want, "{ch}");
    }
}

#[test]
fn an_unknown_category_is_an_error_where_it_is_worn() {
    let mut v: Value = serde_json::from_str(SPEC).unwrap();
    v["cosmetics"][0]["category"] = json!("crown");
    let r = resolve_full(&v.to_string(), None, &HashMap::new(), false);
    assert!(
        r.diagnostics.iter().any(|d| d.path.ends_with("/category")),
        "{:?}",
        r.diagnostics
    );
}

fn side(loadout: Value) -> TransitionSide {
    let (s, d) = apply_loadout(SPEC, &loadout.to_string());
    assert!(d.is_empty(), "{d:?}");
    let r = resolve_full(&s, Some("idle"), &HashMap::new(), false);
    assert!(r.ok, "{:?}", r.diagnostics);
    TransitionSide {
        state: r.state,
        speed: 1.0,
        overrides: r.overrides,
    }
}

fn top(f: &OrbFrame) -> f64 {
    f.fills
        .iter()
        .flat_map(|x| x.points.iter().map(|p| p.y))
        .fold(f64::MAX, f64::min)
}

#[test]
fn a_hat_pops_in_past_its_size_and_settles() {
    let (none, hat) = (
        side(json!({ "wear": [] })),
        side(json!({ "wear": ["party-hat"] })),
    );
    let at = |w: f64| frame_wear(&none, &hat, 200, 1.0, w).unwrap();
    let end =
        crate::frame_with_overrides(hat.state.clone(), 200, 1.0, hat.overrides.clone()).unwrap();
    assert_eq!(at(1.0).fills, end.fills, "it ends on the new loadout");
    let start =
        crate::frame_with_overrides(none.state.clone(), 200, 1.0, none.overrides.clone()).unwrap();
    assert_eq!(
        at(0.0).fills.len(),
        start.fills.len(),
        "it starts on the old one"
    );
    // The hat's tip rises past where it ends (the overshoot), then settles.
    let peak = [0.5, 0.6, 0.7]
        .map(|w| top(&at(w)))
        .into_iter()
        .fold(f64::MAX, f64::min);
    assert!(peak < top(&end) - 0.5, "{peak} vs {}", top(&end));
}

#[test]
fn a_hat_taken_off_shrinks_away_and_the_zoom_eases() {
    let (hat, none) = (
        side(json!({ "wear": ["party-hat"] })),
        side(json!({ "wear": [] })),
    );
    let at = |w: f64| frame_wear(&hat, &none, 200, 1.0, w).unwrap();
    // The hat sinks away (the top drops) ...
    assert!(top(&at(0.5)) > top(&at(0.0)) + 5.0);
    // ... and the bean grows back into the room it took: wider at every step.
    let width = |f: &OrbFrame| {
        let xs = f.fills.iter().flat_map(|x| x.points.iter().map(|p| p.x));
        let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        hi - lo
    };
    let ws: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0].map(|w| width(&at(w))).to_vec();
    assert!(ws.windows(2).all(|p| p[1] >= p[0] - 1e-9), "{ws:?}");
    let end =
        crate::frame_with_overrides(none.state.clone(), 200, 1.0, none.overrides.clone()).unwrap();
    assert_eq!(at(1.0).fills, end.fills, "it ends on the new loadout");
}

#[test]
fn a_new_eye_style_swaps_while_the_eyes_are_shut() {
    let (a, b) = (
        side(json!({ "wear": [] })),
        side(json!({ "wear": [], "eyeStyle": "glossy" })),
    );
    // The glossy eye draws more fills (lens, iris, pupil, highlights, lid).
    let fills = |w: f64| frame_wear(&a, &b, 200, 1.0, w).unwrap().fills.len();
    let plain = crate::frame_with_overrides(a.state.clone(), 200, 1.0, a.overrides.clone())
        .unwrap()
        .fills
        .len();
    assert_eq!(fills(0.0), plain, "it starts on the old style");
    assert_eq!(fills(0.3), plain, "the old style until the blink");
    assert!(fills(0.8) > plain, "the glossy eye after it");
    // Mid-change the eyes are fully shut: the blink hides the swap.
    let mut o = b.overrides.clone();
    o.insert("wearBlink".into(), 1.0);
    let shut = crate::frame_with_overrides(b.state.clone(), 200, 1.0, o).unwrap();
    assert_eq!(frame_wear(&a, &b, 200, 1.0, 0.5).unwrap().fills, shut.fills);
}

#[test]
fn colours_blend_through_the_change() {
    let (a, b) = (
        side(json!({ "wear": [] })),
        side(json!({ "wear": [], "palette": "mint" })),
    );
    let mid = frame_wear(&a, &b, 64, 1.0, 0.5).unwrap();
    let ends = [
        frame_wear(&a, &b, 64, 1.0, 0.0).unwrap(),
        frame_wear(&a, &b, 64, 1.0, 1.0).unwrap(),
    ];
    assert_ne!(mid.fills, ends[0].fills);
    assert_ne!(mid.fills, ends[1].fills);
}

#[test]
fn two_characters_are_not_a_loadout_change() {
    let bean = side(json!({ "wear": [] }));
    let hum = TransitionSide {
        state: "hum".into(),
        speed: 1.0,
        overrides: HashMap::new(),
    };
    assert!(frame_wear(&bean, &hum, 64, 1.0, 0.5).is_none());
}

#[test]
fn a_still_pose_neither_blinks_nor_glances() {
    let frame = |t: f64| {
        let o: HashMap<String, f64> =
            [("still".to_string(), 1.0), ("seed".to_string(), 3.0)].into();
        crate::frame_with_overrides("bean".into(), 64, t, o).unwrap()
    };
    let eyes = |f: &OrbFrame| f.fills.iter().filter(|x| x.white < 0.2).count();
    let base = eyes(&frame(0.0));
    for k in 0..40 {
        assert_eq!(eyes(&frame(f64::from(k) * 0.37)), base, "t {k}");
    }
}

#[test]
fn thumbnails_never_register_a_live_character() {
    use crate::character::registry;
    // Glasses no other test draws (their own colour), so their key is this test's alone.
    let mut v: Value = serde_json::from_str(SPEC).unwrap();
    v["wardrobe"]["cosmetics"][0]["palette"]["rim"] = json!([12, 0.34, 0.56]);
    let spec = v.to_string();
    let lo = json!({ "wear": ["round-glasses"] }).to_string();
    let (applied, _) = apply_loadout(&spec, &lo);
    let worn: Value = serde_json::from_str::<Value>(&applied).unwrap()["cosmetics"].clone();
    let key = registry::key_for("bean", &format!("bean+{worn}"));
    for i in 0..(registry::CAPACITY + 8) {
        let f = crate::frame_still(spec.clone(), lo.clone(), 64, (i as f64) * 0.05);
        assert!(f.is_some(), "thumbnail {i}");
    }
    assert!(
        registry::key(&key).is_none(),
        "a thumbnail went into the live registry"
    );
    // Resolved for a view, the same loadout does register.
    let r = resolve_full(&applied, None, &HashMap::new(), false);
    assert_eq!(r.state, key);
    // A thumbnail is the still pose: the same frame every time.
    assert_eq!(
        crate::frame_still(spec.clone(), lo.clone(), 64, 0.0),
        crate::frame_still(spec, lo, 64, 0.0)
    );
}
