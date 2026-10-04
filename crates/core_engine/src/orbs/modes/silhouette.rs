//! Silhouette (design note 32): a front-facing head and shoulders made of dots that lives
//! with the voice. One shape on screen; the voice states change only how it moves, through
//! five continuous keys the voice-state profile blends (`breath`, `glint`, `inward`,
//! `neuron`, `speech`). The dots never change with the state, so a state change never
//! makes one appear or vanish (the transition contract, design note 31).
//!
//! - Dots: a jittered candidate grid inside the outline, ordered farthest point first,
//!   computed once per silhouette and cached. Every prefix of the order is spread evenly,
//!   so a smaller size (and low power, `silhouetteDots`) draws a prefix. The density is
//!   even everywhere and the dots are one size (the user, 2026-10-04).
//! - Motion: idle breathes and glints; listening sends waves inward from the outline;
//!   thinking flickers around the eye line; speaking sends waves out from the mouth with
//!   the level (amplitude only).
//! - Head turn (`turnYaw`): each row of the head is remapped within its own span, a
//!   smooth squeeze toward the far side, so no dot leaves the outline; the shoulders stay.
//! - Looks: `wire` links near neighbours, `scanlines` moves bands down, `hologram` brightens
//!   the rim and now and then slides a thin band sideways (the hologram look).
//! - The outline is an open line from the bottom, fading into the base over a soft glow;
//!   at 20 px it is drawn alone.
//!
//! The built-ins live here; a file's own silhouette (`silhouette: { path, eyes, mouth }`,
//! FX Spec 1.13) is [`register`]ed and drawn by id.
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Mutex;

use crate::character::path;
use crate::character::region::inside;
use crate::orbs::core::{finalize_frame, Dot, OrbFrame};
use crate::orbs::profiles::ModeOpts;
use crate::primitives::{cycles, Line, Point, Polyline};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// A silhouette: an outline in a 200 × 200 box (open at the bottom: the path starts and
/// ends below the shoulders) and its landmarks.
#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    pub path: String,
    /// The eye line: left x, right x, y.
    pub eyes: [f64; 3],
    /// The mouth: x, y.
    pub mouth: [f64; 2],
}

/// The built-ins by name; the index is the `silhouetteId`.
pub const NAMES: [&str; 2] = ["human", "helmet"];

fn builtin(id: usize) -> Def {
    let (path, eyes, mouth) = match id {
        1 => (
            "M2 214 C5 194 12 183 24 177 C38 170 58 167 72 162 C79 159 82 155 81 149 C79 143 72 141 64 137 C54 131 49 122 48 108 C47 94 47 66 50 52 C54 30 74 24 100 24 C126 24 146 30 150 52 C153 66 153 94 152 108 C151 122 146 131 136 137 C128 141 121 143 119 149 C118 155 121 159 128 162 C142 167 162 170 176 177 C188 183 195 194 198 214 Z",
            [79.0, 121.0, 86.0],
            [100.0, 122.0],
        ),
        _ => (
            "M2 214 C5 194 12 183 24 177 C38 170 58 168 72 163 C79 160 83 156 84 151 C85 146 85 142 82 139 C74 132 67 121 63 108 C59 96 57 82 59 64 C61 38 78 22 100 22 C122 22 139 38 141 64 C143 82 141 96 137 108 C133 121 126 132 118 139 C115 142 115 146 116 151 C117 156 121 160 128 163 C142 168 162 170 176 177 C188 183 195 194 198 214 Z",
            [81.0, 119.0, 86.0],
            [100.0, 120.0],
        ),
    };
    Def {
        path: path.into(),
        eyes,
        mouth,
    }
}

/// Files' own silhouettes, by id (the first 2 are the built-ins). Shared by every thread
/// so a view can draw what another thread's resolve registered.
static CUSTOM: Mutex<Vec<(u32, Def)>> = Mutex::new(Vec::new());
const CUSTOM_CAP: usize = 16;

/// Checks `def` and registers it; returns its id (the same for the same silhouette), or
/// where the problem is (a JSON pointer in the file) and what it is.
#[inline(never)]
pub fn register(def: Def) -> Result<u32, (&'static str, String)> {
    let at = "/silhouette/path";
    let shape = path::parse(&def.path, "the path").map_err(|e| (at, e))?;
    let fits = |v: f64| (-20.0..=220.0).contains(&v);
    if shape.outer.iter().any(|p| !fits(p.x) || !fits(p.y)) {
        return Err((at, "keep the outline in the 200 × 200 box".into()));
    }
    if def
        .eyes
        .iter()
        .chain(&def.mouth)
        .any(|v| !(0.0..=200.0).contains(v))
    {
        return Err((
            "/silhouette",
            "`eyes` and `mouth` are box units, 0 to 200".into(),
        ));
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in def.path.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    for v in def.eyes.iter().chain(&def.mouth) {
        h = (h ^ (*v * 16.0) as u32).wrapping_mul(0x0100_0193);
    }
    let id = 2 + h % 1_000_000;
    let mut c = CUSTOM.lock().unwrap_or_else(|e| e.into_inner());
    if !c.iter().any(|(k, _)| *k == id) {
        c.push((id, def));
        if c.len() > CUSTOM_CAP {
            c.remove(0);
        }
    }
    Ok(id)
}

/// One dot's fixed data (box units).
struct Seed {
    x: f64,
    y: f64,
    /// Distance to the outline, the mouth and the eye line.
    rim: f64,
    mouth: f64,
    eye: f64,
    /// A per-dot phase 0..1.
    ph: f64,
    /// The outline's span on this dot's row (the head turn remaps within it).
    xl: f64,
    xr: f64,
    /// 1 on the head, 0 on the shoulders (only the head turns).
    head: f64,
}

struct Built {
    outline: Vec<Point>,
    seeds: Vec<Seed>,
    /// Each dot's two nearest earlier dots (wire links), made on first use.
    links: RefCell<Option<Vec<(u32, u32)>>>,
}

/// At most this many dots (64 px draws ~1,515).
const MAX_DOTS: usize = 1600;
/// The candidate grid's step (box units).
const STEP: f64 = 2.4;

fn hash(i: u32) -> f64 {
    let mut x = i.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    f64::from(x % 10_000) / 10_000.0
}

fn seg_dist(p: (f64, f64), a: &Point, b: &Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.0 - a.x) * dx + (p.1 - a.y) * dy) / l2).clamp(0.0, 1.0);
    (p.0 - a.x - t * dx).hypot(p.1 - a.y - t * dy)
}

/// The base fade (box units): full above 168, gone at 206.
fn base(y: f64) -> f64 {
    let t = ((206.0 - y) / 38.0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn def_of(id: u32) -> Def {
    if id < 2 {
        return builtin(id as usize);
    }
    let c = CUSTOM.lock().unwrap_or_else(|e| e.into_inner());
    c.iter()
        .find(|(k, _)| *k == id)
        .map(|(_, d)| d.clone())
        .unwrap_or_else(|| builtin(0))
}

/// The silhouette `id`'s dots, laid out once (cached per thread).
#[inline(never)]
fn built(id: u32) -> Rc<Built> {
    thread_local! {
        static CACHE: RefCell<Vec<(u32, Rc<Built>)>> = const { RefCell::new(Vec::new()) };
    }
    if let Some(b) = CACHE.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, b)| b.clone())
    }) {
        return b;
    }
    let def = def_of(id);
    let outline = path::parse(&def.path, "silhouette")
        .map(|s| s.outer)
        .unwrap_or_default();
    let b = Rc::new(lay_out(&def, outline));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.push((id, b.clone()));
        if c.len() > 6 {
            c.remove(0);
        }
    });
    b
}

#[inline(never)]
fn lay_out(def: &Def, outline: Vec<Point>) -> Built {
    let n = outline.len();
    let rim_of = |p: (f64, f64)| {
        (0..n)
            .map(|i| seg_dist(p, &outline[i], &outline[(i + 1) % n]))
            .fold(f64::MAX, f64::min)
    };
    // Candidates: a jittered grid inside the outline.
    let mut cand: Vec<(f64, f64)> = Vec::new();
    let mut k = 0u32;
    let mut y = 0.0;
    while y < 200.0 {
        let mut x = 0.0;
        while x < 200.0 {
            k += 1;
            let p = (
                x + (hash(k) - 0.5) * STEP,
                y + (hash(k ^ 0x5555) - 0.5) * STEP,
            );
            if n > 2 && inside(&Point { x: p.0, y: p.1 }, &outline) {
                cand.push(p);
            }
            x += STEP;
        }
        y += STEP;
    }
    // Farthest point first: every prefix spreads evenly.
    let m = cand.len();
    let mut best = vec![f64::MAX; m];
    let mut order: Vec<usize> = Vec::with_capacity(m.min(MAX_DOTS));
    let mut next = 0;
    while order.len() < m.min(MAX_DOTS) {
        order.push(next);
        best[next] = -1.0;
        let (px, py) = cand[next];
        let mut far = (0, -1.0);
        for (i, c) in cand.iter().enumerate() {
            if best[i] < 0.0 {
                continue;
            }
            best[i] = best[i].min((c.0 - px).powi(2) + (c.1 - py).powi(2));
            if best[i] > far.1 {
                far = (i, best[i]);
            }
        }
        next = far.0;
    }
    let seeds = order
        .iter()
        .enumerate()
        .map(|(j, &i)| {
            let p = cand[i];
            // The row's span: the outline crossings on either side of the dot.
            let (mut xl, mut xr) = (0.0f64, 200.0f64);
            for e in 0..n {
                let (a, c) = (&outline[e], &outline[(e + 1) % n]);
                if (a.y > p.1) != (c.y > p.1) {
                    let x = a.x + (p.1 - a.y) / (c.y - a.y) * (c.x - a.x);
                    if x <= p.0 {
                        xl = xl.max(x);
                    } else {
                        xr = xr.min(x);
                    }
                }
            }
            Seed {
                x: p.0,
                y: p.1,
                rim: rim_of(p),
                mouth: (p.0 - def.mouth[0]).hypot(p.1 - def.mouth[1]),
                eye: (p.1 - def.eyes[2]).abs(),
                ph: hash(j as u32 + 7),
                xl,
                xr,
                head: (1.0 - (p.1 - def.mouth[1] - 18.0) / 22.0).clamp(0.0, 1.0),
            }
        })
        .collect();
    Built {
        outline,
        seeds,
        links: RefCell::new(None),
    }
}

/// Each dot's two nearest earlier dots.
#[inline(never)]
fn links(seeds: &[Seed]) -> Vec<(u32, u32)> {
    (0..seeds.len())
        .map(|j| {
            let (mut a, mut b) = ((u32::MAX, f64::MAX), (u32::MAX, f64::MAX));
            for (i, s) in seeds[..j].iter().enumerate() {
                let d = (s.x - seeds[j].x).powi(2) + (s.y - seeds[j].y).powi(2);
                if d < a.1 {
                    b = a;
                    a = (i as u32, d);
                } else if d < b.1 {
                    b = (i as u32, d);
                }
            }
            (a.0, b.0)
        })
        .collect()
}

pub fn frame_silhouette(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    // Rounded: a blend of two equal ids can land a hair off.
    let b = built(get(o, "silhouetteId", 0.0).max(0.0).round() as u32);
    let k = size / 200.0;
    let level = get(o, "audioLevel", 0.0).clamp(0.0, 1.0);
    let breath = get(o, "breath", 1.0);
    let glint = get(o, "glint", 1.0);
    let inward = get(o, "inward", 0.0);
    let neuron = get(o, "neuron", 0.0);
    let speech = get(o, "speech", 0.0);
    let holo = get(o, "hologram", 0.0).clamp(0.0, 1.0);
    let scan = get(o, "scanlines", 0.0).clamp(0.0, 1.0);
    let wire = get(o, "wire", 0.0).clamp(0.0, 1.0);
    let yaw = get(o, "turnYaw", 0.0).clamp(-1.0, 1.0);
    let ink = get(o, "ink", 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0);
    let dot = get(o, "dotSize", 1.0).max(0.0);
    // Clocks: a rate key reads its accumulated cycles in a transition (design note 31).
    let wave = cycles(o, "waveSpeedCycles").unwrap_or(t * get(o, "waveSpeed", 0.8));
    let breath_c = cycles(o, "periodCycles").unwrap_or(t / get(o, "period", 4.0).max(0.05));
    let scan_c = cycles(o, "scanSpeedCycles").unwrap_or(t * get(o, "scanSpeed", 0.3));
    // 20 px: the outline alone; then ~0.37 × size² dots (a prefix of the same order).
    let n = if size < 24.0 {
        0
    } else {
        (get(o, "silhouetteDots", 0.37 * size * size).max(0.0) as usize).min(b.seeds.len())
    };
    // A rare glitch (hologram): a thin band slides sideways for one 1/12 s step.
    let step = (t * 12.0).floor() as i64 as u32;
    let glitch = (holo > 0.0 && hash(step ^ 0xa5a5) > 0.94).then(|| {
        (
            hash(step) * 180.0,
            (hash(step ^ 0x3c3c) - 0.5) * 10.0 * holo,
        )
    });
    let swell = 1.0 + 0.012 * breath * (2.0 * PI * breath_c).sin();
    let (cx, cy) = (100.0, 120.0);
    let place = |x: f64, y: f64, d: f64| -> (f64, f64) {
        ((cx + (x - cx) * swell + d) * k, (cy + (y - cy) * swell) * k)
    };
    let turn = |s: &Seed| {
        let w = (s.xr - s.xl).max(1e-6);
        let u = ((s.x - s.xl) / w).clamp(0.0, 1.0);
        yaw * 0.5 * s.head * w * (PI * u).sin() / PI
    };
    let mut dots = Vec::with_capacity(n);
    for (j, s) in b.seeds.iter().take(n).enumerate() {
        // A faint light from the upper left; everything else is motion.
        let shade = (1.0 - ((s.x - 40.0) + (s.y - 20.0)) / 330.0).clamp(0.0, 1.0);
        let wave_at = |phase: f64, p: i32| (2.0 * PI * phase).sin().max(0.0).powi(p);
        let mut light = inward * (0.35 + level) * wave_at(wave - s.rim / 22.0, 2)
            + neuron * (-(s.eye * s.eye) / 300.0).exp() * wave_at(wave * (1.0 + s.ph) + s.ph, 6)
            + speech * level * (-(s.mouth / 90.0)).exp() * wave_at(s.mouth / 26.0 - wave * 1.4, 2)
            + holo * 0.7 * (-s.rim / 7.0).exp();
        if j % 37 == 0 {
            light += glint * wave_at(breath_c * 0.5 + s.ph, 8);
        }
        let band = 1.0 - scan * 0.5 * (0.5 + 0.5 * (2.0 * PI * (s.y / 12.0 - scan_c)).sin());
        let (mut x, y) = place(s.x, s.y, turn(s));
        if let Some((y0, dx)) = glitch {
            if s.y > y0 && s.y < y0 + 9.0 {
                x += dx * k;
            }
        }
        dots.push(Dot {
            x,
            y,
            z: 0.5,
            r: (0.42 + 0.45 * light) * dot * k * 2.2,
            white: 0.45 + 0.15 * shade,
            a: ((0.55 + 0.15 * shade + 0.6 * light) * band * ink * (0.2 + 0.8 * base(s.y)))
                .clamp(0.0, 1.0),
            saturation,
            hue,
        });
    }
    let mut lines = Vec::new();
    if wire > 0.0 && n > 0 {
        let mut l = b.links.borrow_mut();
        let l = l.get_or_insert_with(|| links(&b.seeds));
        for (j, &(a, c)) in l.iter().take(n).enumerate() {
            let q = &b.seeds[j];
            for p in [a, c].iter().filter_map(|&i| b.seeds.get(i as usize)) {
                // Only near neighbours (the first dots' nearest can be far away).
                if (p.x - q.x).hypot(p.y - q.y) > 9.0 {
                    continue;
                }
                let (x1, y1) = place(p.x, p.y, turn(p));
                let (x2, y2) = place(q.x, q.y, turn(q));
                lines.push(Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    white: 0.4,
                    a: 0.22 * wire * ink * base(q.y),
                    w: 0.35 * k * 2.2,
                    saturation,
                    hue,
                });
            }
        }
    }
    let mut f = finalize_frame(dots, lines, get(o, "rMin", 0.3));
    outline(
        &mut f,
        &b.outline,
        size,
        ink * get(o, "rim", 0.7),
        saturation,
        hue,
        &place,
    );
    f
}

/// The outline: a run at full strength, then the base fade in six steps (one stroke per
/// step, to keep the element count down); a soft glow under a thin line (the line alone
/// at 20 px).
#[inline(never)]
fn outline(
    f: &mut OrbFrame,
    pts: &[Point],
    size: f64,
    a: f64,
    saturation: f64,
    hue: f64,
    place: &dyn Fn(f64, f64, f64) -> (f64, f64),
) {
    let small = size < 24.0;
    let w = if small { 1.4 } else { 0.8 } * size / 200.0 * 2.2;
    let at = |p: &Point| {
        let (x, y) = place(p.x, p.y, 0.0);
        Point { x, y }
    };
    let mut runs: Vec<(Vec<Point>, f64)> = Vec::new();
    for pq in pts.windows(2) {
        let fade = (base((pq[0].y + pq[1].y) / 2.0) * 6.0).ceil() / 6.0;
        if fade <= 0.0 {
            continue;
        }
        match runs.last_mut() {
            Some((run, f)) if *f == fade && run.last().is_some_and(|p| *p == at(&pq[0])) => {
                run.push(at(&pq[1]));
            }
            _ => runs.push((vec![at(&pq[0]), at(&pq[1])], fade)),
        }
    }
    for glow in [true, false] {
        if glow && small {
            continue;
        }
        let (ww, aa) = if glow { (w * 5.0, a * 0.12) } else { (w, a) };
        for (points, fade) in &runs {
            f.polylines.push(Polyline {
                points: points.clone(),
                white: 0.5,
                a: aa * fade,
                w: ww,
                saturation,
                hue,
                hues: Vec::new(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    fn frame(o: &[(&str, f64)], size: f64, t: f64) -> OrbFrame {
        frame_silhouette(size, t, &opts(o))
    }

    #[test]
    fn the_dots_stay_inside_the_outline_when_the_head_turns() {
        for id in [0.0, 1.0] {
            let b = built(id as u32);
            for yaw in [-1.0, -0.4, 0.0, 0.7, 1.0] {
                let f = frame(
                    &[("silhouetteId", id), ("turnYaw", yaw), ("breath", 0.0)],
                    200.0,
                    1.0,
                );
                assert!(f.dots.len() > 1400, "{} dots", f.dots.len());
                for d in &f.dots {
                    assert!(
                        inside(&Point { x: d.x, y: d.y }, &b.outline),
                        "{id} yaw {yaw}: ({}, {})",
                        d.x,
                        d.y
                    );
                }
            }
        }
    }

    #[test]
    fn the_dots_are_the_same_in_every_state_and_a_smaller_size_is_a_prefix() {
        let at = |o: &[(&str, f64)], size| {
            frame(o, size, 2.5)
                .dots
                .iter()
                .map(|d| (d.x, d.y))
                .collect::<Vec<_>>()
        };
        let idle = at(&[("breath", 0.0)], 64.0);
        for s in ["inward", "neuron", "speech", "glint", "scanlines"] {
            let other = at(&[("breath", 0.0), (s, 1.0), ("audioLevel", 0.8)], 64.0);
            assert_eq!(idle, other, "{s}");
        }
        let small = at(&[("breath", 0.0)], 32.0);
        assert!(small.len() > 300 && small.len() < idle.len());
        for (a, b) in small.iter().zip(&idle) {
            assert!((a.0 * 2.0 - b.0).abs() < 1e-9 && (a.1 * 2.0 - b.1).abs() < 1e-9);
        }
        assert!(at(&[], 20.0).is_empty(), "20 px is the outline alone");
        assert_eq!(at(&[("silhouetteDots", 800.0)], 64.0).len(), 800);
    }

    #[test]
    fn the_dots_spread_evenly() {
        // Even density (the user, 2026-10-04): the nearest-neighbour distance is about
        // the same everywhere -- no crowding at the eyes, the mouth or the rim.
        for id in [0, 1] {
            let b = built(id);
            let s = &b.seeds[..1500];
            let mut d: Vec<f64> = s
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    s.iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, q)| (p.x - q.x).hypot(p.y - q.y))
                        .fold(f64::MAX, f64::min)
                })
                .collect();
            d.sort_by(f64::total_cmp);
            let (p10, p90) = (d[150], d[1350]);
            assert!(
                p90 / p10 < 1.6,
                "{id}: nearest neighbour {p10:.2}..{p90:.2}"
            );
        }
    }

    #[test]
    fn a_files_silhouette_registers_once_and_draws() {
        let def = Def {
            path: "M10 214 C10 150 60 140 70 120 C40 100 50 30 100 30 C150 30 160 100 130 120 C140 140 190 150 190 214 Z".into(),
            eyes: [85.0, 115.0, 70.0],
            mouth: [100.0, 100.0],
        };
        let id = register(def.clone()).unwrap();
        assert_eq!(register(def).unwrap(), id);
        assert!(id >= 2);
        assert!(
            frame(&[("silhouetteId", f64::from(id))], 64.0, 1.0)
                .dots
                .len()
                > 1000
        );
        let off = Def {
            path: "M0 0 L400 0 L400 400 Z".into(),
            eyes: [1.0, 2.0, 3.0],
            mouth: [1.0, 2.0],
        };
        assert!(register(off).is_err());
    }
}
