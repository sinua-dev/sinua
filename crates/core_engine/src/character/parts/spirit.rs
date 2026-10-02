//! WISP's parts: a soft halo, a body whose round head flows into a curling
//! smoke tail (smoke puffs, a shine, a tail that trails the head turn), a mouth
//! that is a small smile, the thinking dots or an oval that opens with the
//! voice, and sparkles that wander, gather, orbit and stream.

use std::f64::consts::{PI, TAU};

use crate::character::face::{self, Face, Mouth};
use crate::character::geom::{self, blurred, linear, outline, pt, solid, Hsl};
use crate::character::kit;
use crate::character::parts::Reader;
use crate::character::recipe::{Ctx, Space};
use crate::character::turn;
use crate::primitives::{Fill, Point};

/// A soft halo: a radial fade (no blur cost; Safari draws a blurred gradient sharp).
pub fn halo(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if ctx.tier.small {
        return;
    }
    let at = r.p2();
    let rad = r.n();
    let segments = r.u();
    let c = ctx.colour(r.col());
    let stops = r.pairs();
    let mut halo = geom::radial(
        geom::ellipse(at.0, at.1, rad, rad, 0.0, segments),
        at,
        rad,
        &stops.iter().map(|(o, _)| (*o, c)).collect::<Vec<_>>(),
    );
    if let Some(g) = halo.gradient.as_mut() {
        for (s, (_, a)) in g.stops.iter_mut().zip(&stops) {
            s.a = *a;
        }
    }
    out.push(ctx.place(halo, space, None));
}

/// The outline: the round head flowing into a tail that curls by `curl` and
/// sways with `sw`.
fn path(curl: f64, sw: f64) -> Vec<Point> {
    geom::join(&[
        geom::cubic((54.0, 86.0), (52.0, 38.0), (148.0, 38.0), (146.0, 86.0), 24),
        geom::cubic(
            (146.0, 86.0),
            (144.0, 122.0),
            (128.0, 142.0),
            (118.0 + sw, 158.0),
            14,
        ),
        geom::cubic(
            (118.0 + sw, 158.0),
            (110.0 + sw, 172.0),
            (120.0 + curl * 22.0 + sw, 186.0),
            (136.0 + curl * 10.0, 176.0 - curl * 14.0),
            14,
        ),
        geom::cubic(
            (136.0 + curl * 10.0, 176.0 - curl * 14.0),
            (120.0 + curl * 8.0, 192.0),
            (88.0 + sw, 182.0),
            (92.0 + sw * 0.5, 154.0),
            14,
        ),
        geom::cubic(
            (92.0 + sw * 0.5, 154.0),
            (70.0, 136.0),
            (55.0, 118.0),
            (54.0, 86.0),
            14,
        ),
    ])
}

/// The spirit's body: smoke puffs off the tail tip (64 px), the head flowing into
/// a tail that curls with `curlGain` (tighter as it speaks), sways, and trails
/// the head turn a moment late; a shine on the head; the outline.
pub fn spirit(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let [hx, hy, hr] = r.v::<3>();
    let stops: Vec<(f64, Hsl)> = r
        .stops()
        .into_iter()
        .map(|(o, c)| (o, ctx.colour(c)))
        .collect();
    let line = ctx.colour(r.col());
    let smoke = ctx.colour(r.col());
    let shine_c = ctx.colour(r.col());
    let (lag, swing) = r.p2();
    let (_, _, talk, level, _) = ctx.w;
    let (t, tn) = (ctx.t, &ctx.tn);
    let trail = -swing * turn::angles_lagged(ctx.o, t, ctx.tier, lag).yaw.sin();
    // Smoke puffs drifting off the tail tip and fading (the tail's smokiness).
    let curl = ctx.get("curlGain", 0.6) + talk * level * 0.4;
    let sw = (t * 2.4).sin() * 6.0;
    if ctx.tier.full {
        for k in 0..2 {
            let u = (t * 0.35 + k as f64 * 0.5).rem_euclid(1.0);
            let (x, y) = (
                136.0 + curl * 10.0 + u * 14.0,
                176.0 - curl * 14.0 + u * 10.0,
            );
            out.push(ctx.place(
                blurred(
                    geom::ellipse(x, y, 5.0 + u * 5.0, 4.0 + u * 4.0, 0.0, 20),
                    smoke,
                    0.35 * (1.0 - u),
                    3.0,
                ),
                space,
                None,
            ));
        }
    }
    let body = path(curl, sw + trail);
    out.push(ctx.place(
        linear(body.clone(), (0.0, 40.0), (0.0, 190.0), &stops),
        space,
        None,
    ));
    let head = geom::ellipse(hx, hy, hr, hr, 0.0, 48);
    if !ctx.tier.small {
        // The light stays put while the head turns under it.
        let (lx, ly) = (-10.0 * tn.yaw.sin(), 8.0 * tn.pitch.sin());
        let shine = geom::clip_convex(
            &geom::ellipse(80.0 + lx, 58.0 + ly, 18.0, 9.0, -0.5, 24),
            &head,
        );
        if shine.len() >= 3 {
            out.push(ctx.place(blurred(shine, shine_c, 0.5, 4.0), space, None));
        }
    }
    out.push(ctx.place(outline(&body, 3.0 * ctx.tier.line, line, 1.0), space, None));
}

/// A mouth that is a small smile, the thinking dots, or an oval that opens with
/// the voice, each at its weight; on an effect, the shared effect mouths.
pub fn oval_mouth(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if ctx.mouth_off {
        return;
    }
    let (x, y) = r.p2();
    let (scale, small_scale) = r.p2();
    let half_width = r.n();
    let ink = kit::effect_ink(&ctx.pose, ctx.colour(r.col()));
    let surface = r.surf();
    let pose = &ctx.pose;
    let (rest, dots, talk, level, phase) = ctx.w;
    let f = Face {
        cx: x,
        cy: y,
        scale: if ctx.tier.small { small_scale } else { scale },
        ink,
        glow: 0.0,
        clip: None,
    };
    let mut fills = Vec::new();
    if pose.effect != 0 {
        fills.extend(face::mouth_fills(&f, &pose.eyes, pose.mouth, half_width));
    } else {
        let (o, frown) = face::rest_mouths(pose.mouth);
        let small_mouth = Mouth::Blend {
            smile: rest,
            dots,
            talk: 0.0,
            level,
            phase,
            o,
            frown,
        };
        fills.extend(face::mouth_fills(&f, &pose.eyes, small_mouth, half_width));
        if talk > 0.0 {
            let s = f.scale;
            let (mx, my) = (
                f.cx + pose.eyes.gx * 0.5 * s,
                f.cy + (22.0 + pose.eyes.gy * 0.3) * s,
            );
            let oval = geom::ellipse(mx, my - 2.0 * s, 7.0 * s, (1.5 + level * 7.0) * s, 0.0, 24);
            fills.push(solid(oval, ink, talk.min(1.0)));
        }
    }
    out.extend(fills.into_iter().map(|x| ctx.place(x, space, surface)));
}

/// Where sparkle `i` of `count` round the head at `head` is and how it looks:
/// `(x, y, radius, alpha)`, the weighted mix of rest, gather, orbit and stream,
/// mixed around the head (angle and distance) so a state change glides a
/// sparkle round the head instead of across the face.
pub(crate) fn sparkle(
    count: usize,
    head: (f64, f64),
    i: usize,
    t: f64,
    float: f64,
    level: f64,
    w: [f64; 4],
) -> (f64, f64, f64, f64) {
    let n = count as f64;
    let k = i as f64;
    let (hx, hy) = head;
    // At rest: wandering slowly round it.
    let a0 = k / n * TAU + t * 0.4;
    let rest = (
        100.0 + a0.cos() * 70.0,
        92.0 + a0.sin() * 50.0 + float,
        3.5,
        0.35,
    );
    // Listening: gathered in, closer as the user speaks.
    let a1 = k / n * TAU + 0.3;
    let r1 = 78.0 - level * 16.0;
    let gather = (
        100.0 + a1.cos() * r1,
        90.0 + a1.sin() * r1 * 0.8,
        3.5,
        0.4 + level * 0.6,
    );
    // Thinking: orbiting the crown, like a halo of ideas (above the eyes).
    let a2 = t * 2.5 + k / n * TAU;
    let orbit = (
        100.0 + a2.cos() * 56.0,
        34.0 + a2.sin() * 11.0 + float,
        3.2,
        0.9,
    );
    // Speaking: streaming out from the head's edge with the voice.
    let u = (t * 0.9 + k / n).rem_euclid(1.0);
    let a3 = -0.9 + k * 0.45;
    let d = 54.0 + u * 34.0;
    let stream = (
        hx + a3.cos() * d,
        hy + 4.0 + a3.sin() * d * 0.8,
        3.0 + level * 2.0,
        (1.0 - u) * (0.3 + level),
    );
    let total: f64 = w.iter().sum::<f64>().max(1e-9);
    let parts = [rest, gather, orbit, stream];
    let (cx, cy) = (hx, hy + 4.0);
    let (mut ux, mut uy, mut dist, mut r, mut a) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, wi) in parts.iter().zip(w) {
        let (dx, dy) = (p.0 - cx, p.1 - cy);
        let d = (dx * dx + dy * dy).sqrt().max(1e-9);
        ux += dx / d * wi;
        uy += dy / d * wi;
        dist += d * wi;
        r += p.2 * wi;
        a += p.3 * wi;
    }
    let ang = uy.atan2(ux);
    let dist = dist / total;
    (
        cx + ang.cos() * dist,
        cy + ang.sin() * dist,
        r / total,
        (a / total).min(1.0),
    )
}

/// Sparkles round the head: wandering at rest, gathered in while listening
/// (closer as the user speaks), orbiting the crown while thinking, streaming
/// out with the voice.
pub fn sparkles(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if !(ctx.accessories && !ctx.tier.small) {
        return;
    }
    let count = r.u();
    let head = r.p2();
    let [odd, even] = r.cols::<2>();
    let (_, dots, talk, _, _) = ctx.w;
    let pose = &ctx.pose;
    let w = [
        (1.0 - pose.ears - dots - talk).max(0.0),
        pose.ears,
        dots,
        talk,
    ];
    for i in 0..count {
        let (x, y, rad, a) = sparkle(count, head, i, ctx.t, ctx.float, pose.drive, w);
        if a > 0.02 {
            let col = ctx.colour(if i % 2 == 1 { odd } else { even });
            out.push(ctx.place(solid(star(x, y, rad * 1.6), col, a), space, None));
        }
    }
}

fn star(cx: f64, cy: f64, r: f64) -> Vec<Point> {
    (0..8)
        .map(|j| {
            let a = j as f64 / 8.0 * TAU - PI / 2.0;
            let rr = if j % 2 == 0 { r } else { r * 0.3 };
            pt(cx + a.cos() * rr, cy + a.sin() * rr)
        })
        .collect()
}
