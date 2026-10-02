//! The character part library (design note 11). A part is a piece of
//! behaviour with its numbers left open: a recipe says where, how big, what
//! colour and how much. Each part draws exactly what the hand-written character
//! code drew, in the same order of operations, so a recipe reproduces it byte
//! for byte.
//!
//! **One table-driven reader.** Every part kind has a [`schema`]: its fields in
//! order, each with a type. [`parse`] reads any part through its schema into
//! flat [`Params`] (numbers, palette indices, names), checks every field and
//! reports unknown ones with their JSON pointer; a part's draw function reads
//! the same fields back in the same order through a [`Reader`]. One small
//! reader instead of one per part keeps the engine lean (the user's call,
//! 2026-10-01: the per-part readers cost ~27 KB gzip).
//!
//! - [`common`]: shadow, body (light, turn light, inner layers, outline), eyes.
//! - [`bird`]: CHIRP's feet, crest, wings, beak, notes and thought dots.
//! - [`mic`]: HUM's stand, yoke, grille (a body layer) and tally light.
//! - [`spirit`]: WISP's halo, smoky body, oval mouth and sparkles.
//! - [`ranger`]: BUZZY's torso, chest core, ear pods, helmet, fin and face screen.
//! - [`arms`]: two arms and hands that follow the voice state (BEEP).
//! - [`steam`]: CUPPA's steam, soft wisps that rise and fade (any cup, candle or chimney).

pub mod arms;
pub mod bird;
pub mod common;
pub mod mic;
pub mod ranger;
pub mod spirit;
pub mod steam;

use serde_json::{Map, Value};

use crate::character::geom;
use crate::character::recipe::{Ctx, Space};
use crate::character::region::Shape;
use crate::primitives::Fill;

/// A field's type in a part's schema.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    /// One number.
    N,
    /// Exactly this many numbers.
    V(usize),
    /// A list of numbers (any length).
    L,
    /// A list of `[a, b]` pairs (any length).
    Pairs,
    /// This many `[x, y]` points.
    Pts(usize),
    /// A palette colour name.
    C,
    /// This many palette colour names.
    Cs(usize),
    /// A name (an opts key a part reads, e.g. `flutterGain`).
    S,
    /// A surface name, or none.
    Surf,
    /// A flag, false when absent.
    B,
    /// An eye style ([`EYE_STYLES`], design note 24), `shape` when absent.
    Eye,
    /// A palette colour name, or none.
    OptC,
    /// A number or `[at 32/64, at 20]`.
    NumOrPair,
    /// A shape: `{ ellipse: [cx, cy, rx, ry, rot, n] }`, `{ roundRect: [x, y, w, h, r, step] }`
    /// or `{ path: "M… Z" }` (an SVG path; later subpaths are holes).
    Shape,
    /// Gradient stops: `[[offset, colour], …]`, each with an optional alpha
    /// (`[offset, colour, alpha]`, 0–1; a soft mass fades to 0).
    Stops,
    /// A body's light: `{ radial: [cx, cy, r] }` or `[cx, cy, rx, ry, angle]` (an
    /// ellipse; + the part's `turnLight`), or `{ linear: [x0, y0, x1, y1] }`.
    Light,
    /// A body's inner layers (parts of the layer kinds).
    Inner,
}

/// The part kinds and the body layer kinds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Shadow,
    Body,
    Eyes,
    Feet,
    Crest,
    Wings,
    Beak,
    Notes,
    Stand,
    Yoke,
    Tally,
    Halo,
    Spirit,
    OvalMouth,
    Sparkles,
    Torso,
    ChestCore,
    EarPods,
    Helmet,
    Fin,
    FaceScreen,
    Steam,
    Arms,
    // Body layers.
    Patch,
    Band,
    Stripes,
    Glints,
    Grille,
    Shade,
    Rim,
}

pub(crate) const KINDS: [(&str, Kind); 30] = [
    ("shadow", Kind::Shadow),
    ("body", Kind::Body),
    ("eyes", Kind::Eyes),
    ("feet", Kind::Feet),
    ("crest", Kind::Crest),
    ("wings", Kind::Wings),
    ("beak", Kind::Beak),
    ("notes", Kind::Notes),
    ("stand", Kind::Stand),
    ("yoke", Kind::Yoke),
    ("tally", Kind::Tally),
    ("halo", Kind::Halo),
    ("spirit", Kind::Spirit),
    ("ovalMouth", Kind::OvalMouth),
    ("sparkles", Kind::Sparkles),
    ("torso", Kind::Torso),
    ("chestCore", Kind::ChestCore),
    ("earPods", Kind::EarPods),
    ("helmet", Kind::Helmet),
    ("fin", Kind::Fin),
    ("faceScreen", Kind::FaceScreen),
    ("steam", Kind::Steam),
    ("arms", Kind::Arms),
    ("patch", Kind::Patch),
    ("band", Kind::Band),
    ("stripes", Kind::Stripes),
    ("glints", Kind::Glints),
    ("grille", Kind::Grille),
    ("shade", Kind::Shade),
    ("rim", Kind::Rim),
];

/// Is `k` a body layer (only inside a body's `inner`)? The eyes may be either.
pub(crate) fn is_layer(k: Kind) -> bool {
    matches!(
        k,
        Kind::Patch
            | Kind::Band
            | Kind::Stripes
            | Kind::Glints
            | Kind::Grille
            | Kind::Shade
            | Kind::Rim
    )
}

use Ty::*;

/// A part kind's fields, in the order its draw function reads them.
pub fn schema(k: Kind) -> &'static [(&'static str, Ty)] {
    match k {
        Kind::Shadow => &[("at", V(2)), ("r", V(2)), ("floats", B)],
        Kind::Body => &[
            ("shape", Shape),
            ("light", Light),
            ("light.stops", Stops),
            ("inner", Inner),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::Eyes => &[
            ("at", V(2)),
            ("scale", V(2)),
            ("ink", C),
            ("glow", NumOrPair),
            ("surface", Surf),
            ("style", Eye),
            ("iris", OptC),
            ("sclera", B),
        ],
        Kind::Feet => &[
            ("x", N),
            ("gap", N),
            ("top", N),
            ("ground", N),
            ("toe", V(2)),
            ("width", N),
            ("color", C),
        ],
        Kind::Crest => &[
            ("base", V(2)),
            ("spread", L),
            ("mid", N),
            ("root", N),
            ("ctrl", V(4)),
            ("tip", V(5)),
            ("lift.ears", V(2)),
            ("lift.talk", V(2)),
            ("lift.dots", N),
            ("turnShift", N),
            ("segments", N),
            ("width", N),
            ("color", C),
        ],
        Kind::Wings => &[
            ("x", N),
            ("at", V(2)),
            ("size", V(2)),
            ("angle", N),
            ("segments", N),
            ("flutter.rate", N),
            ("flutter.gain", S),
            ("surface", Surf),
            ("turn.near", N),
            ("turn.far", N),
            ("color", C),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::Beak => &[
            ("lower", Pts(3)),
            ("drop", N),
            ("upper.quad", Pts(3)),
            ("upper.segments", N),
            ("upper.tip", V(2)),
            ("upper.rise", N),
            ("open.effect", N),
            ("open.rest", N),
            ("colors", Cs(2)),
            ("outline.width", N),
            ("outline.color", C),
            ("surface", Surf),
        ],
        Kind::Notes => &[
            ("count", N),
            ("rate", N),
            ("from", V(2)),
            ("travel", V(2)),
            ("stagger", N),
            ("scale", V(2)),
            ("floor", N),
            ("color", C),
            ("dots.at", V(2)),
            ("dots.step", V(2)),
            ("dots.r", V(2)),
            ("dots.segments", N),
            ("dots.rate", N),
            ("dots.base", N),
            ("dots.gain", N),
        ],
        Kind::Stand => &[
            ("under", V(4)),
            ("foot", V(4)),
            ("segments", N),
            ("stem", V(6)),
            ("dark", C),
            ("color", C),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::Yoke => &[
            ("x", V(2)),
            ("top", N),
            ("knee", N),
            ("bottom", N),
            ("mid", N),
            ("segments", N),
            ("under.width", N),
            ("under.color", C),
            ("over.width", N),
            ("over.color", C),
        ],
        Kind::Tally => &[
            ("at", V(2)),
            ("r", N),
            ("segments", N),
            ("off", C),
            ("listening", C),
            ("thinking", C),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::Halo => &[
            ("at", V(2)),
            ("r", N),
            ("segments", N),
            ("color", C),
            ("stops", Pairs),
        ],
        Kind::Spirit => &[
            ("head", V(3)),
            ("stops", Stops),
            ("line", C),
            ("smoke", C),
            ("shine", C),
            ("tail.lag", N),
            ("tail.swing", N),
        ],
        Kind::OvalMouth => &[
            ("at", V(2)),
            ("scale", V(2)),
            ("halfWidth", N),
            ("ink", C),
            ("surface", Surf),
        ],
        Kind::Sparkles => &[("count", N), ("head", V(2)), ("colors", Cs(2))],
        Kind::Torso => &[
            ("shades", Cs(3)),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::ChestCore => &[("at", V(2)), ("r", N), ("bars", L), ("colors", Cs(3))],
        Kind::EarPods => &[
            ("x", N),
            ("reach", N),
            ("front", B),
            ("pod", C),
            ("line", C),
            ("chevron", C),
            ("arcs", C),
        ],
        Kind::Helmet => &[("shades", Cs(3)), ("line", C)],
        Kind::Fin => &[("color", C), ("line", C)],
        Kind::FaceScreen => &[
            ("screen", C),
            ("ink", C),
            ("line", C),
            ("glass", C),
            ("glassEdge", C),
            ("glint", C),
            ("surface", Surf),
            ("style", Eye),
            ("iris", OptC),
            ("sclera", B),
        ],
        Kind::Steam => &[
            ("at", V(2)),
            ("count", N),
            ("spread", N),
            ("rise", N),
            ("width", N),
            ("sway", N),
            ("rate", N),
            ("blur", N),
            ("color", C),
            ("alpha", N),
        ],
        Kind::Arms => &[
            ("shoulder", V(2)),
            ("mirror", N),
            ("length", V(2)),
            ("width", N),
            ("hand", N),
            ("ear", V(2)),
            ("chin", V(2)),
            ("back", B),
            ("color", C),
            ("handColor", C),
            ("outline.width", N),
            ("outline.color", C),
        ],
        Kind::Patch => &[("shape", Shape), ("surface", Surf), ("color", C)],
        Kind::Band => &[("shape", Shape), ("color", C)],
        Kind::Shade => &[
            ("shape", Shape),
            ("surface", Surf),
            ("light", Light),
            ("light.stops", Stops),
        ],
        Kind::Rim => &[("offset", V(2)), ("color", C), ("alpha", N), ("fade", N)],
        Kind::Stripes => &[("at", L), ("rect", V(5)), ("color", C), ("alpha", N)],
        Kind::Glints => &[("at", Pairs), ("rect", V(4)), ("color", C), ("alpha", N)],
        Kind::Grille => &[
            ("count", N),
            ("x", N),
            ("pitch", N),
            ("y", N),
            ("slot", V(3)),
            ("smallSlot", V(3)),
            ("rise", V(2)),
            ("step", N),
            ("ink", C),
            ("surface", Surf),
        ],
    }
}

/// When a part draws at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum When {
    Always,
    /// Unless the character is 20 px with `accessories` off.
    NotSmallOrAccessories,
}

/// A part's fields, flat, in its schema's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    pub nums: Vec<f64>,
    pub cols: Vec<usize>,
    pub names: Vec<String>,
    /// Path shapes, read once (`character/path.rs`); a `Shape` field points here.
    pub shapes: Vec<Shape>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub kind: Kind,
    pub space: Space,
    pub when: When,
    /// How visible the part is in each voice state (idle, listening, thinking,
    /// speaking); `None` = always, at its own alpha. The states' weights come
    /// from the pose, so a state change fades the part in or out.
    pub show: Option<[f64; 4]>,
    pub params: Params,
    /// A body's inner layers.
    pub inner: Vec<Part>,
}

/// What a recipe's parts resolve names against.
pub struct Names<'a> {
    pub colours: &'a [String],
    pub surfaces: &'a [String],
    pub slots: &'a [String],
}

/// Reads a part's fields back in its schema's order.
pub struct Reader<'a> {
    p: &'a Params,
    n: usize,
    c: usize,
    s: usize,
}

impl<'a> Reader<'a> {
    pub fn n(&mut self) -> f64 {
        self.n += 1;
        self.p.nums[self.n - 1]
    }
    pub fn u(&mut self) -> usize {
        self.n() as usize
    }
    pub fn b(&mut self) -> bool {
        self.n() != 0.0
    }
    pub fn p2(&mut self) -> (f64, f64) {
        (self.n(), self.n())
    }
    pub fn v<const K: usize>(&mut self) -> [f64; K] {
        let mut a = [0.0; K];
        for x in a.iter_mut() {
            *x = self.n();
        }
        a
    }
    /// A list: its length, then its numbers.
    pub fn list(&mut self) -> &'a [f64] {
        let len = self.u();
        let s = &self.p.nums[self.n..self.n + len];
        self.n += len;
        s
    }
    pub fn pairs(&mut self) -> Vec<(f64, f64)> {
        let len = self.u();
        (0..len).map(|_| self.p2()).collect()
    }
    pub fn col(&mut self) -> usize {
        self.c += 1;
        self.p.cols[self.c - 1]
    }
    pub fn cols<const K: usize>(&mut self) -> [usize; K] {
        let mut a = [0; K];
        for x in a.iter_mut() {
            *x = self.col();
        }
        a
    }
    pub fn name(&mut self) -> &'a str {
        self.s += 1;
        &self.p.names[self.s - 1]
    }
    /// A surface index, or none.
    /// An optional colour (`OptC`): `None` when absent.
    pub fn opt_col(&mut self) -> Option<usize> {
        Some(self.col()).filter(|i| *i != usize::MAX)
    }
    pub fn surf(&mut self) -> Option<usize> {
        let i = self.n();
        (i >= 0.0).then_some(i as usize)
    }
    /// A shape: an ellipse or roundRect (convex), or a path read with the recipe.
    pub fn shape(&mut self) -> Shape {
        let [k, a, b, c, d, e, f] = self.v::<7>();
        if k == 2.0 {
            return self.p.shapes[a as usize].clone();
        }
        Shape::plain(if k == 0.0 {
            geom::ellipse(a, b, c, d, e, f as usize)
        } else {
            geom::round_rect(a, b, c, d, e, f)
        })
    }
    /// Gradient stops as (offset, palette index, alpha).
    pub fn stops(&mut self) -> Vec<(f64, usize, f64)> {
        let len = self.u();
        (0..len)
            .map(|_| (self.n(), self.n(), self.col()))
            .map(|(o, a, c)| (o, c, a))
            .collect()
    }
}

impl Params {
    pub fn read(&self) -> Reader<'_> {
        Reader {
            p: self,
            n: 0,
            c: 0,
            s: 0,
        }
    }
}

/// The value at a dotted path in a part object.
fn at_path<'v>(o: &'v Map<String, Value>, path: &str) -> Option<&'v Value> {
    let mut cur: Option<&Value> = None;
    for (i, seg) in path.split('.').enumerate() {
        cur = if i == 0 { o.get(seg) } else { cur?.get(seg) };
    }
    cur
}

fn num(v: &Value, at: &str) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("{at}: expected a number"))
}

fn arr<'v>(v: &'v Value, at: &str, len: Option<usize>) -> Result<&'v Vec<Value>, String> {
    match v.as_array() {
        Some(a) if len.is_none_or(|n| a.len() == n) => Ok(a),
        _ => Err(match len {
            Some(n) => format!("{at}: expected {n} values"),
            None => format!("{at}: expected an array"),
        }),
    }
}

/// Pushes the numbers of an array of exactly `len` (or any, `None`) numbers.
fn nums(v: &Value, at: &str, len: Option<usize>, out: &mut Vec<f64>) -> Result<(), String> {
    for x in arr(v, at, len)? {
        out.push(num(x, at)?);
    }
    Ok(())
}

fn index_of(names: &[String], v: &Value, at: &str, what: &str) -> Result<usize, String> {
    let s = v
        .as_str()
        .ok_or_else(|| format!("{at}: expected a {what} name"))?;
    names
        .iter()
        .position(|n| n == s)
        .ok_or_else(|| format!("{at}: unknown {what} `{s}`"))
}

/// The eye styles (design note 24), in `face::Face::style` order.
pub const EYE_STYLES: [&str; 4] = ["shape", "glossy", "pixel", "dot"];

/// The most entries a list field (feathers, bars, stripes, glints, stops) may hold.
pub const MAX_LIST: usize = 32;

/// An array of at most [`MAX_LIST`] entries.
fn list<'v>(v: &'v Value, at: &str) -> Result<&'v Vec<Value>, String> {
    let a = arr(v, at, None)?;
    if a.len() > MAX_LIST {
        return Err(format!("{at}: {} entries, at most {MAX_LIST}", a.len()));
    }
    Ok(a)
}

/// `x` must be a whole number in `lo..=hi`.
fn whole(x: f64, lo: f64, hi: f64, at: &str) -> Result<(), String> {
    if x.fract() != 0.0 || x < lo || x > hi {
        return Err(format!("{at}: expected a whole number from {lo} to {hi}"));
    }
    Ok(())
}

/// The fields that are a rounded rectangle's edge step (and where in the field).
const STEPS: [(Kind, &str, usize); 4] = [
    (Kind::Stand, "stem", 5),
    (Kind::Stripes, "rect", 4),
    (Kind::Glints, "rect", 3),
    (Kind::Grille, "step", 0),
];

/// The limits that keep any recipe cheap to draw (design note 12): segments
/// 3–128, counted things 1–24, edge steps at least 1.
fn limit(k: Kind, f: &str, got: &[f64], at: &str) -> Result<(), String> {
    if f == "segments" || f.ends_with(".segments") {
        whole(got[0], 3.0, 128.0, at)?;
    }
    if f == "count" {
        whole(got[0], 1.0, 24.0, at)?;
    }
    for (kind, name, i) in STEPS {
        if kind == k && name == f && got[i] < 1.0 {
            let at = if f == "step" {
                at.to_string()
            } else {
                format!("{at}/{i}")
            };
            return Err(format!("{at}: a step of at least 1"));
        }
    }
    Ok(())
}

/// Reads one field of type `ty` from `v` (`None` = absent) into `p`.
fn field(
    ty: Ty,
    v: Option<&Value>,
    at: &str,
    o: &Map<String, Value>,
    names: &Names,
    p: &mut Params,
    inner: &mut Vec<Part>,
) -> Result<(), String> {
    let need = || v.ok_or_else(|| format!("{at}: missing"));
    match ty {
        N => p.nums.push(num(need()?, at)?),
        V(k) => nums(need()?, at, Some(k), &mut p.nums)?,
        L => {
            let a = list(need()?, at)?;
            p.nums.push(a.len() as f64);
            nums(need()?, at, None, &mut p.nums)?;
        }
        Pairs => {
            let a = list(need()?, at)?;
            p.nums.push(a.len() as f64);
            for x in a {
                nums(x, at, Some(2), &mut p.nums)?;
            }
        }
        Pts(k) => {
            for x in arr(need()?, at, Some(k))? {
                nums(x, at, Some(2), &mut p.nums)?;
            }
        }
        C => p.cols.push(index_of(names.colours, need()?, at, "colour")?),
        Cs(k) => {
            for x in arr(need()?, at, Some(k))? {
                p.cols.push(index_of(names.colours, x, at, "colour")?);
            }
        }
        S => p.names.push(
            need()?
                .as_str()
                .ok_or_else(|| format!("{at}: expected a name"))?
                .to_string(),
        ),
        Surf => p.nums.push(match v {
            None => -1.0,
            Some(x) => index_of(names.surfaces, x, at, "surface")? as f64,
        }),
        Eye => p.nums.push(match v {
            None => 0.0,
            Some(x) => x
                .as_str()
                .and_then(|s| EYE_STYLES.iter().position(|n| *n == s))
                .ok_or_else(|| format!("{at}: expected one of {}", EYE_STYLES.join(", ")))?
                as f64,
        }),
        OptC => p.cols.push(match v {
            None => usize::MAX,
            Some(x) => index_of(names.colours, x, at, "colour")?,
        }),
        B => p.nums.push(match v.map(Value::as_bool) {
            None => 0.0,
            Some(Some(b)) => f64::from(u8::from(b)),
            Some(None) => return Err(format!("{at}: expected true or false")),
        }),
        NumOrPair => match need()? {
            x if x.is_array() => nums(x, at, Some(2), &mut p.nums)?,
            x => {
                let g = num(x, at)?;
                p.nums.extend([g, g]);
            }
        },
        Shape => {
            let s = need()?;
            if let Some(d) = s.get("path") {
                let d = d
                    .as_str()
                    .ok_or_else(|| format!("{at}/path: expected an SVG path string"))?;
                let shape = crate::character::path::parse(d, &format!("{at}/path"))?;
                p.nums
                    .extend([2.0, p.shapes.len() as f64, 0.0, 0.0, 0.0, 0.0, 0.0]);
                p.shapes.push(shape);
                return Ok(());
            }
            match (s.get("ellipse"), s.get("roundRect")) {
                (Some(e), _) => {
                    p.nums.push(0.0);
                    nums(e, at, Some(6), &mut p.nums)?;
                    whole(
                        p.nums[p.nums.len() - 1],
                        3.0,
                        128.0,
                        &format!("{at}/ellipse/5"),
                    )?;
                }
                (None, Some(r)) => {
                    p.nums.push(1.0);
                    nums(r, at, Some(6), &mut p.nums)?;
                    if p.nums[p.nums.len() - 1] < 1.0 {
                        return Err(format!("{at}/roundRect/5: a step of at least 1"));
                    }
                }
                _ => return Err(format!("{at}: expected ellipse, roundRect or path")),
            }
        }
        Stops => {
            // A body with `"light": "none"` (an overlay of layers) has no stops.
            if v.is_none() && o.get("light").and_then(Value::as_str) == Some("none") {
                p.nums.push(0.0);
                return Ok(());
            }
            let a = list(need()?, at)?;
            p.nums.push(a.len() as f64);
            for x in a {
                let s = arr(x, at, None)?;
                if s.len() != 2 && s.len() != 3 {
                    return Err(format!(
                        "{at}: expected [offset, colour] or [offset, colour, alpha]"
                    ));
                }
                p.nums.push(num(&s[0], at)?);
                p.nums.push(s.get(2).map_or(Ok(1.0), |x| num(x, at))?);
                p.cols.push(index_of(names.colours, &s[1], at, "colour")?);
            }
        }
        Light => {
            let l = need()?;
            if l.as_str() == Some("none") {
                p.nums.push(2.0);
                return Ok(());
            }
            match (l.get("radial"), l.get("linear")) {
                (Some(r), _) => {
                    p.nums.push(0.0);
                    let from = p.nums.len();
                    nums(r, at, None, &mut p.nums)?;
                    match p.nums.len() - from {
                        3 => p.nums.extend([p.nums[from + 2], 0.0]),
                        5 => {}
                        _ => {
                            return Err(format!(
                                "{at}/radial: expected [cx, cy, r] or [cx, cy, rx, ry, angle]"
                            ))
                        }
                    }
                    match o.get("turnLight") {
                        None => p.nums.extend([0.0, 0.0, 0.0]),
                        Some(t) => {
                            p.nums.push(1.0);
                            nums(t, at, Some(2), &mut p.nums)?;
                        }
                    }
                }
                (None, Some(r)) => {
                    p.nums.push(1.0);
                    nums(r, at, Some(4), &mut p.nums)?;
                }
                _ => return Err(format!("{at}: expected radial or linear")),
            }
        }
        Inner => {
            if let Some(a) = v {
                for (i, x) in list(a, at)?.iter().enumerate() {
                    inner.push(parse_any(x, &format!("{at}/{i}"), names, true, None)?);
                }
            }
        }
    }
    Ok(())
}

/// May a part of kind `k` hold the dotted key `path`?
fn allowed(path: &str, k: Kind) -> bool {
    matches!(path, "part" | "space" | "when" | "show")
        || (k == Kind::Body && path == "turnLight")
        || schema(k)
            .iter()
            .any(|(f, _)| *f == path || f.strip_prefix(path).is_some_and(|r| r.starts_with('.')))
}

/// Reports a key the schema doesn't know (sub-objects of dotted fields too).
fn check_keys(o: &Map<String, Value>, prefix: &str, k: Kind, at: &str) -> Result<(), String> {
    for (key, v) in o {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if !allowed(&path, k) {
            return Err(format!("{at}/{}: unknown field", path.replace('.', "/")));
        }
        let leaf = schema(k).iter().any(|(f, _)| *f == path);
        if let (false, Some(sub)) = (leaf || path == "show", v.as_object()) {
            check_keys(sub, &path, k, at)?;
        }
    }
    Ok(())
}

fn parse_any(
    v: &Value,
    at: &str,
    names: &Names,
    layer: bool,
    slot: Option<Space>,
) -> Result<Part, String> {
    let o = v
        .as_object()
        .ok_or_else(|| format!("{at}: expected an object"))?;
    let name = o
        .get("part")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}/part: expected a part name"))?;
    let kind = KINDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
        .filter(|k| is_layer(*k) == layer || *k == Kind::Eyes)
        .ok_or_else(|| format!("{at}/part: unknown part `{name}`"))?;
    check_keys(o, "", kind, at)?;
    let space = match (o.get("space").and_then(Value::as_str), layer) {
        (Some(_), _) if slot.is_some() => {
            return Err(format!("{at}/space: a cosmetic draws in its slot"))
        }
        (None, false) if slot.is_some() => slot.unwrap(),
        (Some(s), _) => match s.strip_prefix("slot:") {
            Some(n) => Space::Slot(
                names
                    .slots
                    .iter()
                    .position(|x| x == n)
                    .ok_or_else(|| format!("{at}/space: no slot `{n}`"))? as u8,
            ),
            None => Space::parse(s).map_err(|e| format!("{at}/space: {e}"))?,
        },
        (None, true) => Space::Body,
        (None, false) => return Err(format!("{at}/space: missing")),
    };
    let when = match o.get("when").and_then(Value::as_str) {
        None if slot.is_some() => When::NotSmallOrAccessories,
        None => When::Always,
        Some("notSmallOrAccessories") => When::NotSmallOrAccessories,
        Some(w) => return Err(format!("{at}/when: unknown condition `{w}`")),
    };
    let show = match o.get("show") {
        None => None,
        Some(s) => {
            let so = s
                .as_object()
                .ok_or_else(|| format!("{at}/show: expected an object"))?;
            let mut w = [0.0; 4];
            for (key, x) in so {
                let i = ["idle", "listening", "thinking", "speaking"]
                    .iter()
                    .position(|n| n == key)
                    .ok_or_else(|| format!("{at}/show/{key}: not a voice state"))?;
                w[i] = num(x, &format!("{at}/show/{key}"))?;
            }
            Some(w)
        }
    };
    let mut params = Params::default();
    let mut inner = Vec::new();
    for (f, ty) in schema(kind) {
        let fat = format!("{at}/{}", f.replace('.', "/"));
        let from = params.nums.len();
        field(*ty, at_path(o, f), &fat, o, names, &mut params, &mut inner)?;
        limit(kind, f, &params.nums[from..], &fat)?;
    }
    Ok(Part {
        kind,
        space,
        when,
        show,
        params,
        inner,
    })
}

/// A part from its recipe entry at `at` (a JSON pointer, for the errors).
pub fn parse(v: &Value, at: &str, names: &Names) -> Result<Part, String> {
    parse_any(v, at, names, false, None)
}

/// A cosmetic's part (design note 21): drawn in `slot`, left out at 20 px
/// unless it says otherwise.
pub fn parse_in(v: &Value, at: &str, names: &Names, slot: Space) -> Result<Part, String> {
    parse_any(v, at, names, false, Some(slot))
}

/// The voice states' weights in this pose: listening = `earGain`, thinking =
/// the dots mouth, speaking = the talk mouth, idle = what's left.
pub fn state_weights(ctx: &Ctx) -> [f64; 4] {
    let (_, dots, talk, _, _) = ctx.w;
    let ears = ctx.pose.ears;
    [(1.0 - ears - dots - talk).max(0.0), ears, dots, talk]
}

/// Draws `p` into `out`.
pub fn draw(p: &Part, ctx: &Ctx, out: &mut Vec<Fill>) {
    let on = match p.when {
        When::Always => true,
        When::NotSmallOrAccessories => !ctx.tier.small || ctx.accessories,
    };
    if !on {
        return;
    }
    let vis = p.show.map(|s| {
        let w = state_weights(ctx);
        (s[0] * w[0] + s[1] * w[1] + s[2] * w[2] + s[3] * w[3]).clamp(0.0, 1.0)
    });
    if vis == Some(0.0) {
        return;
    }
    let from = out.len();
    let (r, sp) = (&mut p.params.read(), p.space);
    match p.kind {
        Kind::Shadow => common::shadow(r, sp, ctx, out),
        Kind::Body => common::body(r, &p.inner, sp, ctx, out),
        Kind::Eyes => common::eyes(r, sp, ctx, None, out),
        Kind::Feet => bird::feet(r, sp, ctx, out),
        Kind::Crest => bird::crest(r, sp, ctx, out),
        Kind::Wings => bird::wings(r, sp, ctx, out),
        Kind::Beak => bird::beak(r, sp, ctx, out),
        Kind::Notes => bird::notes(r, sp, ctx, out),
        Kind::Stand => mic::stand(r, sp, ctx, out),
        Kind::Yoke => mic::yoke(r, sp, ctx, out),
        Kind::Tally => mic::tally(r, sp, ctx, out),
        Kind::Halo => spirit::halo(r, sp, ctx, out),
        Kind::Spirit => spirit::spirit(r, sp, ctx, out),
        Kind::OvalMouth => spirit::oval_mouth(r, sp, ctx, out),
        Kind::Sparkles => spirit::sparkles(r, sp, ctx, out),
        Kind::Torso => ranger::torso(r, sp, ctx, out),
        Kind::ChestCore => ranger::chest_core(r, sp, ctx, out),
        Kind::EarPods => ranger::ear_pods(r, sp, ctx, out),
        Kind::Helmet => ranger::helmet(r, sp, ctx, out),
        Kind::Fin => ranger::fin(r, sp, ctx, out),
        Kind::FaceScreen => ranger::face_screen(r, sp, ctx, out),
        Kind::Steam => steam::steam(r, sp, ctx, out),
        Kind::Arms => arms::arms(r, sp, ctx, out),
        // Layers draw inside their body.
        Kind::Patch
        | Kind::Band
        | Kind::Stripes
        | Kind::Glints
        | Kind::Grille
        | Kind::Shade
        | Kind::Rim => {}
    }
    if let Some(v) = vis {
        for f in &mut out[from..] {
            f.a *= v;
        }
    }
}
