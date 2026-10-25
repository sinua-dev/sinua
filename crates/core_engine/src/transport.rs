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
//! Version 2 is emitted **only** when the frame has effect runs, so every
//! other frame stays byte-identical v1. It extends the header to 8 --
//! `[2, colorMode, nDots, nLines, nPolylines, nPoints, found, nEffects]` --
//! keeps v1's sections as they are, then appends:
//! - effects `nEffects x 5` `target start count blur blend`
//!
//! (Versions 2 to 4 once also carried fills, their holes and per-vertex
//! stroke colours; no pattern emits those any more, so they are gone and v2's
//! header shrank. The only reader is `packages/core`, shipped with this
//! writer.)

use crate::primitives::{ColorMode, OrbFrame};

pub const PACKED_LAYOUT_VERSION: f64 = 1.0;
pub const PACKED_LAYOUT_VERSION_2: f64 = 2.0;
pub const HEADER: usize = 7;
#[cfg_attr(not(test), allow(dead_code))] // layout doc; the test reader uses it
pub const HEADER_2: usize = 8;

pub fn pack(frame: Option<&OrbFrame>) -> Vec<f64> {
    let Some(f) = frame else {
        return vec![PACKED_LAYOUT_VERSION, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    };
    let v2 = !f.effects.is_empty();
    let points: usize = f.polylines.iter().map(|p| p.points.len()).sum();
    let mut v = Vec::with_capacity(
        HEADER_2
            + f.dots.len() * 8
            + f.lines.len() * 9
            + f.polylines.len() * 6
            + points * 2
            + f.effects.len() * 5,
    );
    v.extend([
        if v2 {
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
    if v2 {
        v.push(f.effects.len() as f64);
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
    for e in &f.effects {
        v.extend([
            e.target as f64,
            e.start as f64,
            e.count as f64,
            e.blur,
            e.blend as f64,
        ]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{Dot, EffectRun, Line, Point, Polyline};
    use std::collections::HashMap;

    /// The reader `packages/core` implements, in Rust, to prove the layout
    /// round-trips.
    fn unpack(v: &[f64]) -> Option<OrbFrame> {
        if v[6] == 0.0 {
            return None;
        }
        let (nd, nl, np) = (v[2] as usize, v[3] as usize, v[4] as usize);
        let v2 = v[0] == PACKED_LAYOUT_VERSION_2;
        let mut i = if v2 { HEADER_2 } else { HEADER };
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
        let polylines: Vec<Polyline> = (0..np)
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
                }
            })
            .collect();
        let effects = if v2 {
            (0..v[7] as usize)
                .map(|k| {
                    let o = after_polylines + k * 5;
                    EffectRun {
                        target: v[o] as u8,
                        start: v[o + 1] as u32,
                        count: v[o + 2] as u32,
                        blur: v[o + 3],
                        blend: v[o + 4] as u8,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        Some(OrbFrame {
            dots,
            lines,
            polylines,
            color_mode: if v[1] == 1.0 {
                ColorMode::Fixed
            } else {
                ColorMode::Ink
            },
            effects,
        })
    }

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn v2_only_when_there_are_effect_runs() {
        let cases = [
            ("working", o(&[("glowStrength", 1.0), ("glowMode", 1.0)])),
            (
                "working",
                o(&[("glowStrength", 1.0), ("glowMode", 1.0), ("glowBlend", 1.0)]),
            ),
            ("connecting", o(&[("glowStrength", 1.0), ("glowMode", 1.0)])),
            // The light volumes draw one light run over all their dots.
            ("veil", o(&[])),
            ("dune", o(&[("progress", 0.4)])),
        ];
        for (state, ov) in cases {
            let f = crate::frame_with_overrides(state.into(), 64, 1.3, ov).unwrap();
            assert!(!f.effects.is_empty(), "{state}");
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
    fn a_synthetic_frame_with_every_section_round_trips() {
        let pt = |x: f64, y: f64| Point { x, y };
        let frame = OrbFrame {
            dots: vec![Dot {
                x: 1.0,
                y: 2.0,
                z: 0.5,
                r: 3.0,
                white: 0.4,
                a: 0.9,
                saturation: 0.7,
                hue: 200.0,
            }],
            lines: vec![Line {
                x1: 0.0,
                y1: 1.0,
                x2: 2.0,
                y2: 3.0,
                white: 0.5,
                a: 0.6,
                w: 1.5,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![Polyline {
                points: vec![pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 0.5)],
                white: 0.3,
                a: 0.8,
                w: 2.0,
                saturation: 0.2,
                hue: 40.0,
            }],
            color_mode: ColorMode::Fixed,
            effects: vec![EffectRun {
                target: 1,
                start: 0,
                count: 1,
                blur: 1.5,
                blend: 3,
            }],
        };
        let v = pack(Some(&frame));
        assert_eq!(v[0], PACKED_LAYOUT_VERSION_2);
        assert_eq!(unpack(&v).as_ref(), Some(&frame));
    }

    #[test]
    fn packs_and_unpacks_every_primitive_exactly() {
        let cases: [(&str, HashMap<String, f64>); 6] = [
            ("working", HashMap::new()),
            ("connecting", HashMap::new()),
            ("veil", HashMap::new()),
            ("crystal", o(&[("level", 0.6)])),
            // The success effect draws a polyline ring and tick.
            ("working", o(&[("effectCode", 1.0), ("effectAge", 0.3)])),
            ("working", o(&[("colorMix", 1.0), ("colorMode", 1.0)])),
        ];
        for (state, ov) in cases {
            let f = crate::frame_with_overrides(state.into(), 64, 1.3, ov).unwrap();
            let v = pack(Some(&f));
            assert_eq!(unpack(&v).as_ref(), Some(&f), "{state}");
        }
        assert!(unpack(&pack(None)).is_none());
    }
}
