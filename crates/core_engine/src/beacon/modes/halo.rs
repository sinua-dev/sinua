//! Beacon / Halo: the location mark (`locating`) -- a solid dot inside a
//! translucent accuracy halo, with a slow ring that pulses from the dot
//! out to the halo's edge. Google Maps' blue dot, as described by the
//! coverage this was researched from: the dot "pulsates, growing into a
//! wide, semi-transparent circle before shrinking quickly back", and the
//! translucent circle around it is the position's uncertainty -- "the
//! smaller it is, the more certain". Hence `accuracy: 0..1` (1 = certain)
//! shrinks the halo, and the pulse ring's reach is the halo's radius, so
//! a precise fix pulses tightly and a vague one pulses wide.
//!
//! The halo is just a big, low-alpha `Dot` (a filled circle is the one
//! fill this engine already has); it's given `z = -1` so `finalize_frame`'s
//! far-to-near z-sort draws it under the dot without any special casing.

use crate::primitives::{
    arc_polyline, cubic_bezier, finalize_frame, lerp, Dot, ModeOpts, OrbFrame,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Halo radius for an accuracy reading: wide when uncertain, tight when
/// certain.
pub fn halo_radius(size: f64, accuracy: f64) -> f64 {
    size * lerp(0.41, 0.15, accuracy.clamp(0.0, 1.0))
}

pub fn frame_halo(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let period = get(o, "period", 2.0).max(0.05);
    let accuracy = get(o, "accuracy", 0.5).clamp(0.0, 1.0);
    let dot_r = size * get(o, "dotSize", 0.1).clamp(0.01, 0.4);
    let halo_alpha = get(o, "haloOpacity", 0.14).clamp(0.0, 1.0);
    let stroke = size * get(o, "ringWidth", 0.025).clamp(0.005, 0.2);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let white = 0.15;

    let halo_r = halo_radius(size, accuracy);
    let halo = Dot {
        x: cx,
        y: cy,
        z: -1.0,
        r: halo_r,
        white: 0.35,
        a: halo_alpha,
        saturation,
        hue,
    };
    let dot = Dot {
        x: cx,
        y: cy,
        z: 0.0,
        r: dot_r,
        white,
        a: 0.95,
        saturation,
        hue,
    };

    // The same ease-out ripple as `ping`, but reaching only the halo's
    // edge and taking a full, slower period to fade.
    let u = (t / period).fract();
    let e = cubic_bezier(0.0, 0.0, 0.2, 1.0, u);
    let ring_r = dot_r + (halo_r - dot_r).max(0.0) * e;
    let ring = arc_polyline(
        cx,
        cy,
        ring_r,
        0.0,
        360.0,
        stroke,
        white,
        0.5 * (1.0 - e),
        saturation,
        hue,
    );

    finalize_frame(vec![halo, dot], vec![], get(o, "rMin", 0.3)).with_polylines(vec![ring])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    fn with_accuracy(a: f64) -> ModeOpts {
        opts(&[("accuracy", a), ("period", 2.0), ("rMin", 0.3)])
    }

    #[test]
    fn halo_shrinks_with_accuracy_and_sits_under_the_dot() {
        let vague = frame_halo(64.0, 0.0, &with_accuracy(0.2));
        let precise = frame_halo(64.0, 0.0, &with_accuracy(1.0));
        assert_eq!(vague.dots.len(), 2);
        let (halo, dot) = (&vague.dots[0], &vague.dots[1]);
        assert!(
            halo.r > dot.r && halo.a < dot.a,
            "z-sort puts the big faint halo first"
        );
        assert!(
            vague.dots[0].r > precise.dots[0].r,
            "a vaguer fix means a wider halo"
        );
        assert!((precise.dots[0].r - halo_radius(64.0, 1.0)).abs() < 1e-9);
    }

    #[test]
    fn the_ring_pulses_from_the_dot_to_the_halo_edge_and_fades() {
        let o = with_accuracy(0.5);
        let radius_at = |t: f64| {
            let f = frame_halo(64.0, t, &o);
            let p = &f.polylines[0].points[0];
            (
                ((p.x - 32.0).powi(2) + (p.y - 32.0).powi(2)).sqrt(),
                f.polylines[0].a,
            )
        };
        let (r0, a0) = radius_at(0.0);
        let (r1, a1) = radius_at(1.0);
        assert!(
            (r0 - 6.4).abs() < 1e-6,
            "born at the dot's edge (dotSize 0.1 of 64)"
        );
        assert!(
            r1 > r0 && r1 <= halo_radius(64.0, 0.5) + 1e-9,
            "grows toward, never past, the halo"
        );
        assert!(a1 < a0, "and fades as it goes");
        assert!(
            frame_halo(64.0, 1.99, &o).polylines.is_empty(),
            "gone by the end of the period"
        );
    }
}
