//! Recipes that came with an FX Spec (FX Spec 1.12, design note 12).
//!
//! Resolving a spec with a `recipe` reads it once and keeps it here under a
//! content key, `recipe:<id>:<fnv-1a 64 of the recipe>`. The resolved pattern
//! is that key, so a view draws it every frame like any other mode, and the
//! same recipe gives the same key (and the same frames) everywhere.
//!
//! The registry holds the last [`CAPACITY`] recipes used (registered or drawn).
//! A key is a `&'static str` like every other mode: each distinct recipe's key
//! is leaked once (about 40 bytes) and reused when it comes back.

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use crate::character::recipe::{frame_recipe, Recipe};
use crate::primitives::{ModeOpts, OrbFrame};

/// How many recipes the registry keeps.
pub const CAPACITY: usize = 32;

/// The prefix of every registered key.
pub const PREFIX: &str = "recipe:";

struct Registry {
    /// Most recently used last.
    live: Vec<(&'static str, Arc<Recipe>)>,
    /// The one recipe a thumbnail is drawn from ([`preview`]): kept apart, so a
    /// grid of thumbnails never pushes the live characters out.
    preview: Option<Arc<Recipe>>,
    /// Every key ever made, so a recipe that comes back reuses its key.
    keys: Vec<&'static str>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    live: Vec::new(),
    preview: None,
    keys: Vec::new(),
});

fn fnv64(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn lock() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads `text` (a recipe's JSON) and keeps it; its key, or the recipe's error
/// (`<JSON pointer>: <what>`).
#[cfg(test)]
pub fn register(text: &str) -> Result<&'static str, String> {
    register_with(text, None, &mut Vec::new())
}

/// [`register`], moving the cosmetics that didn't fit (pointer, why) into
/// `skipped`. `basis` (default: the text) is what the key hashes: a built-in
/// wearing cosmetics hashes its id and the cosmetics, not its own recipe text,
/// so an edit to a built-in that draws the same doesn't change the key (or the
/// FX Spec locks that record it).
pub fn register_with(
    text: &str,
    basis: Option<&str>,
    skipped: &mut Vec<(String, String)>,
) -> Result<&'static str, String> {
    let mut recipe = Recipe::parse(text)?;
    *skipped = std::mem::take(&mut recipe.skipped);
    if PREVIEWING.with(Cell::get) {
        lock().preview = Some(Arc::new(recipe));
        return Ok(PREVIEW_KEY);
    }
    let name = format!(
        "{PREFIX}{}:{:016x}",
        recipe.id,
        fnv64(basis.unwrap_or(text))
    );
    let mut reg = lock();
    if let Some(i) = reg.live.iter().position(|(k, _)| *k == name) {
        let e = reg.live.remove(i);
        let key = e.0;
        reg.live.push(e);
        return Ok(key);
    }
    let key = match reg.keys.iter().find(|k| **k == name) {
        Some(k) => *k,
        None => {
            let k: &'static str = Box::leak(name.into_boxed_str());
            reg.keys.push(k);
            k
        }
    };
    reg.live.push((key, Arc::new(recipe)));
    if reg.live.len() > CAPACITY {
        reg.live.remove(0);
    }
    Ok(key)
}

/// The key a recipe of `id` hashed from `basis` gets (tests: was it registered?).
#[cfg(test)]
pub fn key_for(id: &str, basis: &str) -> String {
    format!("{PREFIX}{id}:{:016x}", fnv64(basis))
}

/// The key of the thumbnail recipe ([`preview`]).
pub const PREVIEW_KEY: &str = "recipe:~preview";

thread_local! {
    static PREVIEWING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` with every recipe it registers kept in the one preview place (a
/// thumbnail: resolved, drawn once, replaced by the next).
pub fn preview<T>(f: impl FnOnce() -> T) -> T {
    PREVIEWING.with(|p| p.set(true));
    let out = f();
    PREVIEWING.with(|p| p.set(false));
    out
}

/// The recipe registered as `key` (and marks it used), if it is still kept.
pub fn get(key: &str) -> Option<Arc<Recipe>> {
    if !key.starts_with(PREFIX) {
        return None;
    }
    let mut reg = lock();
    if key == PREVIEW_KEY {
        return reg.preview.clone();
    }
    let i = reg.live.iter().position(|(k, _)| *k == key)?;
    let e = reg.live.remove(i);
    let r = e.1.clone();
    reg.live.push(e);
    Some(r)
}

/// `key`'s static form, if it is registered.
pub fn key(key: &str) -> Option<&'static str> {
    if !key.starts_with(PREFIX) {
        return None;
    }
    let reg = lock();
    if key == PREVIEW_KEY {
        return reg.preview.as_ref().map(|_| PREVIEW_KEY);
    }
    reg.live.iter().find(|(k, _)| *k == key).map(|(k, _)| *k)
}

/// A registered recipe's frame (`None` once it has left the registry).
pub fn frame(key: &str, size: f64, t: f64, o: &ModeOpts) -> Option<OrbFrame> {
    get(key).map(|r| frame_recipe(&r, size, t, o))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests share the registry; one at a time keeps the eviction test exact.
    static ONE: Mutex<()> = Mutex::new(());

    fn with_id(id: &str) -> String {
        crate::character::recipe::RECIPES
            .iter()
            .find(|(n, _)| *n == "chirp")
            .unwrap()
            .1
            .replacen("\"id\":\"chirp\"", &format!("\"id\":\"{id}\""), 1)
    }

    #[test]
    fn same_recipe_same_key() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let a = register(&with_id("reg-same")).unwrap();
        let b = register(&with_id("reg-same")).unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with("recipe:reg-same:"));
        assert_eq!(key(a), Some(a));
        assert!(get(a).is_some());
    }

    #[test]
    fn keeps_the_last_32() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let first = register(&with_id("reg-first")).unwrap();
        for i in 0..CAPACITY {
            register(&with_id(&format!("reg-{i}"))).unwrap();
        }
        assert!(get(first).is_none(), "the oldest left");
        assert!(frame(first, 64.0, 0.0, &ModeOpts::new()).is_none());
        // It comes back under the same key.
        assert_eq!(register(&with_id("reg-first")).unwrap(), first);
    }

    #[test]
    fn errors_say_where() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let e = register(&with_id("reg-bad").replacen("\"segments\":10", "\"segments\":1000", 1))
            .unwrap_err();
        assert!(e.starts_with("/parts/") && e.contains("segments"), "{e}");
    }
}
