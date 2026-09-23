//! The Web fast path: one `OrbFrame` packed into a flat `f64` buffer, which
//! wasm-bindgen hands to JS as a `Float64Array` -- one memcpy out of linear
//! memory ("the contents of the slice are copied into a JavaScript
//! TypedArray", wasm-bindgen's *Boxed Number Slices*) instead of
//! `serde_json` -> string -> `JSON.parse`, which the orchestrator measured
//! at 10-25x the frame's own compute. `f64`, not `f32`, so an unpacked frame
//! is bit-identical to the JSON one (serde's float output round-trips).
//! A zero-copy `Float64Array::view` was rejected: it's `unsafe` and goes
//! stale on any later allocation (js-sys docs), for a ~33 KB copy.
//!
//! Layout (version 1; `packages/core`'s `unpackFrame`/`readPacked` and
//! docs/platforms/web.md mirror it):
//! - header `[version, colorMode (0 ink, 1 fixed), nDots, nLines,
//!   nPolylines, nPoints, found (1, or 0 = no frame)]`
//! - dots      `nDots x 8`  `x y z r white a saturation hue`
//! - lines     `nLines x 9` `x1 y1 x2 y2 white a w saturation hue`
//! - polylines `nPolylines x 6` `white a w saturation hue pointCount`
//! - points    `nPoints x 2` `x y`, polylines' points back to back
//!
//! Version 2 (materials phase 1) is emitted **only** when the frame has
//! fills or effect runs, so every other frame stays byte-identical v1. It
//! extends the header to 11 -- `[2, colorMode, nDots, nLines, nPolylines,
//! nPoints, found, nFills, nFillPoints, nStops, nEffects]` -- keeps v1's
//! sections as they are, then appends:
//! - fills   `nFills x 14` `white a saturation hue blur blend pointCount
//!   gradKind (-1 none, 0 linear, 1 radial) x0 y0 x1 y1 r stopCount`
//! - fill points `nFillPoints x 2`, back to back
//! - stops   `nStops x 5` `offset white a saturation hue`, back to back
//! - effects `nEffects x 5` `target start count blur blend`
//!
//! Version 3 (materials phase 2) is emitted **only** when some fill has
//! holes (liquid bands): v2 plus `nHoleRings` at header[11] (header 12),
//! fills `x 15` (v2's 14 + `holeRingCount`), and after the fill points a
//! hole-ring table `nHoleRings x 1` (each ring's pointCount; fills in order,
//! rings in order) then the hole points `x 2`; stops and effects as v2.
//!
//! Version 4 (per-vertex stroke colour, 2026-09-19) is emitted **only**
//! when some polyline has `hues`: exactly v3 (all its sections, possibly
//! empty) plus `nPolyHues` at header[12] (header 13), and after the effects
//! a table `nPolylines x 1` (each polyline's hue count: 0, or its
//! pointCount) then the hues back to back, polylines in order.

use crate::primitives::{ColorMode, OrbFrame};

pub const PACKED_LAYOUT_VERSION: f64 = 1.0;
pub const PACKED_LAYOUT_VERSION_2: f64 = 2.0;
pub const HEADER: usize = 7;
#[cfg_attr(not(test), allow(dead_code))] // layout doc; the test reader uses it
pub const HEADER_2: usize = 11;
pub const PACKED_LAYOUT_VERSION_3: f64 = 3.0;
#[cfg_attr(not(test), allow(dead_code))]
pub const HEADER_3: usize = 12;
pub const PACKED_LAYOUT_VERSION_4: f64 = 4.0;
#[cfg_attr(not(test), allow(dead_code))]
pub const HEADER_4: usize = 13;

pub fn pack(frame: Option<&OrbFrame>) -> Vec<f64> {
    let Some(f) = frame else {
        return vec![PACKED_LAYOUT_VERSION, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    };
    let v4 = f.polylines.iter().any(|p| !p.hues.is_empty());
    let v3 = v4 || f.fills.iter().any(|x| !x.holes.is_empty());
    let v2 = v3 || !f.fills.is_empty() || !f.effects.is_empty();
    let points: usize = f.polylines.iter().map(|p| p.points.len()).sum();
    let mut v = Vec::with_capacity(
        HEADER + f.dots.len() * 8 + f.lines.len() * 9 + f.polylines.len() * 6 + points * 2,
    );
    v.extend([
        if v4 {
            PACKED_LAYOUT_VERSION_4
        } else if v3 {
            PACKED_LAYOUT_VERSION_3
        } else if v2 {
            PACKED_LAYOUT_VERSION_2
        } else {
            PACKED_LAYOUT_VERSION
        },
        if f.color_mode == ColorMode::Fixed {
            1.0
        } else {
            0.0
        },
        f.dots.len() as f64,
        f.lines.len() as f64,
        f.polylines.len() as f64,
        points as f64,
        1.0,
    ]);
    let fill_points: usize = f.fills.iter().map(|x| x.points.len()).sum();
    let stops: usize = f
        .fills
        .iter()
        .map(|x| x.gradient.as_ref().map_or(0, |g| g.stops.len()))
        .sum();
    let hole_rings: usize = f.fills.iter().map(|x| x.holes.len()).sum();
    if v2 {
        v.extend([
            f.fills.len() as f64,
            fill_points as f64,
            stops as f64,
            f.effects.len() as f64,
        ]);
    }
    if v3 {
        v.push(hole_rings as f64);
    }
    let poly_hues: usize = f.polylines.iter().map(|p| p.hues.len()).sum();
    if v4 {
        v.push(poly_hues as f64);
    }
    for d in &f.dots {
        v.extend([d.x, d.y, d.z, d.r, d.white, d.a, d.saturation, d.hue]);
    }
    for l in &f.lines {
        v.extend([
            l.x1,
            l.y1,
            l.x2,
            l.y2,
            l.white,
            l.a,
            l.w,
            l.saturation,
            l.hue,
        ]);
    }
    for p in &f.polylines {
        v.extend([
            p.white,
            p.a,
            p.w,
            p.saturation,
            p.hue,
            p.points.len() as f64,
        ]);
    }
    for p in &f.polylines {
        for q in &p.points {
            v.extend([q.x, q.y]);
        }
    }
    if v2 {
        for x in &f.fills {
            let (kind, x0, y0, x1, y1, r, n) = match &x.gradient {
                Some(g) => (g.kind as f64, g.x0, g.y0, g.x1, g.y1, g.r, g.stops.len()),
                None => (-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
            };
            v.extend([
                x.white,
                x.a,
                x.saturation,
                x.hue,
                x.blur,
                x.blend as f64,
                x.points.len() as f64,
                kind,
                x0,
                y0,
                x1,
                y1,
                r,
                n as f64,
            ]);
            if v3 {
                v.push(x.holes.len() as f64);
            }
        }
        for x in &f.fills {
            for q in &x.points {
                v.extend([q.x, q.y]);
            }
        }
        if v3 {
            for x in &f.fills {
                for ring in &x.holes {
                    v.push(ring.len() as f64);
                }
            }
            for x in &f.fills {
                for ring in &x.holes {
                    for q in ring {
                        v.extend([q.x, q.y]);
                    }
                }
            }
        }
        for x in &f.fills {
            if let Some(g) = &x.gradient {
                for st in &g.stops {
                    v.extend([st.offset, st.white, st.a, st.saturation, st.hue]);
                }
            }
        }
        for e in &f.effects {
            v.extend([
                e.target as f64,
                e.start as f64,
                e.count as f64,
                e.blur,
                e.blend as f64,
            ]);
        }
    }
    if v4 {
        v.extend(f.polylines.iter().map(|p| p.hues.len() as f64));
        for p in &f.polylines {
            v.extend(&p.hues);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{
        Dot, EffectRun, Fill, FillGradient, GradientStop, Line, Point, Polyline,
    };
    use std::collections::HashMap;

    /// The reader `packages/core` implements, in Rust, to prove the layout
    /// round-trips.
    fn unpack(v: &[f64]) -> Option<OrbFrame> {
        if v[6] == 0.0 {
            return None;
        }
        let (nd, nl, np) = (v[2] as usize, v[3] as usize, v[4] as usize);
        let v4 = v[0] == PACKED_LAYOUT_VERSION_4;
        let v3 = v4 || v[0] == PACKED_LAYOUT_VERSION_3;
        let v2 = v3 || v[0] == PACKED_LAYOUT_VERSION_2;
        let mut i = if v4 {
            HEADER_4
        } else if v3 {
            HEADER_3
        } else if v2 {
            HEADER_2
        } else {
            HEADER
        };
        let dots = (0..nd)
            .map(|k| {
                let o = i + k * 8;
                Dot {
                    x: v[o],
                    y: v[o + 1],
                    z: v[o + 2],
                    r: v[o + 3],
                    white: v[o + 4],
                    a: v[o + 5],
                    saturation: v[o + 6],
                    hue: v[o + 7],
                }
            })
            .collect();
        i += nd * 8;
        let lines = (0..nl)
            .map(|k| {
                let o = i + k * 9;
                Line {
                    x1: v[o],
                    y1: v[o + 1],
                    x2: v[o + 2],
                    y2: v[o + 3],
                    white: v[o + 4],
                    a: v[o + 5],
                    w: v[o + 6],
                    saturation: v[o + 7],
                    hue: v[o + 8],
                }
            })
            .collect();
        i += nl * 9;
        let mut pts = i + np * 6;
        let npts = v[5] as usize;
        let after_polylines = i + np * 6 + npts * 2;
        let mut polylines: Vec<Polyline> = (0..np)
            .map(|k| {
                let o = i + k * 6;
                let n = v[o + 5] as usize;
                let points = (0..n)
                    .map(|j| Point {
                        x: v[pts + j * 2],
                        y: v[pts + j * 2 + 1],
                    })
                    .collect();
                pts += n * 2;
                Polyline {
                    points,
                    white: v[o],
                    a: v[o + 1],
                    w: v[o + 2],
                    saturation: v[o + 3],
                    hue: v[o + 4],
                    hues: Vec::new(),
                }
            })
            .collect();
        if v4 {
            let ([.., effects], _) = sections(v, after_polylines);
            let table = effects + v[10] as usize * 5;
            let mut h = table + np;
            for (k, p) in polylines.iter_mut().enumerate() {
                let n = v[table + k] as usize;
                p.hues = v[h..h + n].to_vec();
                h += n;
            }
        }
        Some(OrbFrame {
            dots,
            lines,
            polylines,
            color_mode: if v[1] == 1.0 {
                ColorMode::Fixed
            } else {
                ColorMode::Ink
            },
            fills: if v2 {
                unpack_fills(v, after_polylines)
            } else {
                Vec::new()
            },
            effects: if v2 {
                unpack_effects(v, after_polylines)
            } else {
                Vec::new()
            },
        })
    }

    /// Section starts after the polyline points: (fills, fill points,
    /// hole-ring table, hole points, stops, effects) and the fill stride.
    fn sections(v: &[f64], at: usize) -> ([usize; 6], usize) {
        let v3 = v[0] == PACKED_LAYOUT_VERSION_3 || v[0] == PACKED_LAYOUT_VERSION_4;
        let (nf, nfp, ns) = (v[7] as usize, v[8] as usize, v[9] as usize);
        let stride = if v3 { 15 } else { 14 };
        let nh = if v3 { v[11] as usize } else { 0 };
        let fill_pts = at + nf * stride;
        let ring_table = fill_pts + nfp * 2;
        let hole_pts = ring_table + nh;
        let hole_total: usize = (0..nh).map(|k| v[ring_table + k] as usize).sum();
        let stops = hole_pts + hole_total * 2;
        let effects = stops + ns * 5;
        ([at, fill_pts, ring_table, hole_pts, stops, effects], stride)
    }

    fn unpack_fills(v: &[f64], at: usize) -> Vec<Fill> {
        let nf = v[7] as usize;
        let ([_, mut pts, mut ring, mut hpts, mut stops, _], stride) = sections(v, at);
        let v3 = stride == 15;
        let pt = |q: usize| Point {
            x: v[q],
            y: v[q + 1],
        };
        (0..nf)
            .map(|k| {
                let o = at + k * stride;
                let n = v[o + 6] as usize;
                let points = (0..n).map(|j| pt(pts + j * 2)).collect();
                pts += n * 2;
                let mut holes = Vec::new();
                if v3 {
                    for _ in 0..v[o + 14] as usize {
                        let m = v[ring] as usize;
                        ring += 1;
                        holes.push((0..m).map(|j| pt(hpts + j * 2)).collect());
                        hpts += m * 2;
                    }
                }
                let ns = v[o + 13] as usize;
                let gradient = (v[o + 7] >= 0.0).then(|| FillGradient {
                    kind: v[o + 7] as u8,
                    x0: v[o + 8],
                    y0: v[o + 9],
                    x1: v[o + 10],
                    y1: v[o + 11],
                    r: v[o + 12],
                    stops: (0..ns)
                        .map(|j| {
                            let q = stops + j * 5;
                            GradientStop {
                                offset: v[q],
                                white: v[q + 1],
                                a: v[q + 2],
                                saturation: v[q + 3],
                                hue: v[q + 4],
                            }
                        })
                        .collect(),
                });
                stops += if v[o + 7] >= 0.0 { ns * 5 } else { 0 };
                Fill {
                    points,
                    holes,
                    white: v[o],
                    a: v[o + 1],
                    saturation: v[o + 2],
                    hue: v[o + 3],
                    gradient,
                    blur: v[o + 4],
                    blend: v[o + 5] as u8,
                }
            })
            .collect()
    }

    fn unpack_effects(v: &[f64], at: usize) -> Vec<EffectRun> {
        let ne = v[10] as usize;
        let ([.., base], _) = sections(v, at);
        (0..ne)
            .map(|k| {
                let o = base + k * 5;
                EffectRun {
                    target: v[o] as u8,
                    start: v[o + 1] as u32,
                    count: v[o + 2] as u32,
                    blur: v[o + 3],
                    blend: v[o + 4] as u8,
                }
            })
            .collect()
    }

    #[test]
    fn v4_only_when_a_polyline_has_per_vertex_hues() {
        let o = |pairs: &[(&str, f64)]| -> HashMap<String, f64> {
            pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
        };
        // Holo sweeps a ring's tracks per vertex; add a blurred glow (effect
        // runs) and a liquid fill with holes so every section is populated.
        for extra in [
            vec![],
            vec![("glowStrength", 0.8), ("glowMode", 1.0)],
            vec![("glowStrength", 0.8)],
        ] {
            let mut pairs = vec![("holoStrength", 1.0)];
            pairs.extend(extra);
            let f = crate::frame_with_overrides("tracking".into(), 64, 0.6, o(&pairs)).unwrap();
            assert!(f.polylines.iter().any(|p| !p.hues.is_empty()));
            let v = pack(Some(&f));
            assert_eq!(v[0], PACKED_LAYOUT_VERSION_4);
            assert_eq!(unpack(&v).as_ref(), Some(&f), "{pairs:?}");
        }
        // Without per-vertex hues the same state packs exactly as before.
        let plain = crate::frame_with_overrides("tracking".into(), 64, 0.6, o(&[])).unwrap();
        assert!(plain.polylines.iter().all(|p| p.hues.is_empty()));
        assert_eq!(pack(Some(&plain))[0], PACKED_LAYOUT_VERSION);
    }

    #[test]
    fn v3_only_when_a_fill_has_holes() {
        let o = |pairs: &[(&str, f64)]| -> HashMap<String, f64> {
            pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
        };
        // Radar scope rings melt into bands with holes.
        let f = crate::frame_with_overrides(
            "scanning".into(),
            64,
            0.6,
            o(&[
                ("liquidStrength", 1.0),
                ("liquidStyle", 0.0),
                ("liquidKeep", 0.0),
            ]),
        )
        .unwrap();
        let holes = f.fills.iter().map(|x| x.holes.len()).sum::<usize>();
        if holes > 0 {
            let v = pack(Some(&f));
            assert_eq!(v[0], PACKED_LAYOUT_VERSION_3);
            assert_eq!(unpack(&v).as_ref(), Some(&f));
        }
        // A synthetic fill with two holes, a gradient, and an effect run.
        let ring = |c: f64, r: f64| -> Vec<Point> {
            (0..8)
                .map(|k| {
                    let a = k as f64 / 8.0 * std::f64::consts::TAU;
                    Point {
                        x: c + r * a.cos(),
                        y: c + r * a.sin(),
                    }
                })
                .collect()
        };
        let frame = OrbFrame {
            fills: vec![
                Fill {
                    points: ring(32.0, 20.0),
                    holes: vec![ring(26.0, 3.0), ring(38.0, 3.0)],
                    white: 0.2,
                    a: 0.9,
                    gradient: Some(FillGradient {
                        kind: 1,
                        x0: 32.0,
                        y0: 32.0,
                        r: 20.0,
                        stops: vec![
                            GradientStop {
                                offset: 0.0,
                                a: 1.0,
                                ..Default::default()
                            },
                            GradientStop {
                                offset: 1.0,
                                a: 0.0,
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Fill {
                    points: ring(10.0, 4.0),
                    a: 0.5,
                    ..Default::default()
                },
            ],
            effects: vec![EffectRun {
                target: 0,
                start: 0,
                count: 0,
                blur: 1.5,
                blend: 1,
            }],
            ..Default::default()
        };
        let v = pack(Some(&frame));
        assert_eq!(v[0], PACKED_LAYOUT_VERSION_3);
        assert_eq!(unpack(&v).as_ref(), Some(&frame));
    }

    #[test]
    fn v2_only_when_there_are_fills_or_effects() {
        let o = |pairs: &[(&str, f64)]| -> HashMap<String, f64> {
            pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
        };
        let cases = [
            ("generating", o(&[("highlightFill", 1.0)])),
            ("scanning", o(&[("trailFill", 1.0)])),
            ("working", o(&[("glowStrength", 1.0), ("glowMode", 1.0)])),
            (
                "scrolling",
                o(&[("glowStrength", 1.0), ("glowMode", 1.0), ("glowBlend", 1.0)]),
            ),
            ("connecting", o(&[("glowStrength", 1.0), ("glowMode", 1.0)])),
        ];
        for (state, ov) in cases {
            let f = crate::frame_with_overrides(state.into(), 64, 1.3, ov).unwrap();
            assert!(!f.fills.is_empty() || !f.effects.is_empty(), "{state}");
            let v = pack(Some(&f));
            assert_eq!(v[0], PACKED_LAYOUT_VERSION_2, "{state}");
            assert_eq!(unpack(&v).as_ref(), Some(&f), "{state}");
        }
        // Plain frames and stacked glow stay v1.
        for (state, ov) in [
            ("working", o(&[])),
            ("working", o(&[("glowStrength", 1.0)])),
        ] {
            let v = pack(Some(
                &crate::frame_with_overrides(state.into(), 64, 1.3, ov).unwrap(),
            ));
            assert_eq!(v[0], PACKED_LAYOUT_VERSION);
        }
    }

    #[test]
    fn packs_and_unpacks_every_primitive_exactly() {
        let cases: [(&str, HashMap<String, f64>); 5] = [
            ("working", HashMap::new()),
            ("connecting", HashMap::new()),
            ("scrolling", HashMap::new()),
            ("tracking", HashMap::from([("progress0".to_string(), 1.4)])),
            (
                "working",
                HashMap::from([
                    ("colorMix".to_string(), 1.0),
                    ("colorMode".to_string(), 1.0),
                ]),
            ),
        ];
        for (state, ov) in cases {
            let f = crate::frame_with_overrides(state.into(), 64, 1.3, ov).unwrap();
            let v = pack(Some(&f));
            assert_eq!(unpack(&v).as_ref(), Some(&f), "{state}");
        }
        assert!(unpack(&pack(None)).is_none());
    }
}
