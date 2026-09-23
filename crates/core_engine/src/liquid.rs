//! Material: **liquid** (materials phase 2) -- a frame's dots melted into a
//! metaball field and re-drawn as its iso-contours, in this engine's own
//! vector language: ink outlines (default), contour-sampled dots, or filled
//! blobs (with holes). The "gooey, merging" mood of voice orbs without a
//! shader or a raster threshold.
//!
//! Sources (fetched):
//! - Jamie Wong, *Metaballs and Marching Squares*: a summed field, "inside"
//!   above a threshold, sampled only at grid-cell corners with the contour
//!   placed on each edge by **linear interpolation** (per-pixel evaluation
//!   "would be on the order of 14 million operations").
//! - Wikipedia, *Metaballs*: a kernel with **finite support** "goes to zero at
//!   a maximum radius ... any points beyond their maximum radius ... can be
//!   ignored" -- so the kernel here is `(1 - (d/R)^2)^3` and every source only
//!   touches the cells inside its radius (Blinn's `1/r` never reaches zero).
//! - Wikipedia, *Marching squares*: the 16-case cell table from four corner
//!   bits; the **saddle** cases (5, 10) resolved by the cell-centre average.
//! - Not copied: the CSS "gooey" trick (blur + an alpha-contrast colour
//!   matrix, CSS-Tricks) -- it thresholds *rasterized pixels*, needs a
//!   container filter, and has Safari limitations; the vector path rules it
//!   out, and real contours give the same merging topology as geometry.
//!
//! Stateless: the field is rebuilt from the current frame's dots every call,
//! so the liquid moves exactly as the underlying state moves.

use std::collections::HashMap;

use crate::primitives::{Dot, Fill, OrbFrame, Point, Polyline};

fn get(o: &HashMap<String, f64>, key: &str, default: f64) -> f64 {
    o.get(key).copied().unwrap_or(default)
}

/// Compact metaball kernel: 1 at the centre, 0 (with zero slope) at `R`.
pub fn kernel(d2: f64, r2: f64) -> f64 {
    if d2 >= r2 {
        0.0
    } else {
        let u = 1.0 - d2 / r2;
        u * u * u
    }
}

/// The sampled field: `values[j * nx + i]` at `(x0 + i h, y0 + j h)`.
pub struct Grid {
    pub x0: f64,
    pub y0: f64,
    pub h: f64,
    pub nx: usize,
    pub ny: usize,
    pub values: Vec<f64>,
}

impl Grid {
    fn at(&self, i: usize, j: usize) -> f64 {
        self.values[j * self.nx + i]
    }
}

/// Sum each source's kernel into a grid of `cells` across `size`, padded so
/// every contour closes inside it. Sources are `(x, y, R, weight)`.
pub fn field(sources: &[(f64, f64, f64, f64)], size: f64, cells: usize) -> Grid {
    let h = size / cells as f64;
    let max_r = sources.iter().map(|s| s.2).fold(0.0, f64::max);
    let pad = (max_r / h).ceil() as usize + 1;
    let n = cells + 2 * pad + 1;
    let (x0, y0) = (-(pad as f64) * h, -(pad as f64) * h);
    let mut values = vec![0.0; n * n];
    for &(x, y, r, w) in sources {
        if r <= 0.0 || w <= 0.0 {
            continue;
        }
        let r2 = r * r;
        let i0 = (((x - r - x0) / h).floor().max(0.0)) as usize;
        let i1 = ((((x + r - x0) / h).ceil()) as usize).min(n - 1);
        let j0 = (((y - r - y0) / h).floor().max(0.0)) as usize;
        let j1 = ((((y + r - y0) / h).ceil()) as usize).min(n - 1);
        for j in j0..=j1 {
            let gy = y0 + j as f64 * h;
            for i in i0..=i1 {
                let gx = x0 + i as f64 * h;
                let d2 = (gx - x).powi(2) + (gy - y).powi(2);
                values[j * n + i] += w * kernel(d2, r2);
            }
        }
    }
    Grid {
        x0,
        y0,
        h,
        nx: n,
        ny: n,
        values,
    }
}

/// An edge of the grid: horizontal `(i, j)-(i+1, j)` or vertical
/// `(i, j)-(i, j+1)`. Crossing points are keyed by edge so neighbouring
/// cells share them exactly and segments stitch into loops.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Edge {
    H(usize, usize),
    V(usize, usize),
}

/// Marching squares at `threshold`: closed loops (first point not repeated).
pub fn contours(g: &Grid, threshold: f64) -> Vec<Vec<Point>> {
    let inside = |i: usize, j: usize| g.at(i, j) >= threshold;
    let cross = |e: Edge| -> Point {
        let ((ia, ja), (ib, jb)) = match e {
            Edge::H(i, j) => ((i, j), (i + 1, j)),
            Edge::V(i, j) => ((i, j), (i, j + 1)),
        };
        let (a, b) = (g.at(ia, ja), g.at(ib, jb));
        let t = if (b - a).abs() < 1e-12 {
            0.5
        } else {
            ((threshold - a) / (b - a)).clamp(0.0, 1.0)
        };
        Point {
            x: g.x0 + g.h * (ia as f64 + t * (ib as f64 - ia as f64)),
            y: g.y0 + g.h * (ja as f64 + t * (jb as f64 - ja as f64)),
        }
    };
    // Undirected segments between edge crossings, per the 16-case table
    // (bits: top-left 8, top-right 4, bottom-right 2, bottom-left 1; y down).
    let mut adj: HashMap<Edge, Vec<Edge>> = HashMap::new();
    let mut link = |a: Edge, b: Edge| {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    };
    for j in 0..g.ny - 1 {
        for i in 0..g.nx - 1 {
            let (tl, tr, br, bl) = (
                inside(i, j),
                inside(i + 1, j),
                inside(i + 1, j + 1),
                inside(i, j + 1),
            );
            let case = (tl as u8) << 3 | (tr as u8) << 2 | (br as u8) << 1 | bl as u8;
            let (t, r, b, l) = (
                Edge::H(i, j),
                Edge::V(i + 1, j),
                Edge::H(i, j + 1),
                Edge::V(i, j),
            );
            let centre = || {
                (g.at(i, j) + g.at(i + 1, j) + g.at(i + 1, j + 1) + g.at(i, j + 1)) / 4.0
                    >= threshold
            };
            match case {
                1 | 14 => link(l, b),
                2 | 13 => link(b, r),
                3 | 12 => link(l, r),
                4 | 11 => link(t, r),
                6 | 9 => link(t, b),
                7 | 8 => link(l, t),
                5 => {
                    // tr + bl inside. Centre inside: the diagonal joins them,
                    // cut off the outside corners tl and br; else isolate tr, bl.
                    if centre() {
                        link(l, t);
                        link(b, r);
                    } else {
                        link(t, r);
                        link(l, b);
                    }
                }
                10 => {
                    // tl + br inside (mirror of 5).
                    if centre() {
                        link(t, r);
                        link(l, b);
                    } else {
                        link(l, t);
                        link(b, r);
                    }
                }
                _ => {}
            }
        }
    }
    // Walk the degree-2 graph into loops, in a deterministic order.
    let mut keys: Vec<Edge> = adj.keys().copied().collect();
    keys.sort_by_key(|e| match *e {
        Edge::H(i, j) => (j, i, 0),
        Edge::V(i, j) => (j, i, 1),
    });
    let mut seen: HashMap<Edge, bool> = HashMap::new();
    let mut loops = Vec::new();
    for start in keys {
        if seen.contains_key(&start) {
            continue;
        }
        let mut ring = vec![cross(start)];
        seen.insert(start, true);
        let (mut prev, mut cur) = (start, adj[&start][0]);
        while cur != start {
            if seen.contains_key(&cur) {
                break;
            }
            seen.insert(cur, true);
            ring.push(cross(cur));
            let next = adj[&cur]
                .iter()
                .copied()
                .find(|e| *e != prev)
                .unwrap_or(prev);
            prev = cur;
            cur = next;
        }
        if ring.len() >= 3 {
            loops.push(ring);
        }
    }
    loops
}

pub fn signed_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|k| {
            let (a, b) = (&ring[k], &ring[(k + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        * 0.5
}

pub fn contains(ring: &[Point], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (&ring[i], &ring[j]);
        if (a.y > y) != (b.y > y) && x < (b.x - a.x) * (y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Loops grouped into blobs: `(outer, holes)`. A loop is a hole when an odd
/// number of other loops contain it; it belongs to the smallest outer that
/// does. (Orientation-free, so it doesn't depend on the case table's winding.)
pub fn blobs(loops: Vec<Vec<Point>>) -> Vec<(Vec<Point>, Vec<Vec<Point>>)> {
    let depth: Vec<usize> = loops
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let p = &l[0];
            loops
                .iter()
                .enumerate()
                .filter(|(k, o)| *k != i && contains(o, p.x, p.y))
                .count()
        })
        .collect();
    let mut outers: Vec<(Vec<Point>, Vec<Vec<Point>>)> = Vec::new();
    let mut outer_index: Vec<Option<usize>> = vec![None; loops.len()];
    for (i, l) in loops.iter().enumerate() {
        if depth[i].is_multiple_of(2) {
            outer_index[i] = Some(outers.len());
            outers.push((l.clone(), Vec::new()));
        }
    }
    for (i, l) in loops.iter().enumerate() {
        if !depth[i].is_multiple_of(2) {
            let p = &l[0];
            let owner = loops
                .iter()
                .enumerate()
                .filter(|(k, o)| outer_index[*k].is_some() && contains(o, p.x, p.y))
                .min_by(|a, b| {
                    signed_area(a.1)
                        .abs()
                        .partial_cmp(&signed_area(b.1).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .and_then(|(k, _)| outer_index[k]);
            if let Some(o) = owner {
                outers[o].1.push(l.clone());
            }
        }
    }
    outers
}

/// Median distance from each dot to its nearest neighbour (0 for < 2 dots).
fn median_nn(dots: &[Dot]) -> f64 {
    if dots.len() < 2 {
        return 0.0;
    }
    median(
        dots.iter()
            .enumerate()
            .map(|(i, a)| {
                dots.iter()
                    .enumerate()
                    .filter(|(k, _)| *k != i)
                    .map(|(_, b)| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt())
                    .fold(f64::MAX, f64::min)
            })
            .collect(),
    )
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

fn perimeter(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|k| {
            let (a, b) = (&ring[k], &ring[(k + 1) % n]);
            ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
        })
        .sum()
}

/// Evenly resample a closed loop into `n` points by arc length.
fn resample(ring: &[Point], spacing: f64) -> Vec<Point> {
    let len = ring.len();
    let seg = |k: usize| {
        let (a, b) = (&ring[k], &ring[(k + 1) % len]);
        ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
    };
    let perim: f64 = (0..len).map(seg).sum();
    if perim <= 0.0 {
        return vec![];
    }
    let n = ((perim / spacing).round() as usize).max(3);
    let step = perim / n as f64;
    let mut out = Vec::with_capacity(n);
    let (mut k, mut acc) = (0usize, 0.0);
    for m in 0..n {
        let target = m as f64 * step;
        while acc + seg(k) < target && k < len - 1 {
            acc += seg(k);
            k += 1;
        }
        let (a, b) = (&ring[k], &ring[(k + 1) % len]);
        let s = seg(k);
        let t = if s > 0.0 {
            ((target - acc) / s).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(Point {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
        });
    }
    out
}

/// Alpha-weighted ink of the dots inside `outer` (and not in a hole), or of
/// every dot when none is: `(white, saturation, hue, a)`.
fn blob_ink(dots: &[Dot], outer: &[Point], holes: &[Vec<Point>]) -> (f64, f64, f64, f64) {
    let inside: Vec<&Dot> = dots
        .iter()
        .filter(|d| contains(outer, d.x, d.y) && !holes.iter().any(|h| contains(h, d.x, d.y)))
        .collect();
    let pool: Vec<&Dot> = if inside.is_empty() {
        dots.iter().collect()
    } else {
        inside
    };
    let wsum: f64 = pool.iter().map(|d| d.a).sum::<f64>().max(1e-9);
    let white = pool.iter().map(|d| d.white * d.a).sum::<f64>() / wsum;
    let sat = pool.iter().map(|d| d.saturation * d.a).sum::<f64>() / wsum;
    // Hue from the most saturated-and-opaque source (hue is circular).
    let hue = pool
        .iter()
        .max_by(|a, b| {
            (a.saturation * a.a)
                .partial_cmp(&(b.saturation * b.a))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(0.0, |d| d.hue);
    let a = pool.iter().map(|d| d.a * d.a).sum::<f64>() / wsum;
    (white, sat, hue, a)
}

/// `liquidStrength` > 0: the frame's dots become metaball contours.
/// `liquidStyle` 1 outline (default) / 2 dots / 0 fill; see docs/materials.md.
pub fn apply_liquid(mut frame: OrbFrame, size: f64, o: &HashMap<String, f64>) -> OrbFrame {
    let strength = get(o, "liquidStrength", 0.0).clamp(0.0, 1.0);
    if strength <= 0.0 || frame.dots.is_empty() {
        return frame;
    }
    let reach = get(o, "liquidReach", 2.5).clamp(1.0, 12.0);
    let threshold = get(o, "liquidThreshold", 0.5).clamp(0.05, 4.0);
    let cells = get(o, "liquidCells", 40.0).round().clamp(8.0, 96.0) as usize;
    // Outline by default: the most "ink" of the three and the one that reads
    // as liquid on an orb (contact sheet: dot-sampled contours on a sparse
    // orb look too close to the plain orb).
    let style = get(o, "liquidStyle", 1.0).round().clamp(0.0, 2.0) as u8;
    let spacing = get(o, "liquidSpacing", 2.25).clamp(0.5, 12.0);
    let width = get(o, "liquidWidth", 0.8).clamp(0.05, 8.0);
    let keep = get(o, "liquidKeep", 0.0).round() == 1.0;
    let blur = get(o, "liquidBlur", 0.0).clamp(0.0, 32.0);

    // Reach is relative to how far apart the dots are, not only how big:
    // an orb's dots are small and sparse, so `reach x r` alone never meets a
    // neighbour (checked on a contact sheet) -- the base is the larger of a
    // dot's radius and half the median nearest-neighbour distance.
    let spacing_base = 0.5 * median_nn(&frame.dots);
    let sources: Vec<(f64, f64, f64, f64)> = frame
        .dots
        .iter()
        .map(|d| (d.x, d.y, d.r.max(spacing_base) * reach, d.a))
        .collect();
    let groups = blobs(contours(&field(&sources, size, cells), threshold));
    let r_med = median(frame.dots.iter().map(|d| d.r).collect());
    let originals = std::mem::take(&mut frame.dots);

    match style {
        0 => {
            for (outer, holes) in groups {
                let (white, saturation, hue, a) = blob_ink(&originals, &outer, &holes);
                frame.fills.push(Fill {
                    points: outer,
                    holes,
                    white,
                    a: (a * strength).min(1.0),
                    saturation,
                    hue,
                    gradient: None,
                    blur,
                    blend: 0,
                });
            }
        }
        1 => {
            for (outer, holes) in groups {
                let (white, saturation, hue, a) = blob_ink(&originals, &outer, &holes);
                for ring in std::iter::once(&outer).chain(holes.iter()) {
                    let mut points = ring.clone();
                    points.push(ring[0].clone());
                    frame.polylines.push(Polyline {
                        points,
                        white,
                        a: (a * strength).min(1.0),
                        w: width * r_med,
                        saturation,
                        hue,
                        hues: Vec::new(),
                    });
                }
            }
        }
        _ => {
            for (outer, holes) in groups {
                let (white, saturation, hue, a) = blob_ink(&originals, &outer, &holes);
                for ring in std::iter::once(&outer).chain(holes.iter()) {
                    // A loop too small to carry three spaced dots is one dot
                    // at its centre (no triangles of dots around a lone source).
                    let pts = resample(ring, spacing * 2.0 * r_med);
                    let pts = if perimeter(ring) < 3.0 * spacing * 2.0 * r_med {
                        let n = ring.len() as f64;
                        vec![Point {
                            x: ring.iter().map(|p| p.x).sum::<f64>() / n,
                            y: ring.iter().map(|p| p.y).sum::<f64>() / n,
                        }]
                    } else {
                        pts
                    };
                    for p in pts {
                        frame.dots.push(Dot {
                            x: p.x,
                            y: p.y,
                            z: 0.0,
                            r: r_med,
                            white,
                            a: (a * strength).min(1.0),
                            saturation,
                            hue,
                        });
                    }
                }
            }
        }
    }
    if keep {
        frame.dots.extend(originals);
    }
    frame
}

/// Per-mode tuned defaults (contact sheets, 2026-09-19; docs/materials.md):
/// sparse orbs need a longer reach (and a lower threshold for `orbits`) to
/// pool instead of fragmenting; `ribbon`'s dense lanes a longer reach and a
/// higher threshold to band instead of combing; `matrix` keeps its LEDs so
/// the outline reads as an EQ silhouette over the grid.
pub fn mode_defaults(mode: &str) -> &'static [(&'static str, f64)] {
    match mode {
        "orbits" => &[("liquidReach", 4.0), ("liquidThreshold", 0.4)],
        "globe" => &[("liquidReach", 4.0)],
        "ribbon" => &[("liquidReach", 5.0), ("liquidThreshold", 0.6)],
        "matrix" => &[("liquidKeep", 1.0)],
        _ => &[],
    }
}

/// `opts` with the mode's tuned liquid defaults filled in where unset --
/// only when liquid is on, so every other frame is untouched. Explicit keys
/// (Studio sliders, FX Spec `materials.liquid`) always win.
pub fn with_mode_defaults<'a>(
    mode: &str,
    opts: &'a HashMap<String, f64>,
) -> std::borrow::Cow<'a, HashMap<String, f64>> {
    let on = get(opts, "liquidStrength", 0.0) > 0.0;
    let missing = mode_defaults(mode)
        .iter()
        .any(|(k, _)| !opts.contains_key(*k));
    if !on || !missing {
        return std::borrow::Cow::Borrowed(opts);
    }
    let mut o = opts.clone();
    for (k, v) in mode_defaults(mode) {
        o.entry((*k).to_string()).or_insert(*v);
    }
    std::borrow::Cow::Owned(o)
}

/// How well liquid suits a state -- for a Studio badge.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidSuitability {
    /// `"recommended"`, `"ok"` or `"notRecommended"`.
    pub level: String,
    pub reason: String,
    /// The mode's tuned liquid defaults (see `mode_defaults`).
    pub defaults: HashMap<String, f64>,
}

/// Judged on contact sheets (every state, outline and blurred fill, with
/// the tuned defaults; docs/materials.md). Liquid melts *dots*, so modes
/// drawn with strokes or fills, or dim by design, have nothing to pool.
pub fn suitability(mode: &str) -> LiquidSuitability {
    let (level, reason) = match mode {
        "globe" => (
            "recommended",
            "the sphere's dots melt into a liquid lattice of cells",
        ),
        "rubik" => ("recommended", "cube faces pool into rounded liquid tiles"),
        "wave" => ("recommended", "the rings flow into soft liquid bands"),
        "web" => (
            "recommended",
            "nodes pool into blobs joined by the web's lines",
        ),
        "ring" => (
            "recommended",
            "the ring of dots becomes one wavy liquid ring",
        ),
        "morph" => (
            "recommended",
            "the shape's outline turns into a clean liquid ring",
        ),
        "aurora" => ("recommended", "the colored cloud pools along its rim"),
        "webflow" => ("recommended", "drifting nodes read as merging liquid blobs"),
        "spectrum" => (
            "recommended",
            "the spectrum ring becomes a liquid band that splashes with the bars",
        ),
        "matrix" => (
            "recommended",
            "lit LEDs melt into an EQ silhouette over the kept grid",
        ),
        "dots" => ("recommended", "the typing dots merge into one liquid pill"),
        "orbits" => (
            "ok",
            "sparse particles pool into islands; brighter spots merge first",
        ),
        "braid" => (
            "ok",
            "the strands pool into short lanes rather than one flow",
        ),
        "ribbon" => (
            "ok",
            "reads as a band, with a comb fringe where the lanes are densest",
        ),
        "sonar" => (
            "ok",
            "rings pool into a thick band; the echoes lose some detail",
        ),
        "eclipse" => ("ok", "the lit side pools; the dark side stays empty"),
        "radar" => (
            "ok",
            "the lit trail pools into a tongue; the faint scope stays below the threshold",
        ),
        "warp" => (
            "notRecommended",
            "streaking stars are too sparse to pool; only fragments remain",
        ),
        "chladni" => (
            "notRecommended",
            "the dots are too faint to reach the threshold",
        ),
        "crystallize" => (
            "notRecommended",
            "drawn mostly with lines, so there is little to melt",
        ),
        "hush" => ("notRecommended", "dim by design; nothing pools"),
        "bar" | "waveform" | "scroll" => (
            "notRecommended",
            "drawn with strokes, not dots -- nothing for liquid to melt",
        ),
        "arc" | "spinner" | "nested" | "segmented" | "gauge" => (
            "notRecommended",
            "a stroked ring with at most a few dots; liquid changes little",
        ),
        "ping" | "pulse" | "halo" | "broadcast" => (
            "notRecommended",
            "a single dot plus strokes; liquid only rounds the dot",
        ),
        "shimmer" => ("notRecommended", "drawn with a stroke or fill, no dots"),
        _ => ("ok", "not yet judged on a contact sheet"),
    };
    LiquidSuitability {
        level: level.to_string(),
        reason: reason.to_string(),
        defaults: mode_defaults(mode)
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(x: f64, y: f64, r: f64) -> Dot {
        Dot {
            x,
            y,
            z: 0.0,
            r,
            white: 0.15,
            a: 1.0,
            saturation: 0.0,
            hue: 0.0,
        }
    }

    fn frame(dots: Vec<Dot>) -> OrbFrame {
        OrbFrame {
            dots,
            ..Default::default()
        }
    }

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn every_state_is_judged_and_defaults_only_fill_gaps() {
        // One source for "every pattern" (crate::all_states), and a count
        // derived from it rather than a literal: a hard-coded 34 agrees with
        // itself even when the lists hold different names.
        let states = crate::all_states();
        assert!(
            !states.is_empty(),
            "no patterns at all, which cannot be right"
        );
        for s in states {
            let j = crate::liquid_suitability(s.to_string()).unwrap();
            assert_ne!(
                j.reason, "not yet judged on a contact sheet",
                "{s} has no verdict"
            );
            assert!(["recommended", "ok", "notRecommended"].contains(&j.level.as_str()));
        }
        assert!(crate::liquid_suitability("nope".into()).is_none());
        assert_eq!(
            crate::liquid_suitability("metering".into())
                .unwrap()
                .defaults["liquidKeep"],
            1.0
        );
        // Defaults fill only unset keys, and only when liquid is on.
        let off = o(&[]);
        assert!(matches!(
            with_mode_defaults("orbits", &off),
            std::borrow::Cow::Borrowed(_)
        ));
        let on = o(&[("liquidStrength", 1.0), ("liquidReach", 2.0)]);
        let m = with_mode_defaults("orbits", &on);
        assert_eq!(
            (m["liquidReach"], m["liquidThreshold"]),
            (2.0, 0.4),
            "explicit reach wins"
        );
        // Through the engine: working with liquid uses the tuned reach.
        let tuned =
            crate::frame_with_overrides("working".into(), 64, 1.2, o(&[("liquidStrength", 1.0)]))
                .unwrap();
        let manual = crate::frame_with_overrides(
            "working".into(),
            64,
            1.2,
            o(&[
                ("liquidStrength", 1.0),
                ("liquidReach", 4.0),
                ("liquidThreshold", 0.4),
            ]),
        )
        .unwrap();
        assert_eq!(tuned, manual);
    }

    #[test]
    fn kernel_has_finite_support() {
        assert_eq!(kernel(0.0, 4.0), 1.0);
        assert_eq!(kernel(4.0, 4.0), 0.0);
        assert_eq!(kernel(9.0, 4.0), 0.0);
        assert!(kernel(1.0, 4.0) > kernel(2.0, 4.0));
    }

    #[test]
    fn one_source_is_one_closed_round_loop() {
        let g = field(&[(32.0, 32.0, 10.0, 1.0)], 64.0, 40);
        let loops = contours(&g, 0.5);
        assert_eq!(loops.len(), 1);
        // f = (1 - d^2/R^2)^3 = 0.5 -> d = R sqrt(1 - 0.5^(1/3)) ~ 0.454 R.
        let want = 10.0 * (1.0 - 0.5f64.powf(1.0 / 3.0)).sqrt();
        for p in &loops[0] {
            let d = ((p.x - 32.0).powi(2) + (p.y - 32.0).powi(2)).sqrt();
            assert!((d - want).abs() < 0.3, "{d} vs {want}");
        }
    }

    #[test]
    fn near_sources_merge_far_ones_stay_apart() {
        let merged = contours(
            &field(&[(28.0, 32.0, 8.0, 1.0), (36.0, 32.0, 8.0, 1.0)], 64.0, 48),
            0.5,
        );
        assert_eq!(merged.len(), 1, "two close blobs melt into one");
        let apart = contours(
            &field(&[(12.0, 32.0, 8.0, 1.0), (52.0, 32.0, 8.0, 1.0)], 64.0, 48),
            0.5,
        );
        assert_eq!(apart.len(), 2);
    }

    #[test]
    fn a_ring_of_sources_is_a_band_with_a_hole() {
        let dots: Vec<Dot> = (0..24)
            .map(|k| {
                let a = k as f64 / 24.0 * std::f64::consts::TAU;
                dot(32.0 + 20.0 * a.cos(), 32.0 + 20.0 * a.sin(), 1.6)
            })
            .collect();
        let f = apply_liquid(
            frame(dots),
            64.0,
            &o(&[("liquidStrength", 1.0), ("liquidStyle", 0.0)]),
        );
        assert_eq!(f.fills.len(), 1);
        assert_eq!(f.fills[0].holes.len(), 1, "the ring's inside is a hole");
        assert!(f.dots.is_empty() && f.polylines.is_empty());
    }

    #[test]
    fn saddles_resolve_by_the_centre_average() {
        // Two diagonal inside corners meet in the middle cell (a saddle);
        // its centre average decides joined vs apart.
        let two = Grid {
            x0: 0.0,
            y0: 0.0,
            h: 1.0,
            nx: 4,
            ny: 4,
            values: vec![
                0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
        };
        // Centre average of the middle cell = 0.5 -> inside: one joined loop.
        assert_eq!(contours(&two, 0.5).len(), 1);
        // Raise the threshold past the centre average: two separate loops.
        assert_eq!(contours(&two, 0.6).len(), 2);
    }

    #[test]
    fn styles_keep_and_strength() {
        let dots = vec![dot(28.0, 32.0, 2.0), dot(34.0, 32.0, 2.0)];
        let off = apply_liquid(frame(dots.clone()), 64.0, &o(&[]));
        assert_eq!(off.dots, dots, "strength 0 is a no-op");
        let d = apply_liquid(
            frame(dots.clone()),
            64.0,
            &o(&[("liquidStrength", 1.0), ("liquidStyle", 2.0)]),
        );
        assert!(d.dots.len() >= 3 && d.fills.is_empty() && d.polylines.is_empty());
        assert!(d.dots.iter().all(|x| x.r == 2.0));
        let l = apply_liquid(frame(dots.clone()), 64.0, &o(&[("liquidStrength", 1.0)]));
        assert_eq!(l.polylines.len(), 1, "outline by default");
        assert_eq!(
            l.polylines[0].points.first(),
            l.polylines[0].points.last(),
            "closed"
        );
        let k = apply_liquid(
            frame(dots.clone()),
            64.0,
            &o(&[("liquidStrength", 1.0), ("liquidKeep", 1.0)]),
        );
        assert!(k.dots.ends_with(&dots), "sources kept on top");
        let half = apply_liquid(
            frame(dots),
            64.0,
            &o(&[("liquidStrength", 0.5), ("liquidStyle", 0.0)]),
        );
        assert!((half.fills[0].a - 0.5).abs() < 1e-9);
    }
}
