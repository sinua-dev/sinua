//! The character recipe's JSON Schema and reference tables (design note 20),
//! generated from the engine's own tables -- the part kinds and their fields
//! (`parts::schema`), the recipe keys and the limits -- with one sentence per
//! key from `spec/character-recipe-descriptions.json`. Test-only: nothing here
//! reaches a build.
//!
//! - `RECIPE_SCHEMA_WRITE=1 cargo test -p core_engine --lib -- --ignored write_the_recipe_schema`
//!   writes `spec/character-recipe-1.schema.json` and the generated part of
//!   `docs/character-recipe.md`.
//! - The other tests fail when either is stale, when a part or field has no
//!   sentence, or when a built-in recipe or a recipe example doesn't validate.

use serde_json::{json, Map, Value};

use crate::character::parts::{self, Kind, Ty, KINDS};
use crate::character::recipe::{MAX_NUMBER, MAX_PARTS, RECIPES};

pub const SCHEMA_ID: &str = "https://sinua.dev/schema/character-recipe-1.json";
const SCHEMA_FILE: &str = "../../spec/character-recipe-1.schema.json";
const DOC_FILE: &str = "../../docs/character-recipe.md";
const BEGIN: &str = "<!-- generated:parts (crates/core_engine/src/character/recipe_schema.rs) -->";
const END: &str = "<!-- /generated:parts -->";

fn descriptions() -> Value {
    serde_json::from_str(include_str!(
        "../../../../spec/character-recipe-descriptions.json"
    ))
    .expect("descriptions JSON")
}

/// A field's sentence: its part's, else the common one.
fn describe(d: &Value, part: &str, field: &str) -> Option<String> {
    d["parts"][part][field]
        .as_str()
        .or_else(|| d["common"][field].as_str())
        .map(str::to_string)
}

fn num() -> Value {
    json!({ "$ref": "#/$defs/number" })
}
fn nums(k: usize) -> Value {
    json!({ "type": "array", "items": num(), "minItems": k, "maxItems": k })
}
fn colour() -> Value {
    json!({ "$ref": "#/$defs/colour" })
}
fn list(items: Value) -> Value {
    json!({ "type": "array", "items": items, "maxItems": parts::MAX_LIST })
}

/// A field type as JSON Schema (`Ty`'s doc comments in `parts/mod.rs`).
fn ty(t: Ty) -> Value {
    match t {
        Ty::N => num(),
        Ty::V(k) => nums(k),
        Ty::L => list(num()),
        Ty::Pairs => list(nums(2)),
        Ty::Pts(k) => json!({ "type": "array", "items": nums(2), "minItems": k, "maxItems": k }),
        Ty::C => colour(),
        Ty::Cs(k) => json!({ "type": "array", "items": colour(), "minItems": k, "maxItems": k }),
        Ty::S | Ty::Surf => json!({ "type": "string" }),
        Ty::B => json!({ "type": "boolean" }),
        Ty::NumOrPair => json!({ "oneOf": [num(), nums(2)] }),
        Ty::Shape => json!({ "$ref": "#/$defs/shape" }),
        Ty::Stops => json!({ "$ref": "#/$defs/stops" }),
        Ty::Light => json!({ "$ref": "#/$defs/light" }),
        Ty::Inner => json!({
            "type": "array",
            "maxItems": parts::MAX_LIST,
            "items": { "oneOf": layer_refs() }
        }),
    }
}

/// How a field type reads in the reference table.
fn ty_text(t: Ty) -> String {
    match t {
        Ty::N => "number".into(),
        Ty::V(k) => format!("[{k} numbers]"),
        Ty::L => "[numbers]".into(),
        Ty::Pairs => "[[a, b], …]".into(),
        Ty::Pts(k) => format!("[{k} points]"),
        Ty::C => "colour".into(),
        Ty::Cs(k) => format!("[{k} colours]"),
        Ty::S => "name".into(),
        Ty::Surf => "surface".into(),
        Ty::B => "true / false".into(),
        Ty::NumOrPair => "number or [2 numbers]".into(),
        Ty::Shape => "shape".into(),
        Ty::Stops => "[[offset, colour, alpha?], …]".into(),
        Ty::Light => "light".into(),
        Ty::Inner => "[layers]".into(),
    }
}

/// Absent is fine: false, flat, no layers.
fn optional(t: Ty) -> bool {
    matches!(t, Ty::Surf | Ty::B | Ty::Inner)
}

fn layer_refs() -> Vec<Value> {
    KINDS
        .iter()
        .filter(|(_, k)| parts::is_layer(*k) || *k == Kind::Eyes)
        .map(|(n, _)| json!({ "$ref": format!("#/$defs/layer-{n}") }))
        .collect()
}

/// One part kind's object schema; `layer`: as a body's inner layer (no `space` needed).
fn part(d: &Value, name: &str, k: Kind, layer: bool) -> Value {
    let mut props = Map::new();
    let mut required = vec![json!("part")];
    props.insert(
        "part".into(),
        json!({ "const": name, "description": d["common"]["part"] }),
    );
    props.insert(
        "space".into(),
        json!({ "enum": ["body", "face", "mount", "ground", "whole"], "description": d["common"]["space"] }),
    );
    if !layer {
        required.push(json!("space"));
    }
    props.insert(
        "when".into(),
        json!({ "enum": ["notSmallOrAccessories"], "description": d["common"]["when"] }),
    );
    let state = json!({ "type": "number", "minimum": 0, "maximum": 1 });
    props.insert(
        "show".into(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": { "idle": state, "listening": state, "thinking": state, "speaking": state },
            "description": d["common"]["show"]
        }),
    );
    if k == Kind::Body {
        props.insert(
            "turnLight".into(),
            json!({ "allOf": [nums(2)], "description": "How the radial light moves as the body turns: `[dx, k]`." }),
        );
    }
    // Dotted fields (`outline.width`) nest: `outline: { width }`.
    let mut nested: Vec<(String, Map<String, Value>, Vec<Value>)> = Vec::new();
    for (f, t) in parts::schema(k) {
        let mut s = ty(*t);
        if let (Some(text), Some(o)) = (describe(d, name, f), s.as_object_mut()) {
            o.insert("description".into(), json!(text));
        }
        match f.split_once('.') {
            None => {
                props.insert(f.to_string(), s);
                if !optional(*t) {
                    required.push(json!(f));
                }
            }
            Some((outer, inner)) => {
                let i = match nested.iter().position(|(n, _, _)| n == outer) {
                    Some(i) => i,
                    None => {
                        nested.push((outer.to_string(), Map::new(), Vec::new()));
                        nested.len() - 1
                    }
                };
                nested[i].1.insert(inner.to_string(), s);
                if !optional(*t) {
                    nested[i].2.push(json!(inner));
                }
            }
        }
    }
    for (outer, p, req) in nested {
        // `light` is both a field (the light) and the home of `light.stops`.
        if let Some(existing) = props.get_mut(&outer) {
            let base = existing.take();
            let lit =
                json!({ "allOf": [base, { "type": "object", "properties": p, "required": req }] });
            // A body's `"light": "none"` (an overlay) has no stops.
            *existing = if outer == "light" {
                json!({ "oneOf": [lit, { "const": "none" }] })
            } else {
                lit
            };
        } else {
            props.insert(
                outer.clone(),
                json!({ "type": "object", "additionalProperties": false, "properties": p, "required": req }),
            );
            required.push(json!(outer));
        }
    }
    json!({
        "type": "object",
        "description": d["parts"][name][""],
        "additionalProperties": false,
        "required": required,
        "properties": props
    })
}

/// The whole schema.
pub fn schema() -> Value {
    let d = descriptions();
    let top = |k: &str| d["top"][k].clone();
    let mut defs = Map::new();
    defs.insert(
        "number".into(),
        json!({ "type": "number", "minimum": -MAX_NUMBER, "maximum": MAX_NUMBER }),
    );
    defs.insert(
        "colour".into(),
        json!({ "type": "string", "description": "A colour: a name in this recipe's `palette`." }),
    );
    defs.insert(
        "shape".into(),
        json!({
            "description": d["common"]["shape"],
            "oneOf": [
                { "type": "object", "additionalProperties": false, "required": ["ellipse"], "properties": { "ellipse": nums(6) } },
                { "type": "object", "additionalProperties": false, "required": ["roundRect"], "properties": { "roundRect": nums(6) } },
                { "type": "object", "additionalProperties": false, "required": ["path"], "properties": {
                    "path": { "type": "string", "maxLength": crate::character::path::MAX_BYTES,
                        "description": "An SVG path: M L H V C S Q T Z (no arcs; `fitPath` in @sinua/snippets converts them), in the 200-unit box. Later subpaths are holes." } } }
            ]
        }),
    );
    defs.insert(
        "stops".into(),
        list(json!({ "type": "array", "prefixItems": [num(), colour(), { "type": "number", "minimum": 0, "maximum": 1 }], "minItems": 2, "maxItems": 3 })),
    );
    defs.insert(
        "light".into(),
        json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "radial": { "oneOf": [nums(3), nums(5)] },
                        "linear": nums(4),
                        "stops": { "$ref": "#/$defs/stops" }
                    },
                    "oneOf": [{ "required": ["radial"] }, { "required": ["linear"] }]
                },
                { "const": "none" }
            ]
        }),
    );
    let mut part_refs = Vec::new();
    for (name, k) in KINDS.iter() {
        if !parts::is_layer(*k) {
            defs.insert(format!("part-{name}"), part(&d, name, *k, false));
            part_refs.push(json!({ "$ref": format!("#/$defs/part-{name}") }));
        }
        if parts::is_layer(*k) || *k == Kind::Eyes {
            defs.insert(format!("layer-{name}"), part(&d, name, *k, true));
        }
    }
    // A cosmetic's parts: `body` and `eyes`, drawn in its slot (no `space`, no `surface`).
    let c = |k: &str| d["cosmetic"][k].clone();
    let mut cosmetic_parts = Vec::new();
    for (name, k) in [("body", Kind::Body), ("eyes", Kind::Eyes)] {
        let mut p = part(&d, name, k, true);
        if let Some(props) = p["properties"].as_object_mut() {
            props.remove("space");
            props.remove("surface");
        }
        defs.insert(format!("cosmetic-{name}"), p);
        cosmetic_parts.push(json!({ "$ref": format!("#/$defs/cosmetic-{name}") }));
    }
    defs.insert(
        "cosmetic".into(),
        json!({
            "type": "object", "description": c(""), "additionalProperties": false,
            "required": ["id", "slot", "parts"],
            "properties": {
                "id": { "type": "string", "pattern": "^[a-z0-9-]{1,32}$", "description": c("id") },
                "label": { "type": "string", "description": c("label") },
                "slot": { "type": "string", "description": c("slot") },
                "palette": { "type": "object", "description": c("palette"), "additionalProperties": nums(3) },
                "parts": { "type": "array", "description": c("parts"), "minItems": 1, "maxItems": MAX_PARTS,
                    "items": { "oneOf": cosmetic_parts } },
                "fits": { "type": "array", "description": c("fits"), "items": { "type": "string" } },
                "fit": { "type": "object", "description": c("fit"),
                    "additionalProperties": { "type": "object", "additionalProperties": false,
                        "properties": { "at": nums(2), "scale": num(), "angle": num() } } }
            }
        }),
    );
    let p2 = nums(2);
    let rig_common = |kind: &str| json!({ "const": kind });
    let builtins: Vec<&str> = RECIPES.iter().map(|(id, _)| *id).collect();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": SCHEMA_ID,
        "title": "Sinua character recipe, version 1",
        "description": "A character as data (FX Spec 1.12 `recipe`; spec/characters/*.json), and the cosmetics it wears (1.13 `cosmetics`). Generated from the engine's tables: do not edit; see docs/character-recipe.md.",
        "type": "object",
        "additionalProperties": false,
        "required": ["recipe", "id", "palette", "hue", "rig", "parts", "burst"],
        "properties": {
            "$schema": { "type": "string", "description": top("$schema") },
            "$comment": { "type": "string", "description": top("$comment") },
            "recipe": { "const": 1, "description": top("recipe") },
            "id": { "type": "string", "pattern": "^[a-z0-9-]{1,32}$", "description": top("id") },
            "profile": { "enum": builtins, "description": top("profile") },
            "palette": {
                "type": "object",
                "description": top("palette"),
                "additionalProperties": nums(3)
            },
            "hue": {
                "type": "object", "description": top("hue"), "additionalProperties": false,
                "required": ["base", "turns"],
                "properties": { "base": num(), "turns": { "type": "array", "items": colour() } }
            },
            "contrast": {
                "type": "array", "description": top("contrast"),
                "items": { "type": "array", "items": colour(), "minItems": 2, "maxItems": 2 }
            },
            "rig": {
                "description": top("rig"),
                "oneOf": [
                    { "type": "object", "additionalProperties": false, "required": ["kind", "pivot", "lean"], "properties": {
                        "kind": rig_common("pivot"), "pivot": p2, "lean": num(), "squash": { "type": "boolean" },
                        "sway": { "type": "object", "additionalProperties": false, "required": ["gain", "rate", "base"],
                            "properties": { "gain": { "type": "string" }, "rate": num(), "base": num() } },
                        "mount": { "type": "object", "additionalProperties": false, "required": ["bob"], "properties": { "bob": num() } } } },
                    { "type": "object", "additionalProperties": false, "required": ["kind", "base"], "properties": {
                        "kind": rig_common("upright"), "base": p2 } },
                    { "type": "object", "additionalProperties": false, "required": ["kind", "center", "lean", "float"], "properties": {
                        "kind": rig_common("float"), "center": p2, "lean": num(),
                        "float": { "type": "object", "additionalProperties": false, "required": ["rate", "amp"],
                            "properties": { "rate": num(), "amp": num() } } } }
                ]
            },
            "surfaces": {
                "type": "object", "description": top("surfaces"),
                "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["c", "r", "depth"],
                    "properties": { "c": p2, "r": num(), "depth": num(), "cylinder": { "type": "boolean" } } }
            },
            "parts": {
                "type": "array", "description": top("parts"), "maxItems": MAX_PARTS,
                "items": { "oneOf": part_refs }
            },
            "burst": {
                "type": "object", "description": top("burst"), "additionalProperties": false,
                "required": ["at", "r", "colors"],
                "properties": { "at": p2, "r": num(),
                    "colors": { "type": "array", "items": colour(), "minItems": 3, "maxItems": 3 },
                    "space": { "enum": ["body", "face", "mount", "ground", "whole"] } }
            },
            "slots": {
                "type": "object", "description": top("slots"),
                "additionalProperties": { "type": "object", "additionalProperties": false, "required": ["at", "follows"],
                    "properties": { "at": p2, "scale": num(), "angle": num(),
                        "follows": { "enum": ["head", "body", "face"] } } }
            },
            "cosmetics": {
                "type": "array", "description": top("cosmetics"),
                "items": { "$ref": "#/$defs/cosmetic" }
            },
            "roles": {
                "type": "object", "description": top("roles"), "additionalProperties": false,
                "properties": { "primary": colour(), "secondary": colour(), "accent": colour() }
            },
            "grain": {
                "type": "object", "description": top("grain"), "additionalProperties": false,
                "required": ["strength"],
                "properties": { "strength": { "type": "number", "minimum": 0, "maximum": 1 } }
            }
        },
        "$defs": defs
    })
}

/// The reference's generated part: one table per part kind.
pub fn tables() -> String {
    let d = descriptions();
    let mut out = String::new();
    for (name, k) in KINDS.iter() {
        let layer = if parts::is_layer(*k) {
            " *(body layer: inside a body's `inner`)*"
        } else if *k == Kind::Eyes {
            " *(a part or a body layer)*"
        } else {
            ""
        };
        out += &format!(
            "### `{name}`{layer}\n\n{}\n\n| Field | Type | | Meaning |\n|---|---|---|---|\n",
            d["parts"][*name][""].as_str().unwrap_or("")
        );
        for (f, t) in parts::schema(*k) {
            let need = if optional(*t) { "optional" } else { "required" };
            let text = describe(&d, name, f)
                .unwrap_or_default()
                .replace('|', "\\|");
            out += &format!("| `{f}` | {} | {need} | {text} |\n", ty_text(*t));
        }
        out += "\n";
    }
    out
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap() + "\n"
}

fn spliced(doc: &str, generated: &str) -> String {
    let (a, rest) = doc.split_once(BEGIN).expect("begin marker");
    let (_, b) = rest.split_once(END).expect("end marker");
    format!("{a}{BEGIN}\n\n{generated}{END}{b}")
}

// ---- a small validator for the subset this schema uses (tests only) ----

fn resolve<'a>(root: &'a Value, s: &'a Value) -> &'a Value {
    match s.get("$ref").and_then(Value::as_str) {
        Some(r) => resolve(root, &root["$defs"][r.trim_start_matches("#/$defs/")]),
        None => s,
    }
}

/// The first problem with `v` against `s`, as a JSON Pointer and a reason.
pub fn validate(root: &Value, s: &Value, v: &Value, at: &str) -> Option<String> {
    let s = resolve(root, s);
    let fits = match s.get("type").and_then(Value::as_str) {
        Some("number") => v.is_number(),
        Some("string") => v.is_string(),
        Some("boolean") => v.is_boolean(),
        Some("array") => v.is_array(),
        Some("object") => v.is_object(),
        _ => true,
    };
    if !fits {
        return Some(format!("{at}: expected a {}", s["type"].as_str().unwrap()));
    }
    if let Some(c) = s.get("const") {
        if c != v {
            return Some(format!("{at}: expected {c}"));
        }
    }
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        if !e.contains(v) {
            return Some(format!("{at}: not one of {}", Value::Array(e.clone())));
        }
    }
    for x in s
        .get("allOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(e) = validate(root, x, v, at) {
            return Some(e);
        }
    }
    if let Some(one) = s.get("oneOf").and_then(Value::as_array) {
        let errs: Vec<String> = one
            .iter()
            .filter_map(|x| validate(root, x, v, at))
            .collect();
        if one.len() - errs.len() != 1 {
            // The branch the value meant (its `part` / `kind` matches) says what is wrong.
            let tag = |b: &Value| {
                let b = resolve(root, b);
                ["part", "kind"]
                    .iter()
                    .find_map(|k| b["properties"][k].get("const").map(|c| (k, c.clone())))
            };
            let meant = one
                .iter()
                .find(|b| tag(b).is_some_and(|(k, c)| v.get(*k) == Some(&c)));
            return Some(match meant {
                Some(b) => validate(root, b, v, at)
                    .unwrap_or_else(|| format!("{at}: matches more than one form")),
                None => format!("{at}: matches no form"),
            });
        }
    }
    if let Some(x) = v.as_f64() {
        if s.get("minimum")
            .and_then(Value::as_f64)
            .is_some_and(|m| x < m)
            || s.get("maximum")
                .and_then(Value::as_f64)
                .is_some_and(|m| x > m)
        {
            return Some(format!("{at}: {x} out of range"));
        }
    }
    if let Some(t) = v.as_str() {
        if s.get("pattern").is_some()
            && !((1..=32).contains(&t.len())
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        {
            return Some(format!("{at}: `{t}` is not 1–32 of a–z, 0–9 and -"));
        }
        if s.get("maxLength")
            .and_then(Value::as_u64)
            .is_some_and(|m| t.len() as u64 > m)
        {
            return Some(format!("{at}: too long"));
        }
    }
    if let Some(a) = v.as_array() {
        if s.get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|m| (a.len() as u64) < m)
            || s.get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|m| a.len() as u64 > m)
        {
            return Some(format!("{at}: {} items", a.len()));
        }
        let prefix = s.get("prefixItems").and_then(Value::as_array);
        for (i, x) in a.iter().enumerate() {
            let item = prefix.and_then(|p| p.get(i)).or_else(|| s.get("items"));
            if let Some(e) = item.and_then(|it| validate(root, it, x, &format!("{at}/{i}"))) {
                return Some(e);
            }
        }
    }
    if let Some(o) = v.as_object() {
        for r in s
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let r = r.as_str().unwrap();
            if !o.contains_key(r) {
                return Some(format!("{at}/{r}: missing"));
            }
        }
        let props = s.get("properties").and_then(Value::as_object);
        let extra = s.get("additionalProperties");
        for (k, x) in o {
            match props.and_then(|p| p.get(k)) {
                Some(sub) => {
                    if let Some(e) = validate(root, sub, x, &format!("{at}/{k}")) {
                        return Some(e);
                    }
                }
                None => match extra {
                    Some(Value::Bool(false)) => return Some(format!("{at}/{k}: not allowed")),
                    Some(sub) if sub.is_object() => {
                        if let Some(e) = validate(root, sub, x, &format!("{at}/{k}")) {
                            return Some(e);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn examples() -> Vec<(String, Value)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/examples");
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            if let Some(r) = doc.get("recipe") {
                out.push((
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    r.clone(),
                ));
            }
        }
        out
    }

    #[test]
    #[ignore]
    fn write_the_recipe_schema() {
        if std::env::var("RECIPE_SCHEMA_WRITE").is_err() {
            return;
        }
        let base = env!("CARGO_MANIFEST_DIR");
        std::fs::write(format!("{base}/{SCHEMA_FILE}"), pretty(&schema())).unwrap();
        let doc = std::fs::read_to_string(format!("{base}/{DOC_FILE}")).unwrap();
        std::fs::write(format!("{base}/{DOC_FILE}"), spliced(&doc, &tables())).unwrap();
    }

    #[test]
    fn the_schema_and_the_reference_are_current() {
        let base = env!("CARGO_MANIFEST_DIR");
        let on_disk = std::fs::read_to_string(format!("{base}/{SCHEMA_FILE}")).unwrap();
        assert!(
            on_disk == pretty(&schema()),
            "spec/character-recipe-1.schema.json is stale: RECIPE_SCHEMA_WRITE=1 cargo test -p core_engine --lib -- --ignored write_the_recipe_schema"
        );
        let doc = std::fs::read_to_string(format!("{base}/{DOC_FILE}")).unwrap();
        assert!(
            doc == spliced(&doc, &tables()),
            "docs/character-recipe.md's part tables are stale (same command)"
        );
    }

    #[test]
    fn every_part_and_field_has_a_sentence() {
        let d = descriptions();
        let mut missing = Vec::new();
        for (name, k) in KINDS.iter() {
            if d["parts"][*name][""].as_str().is_none() {
                missing.push(format!("{name} (the part)"));
            }
            for (f, _) in parts::schema(*k) {
                if describe(&d, name, f).is_none() {
                    missing.push(format!("{name}.{f}"));
                }
            }
        }
        for k in [
            "recipe",
            "id",
            "profile",
            "palette",
            "hue",
            "contrast",
            "rig",
            "surfaces",
            "parts",
            "burst",
            "slots",
            "cosmetics",
        ] {
            if d["top"][k].as_str().is_none() {
                missing.push(k.to_string());
            }
        }
        for k in std::iter::once("").chain(crate::character::cosmetic::KEYS) {
            if d["cosmetic"][k].as_str().is_none() {
                missing.push(format!("cosmetic.{k}"));
            }
        }
        assert!(
            missing.is_empty(),
            "no sentence in spec/character-recipe-descriptions.json: {missing:?}"
        );
        // And none for a part or field that doesn't exist (a rename left behind).
        for (name, fields) in d["parts"].as_object().unwrap() {
            let k = KINDS.iter().find(|(n, _)| n == name).map(|(_, k)| *k);
            let k = k.unwrap_or_else(|| panic!("`{name}` is not a part"));
            for f in fields.as_object().unwrap().keys().filter(|f| !f.is_empty()) {
                assert!(
                    parts::schema(k).iter().any(|(g, _)| g == f),
                    "`{name}.{f}` is not a field"
                );
            }
        }
    }

    #[test]
    fn every_built_in_and_example_recipe_validates_and_broken_ones_dont() {
        let s = schema();
        let mut all: Vec<(String, Value)> = RECIPES
            .iter()
            .map(|(id, j)| (id.to_string(), serde_json::from_str(j).unwrap()))
            .collect();
        all.extend(examples());
        assert!(
            all.len() >= 9,
            "7 built-ins and the recipe examples (pip, latte): {}",
            all.len()
        );
        for (name, r) in &all {
            assert_eq!(validate(&s, &s, r, ""), None, "{name}");
        }
        let cuppa: Value =
            serde_json::from_str(RECIPES.iter().find(|(id, _)| *id == "cuppa").unwrap().1).unwrap();
        let broken = |f: &dyn Fn(&mut Value)| {
            let mut r = cuppa.clone();
            f(&mut r);
            validate(&s, &s, &r, "")
        };
        assert!(
            broken(&|r| r["parts"][2]["rise"] = json!("high")).is_some(),
            "a word for a number"
        );
        assert!(
            broken(&|r| r["parts"][2]["glow"] = json!(1)).is_some(),
            "a field steam doesn't have"
        );
        assert!(
            broken(&|r| {
                r["parts"][2].as_object_mut().unwrap().remove("count");
            })
            .is_some(),
            "a missing field"
        );
        assert!(
            broken(&|r| r["parts"][2]["part"] = json!("smoke")).is_some(),
            "an unknown part"
        );
        assert!(
            broken(&|r| r["id"] = json!("My Cuppa")).is_some(),
            "a bad id"
        );
        assert!(
            broken(&|r| r["parts"][2]["at"] = json!([0, 2000])).is_some(),
            "out of ±1000"
        );
        assert!(
            broken(&|r| r["parts"][3]["shape"] = json!({ "circle": [1] })).is_some(),
            "an unknown shape"
        );
        // The engine refuses each too: the schema is never the looser of the two.
        for f in [
            &(|r: &mut Value| r["parts"][2]["rise"] = json!("high")) as &dyn Fn(&mut Value),
            &|r: &mut Value| r["parts"][2]["part"] = json!("smoke"),
            &|r: &mut Value| r["id"] = json!("My Cuppa"),
        ] {
            let mut r = cuppa.clone();
            f(&mut r);
            assert!(crate::character::recipe::Recipe::parse(&r.to_string()).is_err());
        }
    }
}
