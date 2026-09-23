//! Hush: the `muted` state -- a dimmed, still, grey-or-warning-tinted
//! sphere for "mic muted" / "connection lost" / "permission denied". NOT
//! a port (no `thinking-orbs` equivalent, no golden vector -- the same
//! tradeoff as `aurora`/`sonar`/the other additive modes). It is the first
//! *negative / attention* moment in the set: every other state, ported or
//! additive, says "something is happening"; this one says "nothing can".
//!
//! Prior art, fetched (fetched 2026-09-17):
//! Google Nest's help page -- "4 solid orange lights: The microphone is
//! off" while listening/thinking are *moving* white lights; Amazon's
//! Voice Interoperability guide -- "It is very important for a product to
//! convey a device's Microphone On/Off state", and an Echo's ring goes
//! solid red with the mic off. The pattern both share: active states
//! animate, muted holds **still** and is a single flat color. Hence this
//! mode's camera does not spin (a fixed yaw, unlike every other `orbs`
//! mode's `t * k` yaw) and its only motion is an optional slow breathing
//! pulse (`pulseAmplitude`, `0` = fully static, Nest/Echo-style). Grey is the
//! default; red (Echo, hue 8) or amber (Nest, hue ~35) are one
//! `saturation`/`hue` opt away -- Amazon's guidance is about not sharing
//! a color between Listening and Mic Off, not about mandating red. ChatGPT
//! Voice and Siri show no distinct muted *orb* (mute is just a mic icon
//! there), so this fills a gap the reference products leave; it stays
//! abstract (no slashed-mic glyph) because abstract geometry is what sets
//! `orbs` apart (see `docs/effects-research.md`).
//!
//! Two related things this is NOT: (1) `primitives::apply_muted`, the opts
//! flag that mechanically dims *any* state -- that's for "keep showing what
//! you were showing, but muted"; this state is a designed resting pose for
//! apps that switch state on mute. They compose. (2) The future `beacon`
//! family (the product plan: event / notification / attention) may supersede
//! or extend this with a more general, composable attention visual; this
//! ships the cheap version now on purpose.

use crate::orbs::core::{fib_dir, finalize_frame, lerp, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub fn frame_hush(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let rs = radius_scale(size, get(o, "rsPow", 0.6));
    let node_n = get(o, "nodeCount", 220.0) as i64;
    let node_r = get(o, "nodeSize", 1.1);
    let pulse_amp = get(o, "pulseAmplitude", 0.03).max(0.0);
    let pulse_period = get(o, "period", 3.2).max(0.05);
    let dim = get(o, "dim", 0.6).clamp(0.0, 1.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let hue = get(o, "hue", 8.0).rem_euclid(360.0);
    // A fixed pose: `yaw` is a constant, not `t * k`. Held still on purpose
    // (see the header) -- the one thing that most distinguishes "muted"
    // from every animated state.
    let yaw = get(o, "yaw", 0.6);
    let pt = Proj::new(yaw, 0.35, cx, cy, 1.0);

    // Slow breathing: the whole shell swells and settles by `pulseAmplitude`
    // every `period` seconds. Zero amplitude = a static frame.
    let breathe = 1.0 + pulse_amp * (2.0 * PI * t / pulse_period).sin();

    let mut dots: Vec<Dot> = Vec::with_capacity(node_n.max(0) as usize);
    for i in 0..node_n {
        let (nx, ny, nz) = fib_dir(i as f64, node_n as f64);
        let (px, py, z) = pt.project(nx * r * breathe, ny * r * breathe, nz * r * breathe);
        let depth = (z / r + 1.0) / 2.0;
        // Same depth-shading shape as `aurora`, then pulled toward paper
        // (lighter ink, lower alpha, slightly smaller dots) by `dim`.
        let white = lerp(0.35 + 0.4 * depth, 0.9, dim * 0.6);
        let alpha = (0.55 + 0.45 * depth) * (1.0 - 0.75 * dim);
        dots.push(Dot {
            x: px,
            y: py,
            z,
            r: node_r * rs * (0.6 + 0.5 * depth) * (1.0 - 0.15 * dim),
            white,
            a: alpha,
            saturation,
            hue,
        });
    }

    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbs::modes::aurora::frame_aurora;
    use crate::orbs::profiles::opts;

    fn base_opts() -> ModeOpts {
        opts(&[("nodeCount", 60.0), ("rMin", 0.3)])
    }

    fn mean_alpha(frame: &OrbFrame) -> f64 {
        frame.dots.iter().map(|d| d.a).sum::<f64>() / frame.dots.len() as f64
    }

    #[test]
    fn is_completely_still_when_the_pulse_is_off() {
        let mut o = base_opts();
        o.insert("pulseAmplitude".to_string(), 0.0);
        let a = frame_hush(64.0, 0.3, &o);
        let b = frame_hush(64.0, 7.9, &o);
        assert_eq!(a.dots.len(), 60);
        assert_eq!(
            a, b,
            "with no pulse, muted must not move at all -- no spin, no drift"
        );
    }

    #[test]
    fn pulse_breathes_the_shell_without_rotating_it() {
        let mut o = base_opts();
        o.insert("pulseAmplitude".to_string(), 0.05);
        o.insert("period".to_string(), 4.0);
        let rest = frame_hush(64.0, 0.0, &o); // sin(0) = 0 -> breathe = 1
        let peak = frame_hush(64.0, 1.0, &o); // quarter period -> breathe = 1.05
        let dist = |d: &Dot| ((d.x - 32.0).powi(2) + (d.y - 32.0).powi(2)).sqrt();
        let angle = |d: &Dot| (d.y - 32.0).atan2(d.x - 32.0);
        let mut grew = 0;
        for (a, b) in rest.dots.iter().zip(peak.dots.iter()) {
            if dist(a) > 1e-6 {
                assert!(
                    (angle(a) - angle(b)).abs() < 1e-9,
                    "a dot's bearing from center must not change -- no spin"
                );
                if dist(b) > dist(a) {
                    grew += 1;
                }
            }
        }
        assert!(
            grew > rest.dots.len() / 2,
            "most dots should sit further out at the pulse peak"
        );
    }

    #[test]
    fn dimmer_than_the_lattice_it_borrows_from() {
        let hush = frame_hush(64.0, 1.0, &base_opts());
        let aurora = frame_aurora(64.0, 1.0, &base_opts());
        assert!(
            mean_alpha(&hush) < mean_alpha(&aurora) * 0.7,
            "muted must read as clearly faded, not just a still aurora"
        );
    }

    #[test]
    fn default_is_grey_and_a_tint_reaches_every_dot() {
        let grey = frame_hush(64.0, 0.0, &base_opts());
        assert!(grey.dots.iter().all(|d| d.saturation == 0.0));
        let mut o = base_opts();
        o.insert("saturation".to_string(), 0.7);
        o.insert("hue".to_string(), 35.0);
        let amber = frame_hush(64.0, 0.0, &o);
        assert!(amber
            .dots
            .iter()
            .all(|d| (d.saturation - 0.7).abs() < 1e-9 && (d.hue - 35.0).abs() < 1e-9));
    }
}
