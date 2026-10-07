//! The generic wardrobe, the named palettes' recipe side and the eye styles were removed
//! in 0.1.0-beta.9 (design note 38): a recipe or a file that still uses one is rejected,
//! whatever its version, with an error that says so.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::recipe::{Recipe, RECIPES};
use crate::fx_spec::resolve_full;

fn bean() -> Value {
    serde_json::from_str(RECIPES.iter().find(|(id, _)| *id == "bean").unwrap().1).unwrap()
}

fn removed(e: &str, at: &str) {
    assert!(
        e.starts_with(at) && e.contains("removed in 0.1.0-beta.9"),
        "{at}: {e}"
    );
}

#[test]
fn a_recipe_with_a_removed_field_is_rejected() {
    for (k, v) in [
        (
            "slots",
            json!({ "head": { "at": [100, 40], "follows": "head" } }),
        ),
        ("tags", json!(["round-head"])),
        ("cosmetics", json!([])),
    ] {
        let mut r = bean();
        r[k] = v;
        removed(
            &Recipe::parse(&r.to_string()).unwrap_err(),
            &format!("/{k}"),
        );
    }
    let mut r = bean();
    r["roles"]["iris"] = json!("body");
    removed(&Recipe::parse(&r.to_string()).unwrap_err(), "/roles/iris");
}

#[test]
fn a_part_with_a_removed_field_is_rejected() {
    let r = bean();
    let parts = r["parts"].as_array().unwrap();
    let eyes = parts
        .iter()
        .position(|p| p["part"] == "eyes")
        .expect("bean has eyes");
    for (k, v) in [
        ("style", json!("glossy")),
        ("iris", json!("body")),
        ("sclera", json!(true)),
    ] {
        let mut r = bean();
        r["parts"][eyes][k] = v;
        removed(
            &Recipe::parse(&r.to_string()).unwrap_err(),
            &format!("/parts/{eyes}/{k}"),
        );
    }
    let mut r = bean();
    r["parts"][0]["role"] = json!("head");
    removed(&Recipe::parse(&r.to_string()).unwrap_err(), "/parts/0/role");
}

#[test]
fn a_file_with_the_wardrobe_or_an_eye_style_is_rejected_in_any_version() {
    for version in ["1.13", "1.14"] {
        for (k, v) in [
            ("cosmetics", json!([])),
            ("wardrobe", json!({ "cosmetics": [] })),
        ] {
            let mut f = json!({ "fxSpec": version, "object": "character", "pattern": "bean" });
            f[k] = v;
            let r = resolve_full(&f.to_string(), None, &HashMap::new(), false);
            assert!(!r.ok, "{k} {version}");
            assert!(
                r.diagnostics
                    .iter()
                    .any(|d| d.path == format!("/{k}")
                        && d.message.contains("removed in 0.1.0-beta.9")),
                "{k} {version}: {:?}",
                r.diagnostics
            );
        }
        let f = json!({ "fxSpec": version, "object": "character", "pattern": "bean",
            "params": { "eyeStyle": "glossy" } });
        let r = resolve_full(&f.to_string(), None, &HashMap::new(), false);
        assert!(!r.ok, "eyeStyle {version}");
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.path == "/params/eyeStyle"
                    && d.message.contains("removed in 0.1.0-beta.9")),
            "eyeStyle {version}: {:?}",
            r.diagnostics
        );
    }
}
