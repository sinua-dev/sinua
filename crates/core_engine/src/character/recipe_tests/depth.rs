//! Depth and limits for imported characters (design note 28, 1.13 item 7a): parts
//! name a `role`, a cosmetic's `behind` / `above` draws against it, and a recipe
//! stays within 96 parts and 4,096 path points.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::character::recipe::{Recipe, Space, MAX_PARTS, MAX_PATH_POINTS};
use crate::fx_spec::resolve_full;

fn body(role: &str, shape: Value) -> Value {
    json!({ "part": "body", "space": "body", "role": role, "shape": shape,
        "light": { "linear": [0, 0, 0, 1], "stops": [[0, "a"], [1, "a"]] },
        "outline": { "width": 0, "color": "b" } })
}

/// A fox-like head: a body, two ears drawn over the head's top, then the head.
fn fox(cosmetic: Option<Value>) -> Value {
    let mut r = json!({
        "recipe": 1, "id": "fox", "palette": { "a": [24, 0.8, 0.55], "b": [24, 0.6, 0.2] },
        "hue": { "base": 24, "turns": ["a"] }, "rig": { "kind": "upright", "base": [100, 184] },
        "parts": [
            body("body", json!({ "ellipse": [100, 140, 40, 40, 0, 32] })),
            body("ears", json!({ "ellipse": [70, 40, 14, 24, 0, 24] })),
            body("ears", json!({ "ellipse": [130, 40, 14, 24, 0, 24] })),
            body("head", json!({ "ellipse": [100, 80, 46, 40, 0, 32] })),
        ],
        "burst": { "at": [100, 100], "r": 80, "colors": ["celebrate", "a", "b"] },
        "slots": { "headTop": { "at": [100, 44], "follows": "head" } },
    });
    if let Some(c) = cosmetic {
        r["cosmetics"] = json!([c]);
    }
    r
}

fn hat(extra: Value) -> Value {
    let mut c = json!({ "id": "beanie", "slot": "headTop", "palette": { "w": [210, 0.5, 0.5] },
        "parts": [{ "part": "body", "shape": { "ellipse": [0, -10, 30, 14, 0, 24] },
            "light": { "linear": [0, 0, 0, 1], "stops": [[0, "w"], [1, "w"]] },
            "outline": { "width": 0, "color": "w" } }] });
    if let (Some(o), Some(e)) = (c.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    c
}

/// Where the cosmetic's part landed among the character's.
fn hat_at(r: &Recipe) -> usize {
    r.parts
        .iter()
        .position(|p| matches!(p.space, Space::Slot(_)))
        .unwrap()
}

#[test]
fn a_part_names_its_role_and_an_unknown_role_is_an_error() {
    let r = Recipe::parse(&fox(None).to_string()).unwrap();
    let roles: Vec<u8> = r.parts.iter().map(|p| p.role).collect();
    assert_eq!(
        roles,
        [15, 3, 3, 1],
        "body, ears, ears, head (1 + ROLES index)"
    );
    let mut v = fox(None);
    v["parts"][1]["role"] = json!("earring");
    let e = Recipe::parse(&v.to_string()).unwrap_err();
    assert!(e.contains("/parts/1/role"), "{e}");
}

#[test]
fn behind_draws_before_the_first_part_with_the_role_and_above_after_the_last() {
    // Without depth: on top, as before.
    let plain = Recipe::parse(&fox(Some(hat(json!({})))).to_string()).unwrap();
    assert_eq!(hat_at(&plain), 4);
    // A beanie behind the ears: before the first ear, so both ears and the head
    // cover it (the fox's ears poke through).
    let behind = Recipe::parse(&fox(Some(hat(json!({ "behind": ["ears"] })))).to_string()).unwrap();
    assert_eq!(hat_at(&behind), 1);
    // Above the ears: after the last ear, under the head.
    let above = Recipe::parse(&fox(Some(hat(json!({ "above": ["ears"] })))).to_string()).unwrap();
    assert_eq!(hat_at(&above), 3);
    // `behind` wins when both match; a role the character lacks: on top.
    let both = Recipe::parse(
        &fox(Some(hat(json!({ "behind": ["ears"], "above": ["head"] })))).to_string(),
    )
    .unwrap();
    assert_eq!(hat_at(&both), 1);
    let none =
        Recipe::parse(&fox(Some(hat(json!({ "behind": ["antenna"] })))).to_string()).unwrap();
    assert_eq!(hat_at(&none), 4);
    // Every built-in has no roles, so depth changes none of them (the golden holds that too).
    let e =
        Recipe::parse(&fox(Some(hat(json!({ "behind": ["earring"] })))).to_string()).unwrap_err();
    assert!(e.contains("lists of part roles"), "{e}");
}

#[test]
fn the_part_and_point_limits_hold_and_the_worst_case_stays_bounded() {
    assert_eq!((MAX_PARTS, MAX_PATH_POINTS), (96, 4096));
    // A wiggly closed path of radius about `big` round the middle, `segs` cubic segments.
    let path = |segs: usize, i: usize, big: f64| {
        let mut d = format!("M{} 100 ", 100.0 - big);
        for k in 0..segs {
            let a = (k as f64 + 1.0) / segs as f64 * std::f64::consts::TAU;
            let r = big + 2.0 * ((k * 7 + i) % 5) as f64;
            let (x, y) = (100.0 - r * a.cos(), 100.0 + r * a.sin());
            d += &format!("C{x:.1} {:.1} {x:.1} {y:.1} {x:.1} {y:.1} ", y - 3.0);
        }
        d + "Z"
    };
    let recipe = |n: usize, segs: usize, big: f64| {
        let mut v = fox(None);
        v["parts"] = json!((0..n)
            .map(|i| body("body", json!({ "path": path(segs, i, big) })))
            .collect::<Vec<_>>());
        v
    };
    // 97 parts: over.
    let e = Recipe::parse(&recipe(97, 4, 8.0).to_string()).unwrap_err();
    assert!(e.contains("97 parts, at most 96"), "{e}");
    // 96 small paths fit; as many over the whole box (flattened, ~120 points each) go
    // over the point limit.
    assert!(Recipe::parse(&recipe(96, 4, 8.0).to_string()).is_ok());
    let e = Recipe::parse(&recipe(96, 4, 80.0).to_string()).unwrap_err();
    assert!(e.contains("path points, at most 4096"), "{e}");
    // Near the worst case that parses (96 parts, just under 4,096 points), every part
    // with a gradient and an outline: never "heavy". Before the point limit, 48 parts of
    // 512 points each (24,576) could reach "heavy".
    let worst = recipe(96, 6, 17.0);
    // Long gradients too, toward the 64 KB limit: the most a recipe can make the parser do.
    let mut worst = worst;
    let stops: Vec<Value> = (0..8)
        .map(|i| json!([f64::from(i) / 7.0, if i % 2 == 0 { "a" } else { "b" }, 0.5]))
        .collect();
    for p in worst["parts"].as_array_mut().unwrap() {
        p["light"] = json!({ "radial": [80, 70, 30, 40, 0.3], "stops": stops });
    }
    let text = worst.to_string();
    let t0 = std::time::Instant::now();
    let points = Recipe::parse(&text).map(|_| ()).err();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("worst-case parse: {ms:.2} ms, {} KB", text.len() / 1024);
    assert_eq!(points, None);
    // Measured 0.76 ms (release) / 3.3 ms (debug) natively, design note 28 (B3); a
    // registered text is parsed once, so this is a one-off per recipe. ~10x headroom.
    let bound = if cfg!(debug_assertions) { 40.0 } else { 8.0 };
    assert!(ms < bound, "parsing the worst recipe took {ms:.1} ms");
    let text = worst.to_string();
    assert!(text.len() < 64 * 1024);
    let spec =
        json!({ "fxSpec": "1.13", "object": "character", "pattern": "fox", "recipe": worst });
    let r = resolve_full(&spec.to_string(), Some("speaking"), &HashMap::new(), false);
    assert!(r.ok, "{:?}", r.diagnostics);
    let cost = crate::estimate_cost(r.state.clone(), 64, r.overrides.clone()).unwrap();
    assert_ne!(cost.class, "heavy");
    assert!(cost.elements <= 200, "{}", cost.elements);
}

/// C2 / design note 29: the snapping helper gives each slot where the drawing puts it.
#[test]
fn character_slots_follow_the_drawing_for_built_ins_and_registered_recipes() {
    let at = |state: &str, o: &[(&str, f64)]| {
        let o: HashMap<String, f64> = o.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        crate::character_slots(state.to_string(), 200, 1.0, o)
    };
    let buzzy = at("buzzy", &[("still", 1.0)]);
    let mut names: Vec<&str> = buzzy.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["chest", "face", "headTop", "neck"]);
    let y = |n: &str, s: &[crate::CharacterSlot]| s.iter().find(|x| x.name == n).unwrap().y;
    assert!(y("headTop", &buzzy) < y("face", &buzzy) && y("face", &buzzy) < y("chest", &buzzy));
    // The face slot turns with the head (wrapped on the face surface).
    let turned = at("buzzy", &[("still", 1.0), ("turn", 1.0), ("turnYaw", 0.9)]);
    let fx = |s: &[crate::CharacterSlot]| s.iter().find(|x| x.name == "face").unwrap().x;
    assert!((fx(&turned) - fx(&buzzy)).abs() > 2.0);
    // A registered recipe's key works the same; anything else has no slots.
    let spec =
        json!({ "fxSpec": "1.13", "object": "character", "pattern": "fox", "recipe": fox(None) });
    let r = resolve_full(&spec.to_string(), None, &HashMap::new(), false);
    let fox_slots = at(&r.state, &[]);
    assert_eq!(fox_slots.len(), 1);
    assert_eq!(fox_slots[0].name, "headTop");
    assert!(at("working", &[]).is_empty());
    // A worn cosmetic's own placement is no slot of the character's.
    let hatted = json!({ "fxSpec": "1.13", "object": "character", "pattern": "buzzy",
        "cosmetics": [hat(json!({}))] });
    let r = resolve_full(&hatted.to_string(), None, &HashMap::new(), false);
    assert_eq!(at(&r.state, &[]).len(), 4);
}
