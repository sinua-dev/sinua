//! Beacon / Pulse: the connection indicator (`reconnecting`) -- a pulsing
//! dot with a four-segment quality ring around it. Two real sources: the
//! common "Reconnecting..." pattern (a pulsing amber dot next to the text
//! -- the text is the caller's job; "color carries none of the meaning on
//! its own", so grey is the default and amber is one `hue` away), and
//! LiveKit's `ConnectionQualityIndicator`, whose `excellent / good / poor /
//! lost` levels map onto a `quality: 0..1` opt here (`1 / 0.66 / 0.33 /
//! 0`), shown as how many of the four ring segments are lit.
//!
//! The pulse is Tailwind's `animate-pulse` (`50% { opacity: .5 }`, 2s,
//! `cubic-bezier(0.4, 0, 0.6, 1)`), scaled by `1 - quality`: a lost
//! connection pulses fully, a poor one faintly, an excellent one holds a
//! steady dot with all four segments lit -- so one state covers the whole
//! reconnect ramp without a state switch.

use crate::primitives::{
    arc_polyline, finalize_frame, pulse_wave, Dot, ModeOpts, OrbFrame, Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

pub const SEGMENTS: usize = 4;

/// Lit segment count for a quality reading -- `0` only for a fully lost
/// connection, so any live-but-poor link still shows something.
pub fn lit_segments(quality: f64) -> usize {
    ((quality.clamp(0.0, 1.0) * SEGMENTS as f64) - 1e-9)
        .ceil()
        .max(0.0) as usize
}

pub fn frame_pulse(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let period = get(o, "period", 2.0).max(0.05);
    let quality = get(o, "quality", 0.0).clamp(0.0, 1.0);
    let dot_r = size * get(o, "dotSize", 0.12).clamp(0.01, 0.4);
    let seg_r = size * get(o, "segmentRadius", 0.3).clamp(0.05, 0.5);
    let seg_stroke = size * get(o, "segmentWidth", 0.04).clamp(0.005, 0.2);
    let seg_gap = get(o, "segmentGap", 14.0).clamp(0.0, 60.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let (cx, cy) = (size * 0.5, size * 0.5);
    let white = 0.15;

    let amp = 1.0 - quality;
    // How far into the dip the pulse is: `0` at the ends of a period, `1` at
    // the midpoint -- Tailwind's pulse curve, shared with `apply_pulse`.
    let d = pulse_wave((t / period).fract()) * amp;
    let dot = Dot {
        x: cx,
        y: cy,
        z: 0.0,
        r: dot_r * (1.0 - 0.1 * d),
        white,
        a: 0.95 * (1.0 - 0.5 * d),
        saturation,
        hue,
    };

    let lit = lit_segments(quality);
    let seg_sweep = 360.0 / SEGMENTS as f64 - seg_gap;
    let mut polylines: Vec<Polyline> = Vec::with_capacity(SEGMENTS);
    for k in 0..SEGMENTS {
        let start = k as f64 * 360.0 / SEGMENTS as f64 + seg_gap * 0.5;
        let is_lit = k < lit;
        polylines.push(arc_polyline(
            cx,
            cy,
            seg_r,
            start,
            seg_sweep,
            seg_stroke,
            white,
            if is_lit { 0.9 } else { 0.15 },
            if is_lit { saturation } else { 0.0 },
            hue,
        ));
    }

    finalize_frame(vec![dot], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    fn with_quality(q: f64) -> ModeOpts {
        opts(&[("quality", q), ("period", 2.0), ("rMin", 0.3)])
    }

    #[test]
    fn a_lost_connection_pulses_between_full_and_half_alpha() {
        let peak = frame_pulse(64.0, 0.0, &with_quality(0.0));
        let dip = frame_pulse(64.0, 1.0, &with_quality(0.0));
        assert!((peak.dots[0].a - 0.95).abs() < 1e-9);
        assert!(
            (dip.dots[0].a - 0.475).abs() < 1e-6,
            "Tailwind's 50% -> opacity .5"
        );
        assert!(
            dip.dots[0].r < peak.dots[0].r,
            "a slight shrink with the dip"
        );
        assert_eq!(lit_segments(0.0), 0);
        assert!(
            peak.polylines.iter().all(|p| p.a < 0.2),
            "no segment lit when lost"
        );
    }

    #[test]
    fn quality_lights_segments_and_steadies_the_dot() {
        let excellent_a = frame_pulse(64.0, 0.0, &with_quality(1.0));
        let excellent_b = frame_pulse(64.0, 1.0, &with_quality(1.0));
        assert_eq!(
            excellent_a.dots[0], excellent_b.dots[0],
            "no pulse at full quality"
        );
        assert_eq!(lit_segments(1.0), 4);
        assert_eq!(
            excellent_a.polylines.iter().filter(|p| p.a > 0.5).count(),
            4
        );
        assert_eq!(lit_segments(0.5), 2);
        let half = frame_pulse(64.0, 0.0, &with_quality(0.5));
        assert_eq!(half.polylines.iter().filter(|p| p.a > 0.5).count(), 2);
        assert_eq!(
            lit_segments(0.33),
            2,
            "LiveKit 'poor' still shows something"
        );
        assert_eq!(lit_segments(0.25), 1);
    }

    #[test]
    fn segments_are_evenly_spaced_with_gaps() {
        let frame = frame_pulse(64.0, 0.0, &with_quality(1.0));
        assert_eq!(frame.polylines.len(), SEGMENTS);
        for p in &frame.polylines {
            let (a, b) = (&p.points[0], p.points.last().unwrap());
            assert!(
                ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt() > 1.0,
                "each segment is an open arc, not a closed ring"
            );
        }
    }
}
