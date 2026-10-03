//! Catalog packs (FX Spec 1.13, design note 26): ready cosmetics and palettes an
//! app loads as data, never part of the engine. A pack names its namespace
//! (`catalog` for Sinua's own, `acme` for a brand's); a file then names an item
//! as `"<namespace>:<id>"` in `cosmetics`, `wardrobe.cosmetics` or `palette`, and
//! the resolver puts the pack's object there. A pack is JSON text, so an app may
//! fetch it from anywhere (seasonal content without an app update).

use std::sync::Mutex;

use serde_json::Value;

use crate::fx_spec::FxDiagnostic;

/// The most items (cosmetics and palettes, every namespace) kept at once.
pub const MAX_ITEMS: usize = 256;

/// The loaded packs: (namespace, the pack as given).
static PACKS: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

fn packs() -> std::sync::MutexGuard<'static, Vec<(String, Value)>> {
    PACKS.lock().unwrap_or_else(|e| e.into_inner())
}

/// `v[k]`, one lookup for every call here (inlined, each costs code).
#[inline(never)]
fn at<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.get(k)
}

/// How many items a pack holds.
fn count(p: &Value) -> usize {
    at(p, "cosmetics")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
        + at(p, "palettes")
            .and_then(Value::as_object)
            .map_or(0, |m| m.len())
}

/// Loads a pack: `{ "catalog": 1, "namespace": "...", "cosmetics": [...],
/// "palettes": { name: palette } }`. Loading a namespace again replaces it. An
/// error loads nothing; a cosmetic is checked where it is worn.
#[inline(never)]
pub fn load(json: &str) -> Vec<FxDiagnostic> {
    let pack = serde_json::from_str::<Value>(json).unwrap_or_default();
    let ns = at(&pack, "namespace")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let version = at(&pack, "catalog").and_then(Value::as_f64).unwrap_or(0.0);
    let mut all = packs();
    let others: usize = all
        .iter()
        .filter(|(n, _)| *n != ns)
        .map(|(_, p)| count(p))
        .sum();
    let (error, path, message) = if !pack.is_object() || version < 1.0 {
        (true, "/catalog", "expected a catalog object, format 1")
    } else if !crate::character::recipe::valid_id(&ns) {
        (true, "/namespace", "a name: 1–32 of a–z, 0–9 and -")
    } else if others + count(&pack) > MAX_ITEMS {
        (true, "", "too many catalog items loaded (256 at most)")
    } else if version > 1.0 {
        (
            false,
            "/catalog",
            "a newer catalog: what this runtime knows loads",
        )
    } else {
        (false, "", "")
    };
    if !error {
        all.retain(|(n, _)| *n != ns);
        all.push((ns, pack));
    }
    if message.is_empty() {
        return Vec::new();
    }
    vec![FxDiagnostic {
        severity: if error { "error" } else { "warning" }.into(),
        path: path.into(),
        message: message.into(),
    }]
}

/// Forgets a namespace's items; whether it had any.
pub fn unload(namespace: &str) -> bool {
    let mut all = packs();
    let n = all.len();
    all.retain(|(k, _)| k != namespace);
    all.len() != n
}

/// The object of `"<namespace>:<id>"` (a palette or a cosmetic), if loaded.
#[inline(never)]
pub fn get(key: &str, palette: bool) -> Option<Value> {
    let (ns, id) = key.split_once(':')?;
    let all = packs();
    let p = &all.iter().find(|(n, _)| n == ns)?.1;
    if palette {
        return at(at(p, "palettes")?, id).cloned();
    }
    at(p, "cosmetics")?
        .as_array()?
        .iter()
        .find(|c| at(c, "id").and_then(Value::as_str) == Some(id))
        .cloned()
}
