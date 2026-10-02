//! The shared face: shape eyes in the Cozmo / Vector line (no pupils, no
//! brows; the eye's *shape* is the expression) and an amplitude-only mouth.
//! Independent of any body: a character mode hands it a [`Face`] (where the
//! face sits, its scale, the colours) and the pose, and gets fills back. A
//! later family can wear the same face by giving it an anchor.
//!
//! Research: sinua-studio/docs/agents/families/research-characters.md
//! (Anki's procedural eyes: expressions are eye polygons, blinks squash the
//! shape to a thin bar; Andrist & Mutlu: averted gaze reads as thinking).
//!
//! Every input is a plain number from the pose ([`super::rig::Pose`]), so a
//! state change interpolates the face instead of snapping it.

use super::geom::{self, blurred, clip_convex, clip_half, hsl, pt, solid, Hsl};
use crate::primitives::{Fill, Point};

/// Which eye shape to draw. `Shape` is the normal, parametric eye; the others
/// are the one-shot effects' expressions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EyeKind {
    Shape,
    /// Celebrate: a four-point sparkle.
    Star,
    /// Error: an X.
    Cross,
}

/// The face's eye and mouth numbers, in design units (the character's
/// 200-unit box, before the face's own `scale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eyes {
    pub w: f64,
    pub h: f64,
    pub r: f64,
    /// Top lid, 0..1 of the height (a squint).
    pub lid: f64,
    /// Lid slope: + lowers the outer corners (a pensive squint; the
    /// prototype's thinking look).
    pub tilt: f64,
    /// The happy arc cut from below, 0..1 (1 = a thin crescent).
    pub smile: f64,
    /// The left eye's height is scaled by `1 - asym` (a quizzical look).
    pub asym: f64,
    /// 0 = shut, 1 = open (blinks).
    pub open: f64,
    /// Gaze offset, design units.
    pub gx: f64,
    pub gy: f64,
    /// Eye spacing multiplier (1 = the default gap).
    pub gap: f64,
    pub kind: EyeKind,
}

impl Default for Eyes {
    fn default() -> Self {
        Eyes {
            w: 20.0,
            h: 26.0,
            r: 8.0,
            lid: 0.0,
            tilt: 0.0,
            smile: 0.0,
            asym: 0.0,
            open: 1.0,
            gx: 0.0,
            gy: 0.0,
            gap: 1.0,
            kind: EyeKind::Shape,
        }
    }
}

/// What the mouth shows. The character picks it from the pose's weights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mouth {
    None,
    /// A small closed smile.
    Smile,
    /// A voice line whose swing follows the level (0..1): speaking. The second
    /// value is the phase (seconds): even waves flow along it, and the level
    /// only sets their height (amplitude only, never the shape).
    Wave(f64, f64),
    /// Three dots, one lit at a time: thinking. The value is the phase.
    Dots(f64),
    /// A wide open smile (success / celebrate).
    Grin,
    /// A small round "O" (the `surprised` expression, design note 16).
    O,
    /// A downturned line (the `sad` expression).
    Frown,
    /// Mid-transition: the smile, the thinking dots and the voice line drawn
    /// together, each at its weight (0..1), so the mouth fades from one to the
    /// next instead of switching at the halfway point.
    Blend {
        smile: f64,
        dots: f64,
        talk: f64,
        /// The voice line's level (as `Wave`).
        level: f64,
        /// The dots' phase (as `Dots`).
        phase: f64,
        /// The expression's resting mouths (design note 16): an "O" and a frown,
        /// each at its weight; 0 without an expression.
        o: f64,
        frown: f64,
    },
}

/// The mouth as weights `(rest, dots, talk, level, phase)`, for a character whose
/// mouth isn't the standard one (Hum's grille, Wisp's oval): each part at its weight,
/// so a state change fades one into the next.
pub fn mouth_weights(m: Mouth) -> (f64, f64, f64, f64, f64) {
    match m {
        Mouth::None => (1.0, 0.0, 0.0, 0.0, 0.0),
        Mouth::Smile => (1.0, 0.0, 0.0, 0.0, 0.0),
        Mouth::Dots(p) => (0.0, 1.0, 0.0, 0.0, p),
        Mouth::Wave(l, p) => (0.0, 0.0, 1.0, l, p),
        Mouth::Grin => (0.0, 0.0, 1.0, 0.6, 0.0),
        Mouth::O | Mouth::Frown => (1.0, 0.0, 0.0, 0.0, 0.0),
        Mouth::Blend {
            smile,
            dots,
            talk,
            level,
            phase,
            ..
        } => (smile, dots, talk, level, phase),
    }
}

/// The expression's resting mouths in `m`: (o, frown), 0 when it has none.
pub fn rest_mouths(m: Mouth) -> (f64, f64) {
    match m {
        Mouth::O => (1.0, 0.0),
        Mouth::Frown => (0.0, 1.0),
        Mouth::Blend { o, frown, .. } => (o, frown),
        _ => (0.0, 0.0),
    }
}

/// Where and how a face is drawn.
pub struct Face<'a> {
    pub cx: f64,
    pub cy: f64,
    /// Design units -> the character's units (eyes and mouth together).
    pub scale: f64,
    /// Eye colour (glowing on a screen, or dark ink on a body).
    pub ink: Hsl,
    /// Glow sigma for the eyes and mouth (0 = no glow).
    pub glow: f64,
    /// Everything is clipped to this convex outline (a screen), if given.
    pub clip: Option<&'a [Point]>,
    /// The eye style (design note 24): 0 shape, 1 glossy, 2 pixel, 3 dot.
    pub style: u8,
    /// The glossy eye's iris colour.
    pub iris: Hsl,
    /// The glossy eye has a white sclera (else a dark lens).
    pub sclera: bool,
    /// The 20 px preset: the glossy eye collapses to the iris and one highlight.
    pub small: bool,
}

const EYE_GAP: f64 = 24.0;
/// The mouth sits this far below the eyes' centre (design units).
const MOUTH_DY: f64 = 22.0;
/// The speaking line: how many even waves, how tall at full level (a share
/// of the half width, so the shape is the same at every width), how fast they
/// flow (radians a second), and its samples.
const WAVE_CYCLES: f64 = 2.0;
const WAVE_HEIGHT: f64 = 0.3;
const WAVE_FLOW: f64 = 8.0;
const WAVE_STEPS: usize = 72;

fn clipped(face: &Face, p: Vec<Point>) -> Vec<Point> {
    match face.clip {
        Some(c) => clip_convex(&p, c),
        None => p,
    }
}

/// One eye's outline, centred on the origin, before the gaze.
pub fn eye_outline(e: &Eyes, side: f64) -> Vec<Point> {
    match e.kind {
        EyeKind::Star => {
            let (ro, ri) = (e.w * 0.62, e.w * 0.2);
            (0..8)
                .map(|i| {
                    let a = i as f64 / 8.0 * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                    let r = if i % 2 == 0 { ro } else { ri };
                    pt(a.cos() * r, a.sin() * r)
                })
                .collect()
        }
        EyeKind::Cross => Vec::new(), // two strokes, see `eye_fills`
        EyeKind::Shape => {
            let h0 = e.h * if side < 0.0 { 1.0 - e.asym } else { 1.0 };
            let h = (h0 * e.open.clamp(0.0, 1.0)).max(1.5);
            let r = e.r.min(h / 2.0).min(e.w / 2.0);
            let mut p = geom::round_rect(-e.w / 2.0, -h / 2.0, e.w, h, r, 1.2);
            // Top lid: a sloped line `lid` of the way down; `tilt` drops the
            // outer corner (mirrored per side).
            if e.lid > 0.0 || e.tilt != 0.0 {
                let y = -h / 2.0 + e.lid * h;
                let k = e.tilt * side * h * 0.5;
                p = clip_half(&p, (-e.w, y - k), (e.w, y + k), (0.0, h));
            }
            // Happy arc: points below an ellipse's top move up onto it,
            // leaving a crescent -- the "^" eye.
            if e.smile > 0.0 {
                let (ecy, rx, ry) = (h * 0.62, e.w * 0.75, h * 0.62 * e.smile);
                for q in p.iter_mut() {
                    if q.x.abs() < rx {
                        let top = ecy - ry * (1.0 - (q.x / rx).powi(2)).sqrt();
                        if q.y > top {
                            q.y = top;
                        }
                    }
                }
            }
            p
        }
    }
}

/// Both eyes as fills (a blurred glow under each when `glow > 0`).
pub fn eye_fills(face: &Face, e: &Eyes) -> Vec<Fill> {
    let mut out = Vec::new();
    let s = face.scale;
    for side in [-1.0, 1.0] {
        let cx = face.cx + (side * EYE_GAP * e.gap + e.gx) * s;
        let cy = face.cy + e.gy * s;
        let parts: Vec<Vec<Point>> = match e.kind {
            EyeKind::Cross => {
                let k = e.w * 0.36;
                let w = e.w * 0.22;
                vec![
                    geom::stroke(&[pt(-k, -k * 0.8), pt(k, k * 0.8)], w),
                    geom::stroke(&[pt(k, -k * 0.8), pt(-k, k * 0.8)], w),
                ]
            }
            _ => {
                // The glossy eye is a little bigger and rounder.
                let k = if face.style == 1 && e.kind == EyeKind::Shape {
                    1.0
                } else {
                    0.0
                };
                let e = Eyes {
                    w: e.w * (1.0 + 0.2 * k),
                    h: e.h * (1.0 + 0.1 * k),
                    r: e.r * (1.0 + 0.5 * k),
                    ..*e
                };
                vec![eye_outline(&e, side)]
            }
        };
        for part in parts {
            let placed: Vec<Point> = part
                .iter()
                .map(|q| pt(cx + q.x * s, cy + q.y * s))
                .collect();
            let placed = clipped(face, placed);
            if placed.len() < 3 {
                continue;
            }
            let style = if e.kind == EyeKind::Shape {
                face.style
            } else {
                0
            };
            match style {
                1 => glossy(face, e, (cx, cy), &placed, &mut out),
                2 => pixels(face, (cx, cy), e.w * s / 4.5, &placed, &mut out),
                3 => dot(face, e, side, (cx, cy), &mut out),
                _ => {
                    if face.glow > 0.0 {
                        out.push(blurred(placed.clone(), face.ink, 0.7, face.glow));
                    }
                    out.push(solid(placed, face.ink, 1.0));
                }
            }
        }
    }
    out
}

fn lighter(c: Hsl, d: f64) -> Hsl {
    Hsl {
        l: (c.l + d).clamp(0.0, 0.97),
        ..c
    }
}

/// `p` clipped to the eye outline (which a smile makes concave).
#[inline(never)]
fn inside_eye(p: Vec<Point>, eye: &[Point]) -> Vec<Vec<Point>> {
    super::region::clip_poly(&p, eye)
        .into_iter()
        .filter(|q| q.len() >= 3)
        .collect()
}

/// An ellipse clipped to the eye.
#[inline(never)]
fn spot(eye: &[Point], (x, y): (f64, f64), rx: f64, ry: f64) -> Vec<Vec<Point>> {
    inside_eye(geom::ellipse(x, y, rx, ry, 0.0, 24), eye)
}

/// The glossy eye (design note 24): a lens or sclera, an iris with a radial
/// gradient and a pupil that follow the gaze inside the eye, highlights that
/// stay with the light, and a lid line; all clipped to the eye's shape, so the
/// blinks and expressions shape it as before.
#[inline(never)]
fn glossy(face: &Face, e: &Eyes, (cx, cy): (f64, f64), eye: &[Point], out: &mut Vec<Fill>) {
    let s = face.scale;
    let (w, h) = (e.w * 1.2 * s, e.h * 1.1 * s);
    let lens = if face.sclera {
        geom::hsl(face.iris.h, 0.15, 0.97)
    } else {
        geom::hsl(face.iris.h, 0.35, 0.09)
    };
    out.push(solid(eye.to_vec(), lens, 1.0));
    let ir = w.min(h) * if face.sclera { 0.42 } else { 0.5 };
    let (ix, iy) = (
        cx + (e.gx * 0.4 * s).clamp(-w * 0.18, w * 0.18),
        cy + (e.gy * 0.4 * s).clamp(-h * 0.15, h * 0.15),
    );
    let iris = face.iris;
    let stops = [
        (0.0, lighter(iris, -iris.l * 0.3)),
        (0.55, iris),
        (1.0, lighter(iris, 0.22)),
    ];
    for p in spot(eye, (ix, iy), ir, ir * 1.06) {
        out.push(geom::radial(p, (ix, iy), ir, &stops));
    }
    if !face.small {
        let dark = geom::hsl(iris.h, 0.4, 0.05);
        for p in spot(eye, (ix, iy), ir * 0.42, ir * 0.46) {
            out.push(solid(p, dark, 1.0));
        }
    }
    // Highlights ride the eye, not the iris (only a fifth of the gaze): the light stays put.
    let (hx, hy) = (cx - e.gx * 0.1 * s, cy - e.gy * 0.1 * s);
    let white = geom::hsl(0.0, 0.0, 1.0);
    let spots: &[(f64, f64, f64, f64)] = if face.small {
        &[(0.2, -0.2, 0.16, 0.95)]
    } else {
        &[
            (0.17, -0.17, 0.17, 0.95),
            (-0.16, 0.2, 0.07, 0.45),
            (0.24, 0.12, 0.05, 0.45),
        ]
    };
    for &(dx, dy, r, a) in spots {
        for p in spot(eye, (hx + dx * w, hy + dy * h), r * w, r * w) {
            out.push(solid(p, white, a));
        }
    }
    // The lid: the eye less itself moved down, a crescent along the top edge.
    if !face.small {
        let k = h * 0.09;
        let moved: Vec<Point> = eye.iter().map(|q| pt(q.x, q.y + k)).collect();
        let holes = inside_eye(moved, eye);
        let (c, a) = if face.sclera {
            (geom::hsl(iris.h, 0.3, 0.08), 0.95)
        } else {
            (lighter(iris, 0.2), 0.45)
        };
        out.push(Fill {
            holes,
            ..solid(eye.to_vec(), c, a)
        });
    }
}

/// Small rounded square cells (octagons) on a grid of pitch `q` through `at`,
/// lit where they fall inside `p` (a thin mouth line: near it too).
#[inline(never)]
fn cells(
    face: &Face,
    at: (f64, f64),
    q: f64,
    p: &[Point],
    a: f64,
    thin: bool,
    out: &mut Vec<Fill>,
) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for v in p {
        (x0, y0, x1, y1) = (x0.min(v.x), y0.min(v.y), x1.max(v.x), y1.max(v.y));
    }
    let (i0, i1) = (
        ((x0 - at.0) / q).floor() as i32,
        ((x1 - at.0) / q).ceil() as i32,
    );
    let (j0, j1) = (
        ((y0 - at.1) / q).floor() as i32,
        ((y1 - at.1) / q).ceil() as i32,
    );
    let k = if thin { q * 0.45 } else { 0.0 };
    let mut lit = 0;
    for j in j0..=j1 {
        for i in i0..=i1 {
            let (x, y) = (at.0 + f64::from(i) * q, at.1 + f64::from(j) * q);
            let on = [(0.0, 0.0), (k, 0.0), (-k, 0.0), (0.0, k), (0.0, -k)]
                .iter()
                .any(|(dx, dy)| super::region::inside(&pt(x + dx, y + dy), p));
            if on {
                lit += 1;
                out.push(solid(cell(x, y, q * 0.4), face.ink, a));
            }
        }
    }
    // A shut eye is a row of cells.
    if lit == 0 && !thin {
        let n = ((x1 - x0) / q / 2.0).floor() as i32;
        for i in -n..=n {
            out.push(solid(
                cell(at.0 + f64::from(i) * q, (y0 + y1) / 2.0, q * 0.4),
                face.ink,
                a,
            ));
        }
    }
}

fn cell(x: f64, y: f64, r: f64) -> Vec<Point> {
    let c = r * 0.45;
    vec![
        pt(x - r + c, y - r),
        pt(x + r - c, y - r),
        pt(x + r, y - r + c),
        pt(x + r, y + r - c),
        pt(x + r - c, y + r),
        pt(x - r + c, y + r),
        pt(x - r, y + r - c),
        pt(x - r, y - r + c),
    ]
}

/// Pixel eyes: the eye's shape lit as a grid of cells, glowing on a screen.
#[inline(never)]
fn pixels(face: &Face, at: (f64, f64), q: f64, eye: &[Point], out: &mut Vec<Fill>) {
    if face.glow > 0.0 {
        out.push(blurred(eye.to_vec(), face.ink, 0.35, face.glow * 1.5));
    }
    cells(face, at, q, eye, 1.0, false, out);
}

/// A light point: a soft glowing dot; blinks squash it, expressions tilt and
/// scale it a little.
#[inline(never)]
fn dot(face: &Face, e: &Eyes, side: f64, (cx, cy): (f64, f64), out: &mut Vec<Fill>) {
    let s = face.scale;
    let r = e.w.min(e.h) * if face.glow > 0.0 { 0.34 } else { 0.3 } * s;
    let h0 = if side < 0.0 { 1.0 - e.asym } else { 1.0 };
    let open = e.open.clamp(0.0, 1.0);
    let ry = (r * h0 * open * (1.0 - e.lid * 0.5) * (1.0 - e.smile * 0.45)).max(0.8 * s);
    let rx = r * (1.0 + (1.0 - open) * 0.25);
    let tilt = e.tilt * side * 0.35;
    let c = face.ink;
    let core = lighter(c, 0.22);
    if !face.small && face.glow > 0.0 {
        let halo = clipped(face, geom::ellipse(cx, cy, rx * 1.7, ry * 1.7, tilt, 24));
        if halo.len() >= 3 {
            out.push(blurred(halo, c, 0.5, r * 0.7));
        }
    }
    let p = clipped(face, geom::ellipse(cx, cy, rx, ry, tilt, 24));
    if p.len() >= 3 {
        out.push(geom::radial(
            p,
            (cx, cy),
            rx.max(ry),
            &[(0.0, core), (1.0, c)],
        ));
    }
}

/// The mouth as fills, centred under the eyes (it follows the gaze halfway).
pub fn mouth_fills(face: &Face, e: &Eyes, mouth: Mouth, half_width: f64) -> Vec<Fill> {
    let s = face.scale;
    let (mx, my) = (
        face.cx + e.gx * 0.5 * s,
        face.cy + (MOUTH_DY + e.gy * 0.3) * s,
    );
    let line_w = 3.2 * s;
    let shapes = mouth_shapes(mouth, mx, my, s, line_w, half_width);
    let mut out = Vec::new();
    if face.style == 2 {
        let q = 3.0 * s;
        for (p, a) in shapes {
            let p = clipped(face, p);
            if p.len() >= 3 {
                cells(face, (mx, my), q, &p, a, true, &mut out);
            }
        }
        return out;
    }
    for (p, a) in shapes {
        let p = clipped(face, p);
        if p.len() < 3 {
            continue;
        }
        if face.glow > 0.0 {
            out.push(blurred(p.clone(), face.ink, 0.6 * a, face.glow * 0.8));
        }
        out.push(solid(p, face.ink, a));
    }
    out
}

/// The mouth's outlines and alphas, before clipping and glow.
fn mouth_shapes(
    mouth: Mouth,
    mx: f64,
    my: f64,
    s: f64,
    line_w: f64,
    half_width: f64,
) -> Vec<(Vec<Point>, f64)> {
    match mouth {
        Mouth::Blend {
            smile,
            dots,
            talk,
            level,
            phase,
            o,
            frown,
        } => {
            let mut out = Vec::new();
            for (w, m) in [
                (smile, Mouth::Smile),
                (dots, Mouth::Dots(phase)),
                (talk, Mouth::Wave(level, phase)),
                (o, Mouth::O),
                (frown, Mouth::Frown),
            ] {
                if w > 0.0 {
                    out.extend(
                        mouth_shapes(m, mx, my, s, line_w, half_width)
                            .into_iter()
                            .map(|(p, a)| (p, a * w.min(1.0))),
                    );
                }
            }
            out
        }
        Mouth::None => Vec::new(),
        Mouth::Smile => vec![(
            geom::stroke(
                &geom::arc(
                    mx,
                    my - 5.0 * s,
                    6.0 * s,
                    0.25 * std::f64::consts::PI,
                    0.75 * std::f64::consts::PI,
                    10,
                ),
                line_w * 0.95,
            ),
            1.0,
        )],
        Mouth::O => vec![(
            geom::ellipse(mx, my - 2.0 * s, 4.0 * s, 5.0 * s, 0.0, 20),
            1.0,
        )],
        Mouth::Frown => vec![(
            geom::stroke(
                &geom::arc(
                    mx,
                    my + 3.0 * s,
                    6.0 * s,
                    1.25 * std::f64::consts::PI,
                    1.75 * std::f64::consts::PI,
                    10,
                ),
                line_w * 0.95,
            ),
            1.0,
        )],
        Mouth::Grin => vec![(
            geom::stroke(
                &geom::arc(
                    mx,
                    my - 6.0 * s,
                    9.0 * s,
                    0.2 * std::f64::consts::PI,
                    0.8 * std::f64::consts::PI,
                    14,
                ),
                line_w,
            ),
            1.0,
        )],
        Mouth::Wave(level, phase) => {
            // Two even waves along the whole mouth (the user's call,
            // 2026-09-30: the same height everywhere, not one peak in the
            // middle), flowing with time; only the last tenth at each end
            // settles onto the line.
            let half = half_width * s;
            let pts: Vec<Point> = (0..=WAVE_STEPS)
                .map(|i| {
                    let u = i as f64 / WAVE_STEPS as f64;
                    let k = (u.min(1.0 - u) / 0.1).min(1.0);
                    let edge = k * k * (3.0 - 2.0 * k);
                    let y = edge
                        * level
                        * WAVE_HEIGHT
                        * half
                        * (std::f64::consts::TAU * WAVE_CYCLES * u - phase * WAVE_FLOW).sin();
                    pt(mx - half + 2.0 * half * u, my + y)
                })
                .collect();
            vec![(geom::stroke(&pts, line_w), 1.0)]
        }
        Mouth::Dots(phase) => (0..3)
            .map(|i| {
                let on = (1.0 - ((phase * 2.2).rem_euclid(3.0) - i as f64).abs()).max(0.0);
                let c = (mx + 6.0 * s + (i as f64 - 1.0) * 8.0 * s, my - 2.0 * s);
                (
                    geom::ellipse(c.0, c.1, 2.4 * s, 2.4 * s, 0.0, 12),
                    0.3 + 0.7 * on,
                )
            })
            .collect(),
    }
}

/// The one-shot effects' colours on a glowing face.
pub const SUCCESS_INK: Hsl = hsl(143.0, 1.0, 0.745);
pub const ERROR_INK: Hsl = hsl(354.0, 1.0, 0.676);
pub const CELEBRATE_INK: Hsl = hsl(44.0, 1.0, 0.676);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::geom::signed_area;

    fn face() -> Face<'static> {
        Face {
            cx: 100.0,
            cy: 90.0,
            scale: 1.0,
            ink: hsl(185.0, 1.0, 0.7),
            glow: 0.0,
            clip: None,
            style: 0,
            iris: hsl(185.0, 1.0, 0.5),
            sclera: false,
            small: false,
        }
    }

    #[test]
    fn a_blink_squashes_the_eye_to_a_bar() {
        let open = eye_outline(&Eyes::default(), 1.0);
        let shut = eye_outline(
            &Eyes {
                open: 0.05,
                ..Eyes::default()
            },
            1.0,
        );
        assert!(signed_area(&shut).abs() < signed_area(&open).abs() * 0.1);
        let height = |p: &[Point]| {
            p.iter().map(|q| q.y).fold(f64::MIN, f64::max)
                - p.iter().map(|q| q.y).fold(f64::MAX, f64::min)
        };
        assert!((height(&shut) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_lid_and_a_smile_take_area_from_the_top_and_bottom() {
        let base = signed_area(&eye_outline(&Eyes::default(), 1.0)).abs();
        let lid = eye_outline(
            &Eyes {
                lid: 0.3,
                ..Eyes::default()
            },
            1.0,
        );
        assert!(lid.iter().all(|q| q.y >= -13.0 + 0.3 * 26.0 - 1e-9));
        assert!(signed_area(&lid).abs() < base * 0.8);
        let happy = eye_outline(
            &Eyes {
                smile: 0.9,
                ..Eyes::default()
            },
            1.0,
        );
        assert!(signed_area(&happy).abs() < base * 0.75);
        // The centre of the bottom edge moved up; the corners didn't go below the eye.
        assert!(happy.iter().filter(|q| q.x.abs() < 2.0).all(|q| q.y < 5.0));
    }

    #[test]
    fn tilt_is_mirrored_between_the_eyes() {
        let e = Eyes {
            lid: 0.2,
            tilt: 0.3,
            ..Eyes::default()
        };
        let (l, r) = (eye_outline(&e, -1.0), eye_outline(&e, 1.0));
        let top_at = |p: &[Point], x: f64| {
            p.iter()
                .filter(|q| (q.x - x).abs() < 3.0)
                .map(|q| q.y)
                .fold(f64::MAX, f64::min)
        };
        // Outer corners are lower (a larger y) than inner ones, on both sides.
        assert!(top_at(&l, -7.0) > top_at(&l, 7.0));
        assert!(top_at(&r, 7.0) > top_at(&r, -7.0));
    }

    #[test]
    fn gaze_moves_both_eyes_and_half_the_mouth() {
        let f = face();
        let e = Eyes {
            gx: 10.0,
            ..Eyes::default()
        };
        let eyes = eye_fills(&f, &e);
        assert_eq!(eyes.len(), 2);
        let cx = |fl: &Fill| fl.points.iter().map(|q| q.x).sum::<f64>() / fl.points.len() as f64;
        assert!((cx(&eyes[0]) - (100.0 - 24.0 + 10.0)).abs() < 0.5);
        let m = mouth_fills(&f, &e, Mouth::Smile, 15.0);
        assert!((cx(&m[0]) - 105.0).abs() < 0.8);
    }

    #[test]
    fn a_quiet_voice_line_is_flat_and_a_loud_one_swings() {
        let f = face();
        let span = |m: &[Fill]| {
            let ys: Vec<f64> = m
                .iter()
                .flat_map(|x| x.points.iter().map(|q| q.y))
                .collect();
            ys.iter().cloned().fold(f64::MIN, f64::max)
                - ys.iter().cloned().fold(f64::MAX, f64::min)
        };
        let quiet = mouth_fills(&f, &Eyes::default(), Mouth::Wave(0.0, 0.0), 15.0);
        let loud = mouth_fills(&f, &Eyes::default(), Mouth::Wave(0.9, 0.0), 15.0);
        assert!(span(&quiet) < 3.3); // just the stroke width
        assert!(span(&loud) > 8.0);
    }

    #[test]
    fn a_blended_mouth_draws_each_shape_at_its_weight() {
        let f = face();
        let e = Eyes::default();
        let smile = mouth_fills(&f, &e, Mouth::Smile, 15.0);
        let dots = mouth_fills(&f, &e, Mouth::Dots(0.0), 15.0);
        let both = mouth_fills(
            &f,
            &e,
            Mouth::Blend {
                smile: 0.3,
                dots: 0.7,
                talk: 0.0,
                level: 0.0,
                phase: 0.0,
                o: 0.0,
                frown: 0.0,
            },
            15.0,
        );
        assert_eq!(
            both.len(),
            smile.len() + dots.len(),
            "a zero weight draws nothing"
        );
        assert!((both[0].a - smile[0].a * 0.3).abs() < 1e-12);
        assert!((both[1].a - dots[0].a * 0.7).abs() < 1e-12);
    }

    #[test]
    fn everything_stays_inside_the_clip() {
        let screen = geom::round_rect(80.0, 80.0, 40.0, 20.0, 6.0, 2.0);
        let f = Face {
            clip: Some(&screen),
            ..face()
        };
        let e = Eyes {
            gy: -20.0,
            ..Eyes::default()
        };
        for fl in eye_fills(&f, &e) {
            assert!(fl.points.iter().all(|q| q.y >= 80.0 - 1e-6));
        }
    }

    #[test]
    fn effect_eyes_are_a_star_and_a_cross() {
        let f = face();
        let star = eye_fills(
            &f,
            &Eyes {
                kind: EyeKind::Star,
                ..Eyes::default()
            },
        );
        assert_eq!(star.len(), 2);
        assert_eq!(star[0].points.len(), 8);
        let cross = eye_fills(
            &f,
            &Eyes {
                kind: EyeKind::Cross,
                ..Eyes::default()
            },
        );
        assert_eq!(cross.len(), 4); // two strokes per eye
    }
}
