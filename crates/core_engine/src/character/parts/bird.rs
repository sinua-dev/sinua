//! CHIRP's parts: feet, a crest that lifts to listen, wings that flutter with
//! the voice and swing round the body as it turns, a beak that is the mouth,
//! and the notes / thought dots it sends up.

use crate::character::geom::{self, outline, pt, solid, Xf};
use crate::character::parts::Reader;
use crate::character::recipe::{Ctx, Space};
use crate::effects;
use crate::primitives::{Fill, Point};

/// Two feet of three strokes each (they stay on the ground: `whole` space).
pub fn feet(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let [x0, gap, top, ground] = r.v::<4>();
    let toe = r.p2();
    let width = r.n();
    let c = ctx.colour(r.col());
    for s in [-1.0, 1.0] {
        let x = x0 + s * gap;
        for leg in [
            vec![pt(x, top), pt(x, ground)],
            vec![pt(x, ground), pt(x + s * toe.0, toe.1)],
            vec![pt(x, ground), pt(x - s * toe.0, toe.1)],
        ] {
            out.push(ctx.place(
                solid(geom::stroke(&leg, width * ctx.lw), c, 1.0),
                space,
                None,
            ));
        }
    }
}

/// Curling feathers that lift while listening (higher with the user's voice),
/// perk while speaking and droop while thinking.
pub fn crest(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let (bx, by) = r.p2();
    let spread = r.list();
    let mid_i = r.u();
    let root = r.n();
    let ctrl = r.v::<4>();
    let tip = r.v::<5>();
    let ears_k = r.p2();
    let talk_k = r.p2();
    let dots_k = r.n();
    let turn_shift = r.n();
    let segments = r.u();
    let width = r.n();
    let c = ctx.colour(r.col());
    let (_, dots, talk, level, _) = ctx.w;
    let p = &ctx.pose;
    let lift = p.ears * (ears_k.0 + p.drive * ears_k.1) + talk * (talk_k.0 + level * talk_k.1)
        - dots * dots_k;
    for (i, dx) in spread.iter().copied().enumerate() {
        let mid = if i == mid_i { 1.0 } else { 0.0 };
        let feather = geom::quad(
            (bx + dx * root, by),
            (bx + dx * ctrl[0], ctrl[1] - lift * ctrl[2] - ctrl[3] * mid),
            (
                bx + dx * tip[0] + (i as f64 - mid_i as f64) * tip[1],
                tip[2] - lift * tip[3] - tip[4] * mid,
            ),
            segments,
        );
        let feather = if ctx.tn.is_zero() {
            feather
        } else {
            Xf::translate(turn_shift * ctx.tn.yaw.sin(), 0.0).map(&feather)
        };
        out.push(ctx.place(
            solid(geom::stroke(&feather, width * ctx.lw), c, 1.0),
            space,
            None,
        ));
    }
}

/// Two wings that flutter with the voice. Turned, they swing round the body:
/// the near one comes forward and grows a little, the far one reaches the rim
/// and thins, seen edge on.
pub fn wings(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let x0 = r.n();
    let at = r.p2();
    let size = r.p2();
    let angle = r.n();
    let segments = r.u();
    let rate = r.n();
    let gain = r.name();
    let ball = r.surf().map(|s| ctx.surface(s));
    let (near_k, far_k) = r.p2();
    let c = ctx.colour(r.col());
    let line_w = r.n();
    let line = ctx.colour(r.col());
    let tn = &ctx.tn;
    let flutter = (ctx.t * rate).sin() * ctx.get(gain, 0.0) * ctx.pose.drive;
    let wing_at = |s: f64| {
        let (mut cx, mut rx) = (x0 + s * at.0, size.0);
        if let (false, Some(ball)) = (tn.is_zero(), ball) {
            cx = tn.map(ball, &pt(cx, at.1)).x;
            rx *= if s == tn.near_side() {
                1.0 + near_k * tn.fade_in()
            } else {
                1.0 - far_k * tn.fade_in()
            };
        }
        geom::ellipse(cx, at.1, rx, size.1, s * (angle + flutter), segments)
    };
    for s in [-1.0, 1.0] {
        let wing = wing_at(s);
        out.push(ctx.place(solid(wing.clone(), c, 1.0), space, None));
        out.push(ctx.place(outline(&wing, line_w * ctx.lw, line, 1.0), space, None));
    }
}

/// The beak is the mouth: the lower half opens with the voice.
pub fn beak(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let lo = r.v::<6>();
    let drop = r.n();
    let up = r.v::<6>();
    let segments = r.u();
    let tip = r.p2();
    let rise = r.n();
    let (effect_open, rest_open) = r.p2();
    let [dark, light] = r.cols::<2>();
    let line_w = r.n();
    let line = ctx.colour(r.col());
    let s = r.surf();
    let (rest, _, talk, level, _) = ctx.w;
    let open = if ctx.mouth_off {
        0.0
    } else if matches!(ctx.pose.effect, effects::SUCCESS | effects::CELEBRATE) {
        effect_open
    } else {
        talk * level + rest * rest_open
    };
    let lw = line_w * ctx.lw;
    let lower = vec![
        pt(lo[0], lo[1]),
        pt(lo[2], lo[3]),
        pt(lo[4], lo[5] + open * drop),
    ];
    out.push(ctx.place(solid(lower.clone(), ctx.colour(dark), 1.0), space, s));
    out.push(ctx.place(outline(&lower, lw, line, 1.0), space, s));
    let upper = geom::join(&[
        geom::quad((up[0], up[1]), (up[2], up[3]), (up[4], up[5]), segments),
        vec![pt(tip.0, tip.1 - open * rise)],
    ]);
    out.push(ctx.place(solid(upper.clone(), ctx.colour(light), 1.0), space, s));
    out.push(ctx.place(outline(&upper, lw, line, 1.0), space, s));
}

/// One note ♪ at `(x, y)`: a head and a flagged stem.
fn note(x: f64, y: f64, s: f64) -> Vec<Vec<Point>> {
    vec![
        geom::ellipse(x, y, 4.5 * s, 3.4 * s, -0.4, 16),
        geom::stroke(
            &[pt(x + 4.0 * s, y - s), pt(x + 4.0 * s, y - 15.0 * s)],
            2.2 * s,
        ),
        geom::stroke(
            &geom::quad(
                (x + 4.0 * s, y - 15.0 * s),
                (x + 9.0 * s, y - 12.0 * s),
                (x + 10.0 * s, y - 8.0 * s),
                8,
            ),
            2.2 * s,
        ),
    ]
}

/// Notes rising while it speaks, and thought dots lighting in turn while it
/// thinks (in the world, not tilted with the head), one note and one dot per
/// step so they interleave in draw order.
pub fn notes(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if !(ctx.accessories && !ctx.tier.small && ctx.pose.effect == 0) {
        return;
    }
    let count = r.u();
    let rate = r.n();
    let from = r.p2();
    let travel = r.p2();
    let stagger = r.n();
    let scale = r.p2();
    let floor = r.n();
    let c = ctx.colour(r.col());
    let dots_at = r.p2();
    let dots_step = r.p2();
    let dots_r = r.p2();
    let dots_segments = r.u();
    let dots_rate = r.n();
    let dots_base = r.n();
    let dots_gain = r.n();
    let (_, dots, talk, level, phase) = ctx.w;
    let s = if ctx.tier.full { scale.0 } else { scale.1 };
    let n = count as f64;
    for i in 0..count {
        let u = (ctx.t * rate + i as f64 / n).rem_euclid(1.0);
        let a = talk * (1.0 - u) * (floor + level).min(1.0);
        if a > 0.02 {
            for part in note(
                from.0 + u * travel.0,
                from.1 - u * travel.1 + i as f64 * stagger,
                s,
            ) {
                out.push(ctx.place(solid(part, c, a), space, None));
            }
        }
        let on = (1.0 - ((phase * dots_rate).rem_euclid(n) - i as f64).abs()).max(0.0);
        let d = dots * (dots_base + dots_gain * on);
        if d > 0.02 {
            let rr = (dots_r.0 + i as f64 * dots_r.1) * s;
            let dot = geom::ellipse(
                dots_at.0 + i as f64 * dots_step.0,
                dots_at.1 - i as f64 * dots_step.1,
                rr,
                rr,
                0.0,
                dots_segments,
            );
            out.push(ctx.place(solid(dot, c, d), space, None));
        }
    }
}
