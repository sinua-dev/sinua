//! A recipe shape from an SVG path's `d` (FX Spec 1.12, design note 13): the
//! commands `M L H V C S Q T Z`, absolute and relative, in the 200-unit design
//! box. Read once, when the recipe is read: curves are flattened by their
//! length (`geom::cubic` / `geom::quad`, 2–32 pieces each), so a frame costs
//! nothing extra and every platform gets the same points. The first subpath is
//! the outline, the others are holes (painted even-odd, like a mug's handle).
//! No arcs (`A`): an explicit error says to convert them to curves.

use crate::character::geom::{self, pt};
use crate::character::region::Shape;
use crate::primitives::Point;

/// Limits on any path (design note 13).
pub const MAX_BYTES: usize = 16 * 1024;
pub const MAX_COMMANDS: usize = 512;
pub const MAX_POINTS: usize = 512;
pub const MAX_HOLES: usize = 8;
/// About one flattened piece per this many units of curve.
const STEP: f64 = 4.0;

struct Lex<'a> {
    b: &'a [u8],
    i: usize,
    at: &'a str,
}

impl Lex<'_> {
    fn err(&self, what: &str) -> String {
        format!("{}: at {}: {what}", self.at, self.i)
    }
    fn skip(&mut self) {
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_whitespace() || self.b[self.i] == b',')
        {
            self.i += 1;
        }
    }
    /// The next command letter, if one is next.
    fn letter(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.b.get(self.i)?;
        c.is_ascii_alphabetic().then(|| {
            self.i += 1;
            c
        })
    }
    /// An SVG number (`-1.5`, `.5`, `5.`, `1e-3`), read by hand: Rust's float
    /// parser would add ~10 KB to the wasm. Digits accumulate exactly and one
    /// division by a power of ten rounds them, so every platform agrees.
    fn num(&mut self) -> Result<f64, String> {
        self.skip();
        let start = self.i;
        let neg = self.b.get(self.i) == Some(&b'-');
        if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
            self.i += 1;
        }
        let (mut m, mut scale, mut digits, mut point) = (0.0f64, 0i32, 0, false);
        while let Some(&c) = self.b.get(self.i) {
            match c {
                b'0'..=b'9' => {
                    m = m * 10.0 + f64::from(c - b'0');
                    scale -= i32::from(point);
                    digits += 1;
                }
                b'.' if !point => point = true,
                _ => break,
            }
            self.i += 1;
        }
        if digits > 0 && matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            let save = self.i;
            self.i += 1;
            let eneg = self.b.get(self.i) == Some(&b'-');
            if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
                self.i += 1;
            }
            let (mut e, mut any) = (0i32, false);
            while let Some(c @ b'0'..=b'9') = self.b.get(self.i).copied() {
                e = (e * 10 + i32::from(c - b'0')).min(400);
                any = true;
                self.i += 1;
            }
            if any {
                scale += if eneg { -e } else { e };
            } else {
                self.i = save;
            }
        }
        let x = (digits > 0).then(|| {
            let v = if scale < 0 {
                m / 10f64.powi(-scale)
            } else {
                m * 10f64.powi(scale)
            };
            if neg {
                -v
            } else {
                v
            }
        });
        match x {
            Some(x) if x.abs() <= crate::character::recipe::MAX_NUMBER => Ok(x),
            Some(x) => {
                self.i = start;
                Err(self.err(&format!(
                    "{x} is outside ±{}",
                    crate::character::recipe::MAX_NUMBER
                )))
            }
            None => {
                self.i = start;
                Err(self.err("expected a number"))
            }
        }
    }
}

/// Pieces for a curve whose control polygon is `len` long.
fn pieces(len: f64) -> usize {
    ((len / STEP).ceil() as usize).clamp(2, 32)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Reads `d` into a shape; errors are `<at>: at <byte>: <what>`.
pub fn parse(d: &str, at: &str) -> Result<Shape, String> {
    if d.len() > MAX_BYTES {
        return Err(format!("{at}: {} bytes, at most {MAX_BYTES}", d.len()));
    }
    let mut l = Lex {
        b: d.as_bytes(),
        i: 0,
        at,
    };
    let mut subs: Vec<Vec<Point>> = Vec::new();
    let (mut cur, mut start) = ((0.0, 0.0), (0.0, 0.0));
    // The last curve's second control point, for S / T's reflection.
    let mut ctrl: Option<(u8, (f64, f64))> = None;
    let mut cmd: Option<u8> = None;
    let mut commands = 0;
    let mut open = false;
    loop {
        let c = match l.letter() {
            Some(c) => c,
            None if l.i >= l.b.len() => break,
            // Numbers after a command repeat it (after M / m, as L / l).
            None => match cmd {
                Some(c) => c,
                None if subs.is_empty() => return Err(l.err("a path starts with M")),
                None => return Err(l.err("expected a command")),
            },
        };
        commands += 1;
        if commands > MAX_COMMANDS {
            return Err(l.err(&format!("more than {MAX_COMMANDS} commands")));
        }
        let rel = c.is_ascii_lowercase();
        let up = c.to_ascii_uppercase();
        if subs.is_empty() && up != b'M' {
            return Err(l.err("a path starts with M"));
        }
        let point = |l: &mut Lex, cur: (f64, f64)| -> Result<(f64, f64), String> {
            let (x, y) = (l.num()?, l.num()?);
            Ok(if rel { (cur.0 + x, cur.1 + y) } else { (x, y) })
        };
        // A drawing command after Z starts a new subpath at the last start.
        if up != b'M' && !open {
            subs.push(vec![pt(start.0, start.1)]);
            open = true;
        }
        let mut next_ctrl = None;
        match up {
            b'M' => {
                cur = point(&mut l, cur)?;
                start = cur;
                subs.push(vec![pt(cur.0, cur.1)]);
                open = true;
            }
            b'Z' => {
                cur = start;
                open = false;
            }
            b'L' | b'H' | b'V' => {
                cur = match up {
                    b'L' => point(&mut l, cur)?,
                    b'H' => (l.num()? + if rel { cur.0 } else { 0.0 }, cur.1),
                    _ => (cur.0, l.num()? + if rel { cur.1 } else { 0.0 }),
                };
                subs.last_mut().unwrap().push(pt(cur.0, cur.1));
            }
            b'C' | b'S' => {
                let c1 = if up == b'C' {
                    point(&mut l, cur)?
                } else {
                    match ctrl {
                        Some((b'C' | b'S', p)) => (2.0 * cur.0 - p.0, 2.0 * cur.1 - p.1),
                        _ => cur,
                    }
                };
                let c2 = point(&mut l, cur)?;
                let p = point(&mut l, cur)?;
                let n = pieces(dist(cur, c1) + dist(c1, c2) + dist(c2, p));
                subs.last_mut()
                    .unwrap()
                    .extend(geom::cubic(cur, c1, c2, p, n).into_iter().skip(1));
                next_ctrl = Some((up, c2));
                cur = p;
            }
            b'Q' | b'T' => {
                let c1 = if up == b'Q' {
                    point(&mut l, cur)?
                } else {
                    match ctrl {
                        Some((b'Q' | b'T', p)) => (2.0 * cur.0 - p.0, 2.0 * cur.1 - p.1),
                        _ => cur,
                    }
                };
                let p = point(&mut l, cur)?;
                let n = pieces(dist(cur, c1) + dist(c1, p));
                subs.last_mut()
                    .unwrap()
                    .extend(geom::quad(cur, c1, p, n).into_iter().skip(1));
                next_ctrl = Some((up, c1));
                cur = p;
            }
            b'A' => {
                l.i -= 1;
                return Err(l.err("arcs (A) aren't supported; convert them to curves (Figma: Flatten; Inkscape: Path > Object to Path)"));
            }
            _ => {
                l.i -= 1;
                return Err(l.err(&format!("unknown command `{}`", c as char)));
            }
        }
        ctrl = next_ctrl;
        cmd = match up {
            b'Z' => None,
            b'M' if rel => Some(b'l'),
            b'M' => Some(b'L'),
            _ => Some(c),
        };
    }
    // Drop a closing point that repeats the start; then check the subpaths.
    let mut total = 0;
    for s in subs.iter_mut() {
        if s.len() > 1 && dist((s[0].x, s[0].y), (s[s.len() - 1].x, s[s.len() - 1].y)) < 1e-9 {
            s.pop();
        }
        total += s.len();
    }
    subs.retain(|s| s.len() > 1);
    if subs.iter().any(|s| s.len() < 3) || subs.is_empty() {
        return Err(format!("{at}: every subpath needs at least 3 points"));
    }
    if total > MAX_POINTS {
        return Err(format!(
            "{at}: {total} points after flattening, at most {MAX_POINTS}"
        ));
    }
    if subs.len() - 1 > MAX_HOLES {
        return Err(format!(
            "{at}: {} holes, at most {MAX_HOLES}",
            subs.len() - 1
        ));
    }
    let outer = subs.remove(0);
    // Separate islands aren't a thing (design note 13): a subpath outside the
    // outline would paint as an island but clip as a hole, so it's an error.
    if let Some(i) = subs.iter().position(|h| {
        !h.iter()
            .all(|p| crate::character::region::inside(p, &outer))
    }) {
        return Err(format!(
            "{at}: subpath {} isn't inside the outline; later subpaths are holes, so put a separate shape in its own part",
            i + 2
        ));
    }
    Ok(Shape {
        outer,
        holes: subs,
        path: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(d: &str) -> Shape {
        parse(d, "/p").unwrap_or_else(|e| panic!("{d}: {e}"))
    }
    fn err(d: &str) -> String {
        parse(d, "/p").unwrap_err()
    }
    fn xy(p: &Point) -> (f64, f64) {
        (p.x, p.y)
    }

    #[test]
    fn lines_absolute_and_relative_agree() {
        let a = ok("M10 10 L50 10 L50 40 H10 Z");
        let b = ok("m10,10 l40,0 0,30 h-40 z");
        assert_eq!(a, b);
        assert_eq!(
            a.outer.iter().map(xy).collect::<Vec<_>>(),
            [(10.0, 10.0), (50.0, 10.0), (50.0, 40.0), (10.0, 40.0)]
        );
        assert!(a.path && a.holes.is_empty());
        // V, numbers run together, exponents.
        let c = ok("M0 0H1e1V10L-0-.5.5.5z");
        assert_eq!(
            c.outer.iter().map(xy).collect::<Vec<_>>(),
            [
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (-0.0, -0.5),
                (0.5, 0.5)
            ]
        );
    }

    #[test]
    fn curves_flatten_through_their_ends_and_reflect() {
        // A circle of radius 50 from four cubics (k = 0.5523).
        let k = 0.5523 * 50.0;
        let d = format!(
            "M150 100 C150 {a} {b} 150 100 150 C{c} 150 50 {a} 50 100 C50 {d2} {c} 50 100 50 S150 {d2} 150 100 Z",
            a = 100.0 + k, b = 100.0 + k, c = 100.0 - k, d2 = 100.0 - k
        );
        let s = ok(&d);
        for p in &s.outer {
            let r = (p.x - 100.0).hypot(p.y - 100.0);
            assert!((r - 50.0).abs() < 0.2, "{r}");
        }
        assert!(s.outer.len() > 40 && s.outer.len() < 4 * 32);
        // T reflects Q's control point: a symmetric wave.
        let w = ok("M0 50 Q25 0 50 50 T100 50 L100 100 L0 100 Z");
        assert!(w
            .outer
            .iter()
            .any(|p| p.y > 70.0 && p.x > 50.0 && p.x < 100.0));
        // Relative curves are the same curve.
        assert_eq!(
            ok("M10 10 C20 0 40 0 50 10 L30 40 Z"),
            ok("m10 10 c10 -10 30 -10 40 0 l-20 30 z")
        );
    }

    #[test]
    fn later_subpaths_are_holes() {
        let s = ok("M0 0 H100 V100 H0 Z M40 40 H60 V60 H40 Z M70 70 h10 v10 h-10 z");
        assert_eq!(s.holes.len(), 2);
        assert_eq!(s.holes[1][0], pt(70.0, 70.0));
    }

    #[test]
    fn an_island_outside_the_outline_is_an_error() {
        let e = err("M0 0 H50 V50 H0 Z M60 0 H90 V30 H60 Z");
        assert!(e.contains("subpath 2 isn't inside the outline"), "{e}");
    }

    /// No input panics: anything is a shape or an error (a file may come from a server).
    #[test]
    fn random_input_never_panics() {
        let alphabet: &[u8] = b"MLHVCSQTZAmlhvcsqtzae0123456789.-+, \t#";
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..20_000 {
            let len = (x % 60) as usize;
            let d: String = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    alphabet[(x % alphabet.len() as u64) as usize] as char
                })
                .collect();
            let _ = parse(&d, "/p");
        }
        // And well-formed shapes clipped to each other.
        let a = ok("M10 10 C60 -20 120 40 150 10 L170 160 C100 120 60 190 20 150 Z");
        let b = ok("M0 50 Q100 -40 200 50 T200 120 L0 120 Z");
        let _ = crate::character::region::clip_poly(&a.outer, &b.outer);
    }

    #[test]
    fn errors_say_where() {
        assert_eq!(err("L10 10"), "/p: at 1: a path starts with M");
        assert_eq!(err("M0 0 L10 x"), "/p: at 9: expected a number");
        assert_eq!(
            err("M0 0 A5 5 0 0 1 10 10 Z"),
            "/p: at 5: arcs (A) aren't supported; convert them to curves (Figma: Flatten; Inkscape: Path > Object to Path)"
        );
        assert!(err("M0 0 L2000 0 L0 10 Z").contains("outside ±1000"));
        assert!(err("M0 0 L10 0 Z").contains("at least 3 points"));
        assert!(err("M0 0 R1 1").contains("unknown command `R`"));
        let long = format!("M0 0 {} Z", "l1 1 ".repeat(600));
        assert!(err(&long).contains("more than 512 commands"));
        let big = format!("M0 0 {}Z", "c0 0 900 0 0 1 ".repeat(20));
        assert!(err(&big).contains("at most 512"), "{}", err(&big));
        let holes = format!("M0 0 H100 V100 H0 Z{}", " M1 1 h1 v1 h-1 z".repeat(9));
        assert!(err(&holes).contains("9 holes"));
        assert!(err(&"M0 0 ".repeat(4000)).contains("at most 16384"));
    }
}
