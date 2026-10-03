//! Named palettes (design note 23, FX Spec 1.13): roles in the recipes, the
//! built-in themes, their dark variants picked by the `dark` opt, precedence,
//! the 1.13 gate, and the 1.12 behaviour kept.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::palette::{self, THEMES};
use crate::character::recipe::{recipes, Recipe, RECIPES};
use crate::fx_spec::{resolve_full, FxSpecResolved};

const CHARACTERS: [&str; 7] = ["buzzy", "hum", "wisp", "chirp", "cuppa", "bean", "beep"];

fn resolve(v: Value) -> FxSpecResolved {
    resolve_full(&v.to_string(), None, &HashMap::new(), false)
}

fn spec(id: &str, palette: Value) -> Value {
    json!({ "fxSpec": "1.13", "object": "character", "pattern": id, "palette": palette })
}

fn errors(r: &FxSpecResolved) -> Vec<(String, String)> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| (d.path.clone(), d.message.clone()))
        .collect()
}

fn slot_of(id: &str, role: &str) -> Option<String> {
    let r = &recipes()[id];
    r.roles
        .iter()
        .find(|(x, _)| x == role)
        .map(|(_, i)| r.palette[*i].0.clone())
}

#[test]
fn every_built_in_maps_primary_and_accent_to_its_own_slots() {
    for id in CHARACTERS {
        assert!(slot_of(id, "primary").is_some(), "{id}: no primary");
        assert!(slot_of(id, "accent").is_some(), "{id}: no accent");
    }
}

#[test]
fn every_theme_paints_every_character_light_and_dark() {
    for (name, light, dark) in THEMES {
        assert_eq!(light.len(), 3, "{name}");
        assert_eq!(dark.len(), 3, "{name}");
        for id in CHARACTERS {
            let r = resolve(spec(id, json!(name)));
            assert!(
                r.ok && errors(&r).is_empty(),
                "{name} on {id}: {:?}",
                r.diagnostics
            );
            let primary = slot_of(id, "primary").unwrap();
            let want = f64::from(light.iter().find(|(k, _)| *k == "primary").unwrap().1[0]);
            let h = r.overrides[&format!("palette.{primary}.h")];
            assert!((h - want).abs() < 1e-3, "{name} on {id}: {h} vs {want}");
            assert!(
                r.overrides
                    .contains_key(&format!("palette.dark.{primary}.w")),
                "{name} on {id}: no dark variant"
            );
        }
    }
}

#[test]
fn the_dark_opt_picks_the_dark_variant_and_changes_nothing_without_one() {
    let r = resolve(spec("cuppa", json!("ocean")));
    let frame = |dark: f64| {
        let mut o = r.overrides.clone();
        o.insert("dark".into(), dark);
        crate::frame_with_overrides(r.state.clone(), 64, 1.0, o).unwrap()
    };
    assert_ne!(format!("{:?}", frame(0.0)), format!("{:?}", frame(1.0)));
    // A plain character draws the same in both themes (its colours are fixed).
    let plain = |dark: f64| {
        let o: HashMap<String, f64> = [("dark".to_string(), dark)].into();
        crate::frame_with_overrides("cuppa".into(), 64, 1.0, o).unwrap()
    };
    assert_eq!(format!("{:?}", plain(0.0)), format!("{:?}", plain(1.0)));
}

#[test]
fn a_slot_beats_its_role_and_a_role_beats_the_theme() {
    let r = resolve(spec(
        "buzzy",
        json!({ "theme": "sunset", "primary": "#00FF00", "accent": "#FF00FF", "body": "#0000FF" }),
    ));
    assert!(r.ok, "{:?}", r.diagnostics);
    // `body` is Buzzy's primary: the slot given outright wins.
    assert!((r.overrides["palette.body.h"] - 240.0).abs() < 1e-6);
    // Its accent role is its `accent` slot (a slot named like a role is reached through
    // the role): the explicit role beats the theme's.
    assert!((r.overrides["palette.accent.h"] - 300.0).abs() < 1e-6);
    // The dark variant keeps the file's own colours too.
    assert!((r.overrides["palette.dark.body.h"] - 240.0).abs() < 1e-6);
}

#[test]
fn a_role_a_character_does_not_have_is_skipped_from_a_theme_and_an_error_when_written() {
    // Buzzy maps no `secondary`: the theme's is skipped quietly.
    let r = resolve(spec("buzzy", json!("candy")));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    // Written outright, it is a slot name Buzzy doesn't have.
    let r = resolve(spec("buzzy", json!({ "secondary": "#FFFFFF" })));
    assert!(
        errors(&r).iter().any(|(p, _)| p == "/palette/secondary"),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn an_unknown_palette_says_what_it_meant() {
    let r = resolve(spec("bean", json!("sunst")));
    let e = errors(&r);
    assert!(
        e.iter()
            .any(|(p, m)| p == "/palette/theme" && m.contains("`sunset`")),
        "{e:?}"
    );
}

#[test]
fn a_palette_name_theme_and_dark_need_fx_spec_1_13() {
    for palette in [
        json!("sunset"),
        json!({ "theme": "ocean" }),
        json!({ "dark": { "body": "#000000" } }),
    ] {
        let mut v = spec("bean", palette.clone());
        v["fxSpec"] = json!("1.12");
        let r = resolve(v);
        assert!(!r.ok, "{palette}: {:?}", r.diagnostics);
    }
    // A 1.12 slot palette is unchanged.
    let mut v = spec("bean", json!({ "body": "#E63946" }));
    v["fxSpec"] = json!("1.12");
    assert!(resolve(v).ok);
}

#[test]
fn a_state_patches_a_named_palette() {
    let r = resolve_full(
        &json!({ "fxSpec": "1.13", "object": "character", "pattern": "chirp", "palette": "forest",
            "states": { "speaking": { "palette": { "accent": "#FF0000" } } } })
        .to_string(),
        Some("speaking"),
        &HashMap::new(),
        false,
    );
    assert!(r.ok, "{:?}", r.diagnostics);
    // The theme's primary stays; the state's accent (Chirp's beak) is red.
    let forest = f64::from(THEMES.iter().find(|t| t.0 == "forest").unwrap().1[0].1[0]);
    assert!((r.overrides["palette.body.h"] - forest).abs() < 1e-3);
    assert!(r.overrides["palette.beak.h"].abs() < 1e-6);
}

#[test]
fn a_role_name_a_recipe_does_not_map_stays_a_slot_name() {
    // A 1.12-style recipe with an `accent` slot and no `roles`.
    let mut v: Value =
        serde_json::from_str(RECIPES.iter().find(|(id, _)| *id == "beep").unwrap().1).unwrap();
    v.as_object_mut().unwrap().remove("roles");
    let r = Recipe::parse(&v.to_string()).unwrap();
    let c = crate::character::geom::hsl(10.0, 0.5, 0.5);
    let got = palette::expand(&r, &[("accent".into(), c)]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, "accent");
}

#[test]
fn the_views_palette_overrides_carry_the_dark_variant() {
    let r = crate::palette_overrides("bean".into(), json!({ "theme": "night" }).to_string());
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert!(
        r.overrides.contains_key("palette.body.w")
            && r.overrides.contains_key("palette.dark.body.w")
    );
}

#[test]
fn the_fx_spec_schema_lists_the_built_in_palettes() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../../../spec/fx-spec-1.schema.json")).unwrap();
    let mut listed: Vec<String> = schema["properties"]["palette"]["oneOf"][0]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    let mut names: Vec<String> = THEMES.iter().map(|t| t.0.to_string()).collect();
    listed.sort();
    names.sort();
    assert_eq!(
        listed, names,
        "spec/fx-spec-1.schema.json's palette names vs spec/palettes.json"
    );
}

/// Design note 27: slots are named by the part they paint, every built-in has an `iris`
/// slot behind the `iris` role, and every slot has a label for a colour picker.
#[test]
fn every_slot_is_named_by_its_part_and_labelled() {
    let labels: Value = serde_json::from_str(include_str!(
        "../../../../../spec/character-slot-labels.json"
    ))
    .unwrap();
    // Colour words and drawing jargon say nothing to someone picking a colour.
    const UNCLEAR: [&str; 14] = [
        "red", "amber", "cyan", "teal", "violet", "blue", "gold", "ice", "white", "ink", "line",
        "off", "steel", "shell",
    ];
    for id in CHARACTERS {
        let r = &recipes()[id];
        let own = labels[id]
            .as_object()
            .unwrap_or_else(|| panic!("{id}: no labels"));
        // `celebrate` is the engine's own burst colour, not a slot to paint.
        let slots: Vec<&String> = r
            .palette
            .iter()
            .map(|(s, _)| s)
            .filter(|s| *s != "celebrate")
            .collect();
        for slot in &slots {
            let slot = slot.as_str();
            assert!(!UNCLEAR.contains(&slot), "{id}: `{slot}`");
            let l = &own
                .get(slot)
                .unwrap_or_else(|| panic!("{id}: `{slot}` has no label"));
            for k in ["label", "description"] {
                assert!(
                    l[k].as_str().is_some_and(|s| !s.is_empty()),
                    "{id}.{slot}.{k}"
                );
            }
        }
        assert_eq!(
            own.len(),
            slots.len(),
            "{id}: a label for a slot it doesn't have"
        );
        assert!(r.colour_named("iris").is_some(), "{id}: no iris slot");
        assert!(
            r.roles.iter().any(|(k, _)| k == "iris"),
            "{id}: no iris role"
        );
    }
}
