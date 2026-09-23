//! Crystallize: loose drift snapping into a rigid wireframe polyhedron, for
//! the `concluding` state. NOT a port -- see `docs/effects-research.md`'s
//! "Crystallize" proposal. Dots wander (borrowing `web.rs`'s value-noise
//! drift technique, per the proposal) around their own resting vertex of
//! an icosahedron or octahedron, periodically pull into exact rigidity
//! (edges drawn as `Line`s), hold, then release back to drift -- alternating
//! polyhedra each cycle. Reads as "settling into certainty": loose/organic
//! to rigid/symmetric is a different kind of transition than `morph`
//! (interpolates between outlines, never leaves a flat 2D shape) or `rubik`
//! (twists but never leaves its rigid grid). No golden vector -- same
//! tradeoff as this file's siblings.
//!
//! Vertex coordinates are computed from the closed-form definition (not
//! hand-transcribed), and edges are derived by connecting each vertex to
//! its `k` nearest neighbors by distance (5 for the icosahedron, 4 for the
//! octahedron -- every edge of a regular polyhedron is the same length, so
//! "nearest neighbors" is exactly "connected by an edge") rather than a
//! hand-typed index list, for the same reason `core.rs`'s Perlin
//! permutation table is generated instead of copied: a transcription
//! mistake in 30-some hand-typed index pairs would be silent and hard to
//! catch. `tests` below check the resulting topology (30 edges / degree 5
//! for the icosahedron, 12 edges / degree 4 for the octahedron) against the
//! polyhedra's well-known properties.

use crate::orbs::core::{finalize_frame, radius_scale, vnoise, Dot, Line, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

type Vec3 = (f64, f64, f64);

fn normalize(v: Vec3) -> Vec3 {
    let len = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(1e-9);
    (v.0 / len, v.1 / len, v.2 / len)
}

fn icosahedron_vertices() -> Vec<Vec3> {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let mut v = Vec::with_capacity(12);
    for &s1 in &[-1.0, 1.0] {
        for &s2 in &[-1.0, 1.0] {
            v.push(normalize((0.0, s1 * 1.0, s2 * phi)));
            v.push(normalize((s1 * 1.0, s2 * phi, 0.0)));
            v.push(normalize((s1 * phi, 0.0, s2 * 1.0)));
        }
    }
    v
}

fn octahedron_vertices() -> Vec<Vec3> {
    vec![
        (1.0, 0.0, 0.0),
        (-1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (0.0, -1.0, 0.0),
        (0.0, 0.0, 1.0),
        (0.0, 0.0, -1.0),
    ]
}

/// Connects each vertex to its `k` nearest others by Euclidean distance --
/// exactly the edge set of a regular polyhedron, since every edge is the
/// same length and every non-edge diagonal is strictly longer. Returns
/// deduplicated `(i, j)` pairs with `i < j`.
fn nearest_neighbor_edges(verts: &[Vec3], k: usize) -> Vec<(usize, usize)> {
    let mut edges = std::collections::BTreeSet::new();
    for i in 0..verts.len() {
        let mut dists: Vec<(usize, f64)> = (0..verts.len())
            .filter(|&j| j != i)
            .map(|j| {
                let dx = verts[i].0 - verts[j].0;
                let dy = verts[i].1 - verts[j].1;
                let dz = verts[i].2 - verts[j].2;
                (j, dx * dx + dy * dy + dz * dz)
            })
            .collect();
        dists.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("distance is never NaN"));
        for &(j, _) in dists.iter().take(k) {
            edges.insert((i.min(j), i.max(j)));
        }
    }
    edges.into_iter().collect()
}

fn smoothstep01(edge0: f64, edge1: f64, x: f64) -> f64 {
    if edge0 == edge1 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where in a drift -> snap-in -> hold -> release cycle `local_phase`
/// (`0..1` within one `period`) falls, as a rigidity amount (`0` = fully
/// drifting, `1` = locked to the exact vertex).
fn rigidity_at(local_phase: f64) -> f64 {
    const DRIFT_END: f64 = 0.35;
    const SNAP_END: f64 = 0.45;
    const HOLD_END: f64 = 0.75;
    if local_phase < DRIFT_END {
        0.0
    } else if local_phase < SNAP_END {
        smoothstep01(DRIFT_END, SNAP_END, local_phase)
    } else if local_phase < HOLD_END {
        1.0
    } else {
        1.0 - smoothstep01(HOLD_END, 1.0, local_phase)
    }
}

pub fn frame_crystallize(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.1, 0.32, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let cycle = get(o, "period", 6.0).max(0.5);
    let drift_amp = get(o, "driftAmplitude", 0.35);
    let dot_r = get(o, "dotSize", 1.1);
    let line_w = get(o, "lineWidth", 0.7);

    let cycle_index = (t / cycle).floor();
    let use_octa = (cycle_index.rem_euclid(2.0)) as i64 == 1;
    let (base, k) = if use_octa {
        (octahedron_vertices(), 4)
    } else {
        (icosahedron_vertices(), 5)
    };
    let local_phase = (t / cycle) - cycle_index;
    let rigidity = rigidity_at(local_phase);

    let mut positions: Vec<Vec3> = Vec::with_capacity(base.len());
    for (i, b) in base.iter().enumerate() {
        let i_f = i as f64;
        let seed = i_f * 13.7;
        let ox = (vnoise(seed + 1.0, t * 0.3) - 0.5) * 2.0;
        let oy = (vnoise(seed + 7.0, t * 0.31) - 0.5) * 2.0;
        let oz = (vnoise(seed + 19.0, t * 0.29) - 0.5) * 2.0;
        let drift = normalize((
            b.0 + drift_amp * ox,
            b.1 + drift_amp * oy,
            b.2 + drift_amp * oz,
        ));
        let blended = normalize((
            drift.0 + (b.0 - drift.0) * rigidity,
            drift.1 + (b.1 - drift.1) * rigidity,
            drift.2 + (b.2 - drift.2) * rigidity,
        ));
        positions.push(blended);
    }

    let mut dots: Vec<Dot> = Vec::with_capacity(positions.len());
    for p in &positions {
        let (px, py, z) = pt.project(p.0 * r, p.1 * r, p.2 * r);
        let depth = (z / r + 1.0) / 2.0;
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: dot_r * rs * (0.7 + 0.5 * rigidity),
            white: 0.55 - 0.5 * rigidity - 0.1 * depth,
            a: (0.35 + 0.6 * rigidity) * (0.6 + 0.4 * depth),
            ..Default::default()
        });
    }

    let mut lines: Vec<Line> = Vec::new();
    if rigidity > 0.15 {
        let edge_alpha = ((rigidity - 0.15) / 0.85).clamp(0.0, 1.0);
        for (i, j) in nearest_neighbor_edges(&base, k) {
            let (x1, y1, z1) =
                pt.project(positions[i].0 * r, positions[i].1 * r, positions[i].2 * r);
            let (x2, y2, z2) =
                pt.project(positions[j].0 * r, positions[j].1 * r, positions[j].2 * r);
            let depth = ((z1 + z2) / 2.0 / r + 1.0) / 2.0;
            lines.push(Line {
                x1,
                y1,
                x2,
                y2,
                white: 0.2,
                a: edge_alpha * (0.35 + 0.55 * depth),
                w: line_w * rs,
                saturation: 0.0,
                hue: 0.0,
            });
        }
    }

    finalize_frame(dots, lines, get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    #[test]
    fn icosahedron_topology_is_regular() {
        let v = icosahedron_vertices();
        assert_eq!(v.len(), 12);
        let edges = nearest_neighbor_edges(&v, 5);
        assert_eq!(edges.len(), 30, "a regular icosahedron has 30 edges");
        let mut degree = vec![0; 12];
        for (i, j) in edges {
            degree[i] += 1;
            degree[j] += 1;
        }
        assert!(
            degree.iter().all(|&d| d == 5),
            "every icosahedron vertex has degree 5: {degree:?}"
        );
    }

    #[test]
    fn octahedron_topology_is_regular() {
        let v = octahedron_vertices();
        assert_eq!(v.len(), 6);
        let edges = nearest_neighbor_edges(&v, 4);
        assert_eq!(edges.len(), 12, "a regular octahedron has 12 edges");
        let mut degree = vec![0; 6];
        for (i, j) in edges {
            degree[i] += 1;
            degree[j] += 1;
        }
        assert!(
            degree.iter().all(|&d| d == 4),
            "every octahedron vertex has degree 4: {degree:?}"
        );
    }

    #[test]
    fn snaps_to_exact_vertex_positions_during_hold() {
        let o = opts(&[("period", 4.0), ("rMin", 0.3)]);
        // local_phase = 0.6 -> within the [0.45, 0.75) hold window, rigidity == 1.0 exactly.
        let frame = frame_crystallize(64.0, 4.0 * 0.6, &o);
        let base = icosahedron_vertices();
        let cx = 32.0;
        let cy = 32.0;
        let r = 32.0 * 0.82;
        let pt = Proj::new(4.0 * 0.6 * 0.1, 0.32, cx, cy, 1.0);
        assert_eq!(frame.dots.len(), base.len());
        for (d, b) in frame.dots.iter().zip(base.iter()) {
            // finalize_frame z-sorts, so match by nearest expected point
            // instead of assuming index order survived.
            let expected: Vec<(f64, f64)> = base
                .iter()
                .map(|v| {
                    let (ex, ey, _) = pt.project(v.0 * r, v.1 * r, v.2 * r);
                    (ex, ey)
                })
                .collect();
            let close = expected
                .iter()
                .any(|(ex, ey)| ((d.x - ex).abs() < 1e-6) && ((d.y - ey).abs() < 1e-6));
            assert!(
                close,
                "dot at ({}, {}) doesn't match any exact vertex projection",
                d.x, d.y
            );
            let _ = b;
        }
    }
}
