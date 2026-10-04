//! Arms (design note 17): two arms from the shoulders, each an upper arm, a
//! forearm and a round hand. Where the hands go follows the voice state's
//! weights: hanging and swaying at rest, the right hand to the ear while
//! listening and to the chin while thinking, small beats with the voice while
//! speaking; up for celebrate, a shrug for error, open for the tap hop. The
//! elbow comes from a two-segment IK, bent outward. Turned, the far arm goes
//! behind the body: the `back` copy (listed before the body) fades in, the
//! `front` one fades it out. Not at 20 px, nor with the `arms` option off.

use std::f64::consts::PI;

use crate::character::geom::{self, outline, pt, solid};
use crate::character::parts::{state_weights, Reader};
use crate::character::recipe::{Ctx, Space};
use crate::primitives::{Fill, Point};

/// The elbow for a hand at `h` from a shoulder at `s`, bent away from the
/// body (`out` = -1 left, 1 right); the hand is pulled in when out of reach.
fn ik(s: (f64, f64), h: (f64, f64), a: f64, b: f64, out: f64) -> ((f64, f64), (f64, f64)) {
    let (dx, dy) = (h.0 - s.0, h.1 - s.1);
    let d = dx.hypot(dy).clamp((a - b).abs() + 1e-6, a + b - 1e-6);
    let base = dy.atan2(dx);
    let alpha = ((a * a + d * d - b * b) / (2.0 * a * d))
        .clamp(-1.0, 1.0)
        .acos();
    let hand = (s.0 + d * base.cos(), s.1 + d * base.sin());
    let elbow = |k: f64| {
        (
            s.0 + a * (base + k * alpha).cos(),
            s.1 + a * (base + k * alpha).sin(),
        )
    };
    let (e1, e2) = (elbow(1.0), elbow(-1.0));
    // Out on its own side and a little down (it hangs). Where the two are about
    // as good (a hand straight out to the side) the elbow eases from one to the
    // other instead of flipping in one frame (design note 31); elsewhere the better.
    let score = |e: (f64, f64)| e.0 * out + 0.6 * e.1;
    let k = (0.5 + (score(e1) - score(e2)) / 12.0).clamp(0.0, 1.0);
    let e = if k >= 1.0 {
        e1
    } else if k <= 0.0 {
        e2
    } else {
        let k = k * k * (3.0 - 2.0 * k);
        (e2.0 + (e1.0 - e2.0) * k, e2.1 + (e1.1 - e2.1) * k)
    };
    (e, hand)
}

/// The weighted blend of `targets` as seen from `s`: their directions summed
/// (by weight) and their reaches averaged; a straight blend if the directions cancel.
fn around(s: (f64, f64), targets: &[(f64, f64); 4], w: &[f64; 4]) -> (f64, f64) {
    let (mut ux, mut uy, mut reach, mut sum) = (0.0, 0.0, 0.0, 0.0);
    let (mut lx, mut ly) = (0.0, 0.0);
    for (t, &k) in targets.iter().zip(w) {
        let (dx, dy) = (t.0 - s.0, t.1 - s.1);
        let d = dx.hypot(dy).max(1e-9);
        ux += dx / d * k;
        uy += dy / d * k;
        reach += d * k;
        lx += t.0 * k;
        ly += t.1 * k;
        sum += k;
    }
    let sum = sum.max(1e-9);
    let n = ux.hypot(uy);
    if n < 1e-6 {
        return (lx / sum, ly / sum);
    }
    let r = reach / sum;
    (s.0 + ux / n * r, s.1 + uy / n * r)
}

pub fn arms(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if ctx.tier.small || ctx.get("arms", 1.0) < 0.5 {
        return;
    }
    let shoulder = r.p2();
    let mirror = r.n();
    let (upper, fore) = r.p2();
    let width = r.n();
    let hand_r = r.n();
    let ear = r.p2();
    let chin = r.p2();
    let back = r.b();
    let color = ctx.colour(r.col());
    let hand_c = ctx.colour(r.col());
    let lw = r.n() * ctx.lw;
    let line = ctx.colour(r.col());
    let (t, w) = (ctx.t, state_weights(ctx));
    let level = ctx.pose.drive;
    // A one-shot effect's pose, with its envelope.
    let age = ctx.o.get("effectAge").copied().unwrap_or(-1.0);
    let code = ctx.o.get("effectCode").copied().unwrap_or(0.0) as u32;
    let effect = crate::effects::duration_of(code)
        .filter(|d| (0.0..*d).contains(&age))
        .map(|d| (code, (PI * age / d).sin().min(1.0)));
    let tn = &ctx.tn;
    let near = tn.near_side();
    let fade = if tn.is_zero() { 0.0 } else { tn.fade_in() };
    for s in [-1.0, 1.0] {
        let far = s == -near && !tn.is_zero();
        // `back` draws the far arm as it turns away; `front` the near one, and
        // the far one while the head faces front.
        let a = match (back, far) {
            (true, true) => fade,
            (true, false) => 0.0,
            (false, true) => 1.0 - fade,
            (false, false) => 1.0,
        };
        if a <= 0.01 {
            continue;
        }
        let sh = (mirror + s * (mirror - shoulder.0).abs(), shoulder.1);
        let rel = |x: f64, y: f64| (sh.0 + s * x, sh.1 + y);
        let sway = (t * 1.3 + s).sin() * 1.5;
        let idle = rel(6.0 + sway, 40.0);
        let right = s > 0.0;
        let listen = if right { ear } else { idle };
        let think = if right { chin } else { idle };
        let beat = -6.0 * level * (t * 5.0 + s).sin().abs();
        let speak = rel(30.0, -10.0 + beat);
        // Blended around the shoulder (direction and reach, not a straight line
        // through it), so a hand swings out along an arc instead of folding in.
        let mut h = around(sh, &[idle, listen, think, speak], &w);
        if let Some((code, env)) = effect {
            let target = match code {
                crate::effects::CELEBRATE => Some(rel(24.0, -58.0)),
                crate::effects::ERROR => Some(rel(30.0, -14.0)),
                crate::effects::HOP => Some(rel(34.0, -4.0)),
                _ => None,
            };
            if let Some(g) = target {
                h = (h.0 + (g.0 - h.0) * env, h.1 + (g.1 - h.1) * env);
            }
        }
        let (e, hand) = ik(sh, h, upper, fore, s);
        let limb: Vec<Point> = vec![pt(sh.0, sh.1), pt(e.0, e.1), pt(hand.0, hand.1)];
        out.push(ctx.place(
            solid(geom::stroke(&limb, width + 2.0 * lw), line, a),
            space,
            None,
        ));
        out.push(ctx.place(solid(geom::stroke(&limb, width), color, a), space, None));
        let palm = geom::ellipse(hand.0, hand.1, hand_r, hand_r, 0.0, 20);
        out.push(ctx.place(solid(palm.clone(), hand_c, a), space, None));
        if ctx.tier.full {
            out.push(ctx.place(outline(&palm, lw, line, a), space, None));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ik_keeps_the_lengths_and_bends_outward() {
        let s = (60.0, 120.0);
        let (e, h) = ik(s, (66.0, 150.0), 22.0, 22.0, -1.0);
        let len = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
        assert!((len(s, e) - 22.0).abs() < 1e-9);
        assert!((len(e, h) - 22.0).abs() < 1e-6);
        assert!(e.0 < s.0.max(h.0), "the left elbow bends left");
        // Out of reach: straight toward the target, at full length.
        let (_, far) = ik(s, (60.0, 300.0), 22.0, 22.0, -1.0);
        assert!((len(s, far) - 44.0).abs() < 1e-5);
    }
}
