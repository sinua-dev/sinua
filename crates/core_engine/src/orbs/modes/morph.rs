//! Morph: a dotted outline cycling circle -> triangle -> square -> circle --
//! the "shaping" state. Each shape is a continuous closed path
//! parameterised by arc length (top-centre start, clockwise). Every frame
//! the engine blends the two neighbouring paths, then lays the dots EVENLY
//! along the blended outline -- spacing stays uniform at every instant of
//! the morph, holds and transitions alike.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/morph.ts`.

use crate::orbs::core::{finalize_frame, Dot, OrbFrame};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

fn smooth_e(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}

/// A closed path, precomputed once per `frame_morph` call (upstream builds
/// TRIANGLE/SQUARE once at module load and reuses the closure; we rebuild
/// per frame instead of introducing a lazy-static -- two loops of 3 and 5
/// iterations at 60fps is not worth the extra machinery).
struct PolyPath {
    verts: &'static [(f64, f64)],
    lengths: Vec<f64>,
    total: f64,
}

impl PolyPath {
    fn new(verts: &'static [(f64, f64)]) -> Self {
        let v = verts.len();
        let mut lengths = Vec::with_capacity(v);
        let mut total = 0.0;
        for i in 0..v {
            let a = verts[i];
            let b = verts[(i + 1) % v];
            let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            lengths.push(l);
            total += l;
        }
        Self {
            verts,
            lengths,
            total,
        }
    }

    fn point(&self, f: f64) -> (f64, f64) {
        let v = self.verts.len();
        let mut target = f * self.total;
        let mut i = 0;
        while target > self.lengths[i] && i < v - 1 {
            target -= self.lengths[i];
            i += 1;
        }
        let a = self.verts[i];
        let b = self.verts[(i + 1) % v];
        let ff = if self.lengths[i] != 0.0 {
            (target / self.lengths[i]).min(1.0)
        } else {
            0.0
        };
        (a.0 + (b.0 - a.0) * ff, a.1 + (b.1 - a.1) * ff)
    }
}

enum Shape {
    Circle,
    Poly(PolyPath),
}

impl Shape {
    fn point(&self, f: f64) -> (f64, f64) {
        match self {
            Shape::Circle => {
                let a = -PI / 2.0 + f * 2.0 * PI;
                (a.cos() * 0.24, a.sin() * 0.24)
            }
            Shape::Poly(p) => p.point(f),
        }
    }
}

static TRIANGLE_VERTS: [(f64, f64); 3] = [(0.0, -0.26), (0.24, 0.16), (-0.24, 0.16)];
// 5-vertex walk so the path STARTS at top-centre like the other shapes
static SQUARE_VERTS: [(f64, f64); 5] = [
    (0.0, -0.2),
    (0.2, -0.2),
    (0.2, 0.2),
    (-0.2, 0.2),
    (-0.2, -0.2),
];

fn cycle() -> [Shape; 3] {
    [
        Shape::Circle,
        Shape::Poly(PolyPath::new(&TRIANGLE_VERTS)),
        Shape::Poly(PolyPath::new(&SQUARE_VERTS)),
    ]
}

const HOLD: f64 = 1.4;
const MORPH: f64 = 0.9;
const SEG: f64 = HOLD + MORPH;

// low floor keeps sparse outlines possible while never degenerating
fn morph_n(d: f64) -> usize {
    ((34.0 * d).round() as i64).max(6) as usize
}

pub fn frame_morph(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cycle = cycle();
    let k_count = cycle.len();
    let seg_total = SEG * k_count as f64;
    let tc = t.rem_euclid(seg_total);

    // `shape` 0 / 1 / 2 holds one outline instead of cycling; anything else
    // (unset, or out of range) keeps the tuned circle -> triangle -> square loop
    let held: i64 = match o.get("shape") {
        Some(&s) if s >= 0.0 && s < k_count as f64 => s.floor() as i64,
        _ => -1,
    };
    let k = if held >= 0 {
        held as usize
    } else {
        (tc / SEG).floor() as usize
    };
    let local = if held >= 0 {
        t.rem_euclid(SEG)
    } else {
        tc - k as f64 * SEG
    };
    let m = if held >= 0 {
        0.0
    } else if local > HOLD {
        smooth_e((local - HOLD) / MORPH)
    } else {
        0.0
    };
    let sprd = get(o, "spread", 1.0);

    // blend the two shape PATHS at m, then measure the blended outline
    let p_a = &cycle[k];
    let p_b = if held >= 0 {
        &cycle[k]
    } else {
        &cycle[(k + 1) % k_count]
    };
    const M: usize = 160;
    let mut pts = [(0.0f64, 0.0f64); M];
    for (i, pt) in pts.iter_mut().enumerate() {
        let f = i as f64 / M as f64;
        let a = p_a.point(f);
        let b = p_b.point(f);
        *pt = (
            (a.0 + (b.0 - a.0) * m) * sprd,
            (a.1 + (b.1 - a.1) * m) * sprd,
        );
    }
    let mut lens = [0.0f64; M];
    let mut total = 0.0;
    for i in 0..M {
        let a = pts[i];
        let b = pts[(i + 1) % M];
        let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        lens[i] = l;
        total += l;
    }

    // dot radius depends ONLY on rDot (the size knob); the count sets the
    // gaps. Formed shapes breathe a little (uniform pulse).
    let n = morph_n(get(o, "iconD", 1.0));
    let re = get(o, "rDot", 0.021) * 1.35 * sprd;
    let pulse = 1.0 + 0.02 * (local * 3.1).sin();

    let mut dots: Vec<Dot> = Vec::with_capacity(n);
    let c2 = size / 2.0;
    let mut seg_i: usize = 0;
    let mut acc = 0.0;
    for k2 in 0..n {
        let target = (k2 as f64 / n as f64) * total;
        while acc + lens[seg_i] < target && seg_i < M - 1 {
            acc += lens[seg_i];
            seg_i += 1;
        }
        let a = pts[seg_i];
        let b = pts[(seg_i + 1) % M];
        let f = if lens[seg_i] != 0.0 {
            ((target - acc) / lens[seg_i]).min(1.0)
        } else {
            0.0
        };
        let x = (a.0 + (b.0 - a.0) * f) * pulse;
        let y = (a.1 + (b.1 - a.1) * f) * pulse;
        dots.push(Dot {
            x: c2 + x * size,
            y: c2 + y * size,
            z: 0.0,
            r: (re * size).max(0.35),
            white: 0.1,
            a: 1.0,
            ..Default::default()
        });
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
