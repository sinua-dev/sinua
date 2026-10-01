//! What every character draws the same way, in one place, so a fix reaches the
//! whole cast and the effects and the mute cue read alike on every character
//! (the user's call, 2026-09-30). A character's own file keeps only what makes
//! it itself: its body, its accessories, its signature voice behaviour.
//!
//! - [`Tier`]: how much detail a size gets (64 full, 32 lighter, 20 minimal).
//! - [`ground_shadow`]: the soft shadow under a character.
//! - [`effect_ink`]: the one-shot effects' colours on the face.
//! - [`celebrate_burst`]: the ring of sparkles that flies out on `celebrate`.
//! - [`finish`]: the mute fade (colours kept) and the frame (`Fixed`, fills only).

use std::f64::consts::TAU;

use crate::character::face::{CELEBRATE_INK, ERROR_INK, SUCCESS_INK};
use crate::character::geom::{blurred, ellipse, pt, solid, Hsl};
use crate::character::rig::Pose;
use crate::effects;
use crate::primitives::{ColorMode, Fill, ModeOpts, OrbFrame, Point};

/// How much a size draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tier {
    /// The 20 preset: silhouette, eyes, mouth; heavier lines.
    pub small: bool,
    /// The 64 preset: every detail.
    pub full: bool,
    /// Line-width multiplier (heavier at 20 so outlines survive).
    pub line: f64,
}

pub fn tier(size: f64) -> Tier {
    let small = size < 26.0;
    Tier {
        small,
        full: size > 48.0,
        line: if small { 1.7 } else { 1.0 },
    }
}

const SHADOW: Hsl = crate::character::geom::hsl(0.0, 0.0, 0.05);

/// The soft shadow under a character (design units; it stays on the floor).
pub fn ground_shadow(cx: f64, cy: f64, rx: f64, ry: f64) -> Fill {
    blurred(ellipse(cx, cy, rx, ry, 0.0, 40), SHADOW, 0.35, 3.0)
}

/// The face's colour: the running effect's (success green, error red,
/// celebrate gold), else the character's own.
pub fn effect_ink(pose: &Pose, own: Hsl) -> Hsl {
    match pose.effect {
        effects::SUCCESS => SUCCESS_INK,
        effects::ERROR => ERROR_INK,
        effects::CELEBRATE => CELEBRATE_INK,
        _ => own,
    }
}

/// True under reduced motion (the view's `effectReduced`).
pub fn reduced(o: &ModeOpts) -> bool {
    o.get("effectReduced").is_some_and(|v| *v >= 0.5)
}

/// `celebrate`: seven sparkles flying out from `(cx, cy)` and fading, in the
/// character's three colours. Nothing at 20 px or under reduced motion.
pub fn celebrate_burst(
    pose: &Pose,
    o: &ModeOpts,
    tier: Tier,
    (cx, cy): (f64, f64),
    radius: f64,
    palette: [Hsl; 3],
) -> Vec<Fill> {
    if pose.effect != effects::CELEBRATE || tier.small || reduced(o) {
        return Vec::new();
    }
    let u = pose.effect_u;
    (0..7)
        .map(|i| {
            let a = i as f64 / 7.0 * TAU - 0.4;
            let r = (radius + (i % 2) as f64 * 8.0) * (0.6 + 0.5 * u);
            let (x, y) = (cx + a.cos() * r * 0.9, cy + a.sin() * r * 0.8);
            let k = 2.6 + (i % 3) as f64;
            let star: Vec<Point> = (0..8)
                .map(|j| {
                    let b = j as f64 / 8.0 * TAU;
                    let rr = if j % 2 == 0 { k * 1.6 } else { k * 0.5 };
                    pt(x + b.cos() * rr, y + b.sin() * rr)
                })
                .collect();
            solid(star, palette[i % 3], (1.0 - u).max(0.0))
        })
        .collect()
}

/// The finished frame: muted squints (the rig) and fades 30 %, the colours
/// kept (the user's call, 2026-09-30, instead of the generic grey-out); a
/// character is fills only, in a `Fixed` palette.
pub fn finish(mut fills: Vec<Fill>, o: &ModeOpts) -> OrbFrame {
    let fade = 1.0 - 0.3 * o.get("muted").copied().unwrap_or(0.0).clamp(0.0, 1.0);
    if fade < 1.0 {
        for f in fills.iter_mut() {
            f.a *= fade;
        }
    }
    OrbFrame {
        fills,
        color_mode: ColorMode::Fixed,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::rig;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn tiers_by_size() {
        assert_eq!(
            tier(20.0),
            Tier {
                small: true,
                full: false,
                line: 1.7
            }
        );
        assert_eq!(
            tier(32.0),
            Tier {
                small: false,
                full: false,
                line: 1.0
            }
        );
        assert_eq!(
            tier(64.0),
            Tier {
                small: false,
                full: true,
                line: 1.0
            }
        );
    }

    #[test]
    fn the_burst_plays_only_on_celebrate_and_not_small_or_reduced() {
        let pal = [CELEBRATE_INK, SUCCESS_INK, ERROR_INK];
        let o = opts(&[("effectCode", 3.0), ("effectAge", 0.5)]);
        let p = rig::pose(&o, 0.0);
        assert_eq!(
            celebrate_burst(&p, &o, tier(64.0), (100.0, 90.0), 80.0, pal).len(),
            7
        );
        assert!(celebrate_burst(&p, &o, tier(20.0), (100.0, 90.0), 80.0, pal).is_empty());
        let r = opts(&[
            ("effectCode", 3.0),
            ("effectAge", 0.5),
            ("effectReduced", 1.0),
        ]);
        assert!(celebrate_burst(
            &rig::pose(&r, 0.0),
            &r,
            tier(64.0),
            (100.0, 90.0),
            80.0,
            pal
        )
        .is_empty());
        let none = opts(&[]);
        assert!(celebrate_burst(
            &rig::pose(&none, 0.0),
            &none,
            tier(64.0),
            (100.0, 90.0),
            80.0,
            pal
        )
        .is_empty());
    }

    #[test]
    fn finish_fades_when_muted_and_is_fixed() {
        let f = || {
            vec![solid(
                vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)],
                SUCCESS_INK,
                1.0,
            )]
        };
        assert_eq!(finish(f(), &opts(&[])).fills[0].a, 1.0);
        let m = finish(f(), &opts(&[("muted", 1.0)]));
        assert!((m.fills[0].a - 0.7).abs() < 1e-12);
        assert_eq!(m.color_mode, ColorMode::Fixed);
    }
}
