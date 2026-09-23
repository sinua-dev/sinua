//! Sonar: a periodic expanding "ping" ring for the `confirming` state. NOT a
//! port -- see `docs/effects-research.md`'s "Sonar ping" proposal. Every
//! other mode is an indefinite loop; a receipt/acknowledgment ("received" /
//! "done" / "sent") reads better as a distinct pulse than as another
//! continuous field. The research proposal called it "one-shot", but
//! `frame(state, size, t)` is a pure function of `t` with no invocation
//! memory -- a literal one-shot would just go permanently blank after its
//! first play, which is wrong for a preset a Studio or a long-running app
//! scrubs/replays at an arbitrary `t`. This renders it as a **periodic**
//! pulse instead (repeats every `period` seconds, optionally staggered into
//! a few echoes) -- same character (an expanding, brightening-then-fading
//! ring, not a steady field), adapted to a stateless-`t` API. No golden
//! vector -- same tradeoff as `aurora`/`webflow`/`spectrum`.

use crate::orbs::core::{finalize_frame, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_sonar(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.08, 0.3, cx, cy, 1.0);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));

    let period = get(o, "period", 1.6).max(0.05);
    let ring_n = get(o, "ringCount", 48.0) as i64;
    let echoes = get(o, "echoCount", 2.0) as i64;
    let echo_gap = get(o, "echoSpacing", 0.18);
    let core_r = get(o, "coreSize", 1.3);

    let mut dots: Vec<Dot> = Vec::new();

    // A dim, persistent nucleus so the frame isn't a blank canvas in the
    // gap between pings.
    let (ccx, ccy, ccz) = pt.project(0.0, 0.0, 0.0);
    dots.push(Dot {
        x: ccx,
        y: ccy,
        z: ccz,
        r: core_r * rs,
        white: 0.25,
        a: 0.5,
        ..Default::default()
    });

    for e in 0..echoes.max(1) {
        let e_f = e as f64;
        let phase = ((t - e_f * echo_gap).rem_euclid(period)) / period;
        let radius_now = r * phase;
        // Brighten then fade across the expansion (a half sine over
        // [0, 1]), with a little extra fade as it nears the silhouette edge.
        let alpha = (phase * PI).sin().max(0.0) * (1.0 - 0.15 * phase);
        if alpha < 0.02 {
            continue;
        }
        for k in 0..ring_n.max(1) {
            let ang = (k as f64 / ring_n.max(1) as f64) * 2.0 * PI;
            let (px, py, z) = pt.project(radius_now * ang.cos(), radius_now * ang.sin(), 0.0);
            dots.push(Dot {
                x: px,
                y: py,
                z,
                r: 0.9 * rs * (1.0 - 0.3 * phase),
                white: 0.15,
                a: alpha,
                ..Default::default()
            });
        }
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::profiles::opts;

    #[test]
    fn pulses_between_silent_and_visible() {
        let o = opts(&[
            ("period", 1.0),
            ("echoCount", 1.0),
            ("ringCount", 24.0),
            ("rMin", 0.3),
        ]);
        // Just after a ping starts (phase ~0): only the dim core.
        let quiet = frame_sonar(64.0, 0.001, &o);
        assert_eq!(
            quiet.dots.len(),
            1,
            "expected only the core dot right at phase 0"
        );
        // Mid-expansion (phase ~0.5): the ring should be visible.
        let mid = frame_sonar(64.0, 0.5, &o);
        assert!(mid.dots.len() > 1, "expected a visible ring mid-expansion");
    }

    #[test]
    fn repeats_every_period() {
        let o = opts(&[
            ("period", 1.0),
            ("echoCount", 1.0),
            ("ringCount", 24.0),
            ("rMin", 0.3),
        ]);
        let a = frame_sonar(64.0, 0.5, &o);
        let b = frame_sonar(64.0, 1.5, &o); // one full period later
        assert_eq!(
            a.dots.len(),
            b.dots.len(),
            "expected the same phase one period later"
        );
    }
}
