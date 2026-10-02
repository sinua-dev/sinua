//! HUM's parts: the stand, the yoke that holds the capsule, the grille slots
//! that are its mouth, and the tally light that shows when it listens.

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Mouth};
use crate::character::geom::{self, blurred, outline, pt, solid};
use crate::character::kit;
use crate::character::parts::Reader;
use crate::character::recipe::{Ctx, Space};
use crate::character::region::{Region, Shape};
use crate::primitives::Fill;

/// A stand: a shadowed round foot and a stem.
pub fn stand(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let u = r.v::<4>();
    let f = r.v::<4>();
    let segments = r.u();
    let st = r.v::<6>();
    let dark = ctx.colour(r.col());
    let c = ctx.colour(r.col());
    let lw = r.n() * ctx.lw;
    let line = ctx.colour(r.col());
    out.push(ctx.place(
        solid(
            geom::ellipse(u[0], u[1], u[2], u[3], 0.0, segments),
            dark,
            1.0,
        ),
        space,
        None,
    ));
    let base = geom::ellipse(f[0], f[1], f[2], f[3], 0.0, segments);
    out.push(ctx.place(solid(base.clone(), c, 1.0), space, None));
    out.push(ctx.place(outline(&base, lw, line, 1.0), space, None));
    let stem = geom::round_rect(st[0], st[1], st[2], st[3], st[4], st[5]);
    out.push(ctx.place(solid(stem.clone(), c, 1.0), space, None));
    out.push(ctx.place(outline(&stem, lw, line, 1.0), space, None));
}

/// A U-shaped yoke holding the body at its sides: a dark stroke under a lighter one.
pub fn yoke(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let (l, rx) = r.p2();
    let [top, knee, bottom, mid] = r.v::<4>();
    let segments = r.u();
    let under = (r.n(), r.col());
    let over = (r.n(), r.col());
    let yoke = geom::join(&[
        vec![pt(l, top), pt(l, knee)],
        geom::quad((l, knee), (l, bottom), (mid, bottom), segments),
        geom::quad((mid, bottom), (rx, bottom), (rx, knee), segments),
        vec![pt(rx, top)],
    ]);
    for (w, c) in [under, over] {
        out.push(ctx.place(
            solid(geom::stroke(&yoke, w * ctx.lw), ctx.colour(c), 1.0),
            space,
            None,
        ));
    }
}

/// How lit slot `i` of `n` is (0..1): resting glow, the thinking scanner and the
/// voice, each at its weight.
pub(crate) fn slot_light(i: usize, n: usize, t: f64, m: Mouth, off: bool) -> f64 {
    let rest_glow = 0.05 + 0.03 * (t * 2.0 + i as f64).sin();
    if off {
        return rest_glow;
    }
    let (rest, scan, talk, level, phase) = face::mouth_weights(m);
    let u = i as f64 / (n - 1) as f64;
    // Thinking: one light sweeping to and fro (a smooth ping-pong).
    let pos = ((phase * 2.4).sin() * 0.5 + 0.5) * (n - 1) as f64;
    let scanner = (1.0 - (i as f64 - pos).abs() / 1.3).max(0.0);
    // Speaking: the level, highest in the middle, with a little flicker.
    let env = (u * PI).sin().powf(0.8);
    let voice = (level * env * (1.2 + 0.5 * (i as f64 * 2.3 + t * 11.0).sin())).clamp(0.0, 1.0);
    (rest * rest_glow + scan * (0.1 + 0.9 * scanner) + talk * voice.max(rest_glow)).clamp(0.0, 1.0)
}

/// The grille is the mouth: its slots light with the level while speaking, one
/// light scans them while thinking. A body layer, clipped to the band and
/// wrapped onto the face surface when the head turns.
pub fn grille(r: &mut Reader, space: Space, ctx: &Ctx, band: &Region, out: &mut Vec<Fill>) {
    let count = r.u();
    let [x0, pitch, y] = r.v::<3>();
    let slot_k = r.v::<3>();
    let small_k = r.v::<3>();
    let rise = r.p2();
    let step = r.n();
    let ink = kit::effect_ink(&ctx.pose, ctx.colour(r.col()));
    let surface = r.surf();
    let small = ctx.tier.small;
    let base_w = slot_k[0];
    for i in 0..count {
        let on = slot_light(i, count, ctx.t, ctx.pose.mouth, ctx.mouth_off);
        let (w, h) = if small {
            (small_k[0], small_k[1] + on * small_k[2])
        } else {
            (slot_k[0], slot_k[1] + on * slot_k[2])
        };
        let x = x0 + i as f64 * pitch - (w - base_w) / 2.0;
        let slot = geom::round_rect(x, y - rise.0 - on * rise.1, w, h, w / 2.0, step);
        let slot = match surface {
            Some(s) if !ctx.tn.is_zero() => ctx.tn.map_points(ctx.surface(s), &slot),
            _ => slot,
        };
        for (slot, holes) in band.clip(&Shape::plain(slot)) {
            if slot.len() < 3 {
                continue;
            }
            if on > 0.15 && !small {
                let glow = blurred(slot.clone(), ink, 0.6 * on, 3.0);
                let glow = Fill {
                    holes: holes.clone(),
                    ..glow
                };
                out.push(ctx.place(glow, space, None));
            }
            let lit = Fill {
                holes,
                ..solid(slot, ink, 0.12 + 0.88 * on)
            };
            out.push(ctx.place(lit, space, None));
        }
    }
}

/// The tally light: red while listening ("on air" for the user), amber
/// blinking while thinking, off otherwise.
pub fn tally(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let (x, y) = r.p2();
    let rad = r.n();
    let segments = r.u();
    let [off, listening, thinking] = r.cols::<3>();
    let lw = r.n() * ctx.lw;
    let line = ctx.colour(r.col());
    let tally = geom::ellipse(x, y, rad, rad, 0.0, segments);
    out.push(ctx.place(solid(tally.clone(), ctx.colour(off), 1.0), space, None));
    if ctx.accessories {
        let (_, scan, _, _, _) = face::mouth_weights(ctx.pose.mouth);
        let blink = 0.5 + 0.5 * (ctx.t * TAU * 1.5).sin();
        for (col, a) in [
            (ctx.colour(listening), ctx.pose.ears),
            (ctx.colour(thinking), scan * blink),
        ] {
            if a > 0.02 {
                if !ctx.tier.small {
                    out.push(ctx.place(blurred(tally.clone(), col, 0.7 * a, 4.0), space, None));
                }
                out.push(ctx.place(solid(tally.clone(), col, a), space, None));
            }
        }
    }
    out.push(ctx.place(outline(&tally, lw, line, 1.0), space, None));
}
