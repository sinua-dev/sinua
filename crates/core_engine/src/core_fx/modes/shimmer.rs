//! Core / Shimmer: the inline "generating..." sweep (`generating`) -- a
//! faint horizontal pill with a bright highlight gliding across it, the
//! skeleton-shimmer every SaaS loading state uses and the closest cousin of
//! Copilot's/Claude's generating bars. Transcribed from
//! react-loading-skeleton's `skeleton.css`: a `linear-gradient(90deg, base
//! 0%, highlight 50%, base 100%)` band swept from `translateX(-100%)` to
//! `translateX(100%)` over `1.5s ease-in-out`, forever. VS Code's infinite
//! progress bar (Copilot's own generating indicator) is the same idea as a
//! thin line -- set `thickness` low for that look.
//!
//! No gradient primitive exists in this engine, so the soft highlight is
//! approximated with `LAYERS` concentric `Polyline` segments sharing one
//! center: each successive layer is shorter, all at the same low alpha, so
//! source-over stacking brightens the middle in steps -- a stepped bell,
//! which at these sizes reads as a smooth glow. Round caps come from the
//! paint contract; because every layer has the same thickness and center,
//! the caps nest rather than bump. Layers are clipped to the track so the
//! highlight enters from off-track on the left and leaves off-track on the
//! right, exactly like the CSS `-100%..100%` sweep.

use std::f64::consts::PI;

use crate::primitives::{
    cubic_bezier, finalize_frame, Fill, FillGradient, GradientStop, ModeOpts, OrbFrame, Point,
    Polyline,
};

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

/// Concentric highlight layers (a stepped stand-in for a gradient). Nine
/// steps, not five: at five the layer ends read as distinct nested pills
/// (checked by rendering), at nine they blur into a glow.
pub const LAYERS: usize = 9;
/// Per-layer alpha; nine stacked give a ~0.61 peak at the center.
const LAYER_ALPHA: f64 = 0.10;

/// CSS `ease-in-out`.
fn ease_in_out(u: f64) -> f64 {
    cubic_bezier(0.42, 0.0, 0.58, 1.0, u)
}

/// Where the highlight's center sits at `t`, in `0..1` of the sweep range
/// (`0` = fully off the left end, `1` = fully off the right end).
pub fn sweep_position(t: f64, period: f64) -> f64 {
    ease_in_out((t / period).fract())
}

#[allow(clippy::too_many_arguments)]
fn segment(
    x_a: f64,
    x_b: f64,
    y: f64,
    w: f64,
    white: f64,
    a: f64,
    saturation: f64,
    hue: f64,
) -> Polyline {
    Polyline {
        points: vec![Point { x: x_a, y }, Point { x: x_b, y }],
        white,
        a,
        w,
        saturation,
        hue,
        hues: Vec::new(),
    }
}

/// The pill's outline as a closed polygon: a stadium spanning `[x0, x1]`
/// with semicircular caps of radius `rad`, `CAP` segments per cap.
fn stadium(x0: f64, x1: f64, y: f64, rad: f64) -> Vec<Point> {
    const CAP: usize = 12;
    let mut pts = Vec::with_capacity(2 * (CAP + 1));
    for i in 0..=CAP {
        // Right cap, top -> bottom.
        let a = -PI / 2.0 + PI * i as f64 / CAP as f64;
        pts.push(Point {
            x: x1 - rad + rad * a.cos(),
            y: y + rad * a.sin(),
        });
    }
    for i in 0..=CAP {
        // Left cap, bottom -> top.
        let a = PI / 2.0 + PI * i as f64 / CAP as f64;
        pts.push(Point {
            x: x0 + rad + rad * a.cos(),
            y: y + rad * a.sin(),
        });
    }
    pts
}

/// `highlightFill` 1 (materials phase 1): what the stepped layers stand in
/// for, drawn for real -- the track as a solid pill `Fill` and the highlight
/// as the *same* pill with a linear gradient band (transparent -> peak ->
/// transparent, padded transparent beyond), i.e. react-loading-skeleton's
/// `linear-gradient(90deg, base, highlight 50%, base)` swept across and
/// clipped to the track by the shape itself. Peak alpha matches the nine
/// stacked layers' `1 - 0.9^9`.
fn frame_shimmer_fill(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let period = get(o, "period", 1.5).max(0.05);
    let length = size * get(o, "length", 0.84).clamp(0.1, 1.0);
    let thick = size * get(o, "thickness", 0.12).clamp(0.01, 0.5);
    let highlight = length * get(o, "highlightLength", 0.35).clamp(0.05, 1.0);
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let y = size * 0.5;
    let white = 0.15;
    let x0 = (size - length) * 0.5;
    let x1 = x0 + length;
    let pill = stadium(x0, x1, y, thick * 0.5);
    let mut fills = Vec::with_capacity(2);
    if track_alpha > 0.0 {
        fills.push(Fill {
            points: pill.clone(),
            white,
            a: track_alpha,
            ..Default::default()
        });
    }
    let center = (x0 - highlight * 0.5) + (length + highlight) * sweep_position(t, period);
    let peak = 1.0 - (1.0 - LAYER_ALPHA).powi(LAYERS as i32);
    let stop = |offset: f64, a: f64| GradientStop {
        offset,
        white,
        a,
        saturation,
        hue,
    };
    fills.push(Fill {
        points: pill,
        holes: Vec::new(),
        white,
        a: peak,
        saturation,
        hue,
        gradient: Some(FillGradient {
            kind: 0,
            x0: center - highlight * 0.5,
            y0: y,
            x1: center + highlight * 0.5,
            y1: y,
            r: 0.0,
            stops: vec![stop(0.0, 0.0), stop(0.5, 1.0), stop(1.0, 0.0)],
        }),
        blur: 0.0,
        blend: 0,
    });
    let mut f = finalize_frame(vec![], vec![], get(o, "rMin", 0.3));
    f.fills = fills;
    f
}

pub fn frame_shimmer(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    if get(o, "highlightFill", 0.0).round() == 1.0 {
        return frame_shimmer_fill(size, t, o);
    }
    let period = get(o, "period", 1.5).max(0.05);
    let length = size * get(o, "length", 0.84).clamp(0.1, 1.0);
    let thick = size * get(o, "thickness", 0.12).clamp(0.01, 0.5);
    let highlight = length * get(o, "highlightLength", 0.35).clamp(0.05, 1.0);
    let track_alpha = get(o, "trackOpacity", 0.18).clamp(0.0, 1.0);
    let hue = get(o, "hue", 200.0);
    let saturation = get(o, "saturation", 0.0).clamp(0.0, 1.0);
    let y = size * 0.5;
    let white = 0.15;

    // The visible pill spans [x0, x1]; the stroke itself is shortened by a
    // cap radius at each end so round caps land exactly on those edges.
    let x0 = (size - length) * 0.5;
    let x1 = x0 + length;
    let (lo, hi) = (x0 + thick * 0.5, x1 - thick * 0.5);

    let mut polylines: Vec<Polyline> = Vec::with_capacity(1 + LAYERS);
    if track_alpha > 0.0 {
        polylines.push(segment(lo, hi, y, thick, white, track_alpha, 0.0, 0.0));
    }

    // Center travels from one highlight-length before the track to one
    // after it, so the glow fully enters and fully leaves.
    let center = (x0 - highlight * 0.5) + (length + highlight) * sweep_position(t, period);
    for k in 0..LAYERS {
        // Outermost layer first (longest), innermost last (shortest).
        let half = highlight * 0.5 * (1.0 - k as f64 / LAYERS as f64);
        let a = (center - half).max(lo);
        let b = (center + half).min(hi);
        if b < a {
            continue;
        }
        polylines.push(segment(a, b, y, thick, white, LAYER_ALPHA, saturation, hue));
    }

    finalize_frame(vec![], vec![], get(o, "rMin", 0.3)).with_polylines(polylines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::opts;

    fn base() -> ModeOpts {
        opts(&[("period", 1.5), ("rMin", 0.3)])
    }

    fn highlight_layers(f: &OrbFrame) -> &[Polyline] {
        &f.polylines[1..]
    }

    #[test]
    fn track_spans_the_pill_and_stays_faint() {
        let f = frame_shimmer(64.0, 0.0, &base());
        let track = &f.polylines[0];
        assert!((track.a - 0.18).abs() < 1e-9);
        // visible pill = stroke + two cap radii = 0.84 * 64
        let visible = (track.points[1].x - track.points[0].x) + track.w;
        assert!((visible - 64.0 * 0.84).abs() < 1e-9);
        assert!((track.points[0].y - 32.0).abs() < 1e-9 && (track.points[1].y - 32.0).abs() < 1e-9);
    }

    #[test]
    fn highlight_is_a_stepped_bell_of_concentric_layers() {
        let f = frame_shimmer(64.0, 0.75, &base()); // mid-sweep: fully on the track
        let layers = highlight_layers(&f);
        assert_eq!(layers.len(), LAYERS);
        let center = |p: &Polyline| (p.points[0].x + p.points[1].x) * 0.5;
        let len = |p: &Polyline| p.points[1].x - p.points[0].x;
        for w in layers.windows(2) {
            assert!(
                (center(&w[0]) - center(&w[1])).abs() < 1e-9,
                "layers share a center"
            );
            assert!(
                len(&w[1]) < len(&w[0]),
                "each layer is shorter than the one under it"
            );
            assert_eq!(
                w[0].w, w[1].w,
                "same thickness, so the caps nest instead of bumping"
            );
        }
    }

    #[test]
    fn highlight_sweeps_left_to_right_inside_the_track_and_wraps() {
        let mut prev = f64::MIN;
        let track = frame_shimmer(64.0, 0.0, &base()).polylines[0].clone();
        for i in 0..=20 {
            let t = i as f64 * 1.5 / 20.0 * 0.999;
            let f = frame_shimmer(64.0, t, &base());
            for p in highlight_layers(&f) {
                assert!(
                    p.points[0].x >= track.points[0].x - 1e-9
                        && p.points[1].x <= track.points[1].x + 1e-9,
                    "clipped to the track"
                );
            }
            if let Some(outer) = highlight_layers(&f).first() {
                let c = (outer.points[0].x + outer.points[1].x) * 0.5;
                assert!(c >= prev - 1e-9, "monotone left-to-right at t={t}");
                prev = c;
            }
        }
        // just after wrapping, the highlight is (almost) entirely off the left end again
        let wrapped = frame_shimmer(64.0, 1.5 + 0.001, &base());
        assert!(
            highlight_layers(&wrapped).len() < LAYERS,
            "off-track layers are dropped after the wrap"
        );
    }

    #[test]
    fn deterministic_and_thin_line_variant() {
        assert_eq!(
            frame_shimmer(64.0, 0.4, &base()),
            frame_shimmer(64.0, 0.4, &base())
        );
        let mut thin = base();
        thin.insert("thickness".to_string(), 0.03);
        assert!((frame_shimmer(64.0, 0.4, &thin).polylines[0].w - 64.0 * 0.03).abs() < 1e-9);
    }
}
