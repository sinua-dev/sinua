//! The JSON the engine embeds, made lean at build time (design note 10, 1.12):
//! the parameter catalog loses its prose (`description`, `$comment`, `note`),
//! which only tools read (the components' doc comments and sinua.dev are
//! generated from `spec/parameters.json`, written by a test from the full
//! source), and the voice-state profile is minified too. The sources stay
//! readable and the one place to edit; the runtime parses the same values either
//! way. Character recipes (`spec/characters/*.json`) are minified the same way.
//! (The conversation samples stay as written: `conversation::sample` hands
//! their text to people to read and edit, and they are 2 KB.)

use std::path::Path;

use serde_json::Value;

fn strip(v: &mut Value, keys: &[&str]) {
    match v {
        Value::Object(o) => {
            for k in keys {
                o.remove(*k);
            }
            for x in o.values_mut() {
                strip(x, keys);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| strip(x, keys)),
        _ => {}
    }
}

fn lean(src: &str, out: &str, drop: &[&str]) {
    println!("cargo:rerun-if-changed={src}");
    let text = std::fs::read_to_string(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    let mut v: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{src}: {e}"));
    strip(&mut v, drop);
    let dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(
        Path::new(&dir).join(out),
        serde_json::to_string(&v).unwrap(),
    )
    .unwrap_or_else(|e| panic!("{out}: {e}"));
}

/// Every `spec/characters/<id>.json` recipe (design note 11), minified, and a
/// generated `recipes.rs` listing them: a new character is a file here.
fn recipes() {
    let src = "../../spec/characters";
    println!("cargo:rerun-if-changed={src}");
    let dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::create_dir_all(Path::new(&dir).join("characters")).unwrap();
    let mut ids: Vec<String> = std::fs::read_dir(src)
        .unwrap_or_else(|e| panic!("{src}: {e}"))
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
        .collect();
    ids.sort();
    let mut table = String::from("pub const RECIPES: &[(&str, &str)] = &[\n");
    for id in &ids {
        lean(
            &format!("{src}/{id}.json"),
            &format!("characters/{id}.json"),
            &["$comment", "$schema"],
        );
        table.push_str(&format!(
            "    (\"{id}\", include_str!(concat!(env!(\"OUT_DIR\"), \"/characters/{id}.json\"))),\n"
        ));
    }
    table.push_str("];\n");
    std::fs::write(Path::new(&dir).join("recipes.rs"), table).unwrap();
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    recipes();
    lean(
        "src/catalog_source.json",
        "catalog_runtime.json",
        &["description", "$comment", "note"],
    );
    lean(
        "../../spec/voice-state-profile.json",
        "voice_state_profile.json",
        &["$comment"],
    );
}
