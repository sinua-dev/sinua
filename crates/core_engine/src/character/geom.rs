//! Drawing helpers for the character family: shapes tessellated into
//! polygons, strokes turned into filled outlines, and clipping to a convex
//! body -- all engine-side, so a character is nothing but ordinary `Fill`s
//! and the paint contract stays as it is (design-07, decided in Plan Mode
//! 2026-09-30).
//!
//! Why fills only: the paint contract draws by type (fills, then polylines,
//! lines, dots), so a stroked outline would always land on top of every fill
//! -- a torso's outline over the head in front of it. Fills keep their own
//! order, so a character built from fills layers exactly as it is listed.
//!
//! These live here until a second family needs them; then they move to
//! `primitives.rs` (the rule `architecture.md` describes for `arc_polyline`).

use std::f64::consts::PI;

use crate::primitives::{Fill, FillGradient, GradientStop, Point};

pub fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A 2D affine map `(x, y) -> (a x + c y + e, b x + d y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Xf {
    pub const ID: Xf = Xf {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(x: f64, y: f64) -> Xf {
        Xf {
            e: x,
            f: y,
            ..Xf::ID
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Xf {
        Xf {
            a: sx,
            d: sy,
            ..Xf::ID
        }
    }

    pub fn rotate(r: f64) -> Xf {
        let (s, c) = r.sin_cos();
        Xf {
            a: c,
            b: s,
            c: -s,
            d: c,
            e: 0.0,
            f: 0.0,
        }
    }

    /// `self` after `then`: applies `self` first.
    pub fn then(self, o: Xf) -> Xf {
        Xf {
            a: o.a * self.a + o.c * self.b,
            b: o.b * self.a + o.d * self.b,
            c: o.a * self.c + o.c * self.d,
            d: o.b * self.c + o.d * self.d,
            e: o.a * self.e + o.c * self.f + o.e,
            f: o.b * self.e + o.d * self.f + o.f,
        }
    }

    pub fn apply(&self, p: &Point) -> Point {
        pt(
            self.a * p.x + self.c * p.y + self.e,
            self.b * p.x + self.d * p.y + self.f,
        )
    }

    pub fn map(&self, pts: &[Point]) -> Vec<Point> {
        pts.iter().map(|p| self.apply(p)).collect()
    }

    /// The map's mean linear scale (for stroke widths and blur sigmas).
    pub fn scale_factor(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

/// An ellipse, `n` points, counter-clockwise from +x, rotated by `rot`.
#[inline(never)]
pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, n: usize) -> Vec<Point> {
    let (s, c) = rot.sin_cos();
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * 2.0 * PI;
            let (x, y) = (rx * a.cos(), ry * a.sin());
            pt(cx + x * c - y * s, cy + x * s + y * c)
        })
        .collect()
}

/// A superellipse `|x/ax|^e + |y/ay|^e = 1` (e = 2 ellipse, e > 2 squarer).
pub fn superellipse(cx: f64, cy: f64, ax: f64, ay: f64, e: f64, n: usize) -> Vec<Point> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * 2.0 * PI;
            let (c, s) = (a.cos(), a.sin());
            pt(
                cx + ax * c.signum() * c.abs().powf(2.0 / e),
                cy + ay * s.signum() * s.abs().powf(2.0 / e),
            )
        })
        .collect()
}

/// A rounded rectangle, clockwise from the top-left corner's end. Straight
/// edges are subdivided every ~`step` so later per-point edits (the happy
/// eye's arc) have points to move.
pub fn round_rect(x: f64, y: f64, w: f64, h: f64, r: f64, step: f64) -> Vec<Point> {
    // `max(0)`: a recipe's negative size must not panic the clamp.
    let r = r.clamp(0.0, (w.min(h) / 2.0).max(0.0));
    let mut out = Vec::new();
    let corner = |out: &mut Vec<Point>, cx: f64, cy: f64, a0: f64| {
        for k in 0..=6 {
            let a = a0 + k as f64 / 6.0 * PI / 2.0;
            out.push(pt(cx + r * a.cos(), cy + r * a.sin()));
        }
    };
    let edge = |out: &mut Vec<Point>, x0: f64, y0: f64, x1: f64, y1: f64| {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let n = (len / step.max(0.1)).ceil().max(1.0) as usize;
        for k in 1..n {
            let u = k as f64 / n as f64;
            out.push(pt(x0 + (x1 - x0) * u, y0 + (y1 - y0) * u));
        }
    };
    corner(&mut out, x + r, y + r, PI);
    edge(&mut out, x + r, y, x + w - r, y);
    corner(&mut out, x + w - r, y + r, 1.5 * PI);
    edge(&mut out, x + w, y + r, x + w, y + h - r);
    corner(&mut out, x + w - r, y + h - r, 0.0);
    edge(&mut out, x + w - r, y + h, x + r, y + h);
    corner(&mut out, x + r, y + h - r, 0.5 * PI);
    edge(&mut out, x, y + h - r, x, y + r);
    out
}

/// A quadratic Bezier, `n + 1` points including both ends.
pub fn quad(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), n: usize) -> Vec<Point> {
    (0..=n)
        .map(|i| {
            let u = i as f64 / n as f64;
            let v = 1.0 - u;
            pt(
                v * v * p0.0 + 2.0 * v * u * p1.0 + u * u * p2.0,
                v * v * p0.1 + 2.0 * v * u * p1.1 + u * u * p2.1,
            )
        })
        .collect()
}

/// A cubic Bezier, `n + 1` points including both ends.
pub fn cubic(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    n: usize,
) -> Vec<Point> {
    (0..=n)
        .map(|i| {
            let u = i as f64 / n as f64;
            let v = 1.0 - u;
            let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
            pt(
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            )
        })
        .collect()
}

/// An arc of a circle from angle `a0` to `a1` (radians, canvas convention:
/// 0 = +x, clockwise on screen since y grows down), `n + 1` points.
pub fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, n: usize) -> Vec<Point> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f64 / n as f64;
            pt(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Concatenates path pieces, dropping a point that repeats the previous one
/// (where one curve ends and the next begins).
pub fn join(parts: &[Vec<Point>]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    for p in parts {
        for q in p {
            if out
                .last()
                .is_some_and(|l| (l.x - q.x).abs() < 1e-9 && (l.y - q.y).abs() < 1e-9)
            {
                continue;
            }
            out.push(q.clone());
        }
    }
    out
}

/// Shoelace signed area (positive = counter-clockwise in y-up terms).
pub fn signed_area(p: &[Point]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (&p[i], &p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Per-vertex unit normals of a path (averaged at joints).
fn normals(p: &[Point], closed: bool) -> Vec<(f64, f64)> {
    let n = p.len();
    let seg = |i: usize, j: usize| {
        let (dx, dy) = (p[j].x - p[i].x, p[j].y - p[i].y);
        let l = (dx * dx + dy * dy).sqrt().max(1e-9);
        (-dy / l, dx / l)
    };
    (0..n)
        .map(|i| {
            let prev = if i > 0 {
                Some(seg(i - 1, i))
            } else if closed {
                Some(seg(n - 1, 0))
            } else {
                None
            };
            let next = if i + 1 < n {
                Some(seg(i, i + 1))
            } else if closed {
                Some(seg(n - 1, 0))
            } else {
                None
            };
            let (x, y) = match (prev, next) {
                (Some(a), Some(b)) => (a.0 + b.0, a.1 + b.1),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => (0.0, 1.0),
            };
            let l = (x * x + y * y).sqrt().max(1e-9);
            (x / l, y / l)
        })
        .collect()
}

/// An open path stroked `w` wide with round caps, as one polygon. Meant for
/// the gentle curves a character uses (arcs, waves, chevrons); a hairpin
/// turn would fold, which none of these have.
pub fn stroke(p: &[Point], w: f64) -> Vec<Point> {
    if p.len() < 2 {
        return p
            .first()
            .map(|c| ellipse(c.x, c.y, w / 2.0, w / 2.0, 0.0, 12))
            .unwrap_or_default();
    }
    let h = w / 2.0;
    let nm = normals(p, false);
    let mut out: Vec<Point> = p
        .iter()
        .zip(&nm)
        .map(|(q, n)| pt(q.x + n.0 * h, q.y + n.1 * h))
        .collect();
    // End cap: a half circle from the left side round to the right side.
    let cap = |c: &Point, n: (f64, f64), out: &mut Vec<Point>| {
        let a0 = n.1.atan2(n.0);
        for k in 1..8 {
            let a = a0 - k as f64 / 8.0 * PI;
            out.push(pt(c.x + h * a.cos(), c.y + h * a.sin()));
        }
    };
    cap(&p[p.len() - 1], nm[nm.len() - 1], &mut out);
    out.extend(
        p.iter()
            .zip(&nm)
            .rev()
            .map(|(q, n)| pt(q.x - n.0 * h, q.y - n.1 * h)),
    );
    cap(&p[0], (-nm[0].0, -nm[0].1), &mut out);
    out
}

/// A closed outline stroked `w` wide: `(outer, inner)`, painted as one fill
/// with the inner ring as its hole (even-odd).
pub fn ring(p: &[Point], w: f64) -> (Vec<Point>, Vec<Point>) {
    let h = w / 2.0;
    let nm = normals(p, true);
    // Normals point left of travel; which side is "outside" depends on the
    // winding, so pick by area.
    let side = if signed_area(p) > 0.0 { -1.0 } else { 1.0 };
    let off = |d: f64| -> Vec<Point> {
        p.iter()
            .zip(&nm)
            .map(|(q, n)| pt(q.x + n.0 * d * side, q.y + n.1 * d * side))
            .collect()
    };
    (off(h), off(-h))
}

/// Sutherland--Hodgman: `subject` clipped to the **convex** polygon `clip`
/// (either winding). Empty when nothing is inside.
pub fn clip_convex(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    let ccw = signed_area(clip) > 0.0;
    let mut out = subject.to_vec();
    let n = clip.len();
    for i in 0..n {
        if out.is_empty() {
            break;
        }
        let (a, b) = (&clip[i], &clip[(i + 1) % n]);
        let inside = |p: &Point| {
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if ccw {
                cross >= 0.0
            } else {
                cross <= 0.0
            }
        };
        let hit = |p: &Point, q: &Point| {
            let (dx, dy) = (q.x - p.x, q.y - p.y);
            let (ex, ey) = (b.x - a.x, b.y - a.y);
            let den = dx * ey - dy * ex;
            let u = if den.abs() < 1e-12 {
                0.0
            } else {
                ((a.x - p.x) * ey - (a.y - p.y) * ex) / den
            };
            pt(p.x + dx * u, p.y + dy * u)
        };
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (&input[j], &input[(j + 1) % input.len()]);
            match (inside(p), inside(q)) {
                (true, true) => out.push(q.clone()),
                (true, false) => out.push(hit(p, q)),
                (false, true) => {
                    out.push(hit(p, q));
                    out.push(q.clone());
                }
                (false, false) => {}
            }
        }
    }
    out
}

/// Keeps the part of `subject` on the side of the line `a -> b` where `keep`
/// lies (a half-plane clip; the lids of an eye).
pub fn clip_half(subject: &[Point], a: (f64, f64), b: (f64, f64), keep: (f64, f64)) -> Vec<Point> {
    // A huge quad on the kept side stands in for the half-plane.
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
    let (ux, uy) = (dx / l, dy / l);
    let cross = ux * (keep.1 - a.1) - uy * (keep.0 - a.0);
    let (nx, ny) = if cross >= 0.0 { (-uy, ux) } else { (uy, -ux) };
    let big = 1e4;
    let quad = [
        pt(a.0 - ux * big, a.1 - uy * big),
        pt(a.0 + ux * big, a.1 + uy * big),
        pt(a.0 + ux * big + nx * big, a.1 + uy * big + ny * big),
        pt(a.0 - ux * big + nx * big, a.1 - uy * big + ny * big),
    ];
    clip_convex(subject, &quad)
}

/// An engine colour: HSL with `l` as the ink value (the frame is `Fixed`,
/// so it isn't mirrored on dark themes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

pub const fn hsl(h: f64, s: f64, l: f64) -> Hsl {
    Hsl { h, s, l }
}

impl Hsl {
    pub fn rotate(self, dh: f64) -> Hsl {
        Hsl {
            h: (self.h + dh).rem_euclid(360.0),
            ..self
        }
    }
}

/// A solid fill.
pub fn solid(points: Vec<Point>, c: Hsl, a: f64) -> Fill {
    Fill {
        points,
        white: c.l,
        a,
        saturation: c.s,
        hue: c.h,
        ..Default::default()
    }
}

/// A solid fill with a Gaussian blur (sigma, engine units): glows, shadows.
pub fn blurred(points: Vec<Point>, c: Hsl, a: f64, sigma: f64) -> Fill {
    Fill {
        blur: sigma,
        ..solid(points, c, a)
    }
}

fn stops(s: &[(f64, Hsl)]) -> Vec<GradientStop> {
    s.iter()
        .map(|(o, c)| GradientStop {
            offset: *o,
            white: c.l,
            a: 1.0,
            saturation: c.s,
            hue: c.h,
        })
        .collect()
}

/// A fill with a linear gradient from `p0` to `p1` (2-3 stops).
pub fn linear(points: Vec<Point>, p0: (f64, f64), p1: (f64, f64), s: &[(f64, Hsl)]) -> Fill {
    Fill {
        gradient: Some(FillGradient {
            kind: 0,
            x0: p0.0,
            y0: p0.1,
            x1: p1.0,
            y1: p1.1,
            r: 0.0,
            stops: stops(s),
        }),
        ..solid(points, s[s.len() / 2].1, 1.0)
    }
}

/// A fill with a radial gradient centred on `c`, radius `r` (2-3 stops).
#[inline(never)]
pub fn radial(points: Vec<Point>, c: (f64, f64), r: f64, s: &[(f64, Hsl)]) -> Fill {
    Fill {
        gradient: Some(FillGradient {
            kind: 1,
            x0: c.0,
            y0: c.1,
            x1: c.0,
            y1: c.1,
            r,
            stops: stops(s),
        }),
        ..solid(points, s[s.len() / 2].1, 1.0)
    }
}

/// A fill with an elliptical radial gradient (`kind` 2, design note 22): centre
/// `c`, radii `rx` along `angle` (radians) and `ry` across it. The first axis's
/// end rides in `x1, y1` and `ry` in `r`, so the record and the transports are
/// unchanged; a transform maps all three exactly (rotation, squash).
pub fn elliptical(
    points: Vec<Point>,
    c: (f64, f64),
    (rx, ry): (f64, f64),
    angle: f64,
    s: &[(f64, Hsl)],
) -> Fill {
    let (sin, cos) = angle.sin_cos();
    Fill {
        gradient: Some(FillGradient {
            kind: 2,
            x0: c.0,
            y0: c.1,
            x1: c.0 + rx * cos,
            y1: c.1 + rx * sin,
            r: ry,
            stops: stops(s),
        }),
        ..solid(points, s[s.len() / 2].1, 1.0)
    }
}

/// Sets each stop's relative alpha (`stop.a`, painted × `fill.a`).
pub fn stop_alphas(mut f: Fill, a: &[f64]) -> Fill {
    if let Some(g) = f.gradient.as_mut() {
        for (st, a) in g.stops.iter_mut().zip(a) {
            st.a = *a;
        }
    }
    f
}

/// [`ring`] with mitred corners for a path's sharp ones (an ear's tip): each
/// point moves by `w / 2 / cos(half the turn)`, at most `w` (a 2× miter), so the
/// line keeps its width round a corner instead of thinning (design note 13).
pub fn ring_miter(p: &[Point], w: f64) -> (Vec<Point>, Vec<Point>) {
    let n = p.len();
    let nm = normals(p, true);
    let side = if signed_area(p) > 0.0 { -1.0 } else { 1.0 };
    let k: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b) = (&p[(i + n - 1) % n], &p[i]);
            let l = (b.x - a.x).hypot(b.y - a.y).max(1e-9);
            let c = nm[i].0 * -(b.y - a.y) / l + nm[i].1 * (b.x - a.x) / l;
            1.0 / c.max(0.5)
        })
        .collect();
    let off = |d: f64| -> Vec<Point> {
        (0..n)
            .map(|i| {
                pt(
                    p[i].x + nm[i].0 * d * k[i] * side,
                    p[i].y + nm[i].1 * d * k[i] * side,
                )
            })
            .collect()
    };
    (off(w / 2.0), off(-w / 2.0))
}

/// The outline of a path shape: its outer edge and every hole's, mitred.
pub fn outline_miter(p: &[Point], w: f64, c: Hsl, a: f64) -> Fill {
    let (outer, inner) = ring_miter(p, w);
    Fill {
        holes: vec![inner],
        ..solid(outer, c, a)
    }
}

/// A stroked-outline ring fill (the character's ink line).
pub fn outline(p: &[Point], w: f64, c: Hsl, a: f64) -> Fill {
    let (outer, inner) = ring(p, w);
    Fill {
        holes: vec![inner],
        ..solid(outer, c, a)
    }
}

/// Maps every point of a fill (outline, holes, gradient geometry) through
/// `xf`, and scales its blur and gradient radius with it.
pub fn transform(mut f: Fill, xf: &Xf) -> Fill {
    let k = xf.scale_factor();
    f.points = xf.map(&f.points);
    f.holes = f.holes.iter().map(|h| xf.map(h)).collect();
    f.blur *= k;
    if let Some(g) = f.gradient.as_mut() {
        let p0 = xf.apply(&pt(g.x0, g.y0));
        let p1 = xf.apply(&pt(g.x1, g.y1));
        (g.x0, g.y0, g.x1, g.y1) = (p0.x, p0.y, p1.x, p1.y);
        g.r *= k;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside_convex(p: &Point, clip: &[Point]) -> bool {
        let ccw = signed_area(clip) > 0.0;
        (0..clip.len()).all(|i| {
            let (a, b) = (&clip[i], &clip[(i + 1) % clip.len()]);
            let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if ccw {
                c >= -1e-9
            } else {
                c <= 1e-9
            }
        })
    }

    #[test]
    fn shapes_have_the_expected_area() {
        let e = ellipse(0.0, 0.0, 10.0, 5.0, 0.3, 256);
        assert!((signed_area(&e).abs() - PI * 50.0).abs() < 0.2);
        let s = superellipse(0.0, 0.0, 10.0, 10.0, 2.0, 256);
        assert!((signed_area(&s).abs() - PI * 100.0).abs() < 0.2);
        // A squarer superellipse holds more of its box.
        let q = superellipse(0.0, 0.0, 10.0, 10.0, 6.0, 256);
        assert!(signed_area(&q).abs() > signed_area(&s).abs());
        let r = round_rect(0.0, 0.0, 20.0, 10.0, 0.0, 1.0);
        assert!((signed_area(&r).abs() - 200.0).abs() < 1e-6);
        let rr = round_rect(0.0, 0.0, 20.0, 10.0, 5.0, 1.0);
        let expect = 200.0 - (4.0 - PI) * 25.0;
        assert!((signed_area(&rr).abs() - expect).abs() < 1.0); // chords lose a little at each corner
    }

    #[test]
    fn beziers_hit_their_end_points() {
        let q = quad((0.0, 0.0), (5.0, 10.0), (10.0, 0.0), 10);
        assert_eq!((q[0].x, q[0].y, q[10].x, q[10].y), (0.0, 0.0, 10.0, 0.0));
        assert!((q[5].y - 5.0).abs() < 1e-9); // the quad's apex is half the control height
        let c = cubic((0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0), 12);
        assert_eq!((c[12].x, c[12].y), (10.0, 0.0));
    }

    #[test]
    fn a_stroke_is_as_wide_as_asked() {
        let line = vec![pt(0.0, 0.0), pt(10.0, 0.0)];
        let s = stroke(&line, 4.0);
        let (min_y, max_y) = s
            .iter()
            .fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
        assert!((max_y - min_y - 4.0).abs() < 1e-9);
        let (min_x, max_x) = s
            .iter()
            .fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.x), m.1.max(p.x)));
        // Round caps reach half the width past each end.
        assert!(min_x < -1.9 && max_x > 11.9);
        // Area = rectangle + one full circle of the caps.
        assert!((signed_area(&s).abs() - (40.0 + PI * 4.0)).abs() < 0.5);
    }

    #[test]
    fn a_ring_is_centred_on_the_outline() {
        let c = ellipse(0.0, 0.0, 10.0, 10.0, 0.0, 128);
        let (outer, inner) = ring(&c, 2.0);
        let r = |p: &Point| (p.x * p.x + p.y * p.y).sqrt();
        assert!(outer.iter().all(|p| (r(p) - 11.0).abs() < 0.01));
        assert!(inner.iter().all(|p| (r(p) - 9.0).abs() < 0.01));
        // Either winding gives the same ring.
        let rev: Vec<Point> = c.iter().rev().cloned().collect();
        let (o2, _) = ring(&rev, 2.0);
        assert!(o2.iter().all(|p| (r(p) - 11.0).abs() < 0.01));
    }

    #[test]
    fn clipping_keeps_everything_inside_the_clip() {
        let body = superellipse(50.0, 50.0, 30.0, 25.0, 4.0, 96);
        let blob = ellipse(75.0, 75.0, 30.0, 15.0, -0.4, 64);
        let clipped = clip_convex(&blob, &body);
        assert!(!clipped.is_empty());
        assert!(clipped.iter().all(|p| inside_convex(p, &body)));
        assert!(signed_area(&clipped).abs() < signed_area(&blob).abs());
        // Fully inside: unchanged area; fully outside: empty.
        let small = ellipse(50.0, 50.0, 5.0, 5.0, 0.0, 32);
        assert!(
            (signed_area(&clip_convex(&small, &body)).abs() - signed_area(&small).abs()).abs()
                < 1e-9
        );
        assert!(clip_convex(&ellipse(500.0, 500.0, 5.0, 5.0, 0.0, 32), &body).is_empty());
    }

    #[test]
    fn a_half_plane_clip_keeps_the_chosen_side() {
        let sq = round_rect(0.0, 0.0, 10.0, 10.0, 0.0, 1.0);
        let top_cut = clip_half(&sq, (0.0, 3.0), (10.0, 3.0), (5.0, 9.0));
        assert!(top_cut.iter().all(|p| p.y >= 3.0 - 1e-9));
        assert!((signed_area(&top_cut).abs() - 70.0).abs() < 1e-6);
    }

    #[test]
    fn transforms_compose_in_order() {
        let xf = Xf::translate(1.0, 0.0).then(Xf::scale(2.0, 2.0));
        let p = xf.apply(&pt(1.0, 1.0));
        assert_eq!((p.x, p.y), (4.0, 2.0)); // translate first, then scale
        let r = Xf::rotate(PI / 2.0).apply(&pt(1.0, 0.0));
        assert!((r.x).abs() < 1e-12 && (r.y - 1.0).abs() < 1e-12);
        assert!((Xf::scale(2.0, 8.0).scale_factor() - 4.0).abs() < 1e-12);
    }
}
