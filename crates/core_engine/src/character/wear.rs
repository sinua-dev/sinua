//! A loadout change, softly (FX Spec 1.13, design note 25). The views keep the
//! clock: when a loadout changes what is worn, the character moves to a new
//! registry key of the same character, and for 0.35 s (the views' constant) the view draws
//! [`frame_wear`] from the old side to the new. Cosmetics that arrive pop in with
//! a small overshoot, those that leave shrink away, the zoom (a hat makes room
//! above the head) eases between the two, colours ease, and a new eye style
//! swaps while the eyes are shut in a blink.

use std::collections::HashMap;

use crate::character::registry;
use crate::primitives::OrbFrame;
use crate::transition::TransitionSide;

/// The slots a recipe has of its own; any other slot name is a cosmetic's id.
pub(crate) const OWN: [&str; 4] = ["headTop", "face", "neck", "chest"];

/// Ease out with a small overshoot (easeOutBack, `c1` 1.2).
fn pop(w: f64) -> f64 {
    let (c1, x) = (1.2, w.clamp(0.0, 1.0) - 1.0);
    1.0 + (c1 + 1.0) * x * x * x + c1 * x * x
}

/// The cosmetics a key wears and its zoom (a built-in wears none, zoom 1).
#[inline(never)]
fn worn(key: &str) -> Option<(String, Vec<String>, f64)> {
    match registry::get(key) {
        Some(r) => Some((
            r.profile.clone().unwrap_or_else(|| r.id.clone()),
            r.slots
                .iter()
                .filter(|s| !OWN.contains(&s.name.as_str()))
                .map(|s| s.name.clone())
                .collect(),
            r.zoom,
        )),
        None => crate::character::recipe::recipes()
            .contains_key(key)
            .then(|| (key.to_string(), Vec::new(), 1.0)),
    }
}

/// `o[k] = v`, out of line (each inlined insert costs code).
#[inline(never)]
fn set(o: &mut HashMap<String, f64>, k: String, v: f64) {
    o.insert(k, v);
}

/// `o[k]`, out of line too.
#[inline(never)]
fn val(o: &HashMap<String, f64>, k: &str) -> Option<f64> {
    o.get(k).copied()
}

/// The two sides' overrides at `s` (eased): colours (`palette.*`) mix, the other
/// keys are the old side's until `early` turns false.
#[inline(never)]
fn blend(
    a: &HashMap<String, f64>,
    b: &HashMap<String, f64>,
    s: f64,
    early: bool,
) -> HashMap<String, f64> {
    let mut o = b.clone();
    for (k, &x) in a {
        let colour = k.starts_with("palette.");
        let y = match (val(b, k), colour && k.ends_with(".w")) {
            (Some(y), _) => y,
            (None, true) => 0.0,
            (None, false) => x,
        };
        if colour {
            let d = if k.ends_with(".h") {
                crate::fx_spec::shortest_hue_delta(x, y)
            } else {
                y - x
            };
            set(&mut o, k.clone(), x + d * s);
        } else if early {
            set(&mut o, k.clone(), x);
        }
    }
    for (k, y) in b {
        if val(a, k).is_some() {
            continue;
        }
        if k.starts_with("palette.") {
            if k.ends_with(".w") {
                set(&mut o, k.clone(), y * s);
            }
        } else if early {
            // Only the new side has it: not yet.
            o.remove(k);
        }
    }
    o
}

/// One side of the change drawn with `o`.
#[inline(never)]
fn draw(state: &str, size: u32, t: f64, o: &HashMap<String, f64>) -> Option<OrbFrame> {
    crate::frame_with_overrides(state.to_string(), size, t, o.clone())
}

/// The frame `w` (0..1, linear) into a loadout change `from` → `to` of one
/// character; `None` unless both sides are the same character.
#[inline(never)]
pub fn frame_wear(
    from: &TransitionSide,
    to: &TransitionSide,
    size: u32,
    t: f64,
    w: f64,
) -> Option<OrbFrame> {
    let (ca, a, za) = worn(&from.state)?;
    let (cb, b, zb) = worn(&to.state)?;
    if ca != cb {
        return None;
    }
    let w = w.clamp(0.0, 1.0);
    let s = w * w * (3.0 - 2.0 * w);
    // Colours ease; any other key is the new side's (the eye style swaps mid-blink).
    let mut o = blend(&from.overrides, &to.overrides, s, w < 0.5);
    if val(&from.overrides, "eyeStyle") != val(&to.overrides, "eyeStyle") {
        set(
            &mut o,
            "wearBlink".into(),
            1.0 - ((w - 0.5).abs() / 0.25).min(1.0),
        );
    }
    set(&mut o, "wearZoom".into(), za + (zb - za) * s);
    for id in b.iter().filter(|id| !a.contains(id)) {
        set(&mut o, format!("wear.{id}"), pop(w));
    }
    let mut f = draw(&to.state, size, t, &o)?;
    let leaving: Vec<&String> = a.iter().filter(|id| !b.contains(id)).collect();
    if !leaving.is_empty() && w < 1.0 {
        let mut ol = o;
        set(&mut ol, "wearOnly".into(), 1.0);
        for id in &a {
            let k = if leaving.contains(&id) { 1.0 - s } else { 0.0 };
            set(&mut ol, format!("wear.{id}"), k);
        }
        if let Some(g) = draw(&from.state, size, t, &ol) {
            f.fills.extend(g.fills);
        }
    }
    Some(f)
}
