//! A cup's steam (design note 14): soft, blurred wisps that rise from a source,
//! curl and fade out at the top. Each wisp has its own phase, so the steam
//! never pulses as one. Listening lifts it higher, thinking curls it, the voice
//! thickens it. Not at 20 px, nor with `accessories` off.

use std::f64::consts::PI;

use crate::character::geom::{self, blurred, pt};
use crate::character::parts::Reader;
use crate::character::recipe::{Ctx, Space};
use crate::primitives::{Fill, Point};

pub fn steam(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if ctx.tier.small || !ctx.accessories {
        return;
    }
    let (x0, y0) = r.p2();
    let count = r.u();
    let [spread, rise, width, sway, rate, blur] = r.v::<6>();
    let c = ctx.colour(r.col());
    let alpha = r.n();
    let (_, dots, talk, level, _) = ctx.w;
    let ears = ctx.pose.ears;
    let voice = talk * level;
    // Never past the top of the box: the wisps end below y = width (design units).
    let rise = (rise * (1.0 + 0.35 * ears + 0.25 * voice)).min(y0 - 1.5 * width);
    let sway = sway * (1.0 + 0.8 * dots);
    let width = width * (1.0 + 0.5 * voice);
    let alpha = alpha * (1.0 + 0.3 * ears + 0.3 * voice);
    let t = ctx.t;
    let seed = ctx.get("seed", 0.0);
    for i in 0..count {
        let k = i as f64;
        let u = (t * rate + k / count as f64 + seed * 0.37).rem_euclid(1.0);
        let x = x0 + (k - (count - 1) as f64 / 2.0) * spread;
        let base = y0 - u * rise * 0.35;
        let pts: Vec<Point> = (0..=12)
            .map(|j| {
                let v = j as f64 / 12.0;
                let curl = (v * 5.0 + t * 2.4 + k * 2.0).sin() * sway * (0.4 + v);
                pt(x + curl, base - v * rise * 0.65)
            })
            .collect();
        let a = ((u * PI).sin() * alpha).min(1.0);
        if a > 0.02 {
            out.push(ctx.place(blurred(geom::stroke(&pts, width), c, a, blur), space, None));
        }
    }
}
