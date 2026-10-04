//! `orbs`-specific primitives: the Fibonacci *sphere* lattice and the
//! spin/tilt/orthographic sphere projection every `orbs` mode uses to place
//! points on screen, plus `LatticeSample` (the cross-mode transition
//! mechanism for the trio of modes sharing that lattice -- see
//! `orbs::modes::transition`'s header).
//!
//! Ported 1:1 from `thinking-orbs/src/engine/core.ts` (upstream:
//! https://github.com/Jakubantalik/Libraries.dev, MIT, Jakub Antalik — see
//! `third_party/thinking-orbs/LICENSE`). Keep this a faithful transcription,
//! not a "Rustified" rewrite: correctness is proven by comparing output
//! against `spec/orbs-golden.json` within a 1e-4 tolerance, not by
//! re-deriving the math independently.
//!
//! Everything that ISN'T specific to a sphere (`Dot`/`Line`/`OrbFrame`, the
//! noise functions, `finalize_frame`, the mode-agnostic pointer/audio
//! post-processes, ...) moved to `crate::primitives` once a second family
//! (`signal`, see `signal/mod.rs`) needed them too -- re-exported below so
//! none of this crate's `orbs::modes::*` files needed an import change.

pub use crate::primitives::{
    angle_delta, audio_band, finalize_frame, frac, hash_d, lerp, lerp_hue, perlin3, radius_scale,
    vnoise, Dot, Line, OrbFrame,
};

/// A mode's per-point data before final projection -- what
/// `orbs::modes::transition` needs to blend two lattice-sharing modes
/// (`aurora`/`chladni`/`eclipse`, see that module's header) point-by-point
/// instead of cross-dissolving two whole frames. `dir`/`radius_frac`
/// describe where the point sits in 3D (before the shared camera used
/// during a transition projects it); `dot_r`/`white`/`alpha`/`saturation`/
/// `hue` are the same values that mode would put directly on a `Dot`,
/// already fully computed (including that mode's own camera-dependent
/// depth shading -- see `transition.rs`'s header for why each mode still
/// does its own internal projection just to compute these, separately
/// from the transition's shared-camera final position). Specific to the
/// `orbs` family's shared Fibonacci lattice, unlike everything re-exported
/// above -- doesn't belong in `crate::primitives`.
pub struct LatticeSample {
    pub dir: (f64, f64, f64),
    pub radius_frac: f64,
    pub dot_r: f64,
    pub white: f64,
    pub alpha: f64,
    pub saturation: f64,
    pub hue: f64,
}

/// Stable directions on a unit sphere (Fibonacci lattice).
pub fn fib_dir(i: f64, n: f64) -> (f64, f64, f64) {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let y = 1.0 - (2.0 * (i + 0.5)) / n;
    let rad = (1.0 - y * y).sqrt();
    let a = i * golden;
    (rad * a.cos(), y, rad * a.sin())
}

/// How many of a count-driven set to draw, and how strongly (design note 31, TS7).
/// Without `<key>Layout` the count is read as before (truncated) and every element
/// draws in full. A view blending voice states whose counts differ passes the largest
/// as `<key>Layout`: the set is laid out once for it, the count becomes a density, and
/// element `i` draws at `fade(i)` = `clamp(count − i, 0, 1)`, so a count change fades
/// elements in or out and nothing moves.
pub struct Density {
    /// Elements to visit.
    pub n: i64,
    count: f64,
    /// The layout size, or 0 for the old one-count layout.
    pub layout: i64,
}

impl Density {
    #[inline(never)]
    pub fn read(
        o: &std::collections::HashMap<String, f64>,
        key: &str,
        layout_key: &str,
        default: f64,
    ) -> Density {
        let count = o.get(key).copied().unwrap_or(default);
        match o.get(layout_key) {
            Some(&l) if l >= count && count >= 0.0 => Density {
                n: count.ceil() as i64,
                count,
                layout: l as i64,
            },
            _ => Density {
                n: count as i64,
                count,
                layout: 0,
            },
        }
    }

    pub fn fade(&self, i: i64) -> f64 {
        if self.layout == 0 {
            1.0
        } else {
            (self.count - i as f64).clamp(0.0, 1.0)
        }
    }
}

/// Dot `i` of a lattice of `n` (design note 31, TS7): the Fibonacci lattice of `n`,
/// or with a `layout` the Fibonacci lattice of `layout` points visited in an order
/// whose every prefix covers the sphere evenly (farthest point first). At the layout
/// size it is the same set of points as the plain lattice.
#[inline(never)]
pub fn lattice_dir(i: i64, n: i64, layout: i64) -> (f64, f64, f64) {
    if layout <= 0 {
        return fib_dir(i as f64, n as f64);
    }
    let k = ranked(layout as usize, i as usize);
    fib_dir(k as f64, layout as f64)
}

/// The `i`-th point of the farthest-point order over the Fibonacci lattice of `n`
/// (computed once per `n`, O(n²)).
#[inline(never)]
fn ranked(n: usize, i: usize) -> usize {
    use std::cell::RefCell;
    thread_local! {
        static ORDERS: RefCell<Vec<(usize, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    }
    ORDERS.with(|o| {
        let mut o = o.borrow_mut();
        if let Some((_, order)) = o.iter().find(|(m, _)| *m == n) {
            return order.get(i).map_or(i, |&k| k as usize);
        }
        let pts: Vec<(f64, f64, f64)> = (0..n).map(|k| fib_dir(k as f64, n as f64)).collect();
        let d2 = |a: (f64, f64, f64), b: (f64, f64, f64)| {
            (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
        };
        let mut dist: Vec<f64> = pts.iter().map(|&p| d2(p, pts[0])).collect();
        let mut order = Vec::with_capacity(n);
        if n > 0 {
            order.push(0u32);
            dist[0] = -1.0;
        }
        for _ in 1..n {
            let mut best = 0;
            for k in 0..n {
                if dist[k] > dist[best] {
                    best = k;
                }
            }
            order.push(best as u32);
            dist[best] = -1.0;
            for k in 0..n {
                if dist[k] >= 0.0 {
                    dist[k] = dist[k].min(d2(pts[k], pts[best]));
                }
            }
        }
        if o.len() >= 4 {
            o.remove(0);
        }
        let k = order.get(i).map_or(i, |&k| k as usize);
        o.push((n, order));
        k
    })
}

/// Shared spin + tilt + orthographic projection.
///
/// A struct instead of upstream's boxed closure — same math, no per-call
/// heap allocation, one instance per frame like the TS `makeProj` call site.
pub struct Proj {
    st: f64,
    ct: f64,
    sy: f64,
    cyw: f64,
    cx: f64,
    cy: f64,
    scale: f64,
}

impl Proj {
    pub fn new(yaw: f64, tilt: f64, cx: f64, cy: f64, scale: f64) -> Self {
        Self {
            st: tilt.sin(),
            ct: tilt.cos(),
            sy: yaw.sin(),
            cyw: yaw.cos(),
            cx,
            cy,
            scale,
        }
    }

    pub fn project(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let x1 = x * self.cyw + z * self.sy;
        let z1 = -x * self.sy + z * self.cyw;
        let y1 = y * self.ct - z1 * self.st;
        let z2 = y * self.st + z1 * self.ct;
        (self.cx + x1 * self.scale, self.cy - y1 * self.scale, z2)
    }
}
