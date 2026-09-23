//! Beacon / Radar: a rotating sweep over a dot-field scope (`scanning`) --
//! "searching", "nearby devices", "discovering". A beam turns clockwise
//! from 12 o'clock; the scope lights up just behind it and fades; a few
//! targets ("blips") flare as the beam passes and glow on, dimming, until
//! it comes round again.
//!
//! Prior art (fetched): CodeFronts'
//! "Weather Radar Sweep" CSS -- beam `conic-gradient(acc 65% 0deg, acc 22%
//! 18deg, transparent 46deg)` turning `3.2s linear infinite` over range
//! rings, cells that go `0%,5% {opacity .95} 55% {.45} 100% {.12}` over one
//! turn, phase-offset by their angle; and the plan position indicator
//! (Wikipedia): "a radial trace ... sweeps in unison with [the antenna]",
//! a long-persistence tube keeping blips visible between rotations, range
//! as concentric circles. Grey by default (`hue: 120` for the CRT green).
//!
//! **No new primitive** (house rule, see `docs/beacon.md`): the beam is a
//! filled, angularly faded wedge, and there's no fill primitive. A
//! radius-wide round-capped arc would bleed its caps R/2 *ahead* of the
//! beam, so instead the scope is a field of `Dot`s on `ringCount` range
//! rings, each dot's alpha taken from its angle behind the beam through
//! CodeFronts' stops (over a 90° trail by default -- dots sample the wedge,
//! so 46° read as a clock hand) -- the same "a gradient on a dot renderer
//! is per-dot color" move `primitives::apply_gradient` makes. The leading edge is a
//! crisp `Polyline`. Stateless: a pure function of `t`.

use std::f64::consts::PI;

use crate::primitives::{
    finalize_frame, hash_d, ring_point, Dot, Fill, FillGradient, GradientStop, ModeOpts, OrbFrame,
    Point, Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

const OUTER: f64 = 0.41;
const IDLE_ALPHA: f64 = 0.10;
const PEAK_ALPHA: f64 = 0.95;
/// Center-to-center dot spacing along a range ring, in dot radii (2.25
/// diameters). Picked on a contact sheet against 6 and 3.5: sparser dots
/// sample the trail too coarsely to read as a sweep; denser ones turn the
/// idle scope into a grey disc.
const SPACING: f64 = 4.5;

/// CodeFronts' conic stops, scaled to a trail of `len` degrees: intensity
/// 0.65 at the beam, 0.22 at 18/46 of the way back, 0 at the end.
pub(crate) fn trail(behind_deg: f64, len: f64) -> f64 {
    let knee = len * 18.0 / 46.0;
    if !(0.0..len).contains(&behind_deg) {
        return 0.0;
    }
    if behind_deg < knee {
        0.65 + (0.22 - 0.65) * behind_deg / knee
    } else {
        0.22 * (1.0 - (behind_deg - knee) / (len - knee))
    }
}

/// CodeFronts' cell keyframes over one turn since the beam passed:
/// 0.95 held to 5%, 0.45 at 55%, 0.12 at 100%.
pub(crate) fn blip_alpha(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    if u <= 0.05 {
        0.95
    } else if u <= 0.55 {
        0.95 + (0.45 - 0.95) * (u - 0.05) / 0.5
    } else {
        0.45 + (0.12 - 0.45) * (u - 0.55) / 0.45
    }
}

/// Clockwise angle (degrees) the beam has travelled past `phi`, in [0, 360).
fn behind(theta: f64, phi: f64) -> f64 {
    (theta - phi).rem_euclid(360.0)
}

pub fn frame_radar(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let period = get(o, "period", 3.2).max(0.05);
    let rings = get(o, "ringCount", 4.0).round().clamp(1.0, 8.0) as usize;
    let dot_r = size * get(o, "dotSize", 0.018).clamp(0.004, 0.08);
    // 90, not CodeFronts' 46: the same stop *ratios*, twice the length,
    // because dots sample the wedge -- at 46° only two or three dots per
    // ring light up and the beam reads as a clock hand, not a sweep
    // (contact-sheet check, see docs/beacon.md).
    // `trailFill` 1 (materials phase 1): the trail is a real filled wedge,
    // so CodeFronts' own 46 degrees works again (the 90 only compensated
    // for dots sampling the wedge). Capped at 120: past that a linear
    // gradient stops reading as an angular fade.
    let wedge = get(o, "trailFill", 0.0).round() == 1.0;
    let trail_len = if wedge {
        get(o, "trailLength", 46.0).clamp(5.0, 120.0)
    } else {
        get(o, "trailLength", 90.0).clamp(5.0, 300.0)
    };
    let blips = get(o, "blipCount", 3.0).round().clamp(0.0, 12.0) as usize;
    let seed = get(o, "seed", 0.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let r_max = size * OUTER - dot_r;
    let white = 0.15;
    let theta = 360.0 * (t / period).rem_euclid(1.0);

    let mut dots: Vec<Dot> = Vec::new();
    // The scope: `rings` range rings of dots, spaced `SPACING` radii apart,
    // so outer rings carry more dots. Ring 1 is the innermost.
    for k in 1..=rings {
        let rr = r_max * k as f64 / rings as f64;
        let n = ((2.0 * PI * rr) / (dot_r * SPACING)).round().max(6.0) as usize;
        for j in 0..n {
            let phi = 360.0 * j as f64 / n as f64;
            let tr = if wedge {
                0.0
            } else {
                trail(behind(theta, phi), trail_len)
            };
            let p = ring_point(cx, cy, rr, phi);
            dots.push(Dot {
                x: p.x,
                y: p.y,
                z: 0.0,
                r: dot_r * (1.0 + 0.4 * tr),
                white,
                a: IDLE_ALPHA + (PEAK_ALPHA - IDLE_ALPHA) * tr / 0.65,
                saturation,
                hue,
            });
        }
    }
    // Blips: seeded targets that flare as the beam passes, then persist.
    for b in 0..blips {
        let phi = 360.0 * hash_d(b as f64 + 1.0, seed + 7.0);
        let rr = r_max * (0.25 + 0.7 * hash_d(b as f64 + 1.0, seed + 31.0));
        let p = ring_point(cx, cy, rr, phi);
        dots.push(Dot {
            x: p.x,
            y: p.y,
            z: 1.0,
            r: dot_r * 2.0,
            white,
            a: blip_alpha(behind(theta, phi) / 360.0),
            saturation,
            hue,
        });
    }
    // The radar origin.
    dots.push(Dot {
        x: cx,
        y: cy,
        z: 2.0,
        r: dot_r * 1.6,
        white,
        a: 0.95,
        saturation,
        hue,
    });

    let tip = ring_point(cx, cy, r_max, theta);
    let beam = Polyline {
        points: vec![ring_point(cx, cy, 0.0, theta), tip],
        white,
        a: 0.9,
        w: dot_r * 2.0,
        saturation,
        hue,
        hues: Vec::new(),
    };

    let mut frame = finalize_frame(dots, vec![], get(o, "rMin", 0.3)).with_polylines(vec![beam]);
    if wedge {
        frame.fills = vec![trail_wedge(
            cx,
            cy,
            r_max + dot_r,
            theta,
            trail_len,
            white,
            saturation,
            hue,
        )];
    }
    frame
}

/// The trail as one filled wedge from the beam back `len` degrees, faded by
/// a linear gradient across it -- perpendicular to the wedge's bisector, from
/// the beam edge to the trail's end, through CodeFronts' stops (0.65 at the
/// beam, 0.22 at 18/46 of the way, 0 at the end). For a wedge up to 120
/// degrees that reads as the conic fade it stands in for; a real conic
/// gradient isn't in the paint contract (SwiftUI `GraphicsContext` conic
/// shading is iOS 18+, SVG has none). One polygon, so no seams between
/// slices.
#[allow(clippy::too_many_arguments)]
fn trail_wedge(
    cx: f64,
    cy: f64,
    r: f64,
    theta: f64,
    len: f64,
    white: f64,
    saturation: f64,
    hue: f64,
) -> Fill {
    let steps = (len / 3.0).ceil().max(2.0) as usize;
    let mut points: Vec<Point> = vec![Point { x: cx, y: cy }];
    for i in 0..=steps {
        points.push(ring_point(cx, cy, r, theta - len * i as f64 / steps as f64));
    }
    // Gradient axis: the chord through the wedge at 0.6 r, beam edge first.
    let a0 = ring_point(cx, cy, r * 0.6, theta);
    let a1 = ring_point(cx, cy, r * 0.6, theta - len);
    let knee = 18.0 / 46.0;
    let stop = |offset: f64, a: f64| GradientStop {
        offset,
        white,
        a,
        saturation,
        hue,
    };
    Fill {
        points,
        holes: Vec::new(),
        white,
        a: 0.65,
        saturation,
        hue,
        gradient: Some(FillGradient {
            kind: 0,
            x0: a0.x,
            y0: a0.y,
            x1: a1.x,
            y1: a1.y,
            r: 0.0,
            stops: vec![stop(0.0, 1.0), stop(knee, 0.22 / 0.65), stop(1.0, 0.0)],
        }),
        blur: 0.0,
        blend: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{opts, Point};

    const SIZE: f64 = 64.0;

    fn angle(p: &Point) -> f64 {
        (p.x - 32.0)
            .atan2(-(p.y - 32.0))
            .to_degrees()
            .rem_euclid(360.0)
    }

    fn near(a: f64, b: f64) -> bool {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d) < 1e-6
    }

    fn beam_angle(f: &OrbFrame) -> f64 {
        angle(f.polylines[0].points.last().unwrap())
    }

    #[test]
    fn the_beam_turns_clockwise_from_twelve_and_repeats() {
        let o = opts(&[("period", 4.0)]);
        assert!(near(beam_angle(&frame_radar(SIZE, 0.0, &o)), 0.0));
        assert!(near(beam_angle(&frame_radar(SIZE, 1.0, &o)), 90.0));
        assert_eq!(frame_radar(SIZE, 1.5, &o), frame_radar(SIZE, 5.5, &o));
    }

    #[test]
    fn trail_lights_behind_the_beam_not_ahead() {
        assert!((trail(0.0, 46.0) - 0.65).abs() < 1e-12);
        assert!((trail(18.0, 46.0) - 0.22).abs() < 1e-12);
        assert_eq!(trail(46.0, 46.0), 0.0);
        assert_eq!(trail(200.0, 46.0), 0.0);
        let behind5 = trail(behind(90.0, 85.0), 46.0);
        let ahead5 = trail(behind(90.0, 95.0), 46.0);
        assert!(behind5 > 0.5 && ahead5 == 0.0);
        // Monotone fade across the trail.
        let mut prev = 1.0;
        for d in 1..46 {
            let v = trail(d as f64, 46.0);
            assert!(v <= prev);
            prev = v;
        }
    }

    #[test]
    fn field_dots_sit_on_the_range_rings_outer_ones_denser() {
        let f = frame_radar(SIZE, 0.0, &opts(&[("ringCount", 3.0), ("blipCount", 0.0)]));
        let mut radii: Vec<f64> = f
            .dots
            .iter()
            .filter(|d| d.z == 0.0)
            .map(|d| ((d.x - 32.0).powi(2) + (d.y - 32.0).powi(2)).sqrt())
            .collect();
        radii.sort_by(|a, b| a.partial_cmp(b).unwrap());
        radii.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert_eq!(radii.len(), 3, "exactly ringCount radii");
        assert!(*radii.last().unwrap() < OUTER * SIZE);
        let count = |r: f64| {
            f.dots
                .iter()
                .filter(|d| d.z == 0.0)
                .filter(|d| (((d.x - 32.0).powi(2) + (d.y - 32.0).powi(2)).sqrt() - r).abs() < 1e-6)
                .count()
        };
        assert!(count(radii[2]) > count(radii[0]));
    }

    #[test]
    fn blips_flare_then_persist_and_fade() {
        assert_eq!(blip_alpha(0.0), 0.95);
        assert_eq!(blip_alpha(0.05), 0.95);
        assert!((blip_alpha(0.55) - 0.45).abs() < 1e-12);
        assert!((blip_alpha(1.0) - 0.12).abs() < 1e-12);
        let mut prev = 1.0;
        for i in 0..=100 {
            let v = blip_alpha(i as f64 / 100.0);
            assert!(v <= prev);
            prev = v;
        }
    }

    #[test]
    fn blip_count_and_seed() {
        let blips = |o: &ModeOpts| -> Vec<(f64, f64)> {
            frame_radar(SIZE, 0.0, o)
                .dots
                .iter()
                .filter(|d| d.z == 1.0)
                .map(|d| (d.x, d.y))
                .collect()
        };
        assert!(blips(&opts(&[("blipCount", 0.0)])).is_empty());
        let a = blips(&opts(&[("blipCount", 3.0), ("seed", 1.0)]));
        assert_eq!(a.len(), 3);
        assert_eq!(a, blips(&opts(&[("blipCount", 3.0), ("seed", 1.0)])));
        assert_ne!(a, blips(&opts(&[("blipCount", 3.0), ("seed", 2.0)])));
    }

    #[test]
    fn grey_by_default_and_hue_reaches_everything() {
        let f = frame_radar(SIZE, 0.5, &opts(&[]));
        assert!(f.dots.iter().all(|d| d.saturation == 0.0));
        let g = frame_radar(SIZE, 0.5, &opts(&[("saturation", 0.8), ("hue", 120.0)]));
        assert!(g.dots.iter().all(|d| d.hue == 120.0 && d.saturation == 0.8));
        assert_eq!(g.polylines[0].hue, 120.0);
    }
}
