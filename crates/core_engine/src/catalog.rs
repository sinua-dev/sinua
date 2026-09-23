//! The **parameter catalog**: the single source of truth for every tunable,
//! per object and pattern (docs/parameters.md, *Parameter catalog*).
//! Typed components (`SinuaOrb`, `SinuaRing`, ...), the Studios and the docs'
//! parameter tables are generated from it; `spec/parameters.json` is a
//! checked-in copy (`tests/parameter_catalog.rs` proves it equals this).
//!
//! Two halves:
//! - **curated** (`catalog_source.json`, embedded): labels, one-line
//!   descriptions, categories, value/style groups, valid and UI ranges,
//!   choices, conditional visibility, material paths, runtime inputs;
//! - **derived from the engine** here: each pattern's mode, sizes and
//!   speed, the keys it reads (`fx_spec::mode_params`, the resolved preset
//!   and the shared radius keys), and every per-size default (the resolved
//!   preset, else the definition's `fallback`), plus the per-pattern
//!   particle / liquid defaults.
//!
//! Shaped after Storybook's ArgTypes (control min/max/step, table category
//! and default, conditional `if`), the DTCG Format Module's naming rule
//! (no leading `$`, no `{`, `}` or `.` in a name) and Rive's data-binding
//! view models (typed properties a runtime discovers by name).
//!
//! `check_overrides` validates an overrides map against it: warnings only
//! (unknown key with a did-you-mean, out of range, not a whole number,
//! renamed key); the frame is unaffected.
use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

use crate::fx_spec::{self, FxDiagnostic};

pub const CATALOG_VERSION: u64 = 1;

fn source() -> &'static Value {
    static SRC: OnceLock<Value> = OnceLock::new();
    SRC.get_or_init(|| {
        serde_json::from_str(include_str!("catalog_source.json"))
            .expect("catalog_source.json is valid JSON")
    })
}

/// Indexed engine keys a pattern accepts as one array definition:
/// (mode, array key, engine key prefix, max length).
const ARRAYS: [(&str, &str, &str, usize); 2] = [
    ("nested", "progress", "progress", 4),
    ("segmented", "segment", "segment", 24),
];

fn internal_keys() -> Vec<&'static str> {
    source()["internalKeys"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Is `key` a name this catalog knows at all -- any pattern's design key,
/// a material key, a runtime input, or an internal one? Used where a key
/// arrives as data rather than from a pattern (the voice-state profile), so
/// a typo fails a test instead of silently drawing nothing. Only the
/// voice-state profile's test needs it today.
#[cfg(test)]
pub(crate) fn knows_key(key: &str) -> bool {
    let named = |v: &Value| {
        v.as_object()
            .is_some_and(|o| o.values().any(|d| d["key"].as_str() == Some(key)))
    };
    let listed = |v: &Value| {
        v.as_array()
            .is_some_and(|a| a.iter().any(|d| d["key"].as_str() == Some(key)))
    };
    named(&source()["definitions"])
        || listed(&source()["runtimeInputs"])
        || internal_keys().contains(&key)
}

/// The definition id for `key` on `mode`: `key@mode`, else `key@shared`.
fn def_id(mode: &str, key: &str) -> Option<String> {
    let defs = source()["definitions"].as_object()?;
    [format!("{key}@{mode}"), format!("{key}@shared")]
        .into_iter()
        .find(|id| defs.contains_key(id))
}

fn is_material(def: &Value) -> bool {
    def.get("material").is_some()
}

/// The design keys a pattern reads, in display order (basic before
/// advanced, then category order, then key), as definition ids.
fn pattern_params(mode: &str, preset: &HashMap<String, f64>, orb: bool) -> Vec<String> {
    let internal = internal_keys();
    let mut keys: Vec<String> = fx_spec::mode_params(mode)
        .iter()
        .map(|k| k.to_string())
        .collect();
    keys.extend(preset.keys().cloned());
    if orb {
        keys.extend(fx_spec::SHARED_PARAMS.iter().map(|k| k.to_string()));
    }
    // `ink` (1.8) belongs to every pattern in every family: it fades the
    // finished frame, whatever drew it. Listing it per pattern is what puts
    // an Ink control in the Studio and a row in each catalog table.
    keys.push("ink".to_string());
    for (m, array, _, _) in ARRAYS {
        if m == mode {
            keys.push(array.to_string());
        }
    }
    let defs = &source()["definitions"];
    let cats = ["appearance", "motion", "energy", "material", "input"];
    let mut ids: Vec<String> = keys
        .iter()
        .filter(|k| !internal.contains(&k.as_str()))
        .filter_map(|k| def_id(mode, k))
        .filter(|id| !is_material(&defs[id]))
        .collect();
    ids.sort();
    ids.dedup();
    ids.sort_by_key(|id| {
        let d = &defs[id];
        (
            d["tier"].as_str() != Some("basic"),
            cats.iter()
                .position(|c| Some(*c) == d["category"].as_str())
                .unwrap_or(9),
            d["key"].as_str().unwrap_or("").to_string(),
        )
    });
    ids
}

fn num(v: f64) -> Value {
    // Integers print without a fraction, so the JSON reads like the docs.
    if v.fract() == 0.0 && v.abs() < 1e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

/// The whole catalog as a JSON value (see the module doc for the shape).
pub fn catalog() -> Value {
    let src = source();
    let defs = src["definitions"].as_object().cloned().unwrap_or_default();
    let mut objects = Vec::new();
    for obj in src["objects"].as_array().into_iter().flatten() {
        let orb = obj["id"] == "orb";
        let mut patterns = Vec::new();
        for p in obj["patterns"].as_array().into_iter().flatten() {
            let id = p["id"].as_str().unwrap_or("");
            let resolved: Vec<(u32, crate::ResolvedOpts)> = fx_spec::SIZES
                .iter()
                .filter_map(|&z| crate::resolved_opts(id.to_string(), z).map(|r| (z, r)))
                .collect();
            let Some((_, first)) = resolved.first() else {
                continue;
            };
            let mode = first.mode.clone();
            let mut all_keys: HashMap<String, f64> = HashMap::new();
            for (_, r) in &resolved {
                all_keys.extend(r.opts.clone());
            }
            let params: Vec<Value> = pattern_params(&mode, &all_keys, orb)
                .into_iter()
                .map(|ref_id| {
                    let d = &defs[&ref_id];
                    let key = d["key"].as_str().unwrap_or("");
                    let mut default = Map::new();
                    for (z, r) in &resolved {
                        let v = match ARRAYS.iter().find(|(m, a, _, _)| *m == mode && *a == key) {
                            Some((_, _, prefix, n)) => Value::Array(
                                (0..*n)
                                    .map_while(|i| {
                                        r.opts.get(&format!("{prefix}{i}")).map(|v| num(*v))
                                    })
                                    .collect(),
                            ),
                            None => r
                                .opts
                                .get(key)
                                .map(|v| num(*v))
                                .unwrap_or_else(|| d["fallback"].clone()),
                        };
                        default.insert(z.to_string(), v);
                    }
                    json!({ "ref": ref_id, "default": default })
                })
                .collect();
            let mut material_defaults = Map::new();
            let particles: Map<String, Value> = crate::particles::defaults_for(&mode)
                .into_iter()
                .map(|(k, v)| (k, num(v)))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect();
            material_defaults.insert("particles".into(), Value::Object(particles));
            let liquid: Map<String, Value> = crate::liquid::mode_defaults(&mode)
                .iter()
                .map(|(k, v)| (k.to_string(), num(*v)))
                .collect();
            material_defaults.insert("liquid".into(), Value::Object(liquid));
            patterns.push(json!({
                "id": id,
                "label": p["label"],
                "mode": mode,
                "speed": num(first.speed),
                "sizes": resolved.iter().map(|(z, _)| *z).collect::<Vec<_>>(),
                "params": params,
                "materialDefaults": material_defaults,
            }));
        }
        objects.push(json!({
            "id": obj["id"],
            "label": obj["label"],
            "component": obj["component"],
            "patterns": patterns,
        }));
    }
    let definitions: Map<String, Value> = defs
        .into_iter()
        .map(|(id, mut d)| {
            if let Some(o) = d.as_object_mut() {
                let spec_path = match o.get("material").and_then(Value::as_str) {
                    Some(section) => {
                        let field = o["path"]
                            .as_str()
                            .unwrap_or("")
                            .split('.')
                            .nth(1)
                            .unwrap_or("");
                        let base = match section {
                            "color" => "/color".to_string(),
                            "gradient" => "/gradient".to_string(),
                            s => format!("/materials/{s}"),
                        };
                        format!("{base}/{field}")
                    }
                    None => format!("/params/{}", o["key"].as_str().unwrap_or("")),
                };
                o.insert("specPath".into(), json!(spec_path));
                o.entry("aliases").or_insert_with(|| json!([]));
                o.entry("if").or_insert(Value::Null);
                o.entry("choices").or_insert(Value::Null);
                o.entry("deprecated").or_insert(Value::Null);
            }
            (id, d)
        })
        .collect();
    json!({
        "catalogVersion": CATALOG_VERSION,
        // The voice-state profile's own version: a tool can cache the
        // profile and tell "tuned by the user" from "came with the engine".
        "profileVersion": crate::voice_state::profile_version(),
        "fxSpec": format!("1.{}", fx_spec::RUNTIME_MINOR),
        "categories": src["categories"],
        "objects": objects,
        "definitions": definitions,
        "materials": src["materials"],
        "runtimeInputs": src["runtimeInputs"],
        "internalKeys": src["internalKeys"],
    })
}

/// `catalog()` as pretty JSON text (what `spec/parameters.json` holds).
pub fn catalog_json() -> String {
    let mut s = serde_json::to_string_pretty(&catalog()).expect("catalog serializes");
    s.push('\n');
    s
}

fn warn(path: String, message: String) -> FxDiagnostic {
    FxDiagnostic {
        severity: "warning".into(),
        path,
        message,
    }
}

/// Validates engine overrides for one pattern against the catalog.
/// Warnings only: the engine still renders (and clamps) whatever it's given.
/// Keys are visited in sorted order so the result is deterministic.
pub fn check_overrides(
    state: &str,
    size: u32,
    overrides: &HashMap<String, f64>,
) -> Vec<FxDiagnostic> {
    let mut out = Vec::new();
    let Some(r) = crate::resolved_opts(state.to_string(), size) else {
        out.push(warn(
            "/".into(),
            format!("unknown pattern `{state}` at size {size}"),
        ));
        return out;
    };
    let mode = r.mode.as_str();
    let defs = source()["definitions"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let orb = source()["objects"][0]["patterns"]
        .as_array()
        .is_some_and(|ps| ps.iter().any(|p| p["id"] == state));
    let object = source()["objects"]
        .as_array()
        .and_then(|os| {
            os.iter().find(|o| {
                o["patterns"]
                    .as_array()
                    .is_some_and(|ps| ps.iter().any(|p| p["id"] == state))
            })
        })
        .and_then(|o| o["id"].as_str())
        .unwrap_or("?");
    let own: Vec<String> = pattern_params(mode, &r.opts, orb);
    let materials: Vec<String> = defs
        .iter()
        .filter(|(_, d)| is_material(d))
        .map(|(id, _)| id.clone())
        .collect();
    let mut known: Vec<&str> = own
        .iter()
        .chain(materials.iter())
        .filter_map(|id| defs[id]["key"].as_str())
        .collect();
    let internal = internal_keys();
    known.extend(internal.iter().copied());
    let mut keys: Vec<&String> = overrides.keys().collect();
    keys.sort();
    for key in keys {
        let v = overrides[key];
        let path = format!("/{key}");
        // An array definition's engine keys (`progress0`, `segment12`).
        let array = ARRAYS.iter().find(|(m, _, prefix, n)| {
            *m == mode
                && key
                    .strip_prefix(prefix)
                    .and_then(|i| i.parse::<usize>().ok())
                    .is_some_and(|i| i < *n)
        });
        let id = if let Some((_, a, _, _)) = array {
            def_id(mode, a)
        } else if known.contains(&key.as_str()) {
            own.iter()
                .chain(materials.iter())
                .find(|id| defs[*id]["key"] == key.as_str())
                .cloned()
        } else if fx_spec::runtime_key(key).is_some()
            || crate::reactive::target(key).is_some()
            || fx_spec::indexed_param(mode, key)
        {
            continue; // live inputs and cues: fed by the SDK, not validated here
        } else {
            // Edit distance first; then a shared prefix of 4+ letters, which
            // catches abbreviations (`lineWidth` for web's `lineW`).
            let hint = fx_spec::suggest(key, &known)
                .or_else(|| {
                    known.iter().copied().find(|k| {
                        let n = k.len().min(key.len());
                        n >= 4 && (key.starts_with(k) || k.starts_with(key.as_str()))
                    })
                })
                .map(|k| format!(" (did you mean `{k}`?)"))
                .unwrap_or_default();
            out.push(warn(
                path,
                format!("unknown key `{key}` for {object}/{state}{hint}"),
            ));
            continue;
        };
        let Some(d) = id.and_then(|id| defs.get(&id).cloned()) else {
            continue;
        };
        let (lo, hi) = (d["min"].as_f64(), d["max"].as_f64());
        if let (Some(lo), Some(hi)) = (lo, hi) {
            if !(lo..=hi).contains(&v) {
                out.push(warn(
                    path.clone(),
                    format!(
                        "`{key}` = {v} is outside {lo}..{hi} (the engine clamps or ignores it)"
                    ),
                ));
                continue;
            }
        }
        match d["type"].as_str() {
            Some("integer") | Some("choice") if v.fract() != 0.0 => out.push(warn(
                path,
                format!("`{key}` expects a whole number, got {v}"),
            )),
            Some("boolean") if v != 0.0 && v != 1.0 => out.push(warn(
                path,
                format!("`{key}` is on/off: use 0 or 1, got {v}"),
            )),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    /// The catalog and the engine must list the *same pattern names*, not just
    /// the same number of them.
    ///
    /// Before this, the 34 patterns were spelled out in four places and the only
    /// checks were two independent `assert_eq!(…, 34)`. Two counts agree happily
    /// while the lists hold different names -- rename a pattern in one and both
    /// assertions still pass. This is the
    /// comparison that was missing; it reports the names, because "34 vs 35" is
    /// precisely the message that does not help.
    #[test]
    fn catalog_pattern_names_match_the_presets() {
        // `source()`, not `catalog()`: `catalog()` drops any pattern the engine
        // does not resolve (the `continue` above), so it can never hold a name
        // the presets lack — comparing against it would only ever test one
        // direction. The curated JSON is the copy this test exists to compare.
        let cat = source();
        let mut in_catalog: Vec<String> = cat["objects"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|o| o["patterns"].as_array().unwrap())
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect();
        let mut in_engine: Vec<String> = crate::all_states()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        in_catalog.sort();
        in_engine.sort();

        let missing_from_catalog: Vec<&String> = in_engine
            .iter()
            .filter(|s| !in_catalog.contains(s))
            .collect();
        let missing_from_presets: Vec<&String> = in_catalog
            .iter()
            .filter(|s| !in_engine.contains(s))
            .collect();
        assert!(
            missing_from_catalog.is_empty() && missing_from_presets.is_empty(),
            "the pattern lists disagree.\n  in presets but not the catalog: {missing_from_catalog:?}\n  in the catalog but not presets: {missing_from_presets:?}"
        );
        assert_eq!(in_catalog, in_engine);
    }

    #[test]
    fn every_engine_key_has_a_definition_and_every_definition_is_used() {
        let cat = catalog();
        let defs = cat["definitions"].as_object().unwrap();
        let mut used: Vec<String> = Vec::new();
        let mut patterns = 0;
        for obj in cat["objects"].as_array().unwrap() {
            for p in obj["patterns"].as_array().unwrap() {
                patterns += 1;
                let mode = p["mode"].as_str().unwrap();
                for k in fx_spec::mode_params(mode) {
                    assert!(
                        def_id(mode, k).is_some() || internal_keys().contains(k),
                        "{mode}: `{k}` has no definition"
                    );
                }
                for prm in p["params"].as_array().unwrap() {
                    let id = prm["ref"].as_str().unwrap();
                    assert!(defs.contains_key(id), "{id}");
                    used.push(id.to_string());
                }
            }
        }
        assert_eq!(
            patterns,
            crate::all_states().len(),
            "every pattern is in exactly one object"
        );
        for m in cat["materials"].as_array().unwrap() {
            for id in m["params"].as_array().unwrap() {
                used.push(id.as_str().unwrap().to_string());
            }
        }
        for id in defs.keys() {
            assert!(
                used.contains(id),
                "`{id}` is defined but no pattern or material uses it"
            );
        }
    }

    #[test]
    fn material_definitions_match_the_fx_spec_key_tables() {
        let defs = source()["definitions"].as_object().unwrap();
        // Every section the resolver accepts, not a copy of the list: a new
        // material is checked here without anyone remembering to add it.
        for section in fx_spec::MATERIAL_SECTIONS {
            for (field, key, lo, hi) in fx_spec::material_keys(section) {
                let d = &defs[&format!("{key}@shared")];
                assert_eq!(d["material"], *section, "{key}");
                assert_eq!(d["path"], format!("{section}.{field}"), "{key}");
                assert_eq!(d["min"].as_f64(), Some(*lo), "{key} min");
                assert_eq!(d["max"].as_f64(), Some(*hi), "{key} max");
            }
        }
    }

    #[test]
    fn color_and_gradient_ranges_match_the_fx_spec_schema() {
        let schema: Value =
            serde_json::from_str(include_str!("../../../spec/fx-spec-1.schema.json")).unwrap();
        let defs = source()["definitions"].as_object().unwrap();
        for (section, schema_def) in [("color", "colorSection"), ("gradient", "gradient")] {
            let props = schema["$defs"][schema_def]["properties"]
                .as_object()
                .unwrap();
            for d in defs.values().filter(|d| d["material"] == section) {
                let field = d["path"].as_str().unwrap().split('.').nth(1).unwrap();
                let Some(p) = props.get(field) else { continue };
                if let (Some(lo), Some(hi)) = (p["minimum"].as_f64(), p["maximum"].as_f64()) {
                    assert_eq!(
                        (d["min"].as_f64(), d["max"].as_f64()),
                        (Some(lo), Some(hi)),
                        "{section}.{field}"
                    );
                }
            }
        }
    }

    #[test]
    fn paths_never_collide_with_a_material_group() {
        // A flat prop named like a material section (`particles`) would clash
        // with that section's props in typed components (voice-adapters' codegen).
        let defs = source()["definitions"].as_object().unwrap();
        let sections: Vec<&str> = source()["materials"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        for (id, d) in defs {
            let path = d["path"].as_str().unwrap();
            if d.get("material").is_none() {
                assert!(
                    !sections.contains(&path),
                    "{id}: path `{path}` collides with a material group"
                );
            }
        }
        assert_eq!(defs["particles@orbits"]["path"], "orbitParticles");
    }

    #[test]
    fn names_ranges_and_choices_are_consistent() {
        let defs = source()["definitions"].as_object().unwrap();
        for (id, d) in defs {
            let key = d["key"].as_str().unwrap();
            assert!(
                !key.starts_with('$') && !key.contains(['{', '}', '.']),
                "{id}: DTCG naming rule"
            );
            assert!(
                d["description"].as_str().is_some_and(|s| s.len() > 8),
                "{id}: description"
            );
            assert!(
                ["appearance", "motion", "energy", "material", "input"]
                    .contains(&d["category"].as_str().unwrap()),
                "{id}: category"
            );
            assert!(
                ["value", "style"].contains(&d["group"].as_str().unwrap()),
                "{id}: group"
            );
            let (lo, hi) = (d["min"].as_f64().unwrap(), d["max"].as_f64().unwrap());
            assert!(lo <= hi, "{id}: min <= max");
            if let (Some(a), Some(b)) = (d["uiMin"].as_f64(), d["uiMax"].as_f64()) {
                assert!(
                    lo <= a && a <= b && b <= hi,
                    "{id}: {lo} <= uiMin {a} <= uiMax {b} <= {hi}"
                );
            }
            if let Some(fb) = d["fallback"].as_f64() {
                assert!(
                    lo <= fb && fb <= hi,
                    "{id}: fallback {fb} outside {lo}..{hi}"
                );
            }
            if let Some(cs) = d["choices"].as_array() {
                let vals: Vec<f64> = cs.iter().map(|c| c["value"].as_f64().unwrap()).collect();
                assert_eq!(
                    vals.first().copied(),
                    Some(lo),
                    "{id}: choices start at min"
                );
                assert_eq!(vals.last().copied(), Some(hi), "{id}: choices end at max");
            }
        }
    }

    #[test]
    fn defaults_come_from_the_engine() {
        let cat = catalog();
        let breathing = cat["objects"][0]["patterns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "breathing")
            .unwrap()
            .clone();
        let lanes = breathing["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["ref"] == "lanes@ring")
            .unwrap();
        for z in [20u32, 32, 64] {
            let r = crate::resolved_opts("breathing".into(), z).unwrap();
            assert_eq!(
                lanes["default"][z.to_string()].as_f64(),
                r.opts.get("lanes").copied(),
                "size {z}"
            );
        }
        // Array definition: tracking's per-ring values.
        let tracking = &cat["objects"][2]["patterns"][2];
        assert_eq!(tracking["id"], "tracking");
        assert!(tracking["params"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["ref"] == "progress@nested" && p["default"]["64"].is_array()));
        // Array definitions carry hard length bounds for codegen (Rev 2).
        for (id, max, from) in [
            ("progress@nested", 4, "ringCount"),
            ("segment@segmented", 24, "segmentCount"),
        ] {
            let d = &cat["definitions"][id];
            assert_eq!(d["minItems"], 1, "{id}");
            assert_eq!(d["maxItems"], max, "{id}");
            assert_eq!(d["lengthFrom"], from, "{id}");
        }
    }

    #[test]
    fn check_overrides_warns_and_never_blocks() {
        let w = check_overrides("breathing", 64, &o(&[("lanse", 6.0)]));
        assert_eq!(w.len(), 1);
        assert!(
            w[0].message
                .contains("unknown key `lanse` for orb/breathing (did you mean `lanes`?)"),
            "{}",
            w[0].message
        );
        assert_eq!(w[0].path, "/lanse");
        let w = check_overrides("breathing", 64, &o(&[("lanes", 40.0)]));
        assert!(w[0].message.contains("outside 1..16"), "{}", w[0].message);
        let w = check_overrides("breathing", 64, &o(&[("segs", 88.5)]));
        assert!(w[0].message.contains("whole number"), "{}", w[0].message);
        // The taxonomy retrofit's old names are unknown keys like any other
        // (no alias table since the FX Spec 1.8 floor).
        let w = check_overrides("glowing", 64, &o(&[("nodeN", 90.0)]));
        assert!(
            w[0].message.contains("unknown key `nodeN`"),
            "{}",
            w[0].message
        );
        // The Studio's orb "Line width" knob writes `lineWidth`; web reads `lineW`.
        let w = check_overrides("connecting", 64, &o(&[("lineWidth", 1.0)]));
        assert!(
            w[0].message.contains("did you mean `lineW`"),
            "{}",
            w[0].message
        );
        // Clean inputs: known keys, materials, live inputs, array entries.
        assert!(check_overrides(
            "tracking",
            64,
            &o(&[
                ("progress2", 1.5),
                ("glowStrength", 0.5),
                ("audioLevel", 0.3),
                ("hueStep", 10.0)
            ])
        )
        .is_empty());
        assert!(
            check_overrides("tracking", 64, &o(&[("progress2", 4.0)]))[0]
                .message
                .contains("outside 0..3")
        );
        // Warnings don't change the frame.
        let with = crate::frame_with_overrides("breathing".into(), 64, 1.0, o(&[("lanse", 6.0)]));
        let without = crate::frame_with_overrides("breathing".into(), 64, 1.0, o(&[]));
        assert_eq!(with, without);
    }
}
