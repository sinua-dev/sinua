//! Shapes with holes, and clipping to any simple polygon (FX Spec 1.12, design
//! note 13). A body drawn from a `path` can be concave (a cat's ears, a bean's
//! dimple) and have holes (a mug's handle), so the inner layers clip to it with
//! Greiner–Hormann instead of `geom::clip_convex`, which is only right on a
//! convex body. Ellipse and roundRect bodies keep the convex path exactly, so the
//! built-in characters draw byte for byte as before.

use crate::character::geom::{self, clip_convex, signed_area};
use crate::primitives::Point;

/// A shape: an outline and its holes (painted even-odd). `path`: it came from
/// an SVG path, so it may be concave and gets the general clip and the miter outline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    pub outer: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
    pub path: bool,
}

impl Shape {
    /// An ellipse or roundRect: convex, no holes.
    pub fn plain(outer: Vec<Point>) -> Shape {
        Shape {
            outer,
            holes: Vec::new(),
            path: false,
        }
    }

    /// Every point through `f` (the head turn's surface map).
    pub fn map(&self, f: impl Fn(&[Point]) -> Vec<Point>) -> Shape {
        Shape {
            outer: f(&self.outer),
            holes: self.holes.iter().map(|h| f(h)).collect(),
            path: self.path,
        }
    }
}

/// What inner layers clip to: one convex polygon (`general` false: the old
/// path), or any simple polygons with holes.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub polys: Vec<Vec<Point>>,
    pub holes: Vec<Vec<Point>>,
    pub general: bool,
}

/// A clipped piece: its outline and holes.
pub type Piece = (Vec<Point>, Vec<Vec<Point>>);

impl Region {
    pub fn of(s: &Shape) -> Region {
        Region {
            polys: vec![s.outer.clone()],
            holes: s.holes.clone(),
            general: s.path,
        }
    }

    /// The pieces of `s` inside the region. Convex (neither is a path): exactly
    /// `clip_convex(s, poly)`, one piece, possibly empty, as before.
    pub fn clip(&self, s: &Shape) -> Vec<Piece> {
        if !self.general && !s.path {
            return vec![(clip_convex(&s.outer, &self.polys[0]), Vec::new())];
        }
        let mut out = Vec::new();
        for poly in &self.polys {
            for piece in clip_poly(&s.outer, poly) {
                let mut holes = Vec::new();
                for h in self.holes.iter().chain(&s.holes) {
                    holes.extend(clip_poly(h, &piece));
                }
                out.push((piece, holes));
            }
        }
        out
    }

    /// The region left after clipping `s` to it (a band inside a body).
    pub fn of_pieces(pieces: Vec<Piece>, general: bool) -> Region {
        let mut r = Region {
            polys: Vec::new(),
            holes: Vec::new(),
            general,
        };
        for (p, h) in pieces {
            r.polys.push(p);
            r.holes.extend(h);
        }
        r
    }
}

/// Even-odd point in polygon.
pub fn inside(p: &Point, poly: &[Point]) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (&poly[i], &poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
    }
    c
}

/// Distance from `p` to segment `a`–`b`.
fn seg_dist(p: &Point, a: &Point, b: &Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let u = if l2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - u * dx).hypot(p.y - a.y - u * dy)
}

const EPS: f64 = 1e-7;

/// Moves every vertex of `p` that touches an edge of `q` a tiny fixed step
/// off it, so no intersection lands on a vertex (Greiner–Hormann's degenerate
/// case). Deterministic: the same input moves the same way everywhere.
fn nudge(p: &mut [Point], q: &[Point]) -> bool {
    let mut moved = false;
    let n = q.len();
    for v in p.iter_mut() {
        let mut k = 0;
        while k < 8 && (0..n).any(|j| seg_dist(v, &q[j], &q[(j + 1) % n]) < EPS) {
            v.x += 3.0 * EPS;
            v.y += 5.0 * EPS;
            moved = true;
            k += 1;
        }
    }
    moved
}

/// Where segments `p0`–`p1` and `q0`–`q1` cross, strictly inside both:
/// `(along p, along q)`.
fn cross(p0: &Point, p1: &Point, q0: &Point, q1: &Point) -> Option<(f64, f64)> {
    let (rx, ry) = (p1.x - p0.x, p1.y - p0.y);
    let (sx, sy) = (q1.x - q0.x, q1.y - q0.y);
    let den = rx * sy - ry * sx;
    if den.abs() < 1e-15 {
        return None;
    }
    let (wx, wy) = (q0.x - p0.x, q0.y - p0.y);
    let a = (wx * sy - wy * sx) / den;
    let b = (wx * ry - wy * rx) / den;
    (a > 0.0 && a < 1.0 && b > 0.0 && b < 1.0).then_some((a, b))
}

struct Node {
    p: Point,
    /// The intersection's index, for a crossing.
    hit: Option<usize>,
    entry: bool,
}

/// Greiner–Hormann: the parts of simple polygon `s` inside simple polygon `c`
/// (either winding), as separate polygons.
pub fn clip_poly(s: &[Point], c: &[Point]) -> Vec<Vec<Point>> {
    if s.len() < 3 || c.len() < 3 {
        return Vec::new();
    }
    let mut s = s.to_vec();
    let mut c = c.to_vec();
    for _ in 0..4 {
        let a = nudge(&mut s, &c);
        let b = nudge(&mut c, &s);
        if !a && !b {
            break;
        }
    }
    let (ns, nc) = (s.len(), c.len());
    // Every crossing: (subject edge, along it, clip edge, along it, point).
    let mut hits: Vec<(usize, f64, usize, f64, Point)> = Vec::new();
    for i in 0..ns {
        let (p0, p1) = (&s[i], &s[(i + 1) % ns]);
        for j in 0..nc {
            let (q0, q1) = (&c[j], &c[(j + 1) % nc]);
            if let Some((a, b)) = cross(p0, p1, q0, q1) {
                let at = geom::pt(p0.x + (p1.x - p0.x) * a, p0.y + (p1.y - p0.y) * a);
                hits.push((i, a, j, b, at));
            }
        }
    }
    if hits.is_empty() {
        return if inside(&s[0], &c) {
            vec![s]
        } else if inside(&c[0], &s) {
            vec![c]
        } else {
            Vec::new()
        };
    }
    // Both vertex lists with the crossings inserted in order along each edge.
    let list = |poly: &[Point], edge: &dyn Fn(usize) -> (usize, f64)| -> Vec<Node> {
        let mut out = Vec::new();
        for (i, p) in poly.iter().enumerate() {
            out.push(Node {
                p: p.clone(),
                hit: None,
                entry: false,
            });
            let mut on: Vec<(f64, usize)> = (0..hits.len())
                .filter_map(|k| {
                    let (e, a) = edge(k);
                    (e == i).then_some((a, k))
                })
                .collect();
            // Insertion sort: a few crossings per edge, and no new sort in the wasm.
            for a in 1..on.len() {
                let mut b = a;
                while b > 0 && on[b - 1].0 > on[b].0 {
                    on.swap(b - 1, b);
                    b -= 1;
                }
            }
            for (_, k) in on {
                out.push(Node {
                    p: hits[k].4.clone(),
                    hit: Some(k),
                    entry: false,
                });
            }
        }
        out
    };
    let mut sl = list(&s, &|k| (hits[k].0, hits[k].1));
    let mut cl = list(&c, &|k| (hits[k].2, hits[k].3));
    // Entry/exit: the first crossing enters when the list starts outside.
    for (l, start_in) in [(&mut sl, inside(&s[0], &c)), (&mut cl, inside(&c[0], &s))] {
        let mut entry = !start_in;
        for n in l.iter_mut().filter(|n| n.hit.is_some()) {
            n.entry = entry;
            entry = !entry;
        }
    }
    let pos = |l: &[Node], k: usize| l.iter().position(|n| n.hit == Some(k)).unwrap();
    let mut seen = vec![false; hits.len()];
    let mut out = Vec::new();
    while let Some(start) = (0..hits.len()).find(|k| !seen[*k]) {
        let mut poly: Vec<Point> = Vec::new();
        let (mut on_s, mut k) = (true, start);
        // Bounded: each step visits a crossing; a well-formed trace closes in
        // at most every crossing once.
        for _ in 0..=hits.len() {
            seen[k] = true;
            let l = if on_s { &sl } else { &cl };
            let n = l.len();
            let mut i = pos(l, k);
            let forward = l[i].entry;
            poly.push(l[i].p.clone());
            loop {
                i = if forward {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                };
                match l[i].hit {
                    Some(next) => {
                        k = next;
                        break;
                    }
                    None => poly.push(l[i].p.clone()),
                }
            }
            on_s = !on_s;
            if k == start {
                break;
            }
        }
        if poly.len() >= 3 && signed_area(&poly).abs() > 1e-9 {
            out.push(poly);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::geom::pt;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
        vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]
    }

    fn area(ps: &[Vec<Point>]) -> f64 {
        ps.iter().map(|p| signed_area(p).abs()).sum()
    }

    #[test]
    fn overlapping_squares_meet_in_their_overlap() {
        let r = clip_poly(&rect(0.0, 0.0, 10.0, 10.0), &rect(5.0, 5.0, 10.0, 10.0));
        assert_eq!(r.len(), 1);
        assert!((area(&r) - 25.0).abs() < 1e-4, "{}", area(&r));
    }

    #[test]
    fn a_band_across_a_u_is_two_pieces() {
        // A U (two ears with a dip between): the band crosses both arms.
        let u = vec![
            pt(0.0, 0.0),
            pt(10.0, 0.0),
            pt(10.0, 20.0),
            pt(30.0, 20.0),
            pt(30.0, 0.0),
            pt(40.0, 0.0),
            pt(40.0, 40.0),
            pt(0.0, 40.0),
        ];
        let band = rect(-5.0, 5.0, 50.0, 10.0);
        let r = clip_poly(&band, &u);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!((area(&r) - 200.0).abs() < 1e-4, "{}", area(&r));
        // Either winding of the clip gives the same.
        let mut rev = u.clone();
        rev.reverse();
        assert!((area(&clip_poly(&band, &rev)) - 200.0).abs() < 1e-4);
        // The convex clip gets this wrong: that's why paths take this one.
        assert!(clip_convex(&band, &u).len() < 3);
    }

    #[test]
    fn inside_outside_and_touching() {
        let big = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(clip_poly(&rect(2.0, 2.0, 2.0, 2.0), &big).len(), 1);
        assert_eq!(clip_poly(&big, &rect(2.0, 2.0, 2.0, 2.0)).len(), 1);
        assert!(clip_poly(&rect(20.0, 0.0, 2.0, 2.0), &big).is_empty());
        // Shared edges and vertices (the degenerate case) still give the overlap.
        let r = clip_poly(&rect(0.0, 0.0, 10.0, 5.0), &big);
        assert!((area(&r) - 50.0).abs() < 1e-3, "{}", area(&r));
        let r = clip_poly(&rect(5.0, 0.0, 10.0, 10.0), &big);
        assert!((area(&r) - 50.0).abs() < 1e-3, "{}", area(&r));
    }

    #[test]
    fn a_region_with_a_hole_cuts_it_from_the_piece() {
        let mut body = Shape::plain(rect(0.0, 0.0, 20.0, 20.0));
        body.holes = vec![rect(8.0, 8.0, 4.0, 4.0)];
        body.path = true;
        let pieces = Region::of(&body).clip(&Shape::plain(rect(5.0, 5.0, 10.0, 10.0)));
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].1.len(), 1, "the hole falls in the piece");
        // Convex and not a path: exactly the old clip.
        let plain = Region::of(&Shape::plain(rect(0.0, 0.0, 20.0, 20.0)));
        let s = rect(5.0, 5.0, 30.0, 10.0);
        assert_eq!(
            plain.clip(&Shape::plain(s.clone()))[0].0,
            clip_convex(&s, &plain.polys[0])
        );
    }
}
