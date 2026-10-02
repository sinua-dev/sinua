//! Parts every character can use: the ground shadow, a body (its fill and
//! light, the light's turn shift, inner layers, the outline) and the eyes.

use crate::character::face::{self, Face};
use crate::character::geom::{self, linear, outline, radial, solid, Hsl};
use crate::character::kit;
use crate::character::parts::{mic, Kind, Part, Reader};
use crate::character::recipe::{Ctx, Space};
use crate::character::region::{Piece, Region, Shape};
use crate::primitives::Fill;

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

fn stops_of(ctx: &Ctx, stops: &[(f64, usize)]) -> Vec<(f64, Hsl)> {
    stops.iter().map(|(o, c)| (*o, ctx.colour(*c))).collect()
}

/// A body: a shape filled with its light (radial, its centre staying put while
/// the body turns under it; or linear), inner layers clipped to it, an outline.
pub fn body(r: &mut Reader, inner: &[Part], space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let tn = &ctx.tn;
    let shape = r.shape();
    let body = shape.outer.clone();
    let fill_light = if r.b() {
        let [x0, y0, x1, y1] = r.v::<4>();
        let stops = stops_of(ctx, &r.stops());
        linear(body.clone(), (x0, y0), (x1, y1), &stops)
    } else {
        let [cx, cy, rad] = r.v::<3>();
        let turns = r.b();
        let k = r.p2();
        let stops = stops_of(ctx, &r.stops());
        let (cx, cy) = if turns {
            // The light stays put while the body turns under it.
            let (lx, ly) = (k.0 * tn.yaw.sin(), k.1 * tn.pitch.sin());
            (cx + lx, cy + ly)
        } else {
            (cx, cy)
        };
        radial(body.clone(), (cx, cy), rad, &stops)
    };
    let fill_light = Fill {
        holes: shape.holes.clone(),
        ..fill_light
    };
    out.push(ctx.place(fill_light, space, None));
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
            Kind::Eyes => eyes(lr, space, ctx, Some(band.as_ref()), out),
            _ => {}
        }
    }
    let w = r.n() * ctx.lw;
    let c = ctx.colour(r.col());
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
