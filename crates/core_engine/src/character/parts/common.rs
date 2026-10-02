//! Parts every character can use: the ground shadow, a body (its fill and
//! light, the light's turn shift, inner layers, the outline) and the eyes.

use crate::character::face::{self, Face};
use crate::character::geom::{self, linear, outline, radial, solid, Hsl};
use crate::character::kit;
use crate::character::parts::{mic, Kind, Part, Reader};
use crate::character::recipe::{Ctx, Space};
use crate::character::region::{Piece, Region, Shape};
use crate::primitives::{Fill, Point};

/// `s` clipped to `region`, each piece a solid fill in `c` at `a`. `all`: also
/// a piece clipped away (the convex band always drew its, empty or not).
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn fill_pieces(
    region: &Region,
    s: &Shape,
    c: Hsl,
    a: f64,
    all: bool,
    ctx: &Ctx,
    space: Space,
    out: &mut Vec<Fill>,
) -> Vec<Piece> {
    let pieces = region.clip(s);
    for (points, holes) in &pieces {
        if all || points.len() >= 3 {
            let f = Fill {
                holes: holes.clone(),
                ..solid(points.clone(), c, a)
            };
            out.push(ctx.place(f, space, None));
        }
    }
    pieces
}

/// The soft shadow under a character (`kit::ground_shadow`); `floats`: it
/// widens with the float rig's drift (Wisp's shadow breathes with it).
pub fn shadow(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let (x, y) = r.p2();
    let (rx, ry) = r.p2();
    let rx = if r.b() { rx + ctx.float } else { rx };
    out.push(ctx.place(kit::ground_shadow(x, y, rx, ry), space, None));
}

fn stops_of(ctx: &Ctx, stops: &[(f64, usize, f64)]) -> Vec<(f64, Hsl)> {
    stops.iter().map(|(o, c, _)| (*o, ctx.colour(*c))).collect()
}

/// A light (`Ty::Light` + its stops) on `points`: linear, radial (circle: the
/// 1.12 fill exactly) or elliptical; the stops' alphas applied.
fn lit(r: &mut Reader, points: Vec<Point>, ctx: &Ctx) -> Option<Fill> {
    let kind = r.n();
    if kind == 2.0 {
        r.stops();
        return None;
    }
    let f = if kind == 1.0 {
        let [x0, y0, x1, y1] = r.v::<4>();
        let st = r.stops();
        let f = linear(points, (x0, y0), (x1, y1), &stops_of(ctx, &st));
        (f, st)
    } else {
        let [cx, cy, rx, ry, angle] = r.v::<5>();
        let (turns, k) = (r.b(), r.p2());
        let st = r.stops();
        let (cx, cy) = if turns {
            (cx + k.0 * ctx.tn.yaw.sin(), cy + k.1 * ctx.tn.pitch.sin())
        } else {
            (cx, cy)
        };
        let s = stops_of(ctx, &st);
        let f = if rx == ry && angle == 0.0 {
            radial(points, (cx, cy), rx, &s)
        } else {
            geom::elliptical(points, (cx, cy), (rx, ry), angle, &s)
        };
        (f, st)
    };
    let (f, st) = f;
    Some(if st.iter().all(|s| s.2 == 1.0) {
        f
    } else {
        geom::stop_alphas(f, &st.iter().map(|s| s.2).collect::<Vec<_>>())
    })
}

fn centre(p: &[Point]) -> (f64, f64) {
    let n = p.len().max(1) as f64;
    (
        p.iter().map(|q| q.x).sum::<f64>() / n,
        p.iter().map(|q| q.y).sum::<f64>() / n,
    )
}

/// A body: a shape filled with its light (radial, its centre staying put while
/// the body turns under it; or linear), inner layers clipped to it, an outline.
pub fn body(r: &mut Reader, inner: &[Part], space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let tn = &ctx.tn;
    let shape = r.shape();
    let body = shape.outer.clone();
    // `"light": "none"`: an overlay body, only its layers (and grain, outline) draw.
    if let Some(fill_light) = lit(r, body.clone(), ctx) {
        let fill_light = Fill {
            holes: shape.holes.clone(),
            ..fill_light
        };
        out.push(ctx.place(fill_light, space, None));
    }
    // What the layers clip to: the body, then (after a band) the band.
    let region = Region::of(&shape);
    let mut band: Option<Region> = None;
    for l in inner {
        let lr = &mut l.params.read();
        match l.kind {
            Kind::Patch => {
                let patch = lr.shape();
                let patch = match lr.surf() {
                    Some(s) if !tn.is_zero() => patch.map(|p| tn.map_points(ctx.surface(s), p)),
                    _ => patch,
                };
                let c = ctx.colour(lr.col());
                fill_pieces(&region, &patch, c, 1.0, false, ctx, space, out);
            }
            Kind::Band => {
                let b = lr.shape();
                let general = region.general || b.path;
                let c = ctx.colour(lr.col());
                let pieces = fill_pieces(&region, &b, c, 1.0, !general, ctx, space, out);
                band = Some(Region::of_pieces(pieces, general));
            }
            Kind::Stripes => {
                let ys = lr.list();
                let rect = lr.v::<5>();
                let (c, alpha) = (ctx.colour(lr.col()), lr.n());
                if !ctx.tier.small {
                    for &y in ys {
                        let edge = geom::round_rect(rect[0], y, rect[1], rect[2], rect[3], rect[4]);
                        fill_pieces(
                            &region,
                            &Shape::plain(edge),
                            c,
                            alpha,
                            false,
                            ctx,
                            space,
                            out,
                        );
                    }
                }
            }
            Kind::Glints => {
                let rects = lr.pairs();
                let rect = lr.v::<4>();
                let (c, alpha) = (ctx.colour(lr.col()), lr.n());
                if !ctx.tier.small && ctx.tier.full {
                    for (y, h) in rects {
                        let g = geom::round_rect(rect[0], y, rect[1], h, rect[2], rect[3]);
                        fill_pieces(&region, &Shape::plain(g), c, alpha, false, ctx, space, out);
                    }
                }
            }
            Kind::Grille => mic::grille(lr, space, ctx, band.as_ref().unwrap_or(&region), out),
            // A soft mass (design note 22): its light fades through the stops'
            // alphas, so it needs no blur; clipped to the body like any layer. On
            // the face's surface it turns with the face (a blush). Off with `shading`.
            Kind::Shade => {
                let sh = lr.shape();
                let surf = lr.surf();
                let lit = lit(lr, Vec::new(), ctx);
                if let (Some(mut template), true) = (lit, ctx.shading) {
                    let sh = match surf {
                        Some(s) if !tn.is_zero() => {
                            template = tn.map_fill(template, ctx.surface(s));
                            sh.map(|p| tn.map_points(ctx.surface(s), p))
                        }
                        _ => sh,
                    };
                    for (points, holes) in region.clip(&sh) {
                        if points.len() >= 3 {
                            let f = Fill {
                                points,
                                holes,
                                ..template.clone()
                            };
                            out.push(ctx.place(f, space, None));
                        }
                    }
                }
            }
            // A rim of light: the body minus itself moved by `offset` (the edge the
            // light reaches first), drawn even-odd as the body with the overlap as
            // a hole; `fade` > 0 fades it out over that distance against the offset.
            Kind::Rim => {
                let (dx, dy) = lr.p2();
                let (c, alpha, fade) = (ctx.colour(lr.col()), lr.n(), lr.n());
                if ctx.shading && !ctx.tier.small {
                    let moved: Vec<Point> =
                        body.iter().map(|p| geom::pt(p.x + dx, p.y + dy)).collect();
                    let holes = if shape.path {
                        crate::character::region::clip_poly(&moved, &body)
                    } else {
                        vec![geom::clip_convex(&moved, &body)]
                    };
                    let f = if fade > 0.0 {
                        let (cx, cy) = centre(&body);
                        let n = dx.hypot(dy).max(1e-9);
                        let from = (cx - dx / n * fade, cy - dy / n * fade);
                        let g = linear(body.clone(), from, (cx, cy), &[(0.0, c), (1.0, c)]);
                        geom::stop_alphas(g, &[1.0, 0.0])
                    } else {
                        solid(body.clone(), c, 1.0)
                    };
                    out.push(ctx.place(
                        Fill {
                            holes,
                            a: alpha,
                            ..f
                        },
                        space,
                        None,
                    ));
                }
            }
            Kind::Eyes => eyes(lr, space, ctx, Some(band.as_ref()), out),
            _ => {}
        }
    }
    // Grain (design note 22): the body's own shape, `blend` 2: the painter fills
    // it with its noise tile at `a`. Under the outline, never on the ground or the face.
    if ctx.grain > 0.0 {
        let f = Fill {
            holes: shape.holes.clone(),
            blend: 2,
            ..solid(
                body.clone(),
                Hsl {
                    h: 0.0,
                    s: 0.0,
                    l: 0.5,
                },
                ctx.grain,
            )
        };
        out.push(ctx.place(f, space, None));
    }
    let w = r.n() * ctx.lw;
    let c = ctx.colour(r.col());
    if w <= 0.0 {
        return;
    }
    if shape.path {
        for ring in std::iter::once(&body).chain(&shape.holes) {
            out.push(ctx.place(geom::outline_miter(ring, w, c, 1.0), space, None));
        }
    } else {
        out.push(ctx.place(outline(&body, w, c, 1.0), space, None));
    }
}

/// The shared shape eyes (`face::eye_fills`), in the effect's colour when one
/// plays. A part on its own (`layer` None): in face space, turned on its
/// surface. A body layer (`layer` Some(band)): clipped to the band, turned on
/// its surface, in the body's space.
pub fn eyes(
    r: &mut Reader,
    space: Space,
    ctx: &Ctx,
    layer: Option<Option<&Region>>,
    out: &mut Vec<Fill>,
) {
    let (x, y) = r.p2();
    let (scale, small_scale) = r.p2();
    let ink = r.col();
    let (glow, small_glow) = r.p2();
    let surface = r.surf();
    let (style, iris, sclera) = ctx.eye_look(r);
    let small = ctx.tier.small;
    let f = Face {
        cx: x,
        cy: y,
        scale: if small { small_scale } else { scale },
        ink: kit::effect_ink(&ctx.pose, ctx.colour(ink)),
        glow: if small { small_glow } else { glow },
        // A convex band clips inside the face code, as before; a path's band after it.
        clip: layer
            .flatten()
            .filter(|r| !r.general)
            .map(|r| r.polys[0].as_slice()),
        style,
        iris,
        sclera,
        small,
    };
    let mut fills = face::eye_fills(&f, &ctx.pose.eyes);
    if let Some(Some(r)) = layer.filter(|l| l.is_some_and(|r| r.general)) {
        let mut clipped = Vec::new();
        for e in fills {
            for (points, holes) in r.clip(&Shape::plain(e.points.clone())) {
                if points.len() >= 3 {
                    clipped.push(Fill {
                        points,
                        holes,
                        ..e.clone()
                    });
                }
            }
        }
        fills = clipped;
    }
    match layer {
        None => out.extend(fills.into_iter().map(|e| ctx.place(e, space, surface))),
        Some(_) => {
            let tn = &ctx.tn;
            for e in fills {
                let e = match surface {
                    Some(s) if !tn.is_zero() => tn.map_fill(e, ctx.surface(s)),
                    _ => e,
                };
                out.push(ctx.place(e, space, None));
            }
        }
    }
}
