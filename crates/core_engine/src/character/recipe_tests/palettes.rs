//! Palettes (design note 23, FX Spec 1.13): roles in the recipes, the dark variant
//! picked by the `dark` opt, precedence, the 1.13 gate, the 1.12 behaviour kept, and
//! the named palettes' removal (design note 38).

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::palette;
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
fn the_dark_opt_picks_the_dark_variant_and_changes_nothing_without_one() {
    let r = resolve(spec(
        "cuppa",
        json!({ "primary": "#E63946", "dark": { "primary": "#1D3557" } }),
    ));
    assert!(r.ok, "{:?}", r.diagnostics);
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
fn a_slot_beats_its_role() {
    let r = resolve(spec(
        "buzzy",
        json!({ "primary": "#00FF00", "accent": "#FF00FF", "body": "#0000FF",
            "dark": { "accent": "#FFFF00" } }),
    ));
    assert!(r.ok, "{:?}", r.diagnostics);
    // `body` is Buzzy's primary: the slot given outright wins.
    assert!((r.overrides["palette.body.h"] - 240.0).abs() < 1e-6);
    // Its accent role is its `accent` slot.
    assert!((r.overrides["palette.accent.h"] - 300.0).abs() < 1e-6);
    // The dark variant keeps the file's own colours, then its `dark`.
    assert!((r.overrides["palette.dark.body.h"] - 240.0).abs() < 1e-6);
    assert!((r.overrides["palette.dark.accent.h"] - 60.0).abs() < 1e-6);
}

#[test]
fn a_role_a_character_does_not_have_is_an_error_when_written() {
    // Buzzy maps no `secondary`: written outright, it is a slot name Buzzy doesn't have.
    let r = resolve(spec("buzzy", json!({ "secondary": "#FFFFFF" })));
    assert!(
        errors(&r).iter().any(|(p, _)| p == "/palette/secondary"),
        "{:?}",
        r.diagnostics
    );
}

/// Named palettes were removed in 0.1.0-beta.9 (design note 38): an error in any version.
#[test]
fn a_named_palette_is_an_error_in_any_version() {
    for (palette, at) in [
        (json!("sunset"), "/palette"),
        (json!({ "theme": "ocean" }), "/palette/theme"),
    ] {
        for version in ["1.12", "1.13", "1.14"] {
            let mut v = spec("bean", palette.clone());
            v["fxSpec"] = json!(version);
            let r = resolve(v);
            assert!(!r.ok, "{palette} {version}");
            assert!(
                errors(&r)
                    .iter()
                    .any(|(p, m)| p == at && m.contains("removed in 0.1.0-beta.9")),
                "{palette} {version}: {:?}",
                r.diagnostics
            );
        }
    }
    // In a state too.
    let r = resolve(
        json!({ "fxSpec": "1.13", "object": "character", "pattern": "chirp",
        "states": { "speaking": { "palette": "forest" } } }),
    );
    assert!(
        errors(&r)
            .iter()
            .any(|(p, _)| p == "/states/speaking/palette"),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn dark_needs_fx_spec_1_13() {
    let mut v = spec("bean", json!({ "dark": { "body": "#000000" } }));
    v["fxSpec"] = json!("1.12");
    assert!(!resolve(v).ok);
    // A 1.12 slot palette is unchanged.
    let mut v = spec("bean", json!({ "body": "#E63946" }));
    v["fxSpec"] = json!("1.12");
    assert!(resolve(v).ok);
}

#[test]
fn a_state_patches_the_palette() {
    let r = resolve_full(
        &json!({ "fxSpec": "1.13", "object": "character", "pattern": "chirp",
            "palette": { "primary": "#00FF00" },
            "states": { "speaking": { "palette": { "accent": "#FF0000" } } } })
        .to_string(),
        Some("speaking"),
        &HashMap::new(),
        false,
    );
    assert!(r.ok, "{:?}", r.diagnostics);
    // The base's primary stays; the state's accent (Chirp's beak) is red.
    assert!((r.overrides["palette.body.h"] - 120.0).abs() < 1e-3);
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
    let r = crate::palette_overrides(
        "bean".into(),
        json!({ "primary": "#E63946", "dark": { "primary": "#1D3557" } }).to_string(),
    );
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert!(
        r.overrides.contains_key("palette.body.w")
            && r.overrides.contains_key("palette.dark.body.w")
    );
}

/// Design note 27: slots are named by the part they paint, and every slot has a label
/// for a colour picker.
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
        assert!(
            r.colour_named("iris").is_none(),
            "{id}: an iris slot (removed)"
        );
    }
}
