//! Character recipes (design note 11, FX Spec 1.12): a character is data.
//!
//! A recipe (`spec/characters/<id>.json`, embedded minified by build.rs) names
//! its palette, how `hue` turns it, its rig, the surfaces its face turns on, and
//! an ordered list of **parts** from the library in `character/parts/`, each with
//! its numbers. The parts are the behaviour (a crest that lifts to listen, wings
//! that flutter with the voice); the recipe says how big, where, what colour and
//! how much. The four launch characters were drawn by hand-written Rust before;
//! their recipes draw them byte for byte the same: a test compared both on 900
//! frames before the old code went (3 sizes × 5 times × 15 poses × 4), and the
//! golden cases hold it since.
//!
//! Draw order is the parts' order. Every part draws in a **space**:
//! - `ground`: the floor (shadow, the celebrate burst), only scaled;
//! - `whole`: the character's frame (shake), not its body's pose (feet, notes);
//! - `body`: the rig's pose (tilt, bob, lean, squash);
//! - `face`: the body's pose, after the head turn wraps it onto a surface.
//!
//! Slots (where 1.13's cosmetics will attach) are declared and resolved, never
//! drawn, in this version.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::character::face::{self, CELEBRATE_INK};
use crate::character::geom::{self, Hsl, Xf};
use crate::character::kit::{self, Tier};
use crate::character::parts::{self, Part};
use crate::character::rig::{self, Pose};
use crate::character::turn::{self, Surface, Turn};
use crate::primitives::{Fill, ModeOpts, OrbFrame};

include!(concat!(env!("OUT_DIR"), "/recipes.rs"));

/// The recipe format this engine reads.
pub const RECIPE_VERSION: u64 = 1;

/// The limits on any recipe (design note 12): a file from outside can't make
/// the engine slow. A recipe over one is an error, never quietly trimmed.
pub const MAX_BYTES: usize = 64 * 1024;
/// Parts, a body's inner layers included.
pub const MAX_PARTS: usize = 48;
/// Every number in a recipe lies within ±this.
pub const MAX_NUMBER: f64 = 1000.0;

/// The keys a recipe may hold.
const RECIPE_KEYS: [&str; 13] = [
    "$schema", "$comment", "recipe", "id", "profile", "palette", "hue", "rig", "surfaces", "parts",
    "burst", "slots", "contrast",
];

/// The first number outside ±[`MAX_NUMBER`], with its JSON pointer.
fn too_big(v: &Value, at: &str) -> Option<String> {
    match v {
        Value::Number(n) => n
            .as_f64()
            .filter(|x| x.abs() > MAX_NUMBER)
            .map(|x| format!("{at}: {x} is outside ±{MAX_NUMBER}")),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .find_map(|(i, x)| too_big(x, &format!("{at}/{i}"))),
        Value::Object(o) => o.iter().find_map(|(k, x)| too_big(x, &format!("{at}/{k}"))),
        _ => None,
    }
}

/// A recipe field reader: every lookup says where it failed.
#[derive(Clone, Copy)]
pub struct J<'a> {
    pub v: &'a Value,
    pub at: &'a str,
}

impl<'a> J<'a> {
    pub fn new(v: &'a Value, at: &'a str) -> J<'a> {
        J { v, at }
    }
    fn err(&self, k: &str, what: &str) -> String {
        format!("{}/{k}: {what}", self.at)
    }
    pub fn get(&self, k: &str) -> Option<&'a Value> {
        self.v.get(k)
    }
    #[inline(never)]
    pub fn f(&self, k: &str) -> Result<f64, String> {
        self.v
            .get(k)
            .and_then(Value::as_f64)
            .ok_or_else(|| self.err(k, "expected a number"))
    }
    #[inline(never)]
    pub fn f_or(&self, k: &str, d: f64) -> Result<f64, String> {
        match self.v.get(k) {
            None => Ok(d),
            Some(_) => self.f(k),
        }
    }
    #[inline(never)]
    pub fn u(&self, k: &str) -> Result<usize, String> {
        self.v
            .get(k)
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .ok_or_else(|| self.err(k, "expected a whole number"))
    }
    #[inline(never)]
    pub fn s(&self, k: &str) -> Result<String, String> {
        self.v
            .get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| self.err(k, "expected a string"))
    }
    #[inline(never)]
    pub fn nums(&self, k: &str) -> Result<Vec<f64>, String> {
        let a = self
            .v
            .get(k)
            .and_then(Value::as_array)
            .ok_or_else(|| self.err(k, "expected an array of numbers"))?;
        a.iter()
            .map(|x| x.as_f64().ok_or_else(|| self.err(k, "expected numbers")))
            .collect()
    }
    #[inline(never)]
    pub fn n(&self, k: &str, len: usize) -> Result<Vec<f64>, String> {
        let v = self.nums(k)?;
        if v.len() != len {
            return Err(self.err(k, &format!("expected {len} numbers")));
        }
        Ok(v)
    }
    #[inline(never)]
    pub fn p2(&self, k: &str) -> Result<(f64, f64), String> {
        let v = self.n(k, 2)?;
        Ok((v[0], v[1]))
    }
    #[inline(never)]
    pub fn obj(&self, k: &str) -> Result<J<'a>, String> {
        match self.v.get(k) {
            Some(v) if v.is_object() => Ok(J { v, at: self.at }),
            _ => Err(self.err(k, "expected an object")),
        }
    }
    #[inline(never)]
    pub fn arr(&self, k: &str) -> Result<&'a Vec<Value>, String> {
        self.v
            .get(k)
            .and_then(Value::as_array)
            .ok_or_else(|| self.err(k, "expected an array"))
    }
}

/// A sway with the voice: `opts[gain] × sin(t × rate) × (base + drive)`, added to the tilt.
#[derive(Clone, Debug, PartialEq)]
pub struct Sway {
    pub gain: String,
    pub rate: f64,
    pub base: f64,
}

/// How the rig poses the body.
#[derive(Clone, Debug, PartialEq)]
pub enum Rig {
    /// Tilts about a pivot (Chirp's feet, Hum's yoke), bobs, leans by `lean` × the pose's lean;
    /// optionally squashes with the voice and sways (Hum). `mount`: the bob share of the
    /// `mount` space (Hum's yoke follows the capsule's bob at 30 %).
    Pivot {
        pivot: (f64, f64),
        lean: f64,
        squash: bool,
        sway: Option<Sway>,
        mount: Option<f64>,
    },
    /// Stands on a base point (Buzzy): leans by the pose's lean, bobs and squashes
    /// about the base, tilts, and shakes inside its own chain.
    Upright { base: (f64, f64) },
    /// Floats about a centre (Wisp): `float` = sin(t × rate) × amp, swells with the
    /// voice (uniform squash), tilts, and shakes inside its own chain.
    Float {
        center: (f64, f64),
        lean: f64,
        rate: f64,
        amp: f64,
    },
}

/// Where a part draws (see the module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Space {
    Ground,
    Whole,
    /// Between `whole` and `body`: the rig's `mount` (Hum's yoke).
    Mount,
    Body,
    Face,
}

impl Space {
    #[inline(never)]
    pub fn parse(s: &str) -> Result<Space, String> {
        Ok(match s {
            "ground" => Space::Ground,
            "whole" => Space::Whole,
            "mount" => Space::Mount,
            "body" => Space::Body,
            "face" => Space::Face,
            o => return Err(format!("unknown space `{o}`")),
        })
    }
}

/// A slot: where a cosmetic will attach (1.13), following one of the chains.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotSpec {
    pub name: String,
    pub at: (f64, f64),
    pub scale: f64,
    pub angle: f64,
    pub follows: Space,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: String,
    /// Named colours; parts refer to them by index. The last is `celebrate`,
    /// the shared celebrate gold.
    pub palette: Vec<(String, Hsl)>,
    pub hue_base: f64,
    /// The palette indices `hue` turns.
    pub hue_turns: Vec<usize>,
    pub rig: Rig,
    pub surfaces: Vec<(String, Surface)>,
    pub parts: Vec<Part>,
    pub burst_space: Space,
    pub burst_at: (f64, f64),
    pub burst_r: f64,
    pub burst_colors: [usize; 3],
    pub slots: Vec<SlotSpec>,
    /// The built-in character whose voice-state profile it takes (`None`: the
    /// shared `character` profile).
    pub profile: Option<String>,
    /// Inks and the colour each sits on (palette indices): a palette override
    /// keeps them readable (`character/palette.rs`).
    pub contrast: Vec<(usize, usize)>,
}

fn hsl_of(v: &Value, at: &str) -> Result<Hsl, String> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| format!("{at}: expected [h, s, l]"))?;
    let n = |i: usize| {
        a[i].as_f64()
            .ok_or_else(|| format!("{at}: expected numbers"))
    };
    Ok(geom::hsl(n(0)?, n(1)?, n(2)?))
}

impl Recipe {
    #[inline(never)]
    pub fn parse(text: &str) -> Result<Recipe, String> {
        if text.len() > MAX_BYTES {
            return Err(format!(": {} bytes, at most {MAX_BYTES}", text.len()));
        }
        let v: Value = serde_json::from_str(text).map_err(|e| format!(": not JSON: {e}"))?;
        let top = v.as_object().ok_or(": expected an object")?;
        if let Some(k) = top.keys().find(|k| !RECIPE_KEYS.contains(&k.as_str())) {
            return Err(format!("/{k}: unknown field"));
        }
        if let Some(e) = too_big(&v, "") {
            return Err(e);
        }
        let r = J::new(&v, "");
        let version = r.u("recipe")? as u64;
        if version != RECIPE_VERSION {
            return Err(format!(
                "/recipe: version {version}, this engine reads {RECIPE_VERSION}"
            ));
        }
        let id = r.s("id")?;
        if id.is_empty()
            || id.len() > 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("/id: 1–32 of a–z, 0–9 and -".into());
        }
        let profile = match r.get("profile") {
            None => None,
            Some(_) => {
                let p = r.s("profile")?;
                if !RECIPES.iter().any(|(id, _)| *id == p) {
                    return Err(format!("/profile: `{p}` is not a built-in character"));
                }
                Some(p)
            }
        };
        let mut palette = Vec::new();
        for (k, c) in r
            .get("palette")
            .and_then(Value::as_object)
            .ok_or("/palette: expected an object")?
        {
            if k == "celebrate" {
                return Err("/palette/celebrate: reserved for the shared celebrate gold".into());
            }
            palette.push((k.clone(), hsl_of(c, &format!("/palette/{k}"))?));
        }
        palette.push(("celebrate".to_string(), CELEBRATE_INK));
        let colour_names: Vec<String> = palette.iter().map(|(n, _)| n.clone()).collect();
        let colour = |v: &Value, at: &str| -> Result<usize, String> {
            let s = v
                .as_str()
                .ok_or_else(|| format!("{at}: expected a colour name"))?;
            colour_names
                .iter()
                .position(|n| n == s)
                .ok_or_else(|| format!("{at}: unknown colour `{s}`"))
        };
        let mut contrast = Vec::new();
        if let Some(c) = r.get("contrast") {
            for (i, pair) in c
                .as_array()
                .ok_or("/contrast: expected [[ink, ground], …]")?
                .iter()
                .enumerate()
            {
                let at = format!("/contrast/{i}");
                let p = pair
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .ok_or_else(|| format!("{at}: expected [ink, ground]"))?;
                contrast.push((colour(&p[0], &at)?, colour(&p[1], &at)?));
            }
        }
        let hue = r.obj("hue")?;
        let hue_turns: Vec<usize> = hue
            .arr("turns")?
            .iter()
            .map(|x| colour(x, "/hue/turns"))
            .collect::<Result<_, _>>()?;
        let rig_j = r.obj("rig")?;
        let rig = match rig_j.s("kind")?.as_str() {
            "pivot" => Rig::Pivot {
                pivot: rig_j.p2("pivot")?,
                lean: rig_j.f("lean")?,
                squash: rig_j
                    .get("squash")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                sway: match rig_j.get("sway") {
                    None => None,
                    Some(v) => {
                        let sj = J::new(v, "/rig/sway");
                        Some(Sway {
                            gain: sj.s("gain")?,
                            rate: sj.f("rate")?,
                            base: sj.f("base")?,
                        })
                    }
                },
                mount: match rig_j.get("mount") {
                    None => None,
                    Some(_) => Some(rig_j.obj("mount")?.f("bob")?),
                },
            },
            "upright" => Rig::Upright {
                base: rig_j.p2("base")?,
            },
            "float" => {
                let fl = rig_j.obj("float")?;
                Rig::Float {
                    center: rig_j.p2("center")?,
                    lean: rig_j.f("lean")?,
                    rate: fl.f("rate")?,
                    amp: fl.f("amp")?,
                }
            }
            o => return Err(format!("/rig/kind: unknown rig `{o}`")),
        };
        let mut surfaces = Vec::new();
        if let Some(s) = r.get("surfaces").and_then(Value::as_object) {
            for (k, v) in s {
                let j = J::new(v, "/surfaces");
                surfaces.push((
                    k.clone(),
                    Surface {
                        c: j.p2("c")?,
                        r: j.f("r")?,
                        depth: j.f("depth")?,
                        cylinder: j.get("cylinder").and_then(Value::as_bool).unwrap_or(false),
                    },
                ));
            }
        }
        let surface_names: Vec<String> = surfaces.iter().map(|(n, _)| n.clone()).collect();
        let names = parts::Names {
            colours: &colour_names,
            surfaces: &surface_names,
        };
        let parts = r
            .arr("parts")?
            .iter()
            .enumerate()
            .map(|(i, p)| parts::parse(p, &format!("/parts/{i}"), &names))
            .collect::<Result<Vec<_>, _>>()?;
        let count: usize = parts.iter().map(|p| 1 + p.inner.len()).sum();
        if count > MAX_PARTS {
            return Err(format!("/parts: {count} parts, at most {MAX_PARTS}"));
        }
        let burst = r.obj("burst")?;
        let bc = burst.arr("colors")?;
        let name = |i: usize| -> Result<usize, String> {
            colour(
                bc.get(i).ok_or("/burst/colors: expected three names")?,
                "/burst/colors",
            )
        };
        let mut slots = Vec::new();
        if let Some(s) = r.get("slots").and_then(Value::as_object) {
            for (k, v) in s {
                let j = J::new(v, "/slots");
                slots.push(SlotSpec {
                    name: k.clone(),
                    at: j.p2("at")?,
                    scale: j.f_or("scale", 1.0)?,
                    angle: j.f_or("angle", 0.0)?,
                    follows: match j.s("follows")?.as_str() {
                        "head" | "body" => Space::Body,
                        "face" => Space::Face,
                        o => return Err(format!("/slots/{k}/follows: unknown chain `{o}`")),
                    },
                });
            }
        }
        let recipe = Recipe {
            id,
            palette,
            hue_base: hue.f("base")?,
            hue_turns,
            rig,
            surfaces,
            parts,
            burst_space: match burst.get("space").and_then(Value::as_str) {
                None => Space::Ground,
                Some(s) => Space::parse(s).map_err(|e| format!("/burst/space: {e}"))?,
            },
            burst_at: burst.p2("at")?,
            burst_r: burst.f("r")?,
            burst_colors: [name(0)?, name(1)?, name(2)?],
            slots,
            profile,
            contrast,
        };
        Ok(recipe)
    }

    /// The opts keys its parts and rig read as gains (`flutterGain`, `swayGain`,
    /// `curlGain` …): an FX Spec with this recipe may set them in `params`.
    pub fn gains(&self) -> Vec<String> {
        let mut g: Vec<String> = Vec::new();
        let mut add = |n: &str| {
            if !g.iter().any(|x| x == n) {
                g.push(n.to_string());
            }
        };
        if let Rig::Pivot { sway: Some(s), .. } = &self.rig {
            add(&s.gain);
        }
        for p in self
            .parts
            .iter()
            .flat_map(|p| std::iter::once(p).chain(&p.inner))
        {
            p.params.names.iter().for_each(|n| add(n));
            if p.kind == parts::Kind::Spirit {
                add("curlGain");
            }
            if p.kind == parts::Kind::Arms {
                add("arms");
            }
        }
        g
    }

    /// A palette colour by name (the recipe's own, after no `hue`).
    #[cfg(test)]
    pub fn colour_named(&self, name: &str) -> Option<Hsl> {
        self.palette
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| *c)
    }
}

/// The built-in recipes, parsed once.
pub fn recipes() -> &'static HashMap<String, Recipe> {
    static R: OnceLock<HashMap<String, Recipe>> = OnceLock::new();
    R.get_or_init(|| {
        RECIPES
            .iter()
            .map(|(id, text)| {
                let r =
                    Recipe::parse(text).unwrap_or_else(|e| panic!("spec/characters/{id}.json{e}"));
                (id.to_string(), r)
            })
            .collect()
    })
}

/// What a part needs to draw this frame.
pub struct Ctx<'a> {
    pub o: &'a ModeOpts,
    pub t: f64,
    pub pose: Pose,
    pub tier: Tier,
    pub tn: Turn,
    pub accessories: bool,
    pub mouth_off: bool,
    pub lw: f64,
    /// `face::mouth_weights`: (rest, dots, talk, level, phase).
    pub w: (f64, f64, f64, f64, f64),
    /// The float rig's drift this frame (0 for the others).
    pub float: f64,
    pal: Vec<Hsl>,
    surfaces: &'a [(String, Surface)],
    ground: Xf,
    whole: Xf,
    mount: Xf,
    body: Xf,
}

impl<'a> Ctx<'a> {
    /// A palette colour (after `hue` turned it).
    pub fn colour(&self, i: usize) -> Hsl {
        self.pal[i]
    }
    pub fn surface(&self, i: usize) -> &Surface {
        &self.surfaces[i].1
    }
    pub fn get(&self, key: &str, default: f64) -> f64 {
        *self.o.get(key).unwrap_or(&default)
    }
    /// A fill drawn in `space`; `surface` is the one a face-space fill turns on.
    #[inline(never)]
    pub fn place(&self, f: Fill, space: Space, surface: Option<usize>) -> Fill {
        match space {
            Space::Ground => geom::transform(f, &self.ground),
            Space::Whole => geom::transform(f, &self.whole),
            Space::Mount => geom::transform(f, &self.mount),
            Space::Body => geom::transform(f, &self.body),
            Space::Face => {
                let f = match surface {
                    Some(s) if !self.tn.is_zero() => self.tn.map_fill(f, self.surface(s)),
                    _ => f,
                };
                geom::transform(f, &self.body)
            }
        }
    }
}

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Everything a frame (or a slot) of `r` needs at `size`, `t`, `o`.
fn setup<'a>(r: &'a Recipe, size: f64, t: f64, o: &'a ModeOpts) -> Ctx<'a> {
    let pose = rig::pose(o, t);
    let tier = kit::tier(size);
    let accessories = get(o, "accessories", 1.0) >= 0.5;
    let mouth_off = get(o, "mouth", 1.0) < 0.5;
    let dh = get(o, "hue", r.hue_base) - r.hue_base;
    let mut pal: Vec<Hsl> = r.palette.iter().map(|(_, c)| *c).collect();
    for &i in &r.hue_turns {
        pal[i] = pal[i].rotate(dh);
    }
    // Palette overrides (design note 19) land after `hue`: a given colour isn't turned.
    if o.keys().any(|k| k.starts_with("palette.")) {
        for (i, (name, _)) in r.palette.iter().enumerate() {
            let w = get(o, &format!("palette.{name}.w"), 0.0).clamp(0.0, 1.0);
            if w > 0.0 {
                let c = pal[i];
                let h = get(o, &format!("palette.{name}.h"), c.h);
                let dh = crate::fx_spec::shortest_hue_delta(c.h, h);
                pal[i] = Hsl {
                    h: (c.h + dh * w).rem_euclid(360.0),
                    s: c.s + (get(o, &format!("palette.{name}.s"), c.s) - c.s) * w,
                    l: c.l + (get(o, &format!("palette.{name}.l"), c.l) - c.l) * w,
                };
            }
        }
    }
    let w = face::mouth_weights(pose.mouth);
    let lw = tier.line;
    let tn = turn::angles(o, t, tier);

    let scale = Xf::scale(size / 200.0, size / 200.0);
    let whole = Xf::translate(pose.shake, 0.0).then(scale);
    let mut float = 0.0;
    let (body, mount) = match &r.rig {
        Rig::Pivot {
            pivot,
            lean,
            squash,
            sway,
            mount,
        } => {
            let mut b = Xf::translate(-pivot.0, -pivot.1);
            if *squash {
                b = b.then(Xf::scale(1.0 + pose.squash, 1.0 - pose.squash));
            }
            let tilt = match sway {
                None => pose.tilt,
                Some(s) => {
                    pose.tilt + get(o, &s.gain, 0.0) * (t * s.rate).sin() * (s.base + pose.drive)
                }
            };
            let b = b
                .then(Xf::rotate(tilt))
                .then(Xf::translate(
                    pivot.0,
                    pivot.1 + pose.bob + pose.lean * lean,
                ))
                .then(whole);
            let m = match mount {
                None => whole,
                Some(k) => Xf::translate(0.0, pose.bob * k).then(whole),
            };
            (b, m)
        }
        Rig::Upright { base } => {
            let b = Xf::translate(pose.shake, pose.lean)
                .then(Xf::translate(-base.0, -base.1 + pose.bob))
                .then(Xf::scale(1.0 + pose.squash, 1.0 - pose.squash))
                .then(Xf::rotate(pose.tilt))
                .then(Xf::translate(base.0, base.1))
                .then(scale);
            (b, whole)
        }
        Rig::Float {
            center,
            lean,
            rate,
            amp,
        } => {
            float = (t * rate).sin() * amp;
            let swell = 1.0 + pose.squash;
            let b = Xf::translate(-center.0, -center.1)
                .then(Xf::scale(swell, swell))
                .then(Xf::rotate(pose.tilt))
                .then(Xf::translate(
                    center.0 + pose.shake,
                    center.1 + float + pose.bob + pose.lean * lean,
                ))
                .then(scale);
            (b, whole)
        }
    };
    Ctx {
        o,
        t,
        pose,
        tier,
        tn,
        accessories,
        mouth_off,
        lw,
        w,
        float,
        pal,
        surfaces: &r.surfaces,
        ground: scale,
        whole,
        mount,
        body,
    }
}

/// A recipe's frame.
pub fn frame_recipe(r: &Recipe, size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let ctx = setup(r, size, t, o);
    let tier = ctx.tier;
    let mut out: Vec<Fill> = Vec::new();
    for p in &r.parts {
        parts::draw(p, &ctx, &mut out);
    }
    out.extend(
        kit::celebrate_burst(
            &ctx.pose,
            o,
            tier,
            r.burst_at,
            r.burst_r,
            [
                ctx.colour(r.burst_colors[0]),
                ctx.colour(r.burst_colors[1]),
                ctx.colour(r.burst_colors[2]),
            ],
        )
        .into_iter()
        .map(|f| ctx.place(f, r.burst_space, None)),
    );
    kit::finish(out, o)
}

/// Where a slot is this frame, in the frame's units: a cosmetic (1.13) attached
/// there is drawn at `(x, y)`, scaled by `scale` and turned by `angle` radians.
// Slots are resolved and tested in 1.12; 1.13's cosmetics are their first caller.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct SlotAt {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub angle: f64,
}

/// Every slot of `r` this frame. A slot follows its chain: `head`/`body` take
/// the rig's pose (tilt, bob, lean, squash, sway); `face` is also wrapped onto
/// the recipe's `face` surface when the head turns. Never drawn in 1.12.
#[allow(dead_code)]
pub fn slots_recipe(r: &Recipe, size: f64, t: f64, o: &ModeOpts) -> Vec<SlotAt> {
    let ctx = setup(r, size, t, o);
    r.slots
        .iter()
        .map(|s| {
            let at = geom::pt(s.at.0, s.at.1);
            let face = r.surfaces.iter().find(|(n, _)| n == "face").map(|(_, f)| f);
            let at = match (s.follows, face) {
                (Space::Face, Some(f)) if !ctx.tn.is_zero() => ctx.tn.map(f, &at),
                _ => at,
            };
            let xf = match s.follows {
                Space::Ground => ctx.ground,
                Space::Whole => ctx.whole,
                Space::Mount => ctx.mount,
                Space::Body | Space::Face => ctx.body,
            };
            let p = xf.apply(&at);
            let q = xf.apply(&geom::pt(at.x + 1.0, at.y));
            let (dx, dy) = (q.x - p.x, q.y - p.y);
            SlotAt {
                name: s.name.clone(),
                x: p.x,
                y: p.y,
                scale: s.scale * dx.hypot(dy),
                angle: s.angle + dy.atan2(dx),
            }
        })
        .collect()
}

/// The built-in character `mode`'s slots (None when it isn't one).
#[allow(dead_code)]
pub fn slots(mode: &str, size: f64, t: f64, o: &ModeOpts) -> Option<Vec<SlotAt>> {
    recipes().get(mode).map(|r| slots_recipe(r, size, t, o))
}

/// The built-in character `mode`'s frame (None when it isn't one).
pub fn frame(mode: &str, size: f64, t: f64, o: &ModeOpts) -> Option<OrbFrame> {
    recipes().get(mode).map(|r| frame_recipe(r, size, t, o))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    /// Poses that reach every part and branch: rest, listening, thinking,
    /// speaking, the turn, effects, mute, the options.
    fn poses() -> Vec<ModeOpts> {
        let mut v = vec![
            opts(&[]),
            opts(&[
                ("tilt", -0.2),
                ("earGain", 1.0),
                ("audioLevel", 0.7),
                ("lean", 3.0),
            ]),
            opts(&[
                ("mouthDots", 1.0),
                ("turnBlink", 1.0),
                ("stateAge", 0.09),
                ("gazeX", -8.0),
            ]),
            opts(&[
                ("mouthTalk", 1.0),
                ("mouthGain", 1.0),
                ("audioLevel", 0.8),
                ("flutterGain", 0.3),
                ("bounceGain", 3.0),
                ("squashGain", 0.04),
                ("swayGain", 0.04),
                ("curlGain", 0.8),
            ]),
            opts(&[("mouthTalk", 0.4), ("mouthDots", 0.3), ("audioLevel", 0.5)]),
            opts(&[("turnYaw", -0.7), ("turnPitch", 0.8), ("turnWander", 1.0)]),
            opts(&[
                ("turn", 1.0),
                ("turnYaw", 0.6),
                ("turnNod", 1.0),
                ("audioLevel", 0.9),
            ]),
            opts(&[("effectCode", 1.0), ("effectAge", 0.3)]),
            opts(&[("effectCode", 2.0), ("effectAge", 0.1)]),
            opts(&[("effectCode", 3.0), ("effectAge", 0.5)]),
            opts(&[
                ("effectCode", 3.0),
                ("effectAge", 0.5),
                ("effectReduced", 1.0),
            ]),
            opts(&[("interruptAge", 0.05), ("look", 0.0)]),
            opts(&[("muted", 1.0)]),
            opts(&[
                ("hue", 212.0),
                ("accessories", 0.0),
                ("mouth", 0.0),
                ("seed", 4.0),
            ]),
        ];
        v.push(opts(&[("look", 2.0), ("breath", 1.0), ("seed", 2.0)]));
        v
    }

    #[test]
    fn every_built_in_recipe_parses() {
        assert_eq!(
            recipes()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            RECIPES.iter().map(|(id, _)| id.to_string()).collect()
        );
        for (id, r) in recipes() {
            assert_eq!(&r.id, id);
        }
    }

    #[test]
    fn a_bad_recipe_says_where() {
        let e = Recipe::parse(r#"{ "recipe": 2 }"#).unwrap_err();
        assert!(e.contains("version 2"), "{e}");
        let e = Recipe::parse(r#"{ "recipe": 1, "id": "x", "palette": {}, "hue": { "base": 0, "turns": ["nope"] },
            "rig": { "kind": "pivot", "pivot": [0, 0], "lean": 0 }, "parts": [], "burst": { "at": [0, 0], "r": 1, "colors": ["celebrate", "celebrate", "celebrate"] } }"#)
            .unwrap_err();
        assert!(e.contains("unknown colour `nope`"), "{e}");
        let e = Recipe::parse(r#"{ "recipe": 1, "id": "x", "palette": {}, "hue": { "base": 0, "turns": [] },
            "rig": { "kind": "pivot", "pivot": [0, 0], "lean": 0 }, "parts": [{ "part": "teapot", "space": "body" }],
            "burst": { "at": [0, 0], "r": 1, "colors": ["celebrate", "celebrate", "celebrate"] } }"#)
            .unwrap_err();
        assert!(e.contains("/parts/0") && e.contains("teapot"), "{e}");
    }

    #[test]
    fn every_character_declares_the_four_slots_and_they_follow_the_pose() {
        for id in ["buzzy", "hum", "wisp", "chirp"] {
            let at = |o: &ModeOpts, size: f64| {
                slots(id, size, 1.0, o)
                    .unwrap()
                    .into_iter()
                    .map(|s| (s.name.clone(), s))
                    .collect::<std::collections::BTreeMap<_, _>>()
            };
            let rest = at(&opts(&[("look", 0.0)]), 64.0);
            assert_eq!(
                rest.keys().cloned().collect::<Vec<_>>(),
                ["chest", "face", "headTop", "neck"],
                "{id}"
            );
            // 64 px design units are size / 200: a slot sits inside the frame.
            for s in rest.values() {
                assert!(
                    s.x > 0.0 && s.x < 64.0 && s.y > 0.0 && s.y < 64.0,
                    "{id}: {s:?}"
                );
                assert!((s.scale - 64.0 / 200.0).abs() < 0.02, "{id}: {s:?}");
            }
            // A tilt turns and moves the head's slot; the head turn moves the face's.
            let tilted = at(&opts(&[("look", 0.0), ("tilt", 0.3)]), 64.0);
            assert!(
                (tilted["headTop"].angle - rest["headTop"].angle).abs() > 0.2,
                "{id}"
            );
            assert!(
                (tilted["headTop"].x - rest["headTop"].x).abs() > 0.5,
                "{id}"
            );
            let turned = at(&opts(&[("look", 0.0), ("turnYaw", 1.0)]), 64.0);
            assert!(
                turned["face"].x - rest["face"].x > 0.5,
                "{id}: the face slot turns right"
            );
            // At 20 px they are still there (the turn is off, the pose is not).
            assert_eq!(at(&opts(&[]), 20.0).len(), 4, "{id}");
        }
    }

    /// Chirp's recipe with `extra` merged into its part `i`.
    fn chirp_with(i: usize, extra: serde_json::Value) -> Result<Recipe, String> {
        let mut v: serde_json::Value =
            serde_json::from_str(RECIPES.iter().find(|(id, _)| *id == "chirp").unwrap().1).unwrap();
        let part = v["parts"][i].as_object_mut().unwrap();
        for (k, x) in extra.as_object().unwrap() {
            part.insert(k.clone(), x.clone());
        }
        Recipe::parse(&v.to_string())
    }

    #[test]
    fn show_fades_a_part_by_voice_state_and_its_absence_changes_nothing() {
        let plain = chirp_with(0, serde_json::json!({})).unwrap();
        assert_eq!(
            &plain,
            &recipes()["chirp"],
            "a round trip is the same recipe"
        );
        // The crest (part 2) only while listening.
        let r = chirp_with(2, serde_json::json!({ "show": { "listening": 1 } })).unwrap();
        let teal = r.colour_named("teal").unwrap();
        let crest_alpha = |o: &ModeOpts| -> f64 {
            frame_recipe(&r, 64.0, 0.4, o)
                .fills
                .iter()
                .filter(|f| f.hue == teal.h && f.points.len() > 30)
                .map(|f| f.a)
                .fold(0.0, f64::max)
        };
        assert_eq!(crest_alpha(&opts(&[("look", 0.0)])), 0.0, "idle: gone");
        assert!((crest_alpha(&opts(&[("look", 0.0), ("earGain", 1.0)])) - 1.0).abs() < 1e-12);
        let half = crest_alpha(&opts(&[("look", 0.0), ("earGain", 0.5)]));
        assert!(half > 0.4 && half < 0.6, "mid-change: {half}");
        let e = chirp_with(2, serde_json::json!({ "show": { "dancing": 1 } })).unwrap_err();
        assert!(e.contains("not a voice state"), "{e}");
    }

    #[test]
    fn a_path_shape_reads_with_the_part() {
        let r = chirp_with(
            3,
            serde_json::json!({ "shape": { "path": "M40 60 C40 20 160 20 160 60 L160 150 L40 150 Z" } }),
        )
        .unwrap();
        let s = &r.parts[3].params.shapes;
        assert_eq!(s.len(), 1);
        assert!(s[0].path && s[0].outer.len() > 10);
        let e = chirp_with(3, serde_json::json!({ "shape": { "path": 5 } })).unwrap_err();
        assert_eq!(e, "/parts/3/shape/path: expected an SVG path string");
    }

    #[test]
    fn every_recipe_in_every_pose_is_deterministic_fills_only_and_in_its_box() {
        for (id, r) in recipes() {
            for size in [20.0, 32.0, 64.0] {
                for o in poses() {
                    let f = frame_recipe(r, size, 1.3, &o);
                    assert_eq!(f, frame_recipe(r, size, 1.3, &o), "{id}");
                    assert_eq!(f.color_mode, crate::primitives::ColorMode::Fixed, "{id}");
                    assert!(
                        f.dots.is_empty() && f.lines.is_empty() && f.polylines.is_empty(),
                        "{id}"
                    );
                    for p in f.fills.iter().flat_map(|x| x.points.iter()) {
                        assert!(
                            p.x > -0.05 * size
                                && p.y > -0.05 * size
                                && p.x < 1.05 * size
                                && p.y < 1.05 * size,
                            "{id} {size}: {p:?} {o:?}"
                        );
                    }
                }
            }
        }
    }

    /// Every built-in recipe's frames, written (`SINUA_RECIPE_SNAPSHOT=write:<path>`)
    /// or compared (`=check:<path>`): a refactor of the reader or the parts must
    /// draw bit for bit the same (`{:?}` prints floats exactly).
    #[test]
    #[ignore = "a refactor check, run on purpose"]
    fn recipe_snapshot() {
        let Ok(arg) = std::env::var("SINUA_RECIPE_SNAPSHOT") else {
            return;
        };
        let mut text = String::new();
        let mut ids: Vec<_> = recipes().keys().cloned().collect();
        ids.sort();
        for id in ids {
            for size in [20.0, 32.0, 64.0] {
                for t in [0.0, 0.6, 1.7, 3.3, 5.1] {
                    for (k, o) in poses().into_iter().enumerate() {
                        text.push_str(&format!(
                            "{id} {size} {t} {k} {:?}\n",
                            frame(&id, size, t, &o).unwrap()
                        ));
                    }
                }
            }
        }
        let (mode, path) = arg.split_once(':').unwrap();
        match mode {
            "write" => std::fs::write(path, text).unwrap(),
            _ => {
                // Every frame in the old snapshot, bit for bit; a recipe added
                // since only adds lines.
                let old = std::fs::read_to_string(path).unwrap();
                let now: std::collections::HashSet<&str> = text.lines().collect();
                let bad = old.lines().filter(|l| !now.contains(l)).count();
                assert_eq!(bad, 0, "{bad} of {} frames differ", old.lines().count());
            }
        }
    }
}
