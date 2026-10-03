//! The head turn (FX Spec 1.11; design note 8): the head yaws and pitches
//! like a solid, not a flat sticker. Stateless like the rest
//! of the rig: the angles are a function of `t`, `seed` and the state's
//! numbers, so a state change slides them like any other rig value.
//!
//! Two cheap layers, no slice stack (the user's call, 2026-09-30):
//! - the face is wrapped onto a [`Surface`] (a flattened sphere, or a
//!   cylinder for Hum's capsule) and turned with it ([`Turn::map_fill`]), so
//!   the eyes slide and narrow;
//! - the character's own file shifts its light, shows a side band and gives
//!   its parts depth.
//!
//! The idea comes from libraries.dev's bot-avatars (MIT); nothing is copied:
//! theirs is a stateful wander and a 17-slice extrusion.
//!
//! Keys (the facing ones are 0 when absent, so a character with no state
//! numbers faces straight ahead and draws exactly as before):
//! - `turn` 0..1: how far the head may turn (1 = ±35° yaw, ±15° pitch; the
//!   default [`TURN_DEFAULT`] is ±25°, the user's call; 0 = the flat character);
//! - `turnYaw`, `turnPitch` -1..1: where the state faces (+ = viewer's
//!   right, + = up);
//! - `turnWander` 0..1: looking corner to corner and holding (idle);
//! - `turnNod` 0..1: little nods with the voice level (speaking).

use crate::character::geom::pt;
use crate::character::kit::{self, Tier};
use crate::primitives::{hash_d, Fill, ModeOpts, Point};

const MAX_YAW: f64 = 35.0 * std::f64::consts::PI / 180.0;
const MAX_PITCH: f64 = 15.0 * std::f64::consts::PI / 180.0;
/// One look of the wander: turn over `WANDER_EASE`, hold for the rest.
const WANDER_SLOT: f64 = 3.2;
const WANDER_EASE: f64 = 0.7;
/// `turn` when absent: ±25° yaw (the catalog's `turn` fallback is the same).
pub const TURN_DEFAULT: f64 = 0.71;

fn get(o: &ModeOpts, key: &str) -> f64 {
    o.get(key).copied().unwrap_or(0.0)
}

pub fn smooth(k: f64) -> f64 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The head's angles, radians: yaw + = the face turns to the viewer's right,
/// pitch + = it looks up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    pub yaw: f64,
    pub pitch: f64,
}

/// What a face is drawn on, for [`Turn::map`]: centre `c`, radius `r`, and
/// `depth`, the share of the radius the front bulges out (a helmet front or a
/// visor is flatter than a ball, so a turn slides it without wrapping it round
/// the edge). A `cylinder` (Hum's capsule) turns about its upright axis only:
/// its x wraps, and a pitch just lifts or lowers the face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub c: (f64, f64),
    pub r: f64,
    pub depth: f64,
    pub cylinder: bool,
}

/// Where the wander looks during slot `n`, in -1..1 (yaw) and -0.5..0.5 (pitch).
fn wander_target(seed: f64, n: f64) -> (f64, f64) {
    (
        hash_d(seed + 41.0, n) * 2.0 - 1.0,
        hash_d(seed + 43.0, n) - 0.5,
    )
}

/// Corner to corner: each slot eases from the last slot's look to its own,
/// then holds it. Continuous in `t`.
pub fn wander(t: f64, seed: f64) -> (f64, f64) {
    let n = (t / WANDER_SLOT).floor();
    let k = smooth((t - n * WANDER_SLOT) / WANDER_EASE);
    let (a, b) = (wander_target(seed, n - 1.0), wander_target(seed, n));
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

/// The turn `lag` seconds ago: a part that follows the head (Wisp's tail)
/// without remembering past frames.
pub fn angles_lagged(o: &ModeOpts, t: f64, tier: Tier, lag: f64) -> Turn {
    angles(o, t - lag, tier)
}

/// The turn at time `t`. Off at 20 px (too small to read) and when `turn` is 0;
/// under reduced motion only the state's own facing stays.
pub fn angles(o: &ModeOpts, t: f64, tier: Tier) -> Turn {
    let amount = o
        .get("turn")
        .copied()
        .unwrap_or(TURN_DEFAULT)
        .clamp(0.0, 1.0);
    if amount == 0.0 || tier.small {
        return Turn::default();
    }
    let seed = get(o, "seed");
    let mut yaw = get(o, "turnYaw");
    let mut pitch = get(o, "turnPitch");
    if !kit::reduced(o) {
        let w = get(o, "turnWander").clamp(0.0, 1.0);
        if w > 0.0 {
            let (wy, wp) = wander(t, seed);
            yaw += wy * w;
            pitch += wp * w;
        }
        let nod = get(o, "turnNod").clamp(0.0, 1.0);
        if nod > 0.0 {
            let level = get(o, "audioLevel").clamp(0.0, 1.0);
            pitch += nod * level * 0.45 * (t * 4.4).sin();
        }
    }
    Turn {
        yaw: yaw.clamp(-1.0, 1.0) * MAX_YAW * amount,
        pitch: pitch.clamp(-1.0, 1.0) * MAX_PITCH * amount,
    }
}

impl Turn {
    pub fn is_zero(&self) -> bool {
        self.yaw == 0.0 && self.pitch == 0.0
    }

    /// A point of the face on `s`, turned with the head and seen straight on
    /// (orthographic). The identity when the turn is zero.
    pub fn map(&self, s: &Surface, p: &Point) -> Point {
        let (cx, cy) = s.c;
        let (x, y) = ((p.x - cx) / s.r, (p.y - cy) / s.r);
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cyaw) = self.yaw.sin_cos();
        if s.cylinder {
            let x = x.clamp(-1.0, 1.0);
            let z = (1.0 - x * x).sqrt() * s.depth;
            // A cylinder nods half as far: its body's own tilt does the rest
            // (Hum's capsule tips on its yoke).
            return pt(
                cx + (x * cyaw + z * sy) * s.r,
                cy + (y - 0.5 * sp * s.depth) * s.r,
            );
        }
        // Past the rim (a breast reaching below the ball) a point lies flat
        // (z = 0): continuous with the rim and still the identity at rest.
        let z = (1.0 - x * x - y * y).max(0.0).sqrt() * s.depth;
        // Pitch about the x axis (y is down, so up is -y), then yaw about y.
        let (y1, z1) = (y * cp - z * sp, y * sp + z * cp);
        pt(cx + (x * cyaw + z1 * sy) * s.r, cy + y1 * s.r)
    }

    /// How far `p` on `s` faces the viewer after the turn: > 0 in front, < 0 round the
    /// back (a point past the rim lies flat, so the far side of a hat goes behind).
    pub fn facing(&self, s: &Surface, p: &Point) -> f64 {
        let (x, y) = ((p.x - s.c.0) / s.r, (p.y - s.c.1) / s.r);
        let z = (1.0 - x * x - y * y).max(0.0).sqrt() * s.depth;
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cyaw) = self.yaw.sin_cos();
        -x * sy + (y * sp + z * cp) * cyaw
    }

    /// How far a part has come round to the front (or gone behind), 0..1 as
    /// the head turns off centre: fades a part across the draw-order switch
    /// so nothing jumps.
    pub fn fade_in(&self) -> f64 {
        smooth(self.yaw.sin().abs() / 0.25)
    }

    /// The side the head turns away from: -1 = the viewer's left comes near
    /// (yaw > 0), 1 = the right does.
    pub fn near_side(&self) -> f64 {
        if self.yaw > 0.0 {
            -1.0
        } else {
            1.0
        }
    }

    /// The side band of a convex outline pushed back by `d`: only the
    /// crescent that shows past the outline (not a second full copy, which
    /// would double the ink), and its outer edge for the rim line.
    /// `None` when `d` is too small to see.
    pub fn side_band(shape: &[Point], d: (f64, f64)) -> Option<(Vec<Point>, Vec<Point>)> {
        let n = shape.len();
        if n < 3 || d.0.hypot(d.1) < 0.05 {
            return None;
        }
        let (mx, my) = shape.iter().fold((0.0, 0.0), |a, p| (a.0 + p.x, a.1 + p.y));
        let (mx, my) = (mx / n as f64, my / n as f64);
        // A point faces `d` when its outward normal does.
        let faces: Vec<bool> = (0..n)
            .map(|i| {
                let (a, b) = (&shape[(i + n - 1) % n], &shape[(i + 1) % n]);
                let (mut nx, mut ny) = (b.y - a.y, a.x - b.x);
                if nx * (shape[i].x - mx) + ny * (shape[i].y - my) < 0.0 {
                    (nx, ny) = (-nx, -ny);
                }
                nx * d.0 + ny * d.1 > 0.0
            })
            .collect();
        let start = (0..n).find(|&i| faces[i] && !faces[(i + n - 1) % n])?;
        let arc: Vec<usize> = (0..n)
            .map(|k| (start + k) % n)
            .take_while(|&i| faces[i])
            .collect();
        if arc.len() < 2 {
            return None;
        }
        let outer: Vec<Point> = arc
            .iter()
            .map(|&i| pt(shape[i].x + d.0, shape[i].y + d.1))
            .collect();
        let mut band = outer.clone();
        band.extend(arc.iter().rev().map(|&i| pt(shape[i].x, shape[i].y)));
        Some((band, outer))
    }

    /// An outline through [`Turn::map`].
    pub fn map_points(&self, s: &Surface, pts: &[Point]) -> Vec<Point> {
        pts.iter().map(|p| self.map(s, p)).collect()
    }

    /// Every point of a fill (outline, holes, gradient ends) through
    /// [`Turn::map`].
    pub fn map_fill(&self, mut f: Fill, s: &Surface) -> Fill {
        let m = |p: &Point| self.map(s, p);
        f.points = f.points.iter().map(m).collect();
        f.holes = f.holes.iter().map(|h| h.iter().map(m).collect()).collect();
        if let Some(g) = f.gradient.as_mut() {
            let (a, b) = (m(&pt(g.x0, g.y0)), m(&pt(g.x1, g.y1)));
            (g.x0, g.y0, g.x1, g.y1) = (a.x, a.y, b.x, b.y);
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kv: &[(&str, f64)]) -> ModeOpts {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn no_facing_means_no_turn_and_turn_0_is_off() {
        assert!(angles(&opts(&[]), 2.0, kit::tier(64.0)).is_zero());
        let o = opts(&[("turn", 0.0), ("turnYaw", 1.0), ("turnWander", 1.0)]);
        assert!(angles(&o, 2.0, kit::tier(64.0)).is_zero());
        // Absent, `turn` is the default ±25°.
        let d = angles(&opts(&[("turnYaw", 1.0)]), 2.0, kit::tier(64.0));
        assert!(
            (d.yaw.to_degrees() - 24.85).abs() < 0.01,
            "{}",
            d.yaw.to_degrees()
        );
    }

    #[test]
    fn off_at_20_px() {
        let o = opts(&[("turn", 1.0), ("turnYaw", 1.0)]);
        assert!(angles(&o, 2.0, kit::tier(20.0)).is_zero());
        assert!(!angles(&o, 2.0, kit::tier(32.0)).is_zero());
    }

    #[test]
    fn the_amount_scales_the_angle() {
        let a = |k: f64| {
            angles(
                &opts(&[("turn", k), ("turnYaw", 1.0)]),
                0.0,
                kit::tier(64.0),
            )
            .yaw
        };
        assert!((a(1.0) - MAX_YAW).abs() < 1e-12);
        assert!((a(0.4) - 0.4 * MAX_YAW).abs() < 1e-12);
    }

    #[test]
    fn the_wander_is_continuous_and_holds() {
        let mut prev = wander(0.0, 3.0);
        for i in 1..4000 {
            let w = wander(i as f64 * 0.005, 3.0);
            assert!((w.0 - prev.0).abs() < 0.02 && (w.1 - prev.1).abs() < 0.02);
            prev = w;
        }
        // After the ease, the look holds still until the next slot.
        let n = 5.0 * WANDER_SLOT;
        assert_eq!(
            wander(n + WANDER_EASE + 0.1, 3.0),
            wander(n + WANDER_SLOT - 0.05, 3.0)
        );
    }

    #[test]
    fn reduced_motion_keeps_only_the_state_facing() {
        let o = opts(&[
            ("turn", 1.0),
            ("turnYaw", -0.5),
            ("turnWander", 1.0),
            ("effectReduced", 1.0),
        ]);
        let a = angles(&o, 1.3, kit::tier(64.0));
        let b = angles(&o, 7.9, kit::tier(64.0));
        assert_eq!(a, b);
        assert!((a.yaw + 0.5 * MAX_YAW).abs() < 1e-12);
    }

    const BALL: Surface = Surface {
        c: (100.0, 92.0),
        r: 64.0,
        depth: 0.55,
        cylinder: false,
    };
    const CAN: Surface = Surface {
        c: (100.0, 92.0),
        r: 44.0,
        depth: 1.0,
        cylinder: true,
    };

    #[test]
    fn a_surface_is_the_identity_at_rest_and_turns_the_centre() {
        let rest = Turn::default();
        for s in [BALL, CAN] {
            let p = pt(120.0, 80.0);
            let q = rest.map(&s, &p);
            assert!((q.x - p.x).abs() < 1e-9 && (q.y - p.y).abs() < 1e-9);
        }
        let right = Turn {
            yaw: 0.5,
            pitch: 0.0,
        };
        let c = right.map(&BALL, &pt(100.0, 92.0));
        assert!((c.x - (100.0 + 64.0 * 0.55 * 0.5f64.sin())).abs() < 1e-9);
        let up = Turn {
            yaw: 0.0,
            pitch: 0.3,
        };
        assert!(up.map(&BALL, &pt(100.0, 92.0)).y < 92.0);
        assert!(up.map(&CAN, &pt(100.0, 92.0)).y < 92.0);
    }

    #[test]
    fn a_cylinder_keeps_its_width_and_squeezes_the_edge() {
        let right = Turn {
            yaw: 0.4,
            pitch: 0.0,
        };
        // The centre slides right; a point near the far (right) edge stays
        // inside the silhouette, bunched against it.
        let c = right.map(&CAN, &pt(100.0, 92.0));
        assert!(c.x > 100.0);
        let e = right.map(&CAN, &pt(140.0, 92.0));
        assert!(e.x <= 144.0 + 1e-9 && e.x > 140.0);
    }

    /// Every corner of the turn, with the voice on so every accessory shows.
    fn turns() -> Vec<ModeOpts> {
        let mut v = Vec::new();
        for y in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            for p in [-1.0, 0.0, 1.0] {
                v.push(opts(&[
                    ("turn", 1.0),
                    ("turnYaw", y),
                    ("turnPitch", p),
                    ("earGain", 1.0),
                    ("mouthTalk", 1.0),
                    ("mouthGain", 1.0),
                    ("audioLevel", 0.8),
                ]));
            }
        }
        v
    }

    fn frame(pattern: &str, size: u32, t: f64, o: &ModeOpts) -> crate::primitives::OrbFrame {
        crate::frame_with_overrides(pattern.into(), size, t, o.clone()).unwrap()
    }

    #[test]
    fn every_character_turned_stays_in_its_box_and_light() {
        for pattern in crate::character::presets::STATES {
            for size in [32u32, 64] {
                for o in turns() {
                    let f = frame(pattern, size, 1.3, &o);
                    let s = size as f64;
                    for p in f.fills.iter().flat_map(|x| x.points.iter()) {
                        assert!(
                            p.x >= 0.0 && p.y >= 0.0 && p.x <= s && p.y <= s,
                            "{pattern} {size}: {p:?}"
                        );
                    }
                    let c = crate::cost::frame_cost(&f, size);
                    assert_eq!(c.class, "light", "{pattern} {size}: {c:?}");
                }
            }
        }
    }

    #[test]
    fn every_character_turn_0_is_the_flat_one_and_20_px_never_turns() {
        for pattern in crate::character::presets::STATES {
            let flat = frame(pattern, 64, 1.7, &opts(&[]));
            let off = frame(
                pattern,
                64,
                1.7,
                &opts(&[("turn", 0.0), ("turnYaw", 1.0), ("turnWander", 1.0)]),
            );
            assert_eq!(flat, off, "{pattern}");
            let turned = frame(pattern, 64, 1.7, &opts(&[("turnYaw", 1.0)]));
            assert_ne!(flat, turned, "{pattern}: the default turn shows");
            let small = frame(pattern, 20, 1.7, &opts(&[]));
            let small_turned = frame(pattern, 20, 1.7, &opts(&[("turn", 1.0), ("turnYaw", 1.0)]));
            assert_eq!(small, small_turned, "{pattern}");
        }
    }

    #[test]
    fn every_character_turns_smoothly_through_the_middle() {
        // Crossing yaw 0 (where near and far parts swap) moves nothing far:
        // the frames either side of it are close to the flat one.
        for pattern in crate::character::presets::STATES {
            let flat = frame(pattern, 64, 1.0, &opts(&[("look", 0.0)]));
            for y in [-0.004, 0.004] {
                let f = frame(
                    pattern,
                    64,
                    1.0,
                    &opts(&[("look", 0.0), ("turn", 1.0), ("turnYaw", y)]),
                );
                let vis = |f: &crate::primitives::OrbFrame| -> Vec<(f64, f64, f64)> {
                    f.fills
                        .iter()
                        .filter(|x| x.a > 0.01)
                        .flat_map(|x| x.points.iter().map(move |p| (p.x, p.y, x.a)))
                        .collect()
                };
                let (a, b) = (vis(&flat), vis(&f));
                assert_eq!(
                    a.len(),
                    b.len(),
                    "{pattern} yaw {y}: a part popped in or out"
                );
                let worst = a
                    .iter()
                    .zip(&b)
                    .map(|(p, q)| (p.0 - q.0).hypot(p.1 - q.1))
                    .fold(0.0, f64::max);
                assert!(worst < 0.2, "{pattern} yaw {y}: moved {worst}");
            }
        }
    }

    /// What the turn costs per character: engine time per frame and the
    /// paint-cost proxy, off (`turn` 0) vs on (the default and the state's
    /// facing), with each state's real profile.
    /// `cargo test --release -p core_engine --lib bench_turn_all -- --ignored --nocapture`
    #[test]
    #[ignore = "a measurement, not a check"]
    fn bench_turn_all() {
        let facing = |st: &str| -> Vec<(&'static str, f64)> {
            match st {
                "idle" => vec![("turnWander", 1.0)],
                "listening" => vec![("turnPitch", 0.35)],
                "thinking" => vec![("turnYaw", -0.7), ("turnPitch", 0.8)],
                _ => vec![("turnNod", 1.0), ("turnWander", 0.25)],
            }
        };
        const N: usize = 10_000;
        for pattern in crate::character::presets::STATES {
            for st in ["idle", "listening", "thinking", "speaking"] {
                let mut off = crate::voice_state::profile(pattern, st).unwrap().overrides;
                off.insert("audioLevel".into(), 0.7);
                let mut on = off.clone();
                off.insert("turn".into(), 0.0);
                for (k, v) in facing(st) {
                    on.insert(k.into(), v);
                }
                let time = |o: &ModeOpts| {
                    let t0 = std::time::Instant::now();
                    for i in 0..N {
                        std::hint::black_box(frame(pattern, 64, 0.5 + i as f64 * 0.016, o));
                    }
                    t0.elapsed().as_secs_f64() * 1e6 / N as f64
                };
                let (a, b) = (time(&off), time(&on));
                let (fa, fb) = (frame(pattern, 64, 7.3, &off), frame(pattern, 64, 7.3, &on));
                let (ca, cb) = (
                    crate::cost::frame_cost(&fa, 64),
                    crate::cost::frame_cost(&fb, 64),
                );
                let pts = |f: &crate::primitives::OrbFrame| -> usize {
                    f.fills.iter().map(|x| x.points.len()).sum()
                };
                println!(
                    "{pattern:<6} {st:<10} engine {a:6.1} -> {b:6.1} us ({:+.1} %) | fills {} -> {} | points {} -> {} | coverage {:.3} -> {:.3} | {} -> {}",
                    (b / a - 1.0) * 100.0, ca.fills, cb.fills, pts(&fa), pts(&fb), ca.coverage, cb.coverage, ca.class, cb.class
                );
            }
        }
    }

    /// The cost class per character and size, as its worst frame over the
    /// states and the turn's corners, and how near the "medium" line it sits.
    /// `cargo test -p core_engine --lib cost_table -- --ignored --nocapture`
    #[test]
    #[ignore = "a measurement, not a check"]
    fn cost_table() {
        let (me, mc) = crate::cost::MEDIUM;
        for pattern in crate::character::presets::STATES {
            for size in [32u32, 64] {
                let mut worst = (0u32, 0.0f64, String::new());
                for st in ["idle", "listening", "thinking", "speaking"] {
                    for y in [-1.0, 0.0, 1.0] {
                        for tn in [0.0, TURN_DEFAULT] {
                            let mut o = crate::voice_state::profile(pattern, st).unwrap().overrides;
                            o.extend([
                                ("turn".to_string(), tn),
                                ("turnYaw".to_string(), y),
                                ("audioLevel".to_string(), 0.8),
                            ]);
                            for t in [0.4, 1.3, 2.9] {
                                let c = crate::cost::frame_cost(&frame(pattern, size, t, &o), size);
                                if c.coverage / mc > worst.1 {
                                    worst.1 = c.coverage / mc;
                                }
                                worst.0 = worst.0.max(c.elements);
                                if c.class != "light" {
                                    worst.2 = c.class.clone();
                                }
                            }
                        }
                    }
                }
                println!(
                    "{pattern:<6} {size:>2}px  {}  coverage {:.2} of medium  elements {} / {me}",
                    if worst.2.is_empty() {
                        "light"
                    } else {
                        &worst.2
                    },
                    worst.1,
                    worst.0
                );
            }
        }
    }

    #[test]
    fn wisps_tail_trails_the_head() {
        let tier = kit::tier(64.0);
        let o = opts(&[("turnWander", 1.0), ("seed", 2.0)]);
        // The lagged turn is the head's a quarter second earlier.
        for i in 0..40 {
            let t = 3.0 + i as f64 * 0.1;
            assert_eq!(angles_lagged(&o, t, tier, 0.25), angles(&o, t - 0.25, tier));
        }
    }

    #[test]
    fn the_fade_is_zero_at_rest_and_full_off_centre() {
        assert_eq!(Turn::default().fade_in(), 0.0);
        let t = Turn {
            yaw: 0.3,
            pitch: 0.0,
        };
        assert_eq!(t.fade_in(), 1.0);
        assert_eq!(t.near_side(), -1.0);
    }
}
