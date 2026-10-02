//! Cosmetics (FX Spec 1.13, design note 21): a hat, glasses, a scarf, a badge.
//!
//! A cosmetic is data: `body` and `eyes` parts (a body with its layers) drawn
//! on one of the character's **slots** (`headTop`, `face`, `neck`, `chest`), in
//! the slot's local units: the slot point is (0, 0), and the slot's `scale` and
//! `angle` size and turn it, so each character makes the same hat fit. It moves
//! with the slot's chain (the body's pose, the head turn). A cosmetic may give a
//! character its own `fit` (`at` in local units, `scale`, `angle`) and say which
//! characters it is made for (`fits`).
//!
//! The engine embeds no cosmetic. An FX Spec's `cosmetics` go into the
//! character's recipe (its `cosmetics` key) and are read here, with the
//! recipe's own readers: the cosmetic's colours join the palette as
//! `<id>.<name>` (so `palette` repaints them), its parts are appended (drawn on
//! top) in a slot of their own, and a hat on `headTop` zooms the character out
//! about its feet until the hat and the hop fit in the box.

use serde_json::Value;

use crate::character::geom::Hsl;
use crate::character::parts::{self, Part};
use crate::character::recipe::{hsl_of, valid_id, SlotSpec, Space, J};

/// The keys a cosmetic may hold.
pub const KEYS: [&str; 7] = ["id", "label", "slot", "palette", "parts", "fits", "fit"];

/// Local units of room above `headTop` a hat may use (design note 21).
pub const HEAD_ROOM: f64 = 40.0;
/// Design units kept free above a hat: the tap hop's lift (9) and a margin.
const HOP_ROOM: f64 = 11.0;
/// Where a zoomed character stays put: its feet (`Recipe::zoom`).
pub const FEET: (f64, f64) = (100.0, 192.0);

/// What the cosmetics add to a recipe being read.
pub struct Into<'a> {
    pub id: &'a str,
    pub palette: &'a mut Vec<(String, Hsl)>,
    pub slots: &'a mut Vec<SlotSpec>,
    pub parts: &'a mut Vec<Part>,
    pub zoom: &'a mut f64,
    /// (pointer, why) for each cosmetic that doesn't fit this character.
    pub skipped: &'a mut Vec<(String, String)>,
}

/// `<at>/<k>: <what>`, the one error and note shape here.
#[inline(never)]
fn bad(at: &str, k: &str, what: &str) -> String {
    format!("{at}/{k}: {what}")
}

/// `v[k]`, one lookup for every call here (inlined, each costs code).
#[inline(never)]
fn get<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.get(k)
}

/// Reads `list` (a recipe's `cosmetics`) into `o`.
#[inline(never)]
pub fn read(list: &Value, o: Into) -> Result<(), String> {
    let list = list.as_array().ok_or("/cosmetics: expected a list")?;
    // The recipe's own slots (a cosmetic's slot copy is never another's slot).
    let own = o.slots.len();
    for (i, v) in list.iter().enumerate() {
        let at = format!("/cosmetics/{i}");
        let c = J::new(v, &at);
        let obj = v
            .as_object()
            .ok_or_else(|| bad(&at, "", "expected an object"))?;
        if let Some(k) = obj.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(bad(&at, k, "unknown field"));
        }
        let cid = c.s("id")?;
        let twice = list[..i]
            .iter()
            .any(|p| get(p, "id").and_then(Value::as_str) == Some(cid.as_str()));
        if !valid_id(&cid) || twice {
            return Err(bad(&at, "id", "a new id: 1–32 of a–z, 0–9 and -"));
        }
        let slot = c.s("slot")?;
        if get(v, "label").is_some() {
            c.s("label")?;
        }
        let fits = get(v, "fits").map(|f| {
            f.as_array()
                .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(o.id)))
        });
        let base = o.slots[..own].iter().position(|s| s.name == slot);
        let skip = match (fits, base) {
            (Some(false), _) => Some(("fits", format!("`{cid}` isn't made for `{}`", o.id))),
            (_, None) => Some(("slot", format!("`{}` has no `{slot}` slot", o.id))),
            _ => None,
        };
        if let Some((k, why)) = skip {
            o.skipped
                .push((format!("{at}/{k}"), why + ", so it isn't drawn"));
            continue;
        }
        let base = base.unwrap_or_default();
        let b = &o.slots[base];
        let (mut x, mut y, mut s, mut a, follows) = (b.at.0, b.at.1, b.scale, b.angle, b.follows);
        if let Some(f) = get(v, "fit").and_then(|f| get(f, o.id)) {
            let fat = format!("{at}/fit/{}", o.id);
            let f = J::new(f, &fat);
            let (dx, dy) = if get(f.v, "at").is_some() {
                f.p2("at")?
            } else {
                (0.0, 0.0)
            };
            let (sin, cos) = a.sin_cos();
            x += (dx * cos - dy * sin) * s;
            y += (dx * sin + dy * cos) * s;
            s *= f.f_or("scale", 1.0)?;
            a += f.f_or("angle", 0.0)?;
        }
        if slot == "headTop" {
            let need = HEAD_ROOM * s + HOP_ROOM;
            *o.zoom = o.zoom.min((FEET.1 - need) / (FEET.1 - y));
        }
        let index = o.slots.len() as u8;
        o.slots.push(SlotSpec {
            name: cid.clone(),
            at: (x, y),
            scale: s,
            angle: a,
            follows,
        });
        // Its colours: `<id>.<name>` in the palette; by their short name (over
        // the character's own) in its parts.
        let from = o.palette.len();
        let mut names: Vec<String> = o.palette.iter().map(|(n, _)| n.clone()).collect();
        if let Some(p) = get(v, "palette").and_then(Value::as_object) {
            for (k, col) in p {
                if let Some(n) = names[..from].iter_mut().find(|n| *n == k) {
                    n.clear();
                }
                names.push(k.clone());
                o.palette.push((
                    format!("{cid}.{k}"),
                    hsl_of(col, &format!("{at}/palette/{k}"))?,
                ));
            }
        }
        let names = parts::Names {
            colours: &names,
            surfaces: &[],
            slots: &[],
        };
        for (j, p) in c.arr("parts")?.iter().enumerate() {
            let pat = format!("{at}/parts/{j}");
            if !matches!(
                get(p, "part").and_then(Value::as_str),
                Some("body" | "eyes")
            ) {
                return Err(bad(
                    &pat,
                    "part",
                    "a cosmetic draws `body` (with its layers) and `eyes`",
                ));
            }
            o.parts
                .push(parts::parse_in(p, &pat, &names, Space::Slot(index))?);
        }
    }
    Ok(())
}
