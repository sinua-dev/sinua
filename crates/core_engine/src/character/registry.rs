//! Recipes that came with an FX Spec (FX Spec 1.12, design note 12).
//!
//! Resolving a spec with a `recipe` reads it once and keeps it here under a
//! content key, `recipe:<id>:<fnv-1a 64 of the recipe>`. The resolved pattern
//! is that key, so a view draws it every frame like any other mode, and the
//! same recipe gives the same key (and the same frames) everywhere.
//!
//! The registry holds the last [`CAPACITY`] recipes used (registered or drawn), and
//! finds any of the last [`SEEN`] texts read (their recipes are kept with them). Each
//! thread also keeps the last [`RECENT`] recipes it registered (design note 31, B5), so
//! a view that resolves a spec draws it at once even while other threads register
//! dozens of others (a Studio grid, an app with many characters).
//! A key is a `&'static str` like every other mode: each distinct recipe's key
//! is leaked once (about 40 bytes) and reused when it comes back.

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::character::recipe::{frame_recipe, Recipe};
use crate::primitives::{ModeOpts, OrbFrame};

/// How many recipes the registry keeps.
pub const CAPACITY: usize = 32;
/// The prefix of every registered key.
pub const PREFIX: &str = "recipe:";

/// A recipe text already read (design note 28, B3): reading the same text again (a
/// picker's thumbnails resolve one file over and over) finds its recipe without parsing.
/// The length and a second hash guard against a crafted fnv64 collision (a recipe can
/// come from a server).
struct Seen {
    len: usize,
    h1: u64,
    h2: u64,
    key: &'static str,
    recipe: Arc<Recipe>,
}

/// How many texts `seen` remembers.
pub const SEEN: usize = 48;
/// How many recipes each thread keeps from its own last registrations.
pub const RECENT: usize = 8;

thread_local! {
    /// This thread's last registered recipes (most recent last).
    static MINE: RefCell<Vec<(&'static str, Arc<Recipe>)>> = const { RefCell::new(Vec::new()) };
}

/// Remembers `key` as one this thread just registered.
#[inline(never)]
fn mine(key: &'static str, recipe: &Arc<Recipe>) {
    MINE.with(|m| {
        let mut m = m.borrow_mut();
        m.retain(|(k, _)| *k != key);
        m.push((key, recipe.clone()));
        if m.len() > RECENT {
            m.remove(0);
        }
    });
}

/// This thread's own recent recipe for `key`.
fn mine_get(key: &str) -> Option<(&'static str, Arc<Recipe>)> {
    MINE.with(|m| m.borrow().iter().rev().find(|(k, _)| *k == key).cloned())
}

struct Registry {
    /// Most recently used last.
    live: Vec<(&'static str, Arc<Recipe>)>,
    /// Every key ever made, so a recipe that comes back reuses its key.
    keys: Vec<&'static str>,
    /// The texts read lately (most recently used last), so a repeat skips the parse.
    seen: Vec<Seen>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    live: Vec::new(),
    keys: Vec::new(),
    seen: Vec::new(),
});

/// A second hash, independent of [`fnv64`] (multiply-rotate, as FxHash), for the
/// collision guard.
#[inline(never)]
fn hash2(s: &str) -> u64 {
    s.bytes().fold(0x9e37_79b9_7f4a_7c15, |h, b| {
        (h.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x517c_c1b7_2722_0a95)
    })
}

/// [`fnv64`], or a forced value in a test (to stage a collision).
fn hash1(s: &str) -> u64 {
    #[cfg(test)]
    if let Some(h) = FORCE_H1.with(Cell::get) {
        return h;
    }
    fnv64(s)
}

#[cfg(test)]
thread_local! {
    static FORCE_H1: Cell<Option<u64>> = const { Cell::new(None) };
    /// Parses on this thread (tests: does a repeat skip it?).
    static PARSES: Cell<usize> = const { Cell::new(0) };
}

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
pub fn register(text: &str) -> Result<&'static str, String> {
    let (len, h1, h2) = (text.len(), hash1(text), hash2(text));
    if let Some(key) = recall(len, h1, h2) {
        return Ok(key);
    }
    #[cfg(test)]
    PARSES.with(|p| p.set(p.get() + 1));
    let recipe = Arc::new(Recipe::parse(text)?);
    let mut reg = lock();
    let name = format!("{PREFIX}{}:{h1:016x}", recipe.id);
    let key = match reg.keys.iter().find(|k| **k == name) {
        Some(k) => *k,
        None => {
            let k: &'static str = Box::leak(name.into_boxed_str());
            reg.keys.push(k);
            k
        }
    };
    // The same key from another text (the fnv64 collided, or the text left `seen`):
    // what was just read wins, so a text never draws another's character.
    reg.live.retain(|(k, _)| *k != key);
    reg.seen.retain(|e| e.key != key);
    reg.live.push((key, recipe.clone()));
    if reg.live.len() > CAPACITY {
        reg.live.remove(0);
    }
    mine(key, &recipe);
    reg.seen.push(Seen {
        len,
        h1,
        h2,
        key,
        recipe,
    });
    if reg.seen.len() > SEEN {
        reg.seen.remove(0);
    }
    Ok(key)
}

/// A text read before (by its digest): its key, the recipe made current again. `None`:
/// parse it.
#[inline(never)]
fn recall(len: usize, h1: u64, h2: u64) -> Option<&'static str> {
    let mut reg = lock();
    let i = reg
        .seen
        .iter()
        .position(|e| (e.len, e.h1, e.h2) == (len, h1, h2))?;
    let e = reg.seen.remove(i);
    // Live again, at the recent end; or, if it left the registry, back in.
    reg.live.retain(|(k, _)| *k != e.key);
    reg.live.push((e.key, e.recipe.clone()));
    if reg.live.len() > CAPACITY {
        reg.live.remove(0);
    }
    mine(e.key, &e.recipe);
    let key = e.key;
    reg.seen.push(e);
    Some(key)
}

/// The recipe registered as `key` (and marks it used), if it is still kept: this
/// thread's own recent ones first, then the live ones, then the texts read lately (a
/// recipe found there is live again).
pub fn get(key: &str) -> Option<Arc<Recipe>> {
    if !key.starts_with(PREFIX) {
        return None;
    }
    if let Some((_, r)) = mine_get(key) {
        return Some(r);
    }
    let mut reg = lock();
    if let Some(i) = reg.live.iter().position(|(k, _)| *k == key) {
        let e = reg.live.remove(i);
        let r = e.1.clone();
        reg.live.push(e);
        return Some(r);
    }
    let e = reg.seen.iter().rev().find(|e| e.key == key)?;
    let (k, r) = (e.key, e.recipe.clone());
    reg.live.push((k, r.clone()));
    if reg.live.len() > CAPACITY {
        reg.live.remove(0);
    }
    Some(r)
}

/// `key`'s static form, if it is registered.
pub fn key(key: &str) -> Option<&'static str> {
    if !key.starts_with(PREFIX) {
        return None;
    }
    if let Some((k, _)) = mine_get(key) {
        return Some(k);
    }
    let reg = lock();
    reg.live
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(k, _)| *k)
        .or_else(|| reg.seen.iter().find(|e| e.key == key).map(|e| e.key))
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
    fn keeps_what_it_read_lately() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        // Registered on another thread, so this one's own recent recipes don't hold it.
        let first = std::thread::spawn(|| register(&with_id("reg-first")).unwrap())
            .join()
            .unwrap();
        for i in 0..CAPACITY {
            register(&with_id(&format!("reg-{i}"))).unwrap();
        }
        assert!(
            get(first).is_some(),
            "out of the live 32, still among the texts read"
        );
        // That use made it live again: it leaves after enough newer ones than both keep.
        for i in CAPACITY..CAPACITY + SEEN + 1 {
            register(&with_id(&format!("reg-{i}"))).unwrap();
        }
        assert!(get(first).is_none(), "the oldest left");
        assert!(frame(first, 64.0, 0.0, &ModeOpts::new()).is_none());
        // It comes back under the same key.
        assert_eq!(register(&with_id("reg-first")).unwrap(), first);
    }

    /// B5: two threads each register more recipes than the registry keeps and draw each
    /// one at once; none goes missing between its resolve and its draw.
    #[test]
    fn a_thread_draws_what_it_just_registered_whatever_other_threads_do() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let run = |tag: &'static str| {
            std::thread::spawn(move || {
                for i in 0..(SEEN + 8) {
                    let k = register(&with_id(&format!("b5-{tag}-{i}"))).unwrap();
                    assert!(frame(k, 64.0, 0.5, &ModeOpts::new()).is_some(), "{tag} {i}");
                    assert_eq!(key(k), Some(k));
                }
            })
        };
        let (a, b) = (run("a"), run("b"));
        a.join().unwrap();
        b.join().unwrap();
    }

    fn parses() -> usize {
        PARSES.with(Cell::get)
    }

    #[test]
    fn a_text_read_before_is_not_parsed_again() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let text = with_id("reg-again");
        let n = parses();
        let a = register(&text).unwrap();
        let b = register(&text).unwrap();
        assert_eq!((a, parses()), (b, n + 1), "parsed once");
        // Another text is parsed (one byte more).
        register(&format!("{text} ")).unwrap();
        assert_eq!(parses(), n + 2);
    }

    #[test]
    fn a_crafted_collision_never_draws_the_other_texts_character() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        // Two texts with the same fnv64 (forced): the second is parsed and drawn as
        // itself, not served from the first's entry; and back again.
        let a = with_id("reg-clash");
        let b = a.replacen("\"base\":", "\"base\": ", 1);
        assert_ne!(a, b);
        FORCE_H1.with(|h| h.set(Some(0xdead_beef)));
        let n = parses();
        let ka = register(&a).unwrap();
        let ra = get(ka).unwrap();
        let kb = register(&b).unwrap();
        assert_eq!(ka, kb, "the same key: the collision is real");
        assert_eq!(
            parses(),
            n + 2,
            "the second text was parsed, not served from the first"
        );
        let rb = get(kb).unwrap();
        assert!(!Arc::ptr_eq(&ra, &rb));
        register(&a).unwrap();
        assert_eq!(parses(), n + 3);
        assert!(!Arc::ptr_eq(&get(ka).unwrap(), &rb));
        FORCE_H1.with(|h| h.set(None));
    }

    #[test]
    fn errors_say_where() {
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let e = register(&with_id("reg-bad").replacen("\"segments\":10", "\"segments\":1000", 1))
            .unwrap_err();
        assert!(e.starts_with("/parts/") && e.contains("segments"), "{e}");
    }
}
