//! Family-agnostic primitives: the `Dot`/`Line`/`OrbFrame` geometry types,
//! generic math (noise, lerp, angle helpers), and the mode-agnostic
//! post-processes (`apply_pointer`, `apply_audio_reactive`, the mute/
//! interrupt cues and the glow/noise/gradient materials) that `render()`
//! (`lib.rs`) applies after ANY family's mode function runs.
//!
//! Pulled out of `orbs::core` specifically because `orbs` stopped being the
//! only family (see `signal/mod.rs`) -- with the product plan's planned
//! component set (Orb, Core, Ring, Signal, Beacon) all needing the same
//! dot-cloud rendering contract, having every future family import from
//! `orbs::core` would make `orbs` (the *first* family, not a shared-code
//! root) a false dependency hub. Doing this extraction now, with only two
//! families, is cheap; doing it after three more families already
//! depended on `orbs::core` would not have been.
//!
//! What's deliberately NOT here: `fib_dir` (Fibonacci *sphere* lattice) and
//! `Proj` (spin/tilt/orthographic *sphere* projection) stay in
//! `orbs::core`, because they're genuinely specific to a spherical family --
//! `signal`'s flat bar layout doesn't use either, and there's no reason to
//! assume `Core`/`Ring`/`Beacon` will either. `orbs::core` re-exports
//! everything below so none of the 18 existing `orbs` mode files' imports
//! needed to change.
//!
//! `Dot`'s `saturation`/`hue` fields, `perlin3`, and both `apply_*`
//! functions are NOT part of the `thinking-orbs` port (see
//! `docs/engine.md`'s "Additive capabilities beyond the port") -- they
//! predate this file's extraction from `orbs/core.rs` and kept their own
//! provenance notes inline below.

use std::collections::HashMap;

/// A mode's runtime options -- string keys, `f64` values, no per-mode
/// struct, so any post-process (`apply_pointer`, `apply_audio_reactive`)
/// or shared helper (`audio_band`) can read a key without knowing which
/// family's mode produced it. `orbs::profiles::ModeOpts` is the same
/// alias, defined separately there rather than re-exported from here, to
/// avoid `signal` (or any future family) needing to import a type through
/// `orbs` for something this generic.
pub type ModeOpts = HashMap<String, f64>;

/// Builds a `ModeOpts` from literal pairs -- a test convenience today
/// (only `signal::modes::bar`'s tests use it; `orbs::profiles::opts` is the
/// same one-liner, used by real preset-building code there too, so it
/// isn't `cfg(test)`-gated the way this one is).
#[cfg(test)]
pub fn opts(pairs: &[(&str, f64)]) -> ModeOpts {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

// Record derive is per-target, same reasoning as the Cargo.toml split:
// uniffi::Record generates FFI scaffolding for the native (iOS/Android/RN)
// bindgen path, serde::Serialize feeds the wasm-bindgen JSON bridge for Web.
// Neither crate is a dependency on the target that doesn't use it.

/// One dot in a rendered frame. `a` is always resolved (upstream's optional
/// `white?: number` defaults to 1 before a frame is finalized), so unlike the
/// TS `Dot` this field is not optional — simpler across the UniFFI boundary.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub r: f64,
    /// Ink value: 0 = darkest ink on paper. Mirrored on dark themes by the renderer.
    pub white: f64,
    pub a: f64,
    /// 0 = achromatic (every ported `orbs` mode's value, via
    /// `..Default::default()` at each `Dot` literal) — the renderer's
    /// existing grayscale-ink path applies unchanged. Above 0, blend
    /// `white` (as lightness) toward `hue` by this amount instead.
    pub saturation: f64,
    /// Hue in degrees `[0, 360)`. Meaningless when `saturation` is `0.0`.
    pub hue: f64,
}

/// A stroked edge between two points (the `connecting`/`web` `orbs` mode is
/// the original user; family-agnostic like everything else here).
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub white: f64,
    pub a: f64,
    pub w: f64,
    /// Same semantics as `Dot.saturation` -- `0` = grayscale ink. Added
    /// 2026-09-18 (Color system): before, lines had no colour at all, so
    /// `connecting`'s edges stayed grey under every material. Every mode
    /// emits `0.0` (grey, exactly the old paint); only post-processes
    /// (`apply_color`, `apply_gradient`, `apply_glow` tint, `apply_muted`,
    /// `apply_interrupt`) colour them.
    pub saturation: f64,
    /// Same semantics as `Dot.hue`.
    pub hue: f64,
}

/// One vertex of a `Polyline`. Its own record rather than a tuple because
/// UniFFI can't export tuples, and `Dot` (which carries `z`/`r`/ink) would
/// be the wrong shape for a bare vertex.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A continuous stroked path through `points`, in order -- the primitive a
/// smooth curve or a pill-shaped bar needs and `Line` can't provide.
///
/// Why this exists: `signal::modes::waveform` first drew its curve as ~48
/// consecutive but *independent* `Line` segments. Every renderer strokes
/// each `Line` as its own butt-capped path, so at every bend the segments
/// left a wedge-shaped gap on the outside and a double-painted (darker, at
/// alpha < 1) notch on the inside -- a visibly segmented, "caterpillar"
/// trace at any point count, and chopped-flat ends. The fix has to be one
/// stroke with round joins and caps, which is a renderer-level property no
/// arrangement of `Line`s can express -- hence a new primitive, not a
/// denser polyline. Not part of the `thinking-orbs` port: no `orbs` mode
/// emits one, so `spec/orbs-golden.json` and `tests/golden.rs` are
/// untouched (same additive pattern as `Dot.saturation`/`hue`).
///
/// Paint contract (the `Polyline` counterpart of `spec/orbs-spec.json`'s
/// `paint` block, ported identically to every renderer -- `drawFrame.ts`,
/// `OrbCanvasView.swift`, `OrbCanvas.kt`, and each platform's SVG
/// exporter): polylines draw **first** (then lines, then dots), each as ONE
/// stroked path with round line caps and round line joins, no fill, stroke
/// width `w`, and the same ink rule as `Dot` (`white` as grey/lightness,
/// switching to HSL via `hue` when `saturation > 0`). One alpha for the
/// whole path -- a per-vertex alpha ramp can't be expressed as a single
/// stroke, so a mode that wants a fade along a curve emits several
/// polylines instead (`waveform`'s layers do exactly this). A zero-length
/// path (two coincident points) renders as a round dot of diameter `w`
/// under round caps on every target renderer -- `bar` relies on this for
/// its shortest pills.
///
/// Per-vertex hue (2026-09-19): when `hues` is non-empty, draw each segment
/// `i-1 -> i` as a round-capped stroke of width `w` with a linear gradient
/// from `ink(white, saturation, hues[i-1])` to `ink(.., hues[i])` at alpha
/// 1 (a zero-length segment: a round dot of diameter `w` in vertex `i`'s
/// colour), all segments into ONE offscreen/transparency layer, then
/// composite that layer once at `a` -- with the element's effect run
/// (blur, blend) applied at that composite. Inside the opaque layer the
/// round caps overlap without double-painting, so the stroke keeps the
/// single-path, no-seam look this primitive exists for. Between vertices
/// the renderer's own gradient interpolation (sRGB) is used: vertices are
/// dense, and RGB interpolation handles a 359 -> 1 hue wrap by itself. A
/// renderer without per-vertex support paints `hue`, exactly as before.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polyline {
    pub points: Vec<Point>,
    /// Ink value, same meaning as `Dot.white`.
    pub white: f64,
    pub a: f64,
    /// Stroke width, in engine units (the same space as `x`/`y`).
    pub w: f64,
    /// Same semantics as `Dot.saturation` -- `0` = grayscale ink.
    pub saturation: f64,
    /// Same semantics as `Dot.hue`. With `hues`, this stays the whole
    /// stroke's single-hue sample (the vertex mean) -- the fallback a
    /// renderer without per-vertex support paints.
    pub hue: f64,
    /// Per-vertex hue (degrees), one per `points` entry, or empty = the
    /// single `hue` for the whole stroke (every mode emits empty). Set by
    /// `apply_gradient`/`apply_holo` only when the vertex hues differ, so a
    /// ring's track sweeps along its length instead of taking one colour.
    /// `white`, `a`, `w`, `saturation` stay uniform. Paint contract: see
    /// the per-vertex paragraph above. Omitted from JSON when empty.
    #[cfg_attr(target_arch = "wasm32", serde(skip_serializing_if = "Vec::is_empty"))]
    pub hues: Vec<f64>,
}

/// How a renderer resolves an element's lightness against its theme -- the
/// frame-level half of the paint contract (added 2026-09-18, Color system).
/// Material 3 draws the same line: normal colour roles adapt per theme,
/// while its *fixed* roles "maintain the same tone in light and dark
/// themes" (androidx `ColorScheme.kt`, `primaryFixed`).
///
/// - `Ink` (default, every frame before this existed): lightness is
///   `white` on light themes and mirrored, `1 - white`, on dark ones --
///   right for grey ink, and why a mode's near-dark/far-light depth ramp
///   reads the same way round in both themes.
/// - `Fixed`: lightness is `white` in **both** themes, for every element
///   of the frame (grey tracks included) -- a brand colour looks identical
///   everywhere. Set by `apply_color`'s `colorMode` opt.
///
/// Frame-level rather than per-element: `apply_color` recolours the whole
/// frame anyway, and a renderer needs one branch, not one per record.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Enum))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "lowercase"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Ink,
    Fixed,
}

/// One rendered instant: a complete, final set of draw instructions. `dots`
/// is already z-sorted into draw order and radius-clamped; draw order is
/// `polylines`, then `lines`, then `dots`. Named `OrbFrame` for historical
/// reasons (it predates the `signal` family) -- kept as-is rather than
/// renamed, since the name is public API surface (the UniFFI record name
/// generated into Swift/Kotlin, the wasm-bindgen JSON shape,
/// `packages/core`'s TS type, and every doc mentioning it), and a rename's
/// churn isn't worth it for a cosmetic fix.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrbFrame {
    pub dots: Vec<Dot>,
    pub lines: Vec<Line>,
    /// Empty for every `orbs` mode; only `signal` emits these today.
    pub polylines: Vec<Polyline>,
    /// How the renderer resolves lightness per theme -- see `ColorMode`.
    /// `Ink` unless `apply_color`'s `colorMode` asked for `Fixed`.
    #[cfg_attr(target_arch = "wasm32", serde(rename = "colorMode"))]
    pub color_mode: ColorMode,
    /// Filled shapes (materials phase 1), drawn *first* -- under polylines,
    /// lines and dots. Empty for every mode unless an opt asks for fills;
    /// omitted from JSON when empty so existing frames serialize unchanged.
    #[cfg_attr(target_arch = "wasm32", serde(skip_serializing_if = "Vec::is_empty"))]
    pub fills: Vec<Fill>,
    /// Paint effects on ranges of `dots` / `lines` / `polylines` (today:
    /// real-blur glow halos). Omitted from JSON when empty.
    #[cfg_attr(target_arch = "wasm32", serde(skip_serializing_if = "Vec::is_empty"))]
    pub effects: Vec<EffectRun>,
}

/// A closed, filled polygon (the engine tessellates arcs into points, the
/// same way polylines are built). Paint contract (docs/engine.md):
/// - ink is `white`/`a`/`saturation`/`hue` exactly like a `Dot` -- mirrored
///   on dark themes unless the frame's `color_mode` is `Fixed`;
/// - with a `gradient`, the stops replace the solid ink and each stop's
///   final alpha is `stop.a * fill.a` (so pulse/decay/mute/cross-fades only
///   ever scale `a`); the solid ink stays as a fallback;
/// - `blur` is a Gaussian sigma in engine units (0 = none), `blend` 0 =
///   normal, 1 = additive (Canvas `lighter`, SwiftUI `plusLighter`, Compose
///   `BlendMode.Plus`).
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fill {
    pub points: Vec<Point>,
    /// Inner rings (liquid's bands have them): painted with the *even-odd*
    /// rule together with `points` as one path. Omitted from JSON when
    /// empty -- every fill before materials phase 2 has none.
    #[cfg_attr(target_arch = "wasm32", serde(skip_serializing_if = "Vec::is_empty"))]
    pub holes: Vec<Vec<Point>>,
    pub white: f64,
    pub a: f64,
    pub saturation: f64,
    pub hue: f64,
    pub gradient: Option<FillGradient>,
    pub blur: f64,
    pub blend: u8,
}

/// `kind` 0 = linear from (x0, y0) to (x1, y1); 1 = radial, centre (x0, y0),
/// radius 0 to `r`. Pad beyond the ends. No conic: SwiftUI's
/// `GraphicsContext` conic shading is iOS 18+ and SVG has none.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FillGradient {
    pub kind: u8,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub r: f64,
    /// 2-3 stops, sorted by `offset` in 0..1.
    pub stops: Vec<GradientStop>,
}

#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradientStop {
    pub offset: f64,
    pub white: f64,
    /// Relative: the painted alpha is `stop.a * fill.a`.
    pub a: f64,
    pub saturation: f64,
    pub hue: f64,
}

/// Elements `[start, start + count)` of one list (`target` 0 = dots, 1 =
/// lines, 2 = polylines) composited with a Gaussian `blur` (sigma, engine
/// units) and `blend` (0 normal, 1 additive). Never changes draw order.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectRun {
    pub target: u8,
    pub start: u32,
    pub count: u32,
    pub blur: f64,
    pub blend: u8,
}

impl Polyline {
    /// Recolours the stroke through an ink rule `f(saturation, hue) ->
    /// (saturation, hue)`: the single `hue` (and `saturation`) as before,
    /// and every per-vertex hue with the same pre-change saturation -- the
    /// `Polyline` counterpart of `Fill::map_ink`. Drops `hues` once they
    /// have all collapsed to one value (e.g. an interrupt flash snapping
    /// every vertex to its hue), so nothing pays for a layer it can't use.
    pub fn map_hue(&mut self, f: impl Fn(f64, f64) -> (f64, f64)) {
        let sat = self.saturation;
        for h in self.hues.iter_mut() {
            *h = f(sat, *h).1;
        }
        let (s, h) = f(sat, self.hue);
        self.saturation = s;
        self.hue = h;
        self.drop_uniform_hues();
    }

    /// Sets per-vertex hues from `hue_at(index, point)`, keeping them only
    /// when they differ -- a stroke whose vertices all get one hue stays a
    /// plain single-hue stroke (no per-vertex paint cost).
    fn set_hues(&mut self, hue_at: impl Fn(usize, &Point) -> f64) {
        self.hues = self
            .points
            .iter()
            .enumerate()
            .map(|(i, q)| hue_at(i, q))
            .collect();
        self.drop_uniform_hues();
    }

    fn drop_uniform_hues(&mut self) {
        if let Some(&h0) = self.hues.first() {
            if self.hues.iter().all(|&h| h == h0) {
                self.hues.clear();
            }
        }
    }
}

impl Fill {
    /// Moves every point of the shape -- and its gradient's geometry, so the
    /// ramp stays attached -- through `f`. The post-processes use this to
    /// treat a fill like a polyline's vertices.
    pub fn map_points(&mut self, mut f: impl FnMut(f64, f64) -> (f64, f64)) {
        for p in self.points.iter_mut() {
            (p.x, p.y) = f(p.x, p.y);
        }
        for ring in self.holes.iter_mut() {
            for p in ring.iter_mut() {
                (p.x, p.y) = f(p.x, p.y);
            }
        }
        if let Some(g) = self.gradient.as_mut() {
            (g.x0, g.y0) = f(g.x0, g.y0);
            (g.x1, g.y1) = f(g.x1, g.y1);
        }
    }

    /// The vertex mean (for gradient sampling and centroids).
    pub fn centroid(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let n = self.points.len() as f64;
        let (x, y) = self
            .points
            .iter()
            .fold((0.0, 0.0), |(ax, ay), q| (ax + q.x, ay + q.y));
        Some((x / n, y / n))
    }

    /// Applies an ink/colour transform to the solid ink and every stop.
    pub fn map_ink(&mut self, mut f: impl FnMut(f64, f64, f64) -> (f64, f64, f64)) {
        (self.white, self.saturation, self.hue) = f(self.white, self.saturation, self.hue);
        if let Some(g) = self.gradient.as_mut() {
            for st in g.stops.iter_mut() {
                (st.white, st.saturation, st.hue) = f(st.white, st.saturation, st.hue);
            }
        }
    }
}

impl OrbFrame {
    /// Attaches stroked paths to a frame `finalize_frame` already built,
    /// applying the same visibility cull (`a < 0.02`) and dropping any
    /// path with fewer than two vertices (nothing to stroke). A separate
    /// step rather than a fourth `finalize_frame` parameter so the 18
    /// `orbs` mode files -- none of which emit polylines -- don't all have
    /// to pass an empty `Vec` for a primitive they never use.
    pub fn with_polylines(mut self, polylines: Vec<Polyline>) -> Self {
        self.polylines = polylines
            .into_iter()
            .filter(|p| p.a >= 0.02 && p.points.len() >= 2)
            .collect();
        self
    }
}

pub fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

/// Interpolates a hue (degrees, `[0, 360)`) along whichever direction is
/// shorter around the color wheel -- a plain `lerp` on raw degree values
/// can take the long way around (350 -> 10 via 300..0 instead of the 20
/// degrees the other way), which reads as an unwanted rainbow sweep during
/// a transition. Used by `orbs::modes::transition`.
pub fn lerp_hue(a: f64, b: f64, f: f64) -> f64 {
    let delta = ((b - a + 540.0) % 360.0) - 180.0;
    (a + delta * f).rem_euclid(360.0)
}

pub fn frac(x: f64) -> f64 {
    x - x.floor()
}

/// Value noise on a 2D lattice — smooth, deterministic, cheap.
pub fn vnoise(x: f64, y: f64) -> f64 {
    let xi = x.floor();
    let yi = y.floor();
    let mut fx = x - xi;
    let mut fy = y - yi;
    fx = fx * fx * (3.0 - 2.0 * fx);
    fy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash_d(xi, yi);
    let b = hash_d(xi + 1.0, yi);
    let c = hash_d(xi, yi + 1.0);
    let d = hash_d(xi + 1.0, yi + 1.0);
    a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy
}

/// Deterministic hash in [0, 1).
pub fn hash_d(a: f64, b: f64) -> f64 {
    let h = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    h - h.floor()
}

/// Shortest signed angular distance, wrapped to (-pi, pi].
pub fn angle_delta(a: f64, b: f64) -> f64 {
    (a - b).sin().atan2((a - b).cos())
}

/// Turn raw mode output into a finished frame: drop invisible marks, clamp
/// radii to the mode's floor, and z-sort far->near into draw order.
///
/// Runs in the geometry step so a frame is a complete set of draw
/// instructions — every value final, array order is draw order. `Vec::sort_by`
/// is a stable sort (matches JS `Array.prototype.sort`), which matters for
/// dots that tie exactly on `z`.
pub fn finalize_frame(dots: Vec<Dot>, lines: Vec<Line>, r_min: f64) -> OrbFrame {
    let mut visible: Vec<Dot> = Vec::with_capacity(dots.len());
    for mut d in dots {
        if d.a < 0.02 {
            continue;
        }
        d.r = d.r.max(r_min);
        visible.push(d);
    }
    visible.sort_by(|a, b| a.z.partial_cmp(&b.z).expect("z is never NaN"));
    OrbFrame {
        color_mode: ColorMode::Ink,
        dots: visible,
        lines: lines.into_iter().filter(|l| l.a >= 0.02).collect(),
        // Attached separately via `OrbFrame::with_polylines` by the modes
        // that emit them (`signal` only, today).
        polylines: Vec::new(),
        fills: Vec::new(),
        effects: Vec::new(),
    }
}

/// Dot radii were tuned for a 300pt frame; sub-linear scaling keeps small
/// spinners legible. Lower `pow` = radii shrink less with size.
pub fn radius_scale(size: f64, pow: f64) -> f64 {
    (size / 300.0).powf(pow)
}

/// One entry of a real, externally-supplied audio spectrum -- fed in via
/// `frame_with_overrides` as `audioBand0`, `audioBand1`, ... (a flat
/// `HashMap<String, f64>` opts map can't carry a `Vec`, so indexed keys are
/// the encoding, same as every other per-axis opt in this engine). No
/// allocation, unlike a `format!`-built key, since the count is small and
/// fixed. Capped at 16 -- comfortably above any bar count a caller's own
/// audio analysis would realistically produce (the reference Web Audio
/// pipeline in the Web Studio defaults to 5). Used by both
/// `orbs::modes::spectrum` and `signal::modes::bar`.
pub fn audio_band(o: &HashMap<String, f64>, index: usize) -> Option<f64> {
    let key = match index {
        0 => "audioBand0",
        1 => "audioBand1",
        2 => "audioBand2",
        3 => "audioBand3",
        4 => "audioBand4",
        5 => "audioBand5",
        6 => "audioBand6",
        7 => "audioBand7",
        8 => "audioBand8",
        9 => "audioBand9",
        10 => "audioBand10",
        11 => "audioBand11",
        12 => "audioBand12",
        13 => "audioBand13",
        14 => "audioBand14",
        15 => "audioBand15",
        _ => return None,
    };
    o.get(key).copied()
}

/// The point on a ring of radius `r` at `deg` degrees **clockwise from 12
/// o'clock** (canvas y grows downward, so "up" is `cy - r`) -- Material's
/// `270f` start angle in canvas terms. Shared by `ring` and `beacon`.
pub fn ring_point(cx: f64, cy: f64, r: f64, deg: f64) -> Point {
    let a = deg.to_radians();
    Point {
        x: cx + r * a.sin(),
        y: cy - r * a.cos(),
    }
}

/// Samples the arc starting at `start_deg` and sweeping `sweep_deg`
/// clockwise into one polyline, with a vertex at most every ~4 degrees and
/// always at both ends. A zero sweep yields two coincident points, which
/// the paint contract renders as a round dot of diameter `w` (Material's
/// "0% progress" look, for free). A 360 sweep is a closed circle. Lived in
/// `ring::geom` first; moved here once `beacon` needed it too, so the
/// second family to draw arcs doesn't import from the first (the same
/// "don't make an earlier family a false hub" reasoning as this file's
/// own extraction from `orbs::core`).
#[allow(clippy::too_many_arguments)]
pub fn arc_polyline(
    cx: f64,
    cy: f64,
    r: f64,
    start_deg: f64,
    sweep_deg: f64,
    w: f64,
    white: f64,
    a: f64,
    saturation: f64,
    hue: f64,
) -> Polyline {
    const STEP_DEG: f64 = 4.0;
    let sweep = sweep_deg.max(0.0);
    let segments = ((sweep / STEP_DEG).ceil() as usize).max(1);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let deg = start_deg + sweep * (i as f64 / segments as f64);
        points.push(ring_point(cx, cy, r, deg));
    }
    Polyline {
        points,
        white,
        a,
        w,
        saturation,
        hue,
        hues: Vec::new(),
    }
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` easing: solves `x(s) = u` for the
/// curve parameter `s` (Newton steps with a bisection fallback, the same
/// approach browsers use), then returns `y(s)`. Lets a mode transcribe a
/// CSS/Material animation curve exactly instead of approximating it.
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let bez = |p1: f64, p2: f64, s: f64| {
        let inv = 1.0 - s;
        3.0 * inv * inv * s * p1 + 3.0 * inv * s * s * p2 + s * s * s
    };
    let bez_dx = |s: f64| {
        let inv = 1.0 - s;
        3.0 * inv * inv * x1 + 6.0 * inv * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2)
    };
    let mut s = u;
    for _ in 0..8 {
        let x = bez(x1, x2, s) - u;
        if x.abs() < 1e-7 {
            return bez(y1, y2, s);
        }
        let d = bez_dx(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= x / d;
    }
    // Bisection fallback for the rare flat-derivative case.
    let (mut lo, mut hi) = (0.0, 1.0);
    s = u;
    for _ in 0..40 {
        let x = bez(x1, x2, s);
        if (x - u).abs() < 1e-7 {
            break;
        }
        if x < u {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    bez(y1, y2, s)
}

/// Material's standard easing, `cubic-bezier(0.4, 0, 0.2, 1)` -- the curve
/// both the MDC Web spinner (`$timing-function`) and Compose M3
/// (`EasingStandardCubicBezier`) use.
pub fn standard_ease(u: f64) -> f64 {
    cubic_bezier(0.4, 0.0, 0.2, 1.0, u)
}

/// One period of Tailwind's `animate-pulse` as a `0..1` depth: `0` at both
/// ends of the period, `1` at the midpoint (Tailwind's `50% { opacity: .5 }`
/// keyframe), eased with its `cubic-bezier(0.4, 0, 0.6, 1)`. Shared by
/// `beacon`'s `reconnecting` dot and the generic `apply_pulse`; `u` is the
/// phase within a period.
pub fn pulse_wave(u: f64) -> f64 {
    let tri = if u < 0.5 { u / 0.5 } else { (1.0 - u) / 0.5 };
    cubic_bezier(0.4, 0.0, 0.6, 1.0, tri)
}

/// `decayCurve` values for `decay_envelope`.
pub const DECAY_LINEAR: u32 = 0;
/// `(1 - u)^2` -- fast out, soft landing; `apply_interrupt`'s flash curve.
pub const DECAY_QUAD: u32 = 1;
/// Web Audio's `setTargetAtTime` shape (`e^(-t/tau)`, 95% of the way there
/// after `3 tau`) with `tau = duration / 3`, renormalised so it reaches
/// exactly `0` at `duration` instead of approaching it forever.
pub const DECAY_EXP: u32 = 2;
/// Material 3's exit curve, `EasingEmphasizedAccelerate`
/// `cubic-bezier(0.3, 0, 0.8, 0.15)`: holds, then drops away at the end.
pub const DECAY_EMPHASIZED: u32 = 3;

/// The shared one-shot fade envelope: `1` before the event (`age < 0`),
/// falling to `0` over `[0, duration)` along `curve` (`DECAY_*`; unknown
/// values fall back to `DECAY_QUAD`), `0` from `duration` on. `age` is in
/// seconds on the caller's clock, the same contract as `interruptAge`.
pub fn decay_envelope(age: f64, duration: f64, curve: u32) -> f64 {
    if age < 0.0 {
        return 1.0;
    }
    if age >= duration {
        return 0.0;
    }
    let u = age / duration;
    match curve {
        DECAY_LINEAR => 1.0 - u,
        DECAY_EXP => {
            let floor = (-3.0f64).exp();
            ((-3.0 * u).exp() - floor) / (1.0 - floor)
        }
        DECAY_EMPHASIZED => 1.0 - cubic_bezier(0.3, 0.0, 0.8, 0.15, u),
        _ => (1.0 - u).powi(2),
    }
}

/// Centroid over every dot, line endpoint and polyline vertex -- the point
/// the whole-frame "swell"/"shrink" post-processes scale about, so a
/// polyline-only `signal` frame has one too. `None` for an empty frame.
fn frame_centroid(frame: &OrbFrame) -> Option<(f64, f64)> {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0usize);
    for d in &frame.dots {
        sx += d.x;
        sy += d.y;
        n += 1;
    }
    for l in &frame.lines {
        sx += l.x1 + l.x2;
        sy += l.y1 + l.y2;
        n += 2;
    }
    for p in &frame.polylines {
        for pt in &p.points {
            sx += pt.x;
            sy += pt.y;
            n += 1;
        }
    }
    for f in &frame.fills {
        for pt in &f.points {
            sx += pt.x;
            sy += pt.y;
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some((sx / n as f64, sy / n as f64))
    }
}

/// Scales every element's position about `(cx, cy)` by `pos`, its size
/// (dot radius, stroke width) by `size`, and its alpha by `alpha`.
fn scale_frame(frame: &mut OrbFrame, (cx, cy): (f64, f64), pos: f64, size: f64, alpha: f64) {
    for d in frame.dots.iter_mut() {
        d.x = cx + (d.x - cx) * pos;
        d.y = cy + (d.y - cy) * pos;
        d.r *= size;
        d.a = (d.a * alpha).clamp(0.0, 1.0);
    }
    for l in frame.lines.iter_mut() {
        l.x1 = cx + (l.x1 - cx) * pos;
        l.y1 = cy + (l.y1 - cy) * pos;
        l.x2 = cx + (l.x2 - cx) * pos;
        l.y2 = cy + (l.y2 - cy) * pos;
        l.w *= size;
        l.a = (l.a * alpha).clamp(0.0, 1.0);
    }
    for p in frame.polylines.iter_mut() {
        for pt in p.points.iter_mut() {
            pt.x = cx + (pt.x - cx) * pos;
            pt.y = cy + (pt.y - cy) * pos;
        }
        p.w *= size;
        p.a = (p.a * alpha).clamp(0.0, 1.0);
    }
    for f in frame.fills.iter_mut() {
        f.map_points(|x, y| (cx + (x - cx) * pos, cy + (y - cy) * pos));
        if let Some(g) = f.gradient.as_mut() {
            g.r *= pos;
        }
        f.blur *= size;
        f.a = (f.a * alpha).clamp(0.0, 1.0);
    }
    for e in frame.effects.iter_mut() {
        e.blur *= size;
    }
}

#[cfg(test)]
mod geom_tests {
    use super::*;

    #[test]
    fn ring_point_walks_clockwise_from_twelve() {
        let top = ring_point(10.0, 10.0, 5.0, 0.0);
        let right = ring_point(10.0, 10.0, 5.0, 90.0);
        let bottom = ring_point(10.0, 10.0, 5.0, 180.0);
        assert!(
            (top.x - 10.0).abs() < 1e-9 && (top.y - 5.0).abs() < 1e-9,
            "0 deg is straight up"
        );
        assert!(
            (right.x - 15.0).abs() < 1e-9 && (right.y - 10.0).abs() < 1e-9,
            "90 deg is 3 o'clock"
        );
        assert!((bottom.y - 15.0).abs() < 1e-9, "180 deg is straight down");
    }

    #[test]
    fn arc_polyline_hits_both_ends_and_collapses_to_a_dot_at_zero_sweep() {
        let p = arc_polyline(10.0, 10.0, 5.0, 30.0, 90.0, 1.0, 0.1, 1.0, 0.0, 0.0);
        let first = &p.points[0];
        let last = p.points.last().unwrap();
        let want_first = ring_point(10.0, 10.0, 5.0, 30.0);
        let want_last = ring_point(10.0, 10.0, 5.0, 120.0);
        assert!((first.x - want_first.x).abs() < 1e-9 && (first.y - want_first.y).abs() < 1e-9);
        assert!((last.x - want_last.x).abs() < 1e-9 && (last.y - want_last.y).abs() < 1e-9);
        assert!(p.points.len() >= 23, "a 90 deg arc at ~4 deg steps");
        let dot = arc_polyline(10.0, 10.0, 5.0, 30.0, 0.0, 1.0, 0.1, 1.0, 0.0, 0.0);
        assert_eq!(dot.points.len(), 2);
        assert_eq!(dot.points[0], dot.points[1]);
        let circle = arc_polyline(10.0, 10.0, 5.0, 0.0, 360.0, 1.0, 0.1, 1.0, 0.0, 0.0);
        let (a, b) = (&circle.points[0], circle.points.last().unwrap());
        assert!(
            (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9,
            "a 360 sweep closes"
        );
    }

    #[test]
    fn standard_ease_is_a_monotone_curve_through_the_corners() {
        assert!(standard_ease(0.0).abs() < 1e-9);
        assert!((standard_ease(1.0) - 1.0).abs() < 1e-9);
        let mut prev = 0.0;
        for i in 1..=20 {
            let v = standard_ease(i as f64 / 20.0);
            assert!(v >= prev - 1e-9, "must not decrease");
            prev = v;
        }
        assert!(
            standard_ease(0.25) < 0.25 && standard_ease(0.75) > 0.75,
            "ease-in-out"
        );
        // Tailwind's ping curve is a pure ease-out: fast start, slow end.
        assert!(cubic_bezier(0.0, 0.0, 0.2, 1.0, 0.25) > 0.25);
    }
}

/// General indexed-key lookup (`prefix0`, `prefix1`, ...) -- the same
/// encoding `audio_band` uses for `audioBandN`, but unbounded: builds the
/// key with `format!`, so each call allocates one short `String`. That's
/// deliberate. `audio_band`'s fixed `match` exists to stay allocation-free
/// in a dense mode's per-dot hot path; a history buffer is read once per
/// bar (a few dozen to a few hundred times per frame), where that many
/// tiny allocations are far below anything measurable, and a 16-entry cap
/// would be a real limit. Used by `signal::modes::scroll` for `historyN`.
pub fn indexed_opt(o: &HashMap<String, f64>, prefix: &str, index: usize) -> Option<f64> {
    o.get(&format!("{prefix}{index}")).copied()
}

/// Mode-agnostic post-process: if `pointerStrength` is present and above
/// `0` in `opts`, pushes every dot, line endpoint and polyline vertex within
/// `pointerRadius` of (`pointerX`, `pointerY`) outward, with a squared
/// ease-out falloff; dots and lines also fade slightly as they scatter
/// (polylines only move -- one alpha per whole ring/trace can't fade
/// locally). Otherwise returns `frame` completely
/// unchanged. Not part of any mode's own geometry -- runs once, generically,
/// on whatever `OrbFrame` a mode already produced, which is why it works
/// identically across every state in every family without any mode file
/// needing to change: every mode's marks are already final, screen-space
/// `x`/`y` by the time this runs. (Until 2026-09-18 it moved dots only, so
/// it did nothing on dot-less `ring`/`signal`/`core` frames and detached
/// `web`'s edges from their dots.)
///
/// Threaded through the *existing* `frame_with_overrides` opts map, not a
/// new API -- see `docs/engine.md`'s API surface section on why that entry
/// point is "the correct one for any future tune-this-at-runtime use case."
/// No preset sets these keys, so plain `frame()` (and every existing golden
/// test, which only ever calls `frame()`) is completely unaffected; only a
/// caller that explicitly passes `pointerStrength` via
/// `frame_with_overrides` opts into this.
///
/// Deliberately only touches `x`/`y`, not `z` -- `finalize_frame`'s z-sort
/// (already applied by the mode function that produced `frame`) stays
/// valid, and a 2D screen-space push is what most real "particles scatter
/// away from the cursor" effects actually do, even over visually-3D fields.
pub fn apply_pointer(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = *opts.get("pointerStrength").unwrap_or(&0.0);
    let radius = *opts.get("pointerRadius").unwrap_or(&0.0);
    if strength <= 0.0 || radius <= 0.0 {
        return frame;
    }
    let px = *opts.get("pointerX").unwrap_or(&0.0);
    let py = *opts.get("pointerY").unwrap_or(&0.0);

    // The push is a function of position alone, so a vertex shared between
    // elements (a line endpoint on a dot, two polylines meeting) moves
    // identically everywhere and stays attached -- `apply_noise`'s rule.
    // Returns the pushed point and its falloff (0 outside the radius).
    let push = |x: f64, y: f64| -> (f64, f64, f64) {
        let dx = x - px;
        let dy = y - py;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist >= radius || dist < 1e-6 {
            return (x, y, 0.0);
        }
        let falloff = 1.0 - dist / radius;
        let p = strength * falloff * falloff;
        (x + (dx / dist) * p, y + (dy / dist) * p, falloff)
    };

    for d in frame.dots.iter_mut() {
        let (x, y, f) = push(d.x, d.y);
        if f > 0.0 {
            d.x = x;
            d.y = y;
            d.a *= 1.0 - 0.5 * f;
        }
    }
    // Moving line endpoints and polyline vertices is what makes the pointer
    // register on dot-less frames (every `ring` state, the `signal`
    // polyline styles, `core`'s shimmer) and keeps `web`'s edges on their
    // dots instead of detaching from them. Alpha: a `Line` is a short
    // segment, so dimming it by its most-affected endpoint stays local like
    // a dot's fade; a `Polyline` carries ONE alpha for a whole ring or
    // trace, and dimming it greyed out the entire ring far from the cursor
    // (seen on the contact sheet), so polylines only move.
    for l in frame.lines.iter_mut() {
        let (x1, y1, f1) = push(l.x1, l.y1);
        let (x2, y2, f2) = push(l.x2, l.y2);
        (l.x1, l.y1, l.x2, l.y2) = (x1, y1, x2, y2);
        l.a *= 1.0 - 0.5 * f1.max(f2);
    }
    for p in frame.polylines.iter_mut() {
        for pt in p.points.iter_mut() {
            let (x, y, _) = push(pt.x, pt.y);
            pt.x = x;
            pt.y = y;
        }
    }
    // Fills move like polylines (shape and gradient together).
    for f in frame.fills.iter_mut() {
        f.map_points(|x, y| {
            let (x, y, _) = push(x, y);
            (x, y)
        });
    }
    frame
}

/// Mode-agnostic post-process, same shape as `apply_pointer` above: if
/// `audioStrength` is present and non-zero in `opts`, breathes the whole
/// frame from its own centroid -- outward when it is positive, **inward
/// when it is negative** (the listening "inhale", 1.8) -- and boosts
/// radius/alpha by
/// `audioLevel` (`0..1`, meant to be a smoothed real-time volume reading
/// fed in by a caller's audio pipeline -- see the
/// voice-reactive-pipeline plan). A no-op otherwise, for the same reason
/// `apply_pointer` is: no preset sets `audioStrength`, so plain `frame()`
/// and every golden test are unaffected, and this works identically across
/// every state in every family without any mode file needing to change,
/// since it runs once on the already-finished `OrbFrame`.
///
/// `audioLevel` itself is intentionally NOT computed here -- FFT/RMS
/// extraction happens in the caller's own platform-native audio layer (Web
/// Audio `AnalyserNode`, iOS `AVAudioEngine`+vDSP, Android `AudioRecord`/
/// `Visualizer`), which cannot be shared across platforms the way this
/// scaling math can. This function is the one shared convergence point all
/// of them feed into, the same role `apply_pointer` plays for pointer input.
pub fn apply_audio_reactive(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = *opts.get("audioStrength").unwrap_or(&0.0);
    let level = opts
        .get("audioLevel")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength == 0.0 || level <= 0.0 {
        return frame;
    }
    // The sign is the *direction* (FX Spec 1.8, the voice-state language):
    // positive swells outward as the level rises -- "giving out", what a
    // speaking agent does -- and negative draws the shape inward as the
    // level rises, the "inhale" a listening agent needs (orb-ui's membrane
    // moves inward with input volume; our own review page showed that with
    // both states swelling outward, listening and speaking were
    // indistinguishable). Brightness follows the magnitude either way: an
    // inhale is a tightening, not a fade. No preset sets `audioStrength`,
    // and negatives were an early return before, so nothing existing moves.
    let gain = strength.abs();

    // Radial "breathing": push every point away from the frame's own
    // centroid (not a hardcoded size/2 -- works for any mode's geometry,
    // including 2D-flat ones like `orbs::modes::morph` or `signal::modes::bar`)
    // proportionally to its current distance from it, so the whole shape
    // visibly swells and settles with volume. A pure radius/alpha scale
    // makes dots fatter in place, which reads as static -- real perceived
    // motion needs a position change, not just a size change. `Line`
    // endpoints get the identical transform so line-based modes
    // (`orbs::modes::web`/`webflow`, `crystallize`'s edges) don't visually
    // detach from the dots they connect.
    // Dots-only centroid whenever there are dots (every `orbs` frame: its
    // output stays exactly what it always was); otherwise the centroid over
    // line endpoints and polyline vertices, so dot-less frames -- every
    // `ring` state, the `signal` polyline styles, `core`'s shimmer --
    // breathe too instead of returning untouched.
    let (cx, cy) = if frame.dots.is_empty() {
        match frame_centroid(&frame) {
            Some(c) => c,
            None => return frame,
        }
    } else {
        let (mut sx, mut sy) = (0.0, 0.0);
        for d in &frame.dots {
            sx += d.x;
            sy += d.y;
        }
        let n = frame.dots.len() as f64;
        (sx / n, sy / n)
    };

    // Deliberately gentle: the position swell (least sensitive term) carries
    // less weight than the radius/alpha "glimmer" -- a subtle brighten-and-
    // swell reads better than a large, jarring displacement. Callers still
    // control overall intensity via `audioStrength`; these fractions just
    // set the *balance* between the two cues, not the overall amount.
    let expand = 1.0 + level * strength * 0.4;
    for d in frame.dots.iter_mut() {
        d.x = cx + (d.x - cx) * expand;
        d.y = cy + (d.y - cy) * expand;
        d.r *= 1.0 + level * gain * 0.5;
        d.a = (d.a * (1.0 + level * gain * 0.6)).min(1.0);
    }
    for l in frame.lines.iter_mut() {
        l.x1 = cx + (l.x1 - cx) * expand;
        l.y1 = cy + (l.y1 - cy) * expand;
        l.x2 = cx + (l.x2 - cx) * expand;
        l.y2 = cy + (l.y2 - cy) * expand;
    }
    // Same treatment as `Line` endpoints, for the same reason: a stroked
    // path must not detach from dots it shares a frame with. Polylines also
    // get the dots' radius/alpha "glimmer" (stroke width for radius) -- on a
    // single-arc `ring` the position swell alone is a few percent and reads
    // as static. `orbs` emit no polylines, so their output is unchanged.
    for p in frame.polylines.iter_mut() {
        for pt in p.points.iter_mut() {
            pt.x = cx + (pt.x - cx) * expand;
            pt.y = cy + (pt.y - cy) * expand;
        }
        p.w *= 1.0 + level * gain * 0.5;
        p.a = (p.a * (1.0 + level * gain * 0.6)).min(1.0);
    }
    for f in frame.fills.iter_mut() {
        f.map_points(|x, y| (cx + (x - cx) * expand, cy + (y - cy) * expand));
        if let Some(g) = f.gradient.as_mut() {
            g.r *= expand;
        }
        f.a = (f.a * (1.0 + level * gain * 0.6)).min(1.0);
    }
    frame
}

/// Mode-agnostic post-process (FX Spec 1.8's `ink`): the whole visual's
/// opacity. `ink` (`0..1`, default `1`) multiplies the alpha of every
/// `Dot`, `Line`, `Polyline` and `Fill` -- glow halos included, since it
/// runs after `apply_glow`, so a faded visual fades as one thing rather
/// than leaving its halos behind.
///
/// It exists because "how present is this visual right now" is a state
/// cue, not a colour choice: a voice assistant's idle rests at ~0.8 and
/// comes to full ink when it starts listening (`spec/voice-state-profile.
/// draft.json`). Callers previously had to fake it outside the engine
/// (CSS opacity), which no native view or export could reproduce.
///
/// Deliberately *only* alpha: no ink/lightness shift like `apply_muted`'s
/// (that cue means something specific), and no geometry change, so it
/// composes with every mode and material. A no-op at `1` (and above), so
/// plain `frame()` and every golden case are unaffected.
pub fn apply_ink(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let ink = opts.get("ink").copied().unwrap_or(1.0).clamp(0.0, 1.0);
    if ink >= 1.0 {
        return frame;
    }
    for d in frame.dots.iter_mut() {
        d.a *= ink;
    }
    for l in frame.lines.iter_mut() {
        l.a *= ink;
    }
    for p in frame.polylines.iter_mut() {
        p.a *= ink;
    }
    for f in frame.fills.iter_mut() {
        f.a *= ink;
    }
    frame
}

/// Mode-agnostic post-process, same shape and place as `apply_pointer`/
/// `apply_audio_reactive`: the "mic muted" / "connection lost" cue every
/// real voice product needs (Amazon's Voice Interoperability design guide:
/// "It is very important for a product to convey a device's Microphone
/// On/Off state"; an Echo's light ring goes solid red when the mic is off;
/// Google Meet turns the mic icon red with a slash). Reads `muted`
/// (`0..1`, `1` = fully muted; `0|1` is fine, a continuous value just lets
/// a caller ease the transition), `mutedTint` (`0..1`, how strongly to
/// tint toward `mutedHue` instead of going grey -- `0` by default, since a
/// grey, dimmed strip is the neutral cue and red is a product choice) and
/// `mutedHue` (degrees, default `8`, Alexa's mic-off red-orange). A no-op
/// unless `muted > 0` -- no preset sets it, so plain `frame()` and every
/// golden test are unaffected.
///
/// What it does, to every `Dot`, `Line` and `Polyline` alike: dims (alpha
/// `x (1 - 0.55 m)`), lightens the ink (`white -> lerp(white, 0.62, m)`),
/// desaturates or re-tints (`saturation -> lerp(sat, 0.85 * tint, m)`, hue
/// snapped to `mutedHue` when tinting), and deflates slightly (radius and
/// stroke width `x (1 - 0.15 m)`). Geometry is otherwise untouched, which
/// is what keeps it mode-agnostic across every family.
///
/// **Deliberately not done here: freezing or slowing the animation.** The
/// engine is a pure function of `t`; scaling `t` inside would make the
/// pattern jump the moment `muted` toggles (at `t = 30s`, a 0.1x scale
/// snaps the frame to the `t = 3s` pose). Slowing the clock is the caller's
/// job, exactly like `t * speed` already is (see `docs/engine.md`'s
/// callout): advance your clock at ~10% while muted, and nothing jumps.
/// The Web Studio's `SignalStudio` is the reference implementation.
pub fn apply_muted(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let m = opts.get("muted").copied().unwrap_or(0.0).clamp(0.0, 1.0);
    if m <= 0.0 {
        return frame;
    }
    let tint = opts
        .get("mutedTint")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let hue = opts
        .get("mutedHue")
        .copied()
        .unwrap_or(8.0)
        .rem_euclid(360.0);

    let dim = 1.0 - 0.55 * m;
    let deflate = 1.0 - 0.15 * m;
    let ink = |white: f64| lerp(white, 0.62, m);
    let color = |sat: f64, h: f64| -> (f64, f64) {
        let target_sat = 0.85 * tint;
        let s = lerp(sat, target_sat, m);
        // Only re-aim the hue when there's a tint to aim at; a grey mute
        // shouldn't disturb a hue the renderer ignores anyway.
        let h = if tint > 0.0 { lerp_hue(h, hue, m) } else { h };
        (s, h)
    };

    for d in frame.dots.iter_mut() {
        d.a *= dim;
        d.r *= deflate;
        d.white = ink(d.white);
        let (s, h) = color(d.saturation, d.hue);
        d.saturation = s;
        d.hue = h;
    }
    for l in frame.lines.iter_mut() {
        l.a *= dim;
        l.w *= deflate;
        l.white = ink(l.white);
        let (s, h) = color(l.saturation, l.hue);
        l.saturation = s;
        l.hue = h;
    }
    for p in frame.polylines.iter_mut() {
        p.a *= dim;
        p.w *= deflate;
        p.white = ink(p.white);
        p.map_hue(color);
    }
    for f in frame.fills.iter_mut() {
        f.a *= dim;
        f.map_ink(|w, sat, hue| {
            let (s, h) = color(sat, hue);
            (ink(w), s, h)
        });
    }
    frame
}

/// Mode-agnostic post-process: the one-shot **interrupt / barge-in flash**
/// -- a brief acknowledgment for the moment a user starts talking while the
/// agent is mid-`speaking`. That moment is a real, named server event on
/// both major realtime voice APIs (fetched): OpenAI Realtime emits `input_audio_buffer.speech_started`
/// ("the server will automatically cancel any in-progress model response",
/// then `response.cancelled`); Gemini Live sets `serverContent.interrupted`
/// ("stop playing audio and clear queued playback"). Gemini exposes *only*
/// that single signal -- no VAD start/stop pair -- so the contract here is
/// "a moment fired", nothing more.
///
/// Reads `interruptAge` (seconds since that moment, on the caller's wall
/// clock -- an **age, not a timestamp**, because the engine's `t` is
/// pre-scaled by each state's `speed`, so a timestamp would force the
/// caller to know that scaling; an age is unit-safe and maps one-to-one
/// onto either provider's single event), `interruptDuration` (default
/// `0.45`s), `interruptStrength` (`0..1`, default `1`), and an optional
/// tint (`interruptTint` `0..1`, default `0`; `interruptHue`, default `45`,
/// warm amber). A no-op when the key is absent or the age has passed the
/// duration -- no preset sets it, so plain `frame()` and every golden test
/// are unaffected.
///
/// The flash, with `k = strength * (1 - age/duration)^2` (an ease-out):
/// contrast up (`white -> lerp(white, 0, 0.5k)`: darker ink on paper,
/// brighter once a dark theme mirrors it), alpha `x (1 + 0.6k)` (capped),
/// radius/stroke `x (1 + 0.35k)`, and a short outward kick from the
/// frame's own centroid (`x (1 + 0.12k)`). The centroid is taken over
/// dots, line endpoints *and* polyline vertices -- unlike
/// `apply_audio_reactive`, which only uses dots -- so a polyline-only
/// `signal` frame flashes too. Geometry aside, no mode knows this exists.
///
/// Runs after `apply_audio_reactive` and before `apply_muted`: a muted
/// frame still flashes briefly on a barge-in, but the mute's dimming has
/// the last word. **This is probably a `beacon` behavior in waiting** -- a
/// one-shot, attention-grabbing, decaying event is that family's exact
/// definition (the product plan); when `beacon` exists this should either move
/// there or be wrapped by it. Shipped here now because the event is real
/// today and `beacon` isn't.
pub fn apply_interrupt(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let age = match opts.get("interruptAge") {
        Some(a) => *a,
        None => return frame,
    };
    let duration = opts
        .get("interruptDuration")
        .copied()
        .unwrap_or(0.45)
        .max(0.01);
    let strength = opts
        .get("interruptStrength")
        .copied()
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    if !(0.0..duration).contains(&age) || strength <= 0.0 {
        return frame;
    }
    let k = strength * decay_envelope(age, duration, DECAY_QUAD);
    let tint = opts
        .get("interruptTint")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let hue = opts
        .get("interruptHue")
        .copied()
        .unwrap_or(45.0)
        .rem_euclid(360.0);

    let (cx, cy) = match frame_centroid(&frame) {
        Some(c) => c,
        None => return frame,
    };

    let kick = 1.0 + 0.12 * k;
    let swell = 1.0 + 0.35 * k;
    let brighten = 1.0 + 0.6 * k;
    let ink = |w: f64| lerp(w, 0.0, 0.5 * k);
    // Tint timeline: full `interruptHue` at the peak, through grey at the
    // halfway point, then the original color fades back in. Deliberately
    // NOT a hue lerp -- interpolating a strongly colored state (say
    // `spectrum`'s blue, 200) toward amber (45) passes through green
    // mid-flash, which reads as a rainbow, not a flash. A grey valley
    // keeps every intermediate frame neutral.
    let color = |sat: f64, h: f64| -> (f64, f64) {
        if tint <= 0.0 {
            (sat, h)
        } else if k >= 0.5 {
            (0.9 * tint * ((k - 0.5) / 0.5), hue)
        } else {
            (sat * (1.0 - k / 0.5), h)
        }
    };

    for d in frame.dots.iter_mut() {
        d.x = cx + (d.x - cx) * kick;
        d.y = cy + (d.y - cy) * kick;
        d.r *= swell;
        d.a = (d.a * brighten).min(1.0);
        d.white = ink(d.white);
        let (s, h) = color(d.saturation, d.hue);
        d.saturation = s;
        d.hue = h;
    }
    for l in frame.lines.iter_mut() {
        l.x1 = cx + (l.x1 - cx) * kick;
        l.y1 = cy + (l.y1 - cy) * kick;
        l.x2 = cx + (l.x2 - cx) * kick;
        l.y2 = cy + (l.y2 - cy) * kick;
        l.w *= swell;
        l.a = (l.a * brighten).min(1.0);
        l.white = ink(l.white);
        let (s, h) = color(l.saturation, l.hue);
        l.saturation = s;
        l.hue = h;
    }
    for p in frame.polylines.iter_mut() {
        for pt in p.points.iter_mut() {
            pt.x = cx + (pt.x - cx) * kick;
            pt.y = cy + (pt.y - cy) * kick;
        }
        p.w *= swell;
        p.a = (p.a * brighten).min(1.0);
        p.white = ink(p.white);
        p.map_hue(color);
    }
    for f in frame.fills.iter_mut() {
        f.map_points(|x, y| (cx + (x - cx) * kick, cy + (y - cy) * kick));
        f.a = (f.a * brighten).min(1.0);
        f.map_ink(|w, sat, hue| {
            let (s, h) = color(sat, hue);
            (ink(w), s, h)
        });
    }
    frame
}

/// Mode-agnostic post-process: a **periodic pulse** on any frame -- the
/// generic version of the breathing `beacon`'s `reconnecting` dot, `hush`
/// and `aurora` each do on their own. Reads `pulseStrength` (`0..1`;
/// absent or `0` = no-op, so no preset and no golden vector notices),
/// `pulsePeriod` (real seconds: `render` divides the preset speed out of
/// `t`, so a 2 s pulse is 2 s on every state; a spec's own `speed` still
/// scales it; default `2`), `pulseOpacity`
/// (alpha dip at the peak, default `0.5`), `pulseScale` (size swing at the
/// peak, default `0`) and `pulsePhase` (`0..1` offset, so several
/// instances don't pulse in lockstep).
///
/// Prior art, transcribed rather than invented: Tailwind's `animate-pulse`
/// (`50% { opacity: .5 }`, 2s, `cubic-bezier(0.4, 0, 0.6, 1)`) gives the
/// curve (`pulse_wave`) and the defaults; SF Symbols 6's split gives the
/// two knobs -- **Pulse** "conveys ongoing activity by changing its
/// opacity", **Breathe** "uses opacity and size". `pulseScale 0` is the
/// former, `pulseScale > 0` the latter: positions, radii and stroke widths
/// swell about the frame's centroid (dots, line endpoints *and* polyline
/// vertices, so `signal` frames breathe too), by `1 + pulseScale * k`.
///
/// Stateless: `k = strength * pulse_wave(fract(t / period + phase))` is a
/// pure function of `t`, so a scrubbed or replayed frame is identical.
pub fn apply_pulse(mut frame: OrbFrame, t: f64, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = opts
        .get("pulseStrength")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength <= 0.0 {
        return frame;
    }
    let period = opts.get("pulsePeriod").copied().unwrap_or(2.0).max(0.05);
    let depth = opts
        .get("pulseOpacity")
        .copied()
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let scale = opts
        .get("pulseScale")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let phase = opts.get("pulsePhase").copied().unwrap_or(0.0);
    let k = strength * pulse_wave((t / period + phase).rem_euclid(1.0));
    let center = match frame_centroid(&frame) {
        Some(c) => c,
        None => return frame,
    };
    let swell = 1.0 + scale * k;
    scale_frame(&mut frame, center, swell, swell, 1.0 - depth * k);
    frame
}

/// Mode-agnostic post-process: a **one-shot fade-out** of the whole frame
/// after an event -- a `notifying` badge that goes away, a `completing`
/// ring that fades once it's full, a "connected" cue that clears itself.
/// Reads `decayAge` (seconds since the event on the caller's wall clock,
/// the same contract and the same reason as `interruptAge`: the engine's
/// `t` is pre-scaled per state; absent = no-op, negative = the event
/// hasn't happened yet, frame untouched), `decayDuration` (default `0.6`),
/// `decayCurve` (`0` linear, `1` quadratic ease-out -- the default, and
/// `apply_interrupt`'s curve -- `2` exponential, Web Audio's
/// `setTargetAtTime` shape, `3` Material 3's emphasized-accelerate exit
/// curve; see `decay_envelope`) and `decayScale` (`0..1`, how far the frame
/// shrinks toward its centroid by the end; default `0` = a pure fade).
///
/// From `decayDuration` on, the frame is returned **empty** rather than as
/// alpha-0 geometry, so a renderer has nothing left to walk. Runs after
/// `apply_interrupt` (the event's envelope governs everything drawn,
/// flash included) and before `apply_muted`, which keeps the last word.
pub fn apply_decay(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let age = match opts.get("decayAge") {
        Some(a) => *a,
        None => return frame,
    };
    if age < 0.0 {
        return frame;
    }
    let duration = opts.get("decayDuration").copied().unwrap_or(0.6).max(0.01);
    let curve = opts
        .get("decayCurve")
        .copied()
        .unwrap_or(DECAY_QUAD as f64)
        .round()
        .max(0.0) as u32;
    let e = decay_envelope(age, duration, curve);
    if e <= 0.0 {
        return OrbFrame::default();
    }
    let shrink = opts
        .get("decayScale")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let s = lerp(1.0 - shrink, 1.0, e);
    let center = frame_centroid(&frame).unwrap_or((0.0, 0.0));
    scale_frame(&mut frame, center, s, s, e);
    frame
}

// --- Materials: glow / noise / gradient ---
//
// The parameter plan frames Glow,
// Noise and Gradient as *materials/behaviors* any object can wear, not as
// objects of their own -- and the four post-processes above are already
// exactly that mechanism: generic, opts-activated, run once on any
// family's finished frame, invisible to every mode and to the golden
// suite. The three below follow the same shape. See `docs/materials.md`.

/// Material: a **layered glow** -- a soft halo behind every dot, line and
/// polyline. Reads `glowStrength` (`0..1`; absent or `0` = no-op, so no
/// preset and no golden vector notices), `glowRadius` (the halo's outer
/// edge as a multiple of the dot's radius / the stroke's width, default
/// `2.5` -- Android's classic "glowing dot" recipe draws its gradient
/// circle at "nearly twice" the plain dot), `glowLayers` (default `4`,
/// `1..8`), and an optional re-tint (`glowTint` `0..1`, `glowHue` default
/// `200`; `0` = the halo keeps the source's own color).
///
/// Technique: there is no blur or gradient fill in this engine (and no
/// bloom pass on any renderer), so the glow is the CSS-world's layered
/// fake -- CSS-Tricks' neon text stacks eight `text-shadow`s of increasing
/// blur "so that they can be stacked over one another to add more depth",
/// Tobias Ahlin's layered `box-shadow`s double the blur per layer and drop
/// the alpha as layers are added -- transcribed to concentric translucent
/// copies. Layer `k` of `L` is scaled to `s_k = 1 + (glowRadius - 1)(k+1)/L`
/// and gets alpha `a * strength * G(u_k) / L`, where `G` is a Gaussian in
/// the extra radius with `sigma = (glowRadius - 1) * r / 2` -- the canvas
/// spec's own shadow model (`sigma = shadowBlur / 2`; Skia's
/// `ConvertRadiusToSigma` is `0.577 * radius + 0.5`, the same order). With
/// the default four layers the stack peaks at about `0.49 * strength * a`
/// next to the source, so a full-strength glow never out-inks the thing it
/// surrounds.
///
/// Draw order: the copies are spliced in **immediately before their own
/// element**, in the frame's existing (z-sorted) order -- a front dot's
/// halo therefore paints over a back dot, as a real halo would, instead of
/// every halo sitting under every dot. `z` is copied, `finalize_frame` is
/// not re-run. Copies whose alpha would fall under the renderer cull
/// threshold (`0.02`) are skipped, so a faint frame doesn't triple its
/// draw count for nothing. Runs after `apply_gradient` so a halo inherits
/// its source's final color, and before `apply_interrupt`/`apply_muted` so
/// those still have the last word over the halo too.
pub fn apply_glow(frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = opts
        .get("glowStrength")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength <= 0.0 {
        return frame;
    }
    // Real-blur mode (materials phase 1): one blurred halo per element
    // instead of stacked copies. `blurScale` 0 (low power) falls back to the
    // stacked copies, which every renderer can draw.
    let blur_scale = opts.get("blurScale").copied().unwrap_or(1.0).max(0.0);
    if opts.get("glowMode").copied().unwrap_or(0.0).round() == 1.0 && blur_scale > 0.0 {
        return glow_blurred(frame, opts, strength, blur_scale);
    }
    let color_mode = frame.color_mode;
    let fills = frame.fills.clone();
    let effects = frame.effects.clone();
    let radius = opts.get("glowRadius").copied().unwrap_or(2.5).max(1.05);
    let layers = opts
        .get("glowLayers")
        .copied()
        .unwrap_or(4.0)
        .round()
        .clamp(1.0, 8.0) as usize;
    let tint = opts.get("glowTint").copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let hue = opts
        .get("glowHue")
        .copied()
        .unwrap_or(200.0)
        .rem_euclid(360.0);
    const MIN_A: f64 = 0.02;

    // (scale, alpha factor) per layer, innermost first. The Gaussian is
    // evaluated in sigmas: the outermost layer sits two sigmas out.
    let profile: Vec<(f64, f64)> = (0..layers)
        .map(|k| {
            let s = 1.0 + (radius - 1.0) * (k as f64 + 1.0) / layers as f64;
            let u = (s - 1.0) / ((radius - 1.0) * 0.5);
            (s, strength * (-0.5 * u * u).exp() / layers as f64)
        })
        .collect();
    let color = |sat: f64, h: f64| -> (f64, f64) {
        if tint > 0.0 {
            (lerp(sat, 0.9 * tint, tint), hue)
        } else {
            (sat, h)
        }
    };

    let mut dots = Vec::with_capacity(frame.dots.len() * (layers + 1));
    for d in frame.dots {
        for &(s, af) in &profile {
            let a = d.a * af;
            if a < MIN_A {
                continue;
            }
            let (saturation, hue) = color(d.saturation, d.hue);
            dots.push(Dot {
                r: d.r * s,
                a,
                saturation,
                hue,
                ..d.clone()
            });
        }
        dots.push(d);
    }
    let mut lines = Vec::with_capacity(frame.lines.len() * (layers + 1));
    for l in frame.lines {
        for &(s, af) in &profile {
            let a = l.a * af;
            if a < MIN_A {
                continue;
            }
            let (saturation, hue) = color(l.saturation, l.hue);
            lines.push(Line {
                w: l.w * s,
                a,
                saturation,
                hue,
                ..l.clone()
            });
        }
        lines.push(l);
    }
    let mut polylines = Vec::with_capacity(frame.polylines.len() * (layers + 1));
    for p in frame.polylines {
        for &(s, af) in &profile {
            let a = p.a * af;
            if a < MIN_A {
                continue;
            }
            let (saturation, hue) = color(p.saturation, p.hue);
            polylines.push(Polyline {
                points: p.points.clone(),
                white: p.white,
                a,
                w: p.w * s,
                saturation,
                hue,
                hues: if tint > 0.0 {
                    Vec::new()
                } else {
                    p.hues.clone()
                },
            });
        }
        polylines.push(p);
    }
    OrbFrame {
        // Keep the frame's paint mode: glow runs after `apply_color`, and
        // this used to reset a `Fixed` frame to `Ink` (fixed + glow lost its
        // fixed colour on dark themes).
        color_mode,
        dots,
        lines,
        polylines,
        fills,
        effects,
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// `apply_glow`'s real-blur mode: per list, one halo copy of every element
/// -- enlarged to the middle of the stacked profile, alpha at its peak --
/// emitted as a single block *before* the originals, plus one `EffectRun`
/// telling the painter to blur that block (sigma from the list's median
/// size, so it scales with the object) and, with `glowBlend` 1, to add it.
/// Halos sit under every original; in stacked mode each element's copies
/// are interleaved with it instead. Fills are not glowed.
fn glow_blurred(
    mut frame: OrbFrame,
    opts: &HashMap<String, f64>,
    strength: f64,
    blur_scale: f64,
) -> OrbFrame {
    let radius = opts.get("glowRadius").copied().unwrap_or(2.5).max(1.05);
    let tint = opts.get("glowTint").copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let hue = opts
        .get("glowHue")
        .copied()
        .unwrap_or(200.0)
        .rem_euclid(360.0);
    let blend = if opts.get("glowBlend").copied().unwrap_or(0.0).round() == 1.0 {
        1
    } else {
        0
    };
    let grow = 1.0 + (radius - 1.0) * 0.5;
    let spread = (radius - 1.0) * 0.5 * blur_scale;
    let peak = (0.6 * strength).min(1.0);
    const MIN_A: f64 = 0.02;
    let color = |sat: f64, h: f64| -> (f64, f64) {
        if tint > 0.0 {
            (lerp(sat, 0.9 * tint, tint), hue)
        } else {
            (sat, h)
        }
    };

    let mut halos: Vec<Dot> = Vec::new();
    for d in &frame.dots {
        let a = d.a * peak;
        if a >= MIN_A {
            let (saturation, hue) = color(d.saturation, d.hue);
            halos.push(Dot {
                r: d.r * grow,
                a,
                saturation,
                hue,
                ..d.clone()
            });
        }
    }
    if !halos.is_empty() {
        let sigma = median(frame.dots.iter().map(|d| d.r).collect()) * spread;
        frame.effects.push(EffectRun {
            target: 0,
            start: 0,
            count: halos.len() as u32,
            blur: sigma,
            blend,
        });
        halos.append(&mut frame.dots);
        frame.dots = halos;
    }

    let mut halos: Vec<Line> = Vec::new();
    for l in &frame.lines {
        let a = l.a * peak;
        if a >= MIN_A {
            let (saturation, hue) = color(l.saturation, l.hue);
            halos.push(Line {
                w: l.w * grow,
                a,
                saturation,
                hue,
                ..l.clone()
            });
        }
    }
    if !halos.is_empty() {
        let sigma = median(frame.lines.iter().map(|l| l.w).collect()) * spread;
        frame.effects.push(EffectRun {
            target: 1,
            start: 0,
            count: halos.len() as u32,
            blur: sigma,
            blend,
        });
        halos.append(&mut frame.lines);
        frame.lines = halos;
    }

    let mut halos: Vec<Polyline> = Vec::new();
    for p in &frame.polylines {
        let a = p.a * peak;
        if a >= MIN_A {
            let (saturation, hue) = color(p.saturation, p.hue);
            halos.push(Polyline {
                w: p.w * grow,
                a,
                saturation,
                hue,
                points: p.points.clone(),
                white: p.white,
                // A tint replaces the hue, so the halo is one colour;
                // untinted, it follows the stroke's per-vertex sweep.
                hues: if tint > 0.0 {
                    Vec::new()
                } else {
                    p.hues.clone()
                },
            });
        }
    }
    if !halos.is_empty() {
        let sigma = median(frame.polylines.iter().map(|p| p.w).collect()) * spread;
        frame.effects.push(EffectRun {
            target: 2,
            start: 0,
            count: halos.len() as u32,
            blur: sigma,
            blend,
        });
        halos.append(&mut frame.polylines);
        frame.polylines = halos;
    }
    frame
}

/// The last post-process: `blurScale` (default 1; FX Spec low power sets 0)
/// scales every blur sigma, and 0 strips blur -- fills draw sharp, effect
/// runs with nothing left to do are dropped. Glow's blur mode already fell
/// back to stacked copies at 0, in `apply_glow`.
pub fn apply_blur_scale(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    let k = match opts.get("blurScale") {
        Some(k) if *k != 1.0 => k.max(0.0),
        _ => return frame,
    };
    for f in frame.fills.iter_mut() {
        f.blur *= k;
    }
    for e in frame.effects.iter_mut() {
        e.blur *= k;
    }
    frame.effects.retain(|e| e.blur > 0.0 || e.blend != 0);
    frame
}

/// Material: **organic positional jitter** from real gradient noise --
/// every dot, line endpoint and polyline vertex drifts by a smooth,
/// time-varying `perlin3` field, so a rigid lattice (a `globe`, a `bar`
/// row, an `arc`) breathes like something alive without any mode knowing.
/// Reads `noiseStrength` (`0..1`; absent or `0` = no-op), `noiseAmplitude` (the
/// maximum displacement as a fraction of `size`, default `0.04`),
/// `noiseScale` (lattice cells across the frame, default `2` -- lower is
/// one slow swell, higher is fine shimmer), `noiseSpeed` (default `0.4`
/// per real second: `render` divides the preset speed out of `t`) and `noiseSeed` (default
/// `0`, a different field per object when several share a screen).
///
/// The recipe is Nature of Code's noise walker: sample the field once per
/// axis from **different regions of noise space** (the book starts `x` at
/// `0` and `y` at `10,000` "so that x and y appear to act independently")
/// and advance slowly through the third dimension for time. Displacement
/// is computed from each point's *pre-jitter* position, so a dot and a
/// line endpoint or polyline vertex at the same coordinate move together
/// -- `web`/`crystallize` edges stay attached to their dots. Only `x`/`y`
/// change; `z`, radius and alpha are untouched (the frame's z-sort holds).
/// Takes `size` and `t` because it needs both, unlike the post-processes
/// above; `render()` has them.
pub fn apply_noise(
    mut frame: OrbFrame,
    size: f64,
    t: f64,
    opts: &HashMap<String, f64>,
) -> OrbFrame {
    let strength = opts
        .get("noiseStrength")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength <= 0.0 || size <= 0.0 {
        return frame;
    }
    let amp = opts.get("noiseAmplitude").copied().unwrap_or(0.04).max(0.0) * size * strength;
    let scale = opts.get("noiseScale").copied().unwrap_or(2.0).max(1e-6);
    let speed = opts.get("noiseSpeed").copied().unwrap_or(0.4);
    let seed = opts.get("noiseSeed").copied().unwrap_or(0.0);
    let f = scale / size;
    let z = t * speed + seed;
    let shift = |x: f64, y: f64| -> (f64, f64) {
        (
            amp * perlin3(x * f, y * f, z),
            amp * perlin3(x * f + 31.7, y * f + 17.3, z + 10000.0),
        )
    };

    for d in frame.dots.iter_mut() {
        let (dx, dy) = shift(d.x, d.y);
        d.x += dx;
        d.y += dy;
    }
    for l in frame.lines.iter_mut() {
        let (dx1, dy1) = shift(l.x1, l.y1);
        let (dx2, dy2) = shift(l.x2, l.y2);
        l.x1 += dx1;
        l.y1 += dy1;
        l.x2 += dx2;
        l.y2 += dy2;
    }
    for p in frame.polylines.iter_mut() {
        for pt in p.points.iter_mut() {
            let (dx, dy) = shift(pt.x, pt.y);
            pt.x += dx;
            pt.y += dy;
        }
    }
    for f in frame.fills.iter_mut() {
        f.map_points(|x, y| {
            let (dx, dy) = shift(x, y);
            (x + dx, y + dy)
        });
    }
    frame
}

/// The order-preserving lightness bias `apply_color` uses: `b >= 0` moves
/// `w` toward 1 by `b` of the remaining distance, `b < 0` scales it toward
/// 0 by `|b|`. Monotone in `w` for every `|b| < 1`, so a depth ramp keeps
/// its order and its relative spread -- a flat overwrite would turn a
/// shaded sphere into a disc. In `Ink` mode `w = 1` is "paper" in *both*
/// themes (the renderer mirrors ink and paper together), so `+` reads as
/// "toward the background" and `-` as "toward full ink" everywhere.
pub fn lightness_bias(w: f64, b: f64) -> f64 {
    let b = b.clamp(-1.0, 1.0);
    if b >= 0.0 {
        w + b * (1.0 - w)
    } else {
        w * (1.0 + b)
    }
}

/// Material: **one colour for the whole frame** -- the engine half of the
/// Studio's colour picker (2026-09-18, Color system). Mode-agnostic like
/// every post-process here: it recolours whatever a mode drew -- dots,
/// lines and polylines -- so a grey ported orb, a `ring`, a `connecting`
/// web (its edges included, now that `Line` has colour fields) all take a
/// brand colour without a mode file changing.
///
/// Keys:
/// - `colorMix` (`0..1`; absent or `0` = no recolour) -- how far each
///   element moves toward the target colour, CSS `color-mix()` style.
/// - `colorHue` (degrees, default `200`) -- achromatic elements snap to it
///   (their old hue was meaningless, same rule as `apply_gradient`);
///   coloured ones ease toward it along the *shorter* arc (`lerp_hue`,
///   CSS's default `shorter hue` interpolation).
/// - `colorSaturation` (`0..1`, default `0.8`) -- `lerp(sat, S, mix)`.
/// - `colorLightness` (`-1..1`, default `0`) -- a **bias**, never an
///   overwrite: `lightness_bias(white, b * mix)`, so each element's own
///   depth shading survives.
/// - `colorMode` (`0` = ink, `1` = fixed) -- sets `OrbFrame.color_mode`
///   whenever present, even without `colorMix`, so a natively coloured
///   state (`glowing`) can be pinned to one appearance in both themes.
///
/// Runs after `apply_noise` and before `apply_gradient`: colour is the
/// base tint, a gradient (more specific) wins where it's set, glow halos
/// inherit the result, and the interrupt flash / mute cue keep the last
/// word. Absent keys = the frame returned untouched (golden suites safe).
pub fn apply_color(mut frame: OrbFrame, opts: &HashMap<String, f64>) -> OrbFrame {
    if let Some(mode) = opts.get("colorMode") {
        frame.color_mode = if *mode >= 0.5 {
            ColorMode::Fixed
        } else {
            ColorMode::Ink
        };
    }
    let mix = opts.get("colorMix").copied().unwrap_or(0.0).clamp(0.0, 1.0);
    if mix <= 0.0 {
        return frame;
    }
    let hue = opts
        .get("colorHue")
        .copied()
        .unwrap_or(200.0)
        .rem_euclid(360.0);
    let sat = opts
        .get("colorSaturation")
        .copied()
        .unwrap_or(0.8)
        .clamp(0.0, 1.0);
    let bias = opts
        .get("colorLightness")
        .copied()
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0)
        * mix;
    let recolor = |white: f64, s: f64, h: f64| -> (f64, f64, f64) {
        let h2 = if s > 0.0 { lerp_hue(h, hue, mix) } else { hue };
        (lightness_bias(white, bias), lerp(s, sat, mix), h2)
    };
    for d in frame.dots.iter_mut() {
        (d.white, d.saturation, d.hue) = recolor(d.white, d.saturation, d.hue);
    }
    for l in frame.lines.iter_mut() {
        (l.white, l.saturation, l.hue) = recolor(l.white, l.saturation, l.hue);
    }
    for p in frame.polylines.iter_mut() {
        (p.white, p.saturation, p.hue) = recolor(p.white, p.saturation, p.hue);
    }
    for f in frame.fills.iter_mut() {
        f.map_ink(recolor);
    }
    frame
}

/// Material: a **linear color gradient across the frame** -- the "real
/// gradient" a dot/stroke renderer can actually have: not a fill, but a
/// per-element color ramp by position, so a grey `globe` becomes a
/// blue-to-magenta one and a `waveform`'s layers shade left to right.
/// Reads `gradientStrength` (`0..1`; absent or `0` = no-op), `gradientAngle`
/// (degrees, default `90`), `gradientHue` (start, default `200`),
/// `gradientHue2` (end, default `320`) and `gradientSaturation` (default
/// `0.8`).
///
/// Coordinates follow CSS Images Level 3's `linear-gradient()` exactly, so
/// a designer's mental model transfers: "0deg points upward, and positive
/// angles represent clockwise rotation, so 90deg point toward the right";
/// the gradient line passes through the center of the box and has length
/// `abs(W * sin(A)) + abs(H * cos(A))`, which puts `0` and `1` exactly on
/// the two corners the angle points away from and toward. The box is the
/// `size x size` frame, not the geometry's bounding box -- a bbox changes
/// every frame as dots move, which would make colors flicker. The hue ramp
/// is a plain `h1 + (h2 - h1) * u` with no wrap logic: `h2 = h1 + 360`
/// gives a full rainbow, `h2 < h1` runs the other way around the wheel.
///
/// Per element: `saturation -> lerp(sat, gradientSaturation, strength)`;
/// hue snaps to the ramp when the element was achromatic (its old hue was
/// meaningless) and eases toward it via `lerp_hue` otherwise. `white` is
/// kept -- it's the lightness in the renderers' HSL branch, so a mode's
/// depth shading survives. A `Polyline` has one color per path (its paint
/// contract), so it samples the ramp at its mean vertex; a `Line` samples it
/// at its midpoint (lines gained colour fields on 2026-09-18 -- before that
/// they stayed grey). Optional third stop: `gradientHue3` + `gradientMid`
/// (`0.05..0.95`, default `0.5`, the ramp position of stop 2); without
/// `gradientHue3` the ramp is the original two-stop one exactly. Runs
/// after `apply_color` (it's the more specific colour) and before
/// `apply_glow` so halos inherit the ramped color, and before
/// `apply_interrupt`/`apply_muted`.
pub fn apply_gradient(mut frame: OrbFrame, size: f64, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = opts
        .get("gradientStrength")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength <= 0.0 || size <= 0.0 {
        return frame;
    }
    let angle = opts
        .get("gradientAngle")
        .copied()
        .unwrap_or(90.0)
        .to_radians();
    let h1 = opts.get("gradientHue").copied().unwrap_or(200.0);
    let h2 = opts.get("gradientHue2").copied().unwrap_or(320.0);
    // Optional third stop (2026-09-18): absent = the original two-stop
    // ramp, bit for bit. Present: h1 -> h2 over [0, mid], h2 -> h3 over
    // [mid, 1], each segment the same no-wrap linear ramp as before.
    let h3 = opts.get("gradientHue3").copied();
    let mid = opts
        .get("gradientMid")
        .copied()
        .unwrap_or(0.5)
        .clamp(0.05, 0.95);
    let gsat = opts
        .get("gradientSaturation")
        .copied()
        .unwrap_or(0.8)
        .clamp(0.0, 1.0);

    // Screen space has y pointing down, so "0deg = up" is (0, -1).
    let (dx, dy) = (angle.sin(), -angle.cos());
    let len = (size * angle.sin()).abs() + (size * angle.cos()).abs();
    let c = size * 0.5;
    let u_at =
        |x: f64, y: f64| -> f64 { (0.5 + ((x - c) * dx + (y - c) * dy) / len).clamp(0.0, 1.0) };
    let color = |sat: f64, hue: f64, u: f64| -> (f64, f64) {
        let raw = match h3 {
            None => h1 + (h2 - h1) * u,
            Some(_) if u <= mid => h1 + (h2 - h1) * (u / mid),
            Some(h3) => h2 + (h3 - h2) * ((u - mid) / (1.0 - mid)),
        };
        let target = raw.rem_euclid(360.0);
        let s = lerp(sat, gsat, strength);
        let h = if sat > 0.0 {
            lerp_hue(hue, target, strength)
        } else {
            target
        };
        (s, h)
    };

    for d in frame.dots.iter_mut() {
        let (s, h) = color(d.saturation, d.hue, u_at(d.x, d.y));
        d.saturation = s;
        d.hue = h;
    }
    for p in frame.polylines.iter_mut() {
        if p.points.is_empty() {
            continue;
        }
        let n = p.points.len() as f64;
        let (mx, my) = p
            .points
            .iter()
            .fold((0.0, 0.0), |(ax, ay), q| (ax + q.x, ay + q.y));
        // Per vertex (Q1, 2026-09-19): each vertex samples the ramp at its
        // own position, easing from its own previous hue; `hue` stays the
        // vertex-mean sample as the single-hue fallback.
        let sat = p.saturation;
        let prev = p.hues.clone();
        let base = p.hue;
        p.set_hues(|i, q| color(sat, prev.get(i).copied().unwrap_or(base), u_at(q.x, q.y)).1);
        let (s, h) = color(sat, base, u_at(mx / n, my / n));
        p.saturation = s;
        p.hue = h;
    }
    // A fill samples the ramp at its vertex mean, for its ink and its stops.
    for f in frame.fills.iter_mut() {
        let Some((mx, my)) = f.centroid() else {
            continue;
        };
        let u = u_at(mx, my);
        f.map_ink(|w, sat, hue| {
            let (s, h) = color(sat, hue, u);
            (w, s, h)
        });
    }
    // A line has one colour, so it samples the ramp at its midpoint.
    for l in frame.lines.iter_mut() {
        let (s, h) = color(
            l.saturation,
            l.hue,
            u_at((l.x1 + l.x2) * 0.5, (l.y1 + l.y2) * 0.5),
        );
        l.saturation = s;
        l.hue = h;
    }
    frame
}

/// Material: **holographic-lite** (2026-09-19, materials phase 4) -- a
/// foil/iridescent hue sweep over each element's *existing* lightness, no
/// shader. Thin-film iridescence shifts colour with the view angle,
/// `thickness = min + range * (1 - cos(theta))` (Papadopoulos, "Implementing
/// a foil sticker effect"), and a phase walks the hue wheel (Quilez's cosine
/// palette); in this HSL ink model both reduce to a per-element **hue
/// offset**:
///
/// `hue = holoHue + holoSpan * (holoDepth * d + holoFacing * f) + 360 * holoSpeed * t`
///
/// - `d` (depth) = `(z + 1) / 2` clamped, a dot's `z` (orbs' sphere is
///   `-1..1`); lines, polylines and fills have no `z` and take `0.5`.
/// - `f` (facing, Fresnel-like `1 - cos(theta)`) = `1 - sqrt(1 - rho^2)`,
///   `rho` = distance from the *view point* over `0.75 * size`, clamped --
///   elements far from where the viewer looks shift most. The view point
///   circles the frame centre at radius `size / 4`, angle
///   `2 pi * holoSpeed * t` (a tilting card), starting to the right of
///   centre. Position: a dot's centre, a line's midpoint, a polyline's
///   vertex mean, a fill's centroid (for its ink and stops). Frame
///   geometry, not a bbox, for the same no-flicker reason as
///   `apply_gradient`.
///
/// Keys: `holoStrength` (`0..1`; absent or `0` = no-op), `holoHue`
/// (default `180`), `holoSpan` (`300`), `holoSaturation` (`0.7`),
/// `holoDepth` (`0.5`), `holoFacing` (`0.5`), `holoSpeed` (turns per
/// second of engine time, `0.05`). Per element, exactly `apply_gradient`'s
/// rule: `saturation -> lerp(sat, holoSaturation, strength)`, hue snaps
/// when the element was achromatic and eases via `lerp_hue` otherwise;
/// `white`, alpha, geometry and `color_mode` are untouched. Runs after
/// `apply_gradient` (it's the top tint) and before `apply_glow`, so halos
/// inherit it, and before `apply_interrupt`/`apply_muted`.
pub fn apply_holo(mut frame: OrbFrame, size: f64, t: f64, opts: &HashMap<String, f64>) -> OrbFrame {
    let strength = opts
        .get("holoStrength")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if strength <= 0.0 || size <= 0.0 {
        return frame;
    }
    let get = |k: &str, d: f64| opts.get(k).copied().unwrap_or(d);
    let base = get("holoHue", 180.0);
    let span = get("holoSpan", 300.0);
    let hsat = get("holoSaturation", 0.7).clamp(0.0, 1.0);
    let wd = get("holoDepth", 0.5).clamp(0.0, 1.0);
    let wf = get("holoFacing", 0.5).clamp(0.0, 1.0);
    let turns = get("holoSpeed", 0.05) * t;
    let drift = 360.0 * turns;
    let c = size * 0.5;
    // The view point circles the centre at half the frame's half-width --
    // a card tilting in the hand -- so a flat, centred shape (a ring, whose
    // every element is equidistant from the centre) still gets a sweep.
    let a = std::f64::consts::TAU * turns;
    let (vx, vy) = (c + 0.5 * c * a.cos(), c + 0.5 * c * a.sin());
    let color = |sat: f64, hue: f64, x: f64, y: f64, z: Option<f64>| -> (f64, f64) {
        let d = z.map_or(0.5, |z| ((z + 1.0) * 0.5).clamp(0.0, 1.0));
        let rho = (((x - vx).powi(2) + (y - vy).powi(2)).sqrt() / (1.5 * c)).min(1.0);
        let f = 1.0 - (1.0 - rho * rho).sqrt();
        let target = (base + span * (wd * d + wf * f) + drift).rem_euclid(360.0);
        let h = if sat > 0.0 {
            lerp_hue(hue, target, strength)
        } else {
            target
        };
        (lerp(sat, hsat, strength), h)
    };

    for d in frame.dots.iter_mut() {
        (d.saturation, d.hue) = color(d.saturation, d.hue, d.x, d.y, Some(d.z));
    }
    for l in frame.lines.iter_mut() {
        let (x, y) = ((l.x1 + l.x2) * 0.5, (l.y1 + l.y2) * 0.5);
        (l.saturation, l.hue) = color(l.saturation, l.hue, x, y, None);
    }
    for p in frame.polylines.iter_mut() {
        if p.points.is_empty() {
            continue;
        }
        let n = p.points.len() as f64;
        let (x, y) = p
            .points
            .iter()
            .fold((0.0, 0.0), |(ax, ay), q| (ax + q.x, ay + q.y));
        // Per vertex, as in `apply_gradient`: each vertex takes the hue at
        // its own position (a sweep along a ring), `hue` the vertex mean.
        let sat = p.saturation;
        let prev = p.hues.clone();
        let base = p.hue;
        p.set_hues(|i, q| color(sat, prev.get(i).copied().unwrap_or(base), q.x, q.y, None).1);
        (p.saturation, p.hue) = color(sat, base, x / n, y / n, None);
    }
    for f in frame.fills.iter_mut() {
        let Some((x, y)) = f.centroid() else {
            continue;
        };
        f.map_ink(|w, sat, hue| {
            let (s, h) = color(sat, hue, x, y, None);
            (w, s, h)
        });
    }
    frame
}

#[cfg(test)]
mod holo_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn dot(x: f64, y: f64, z: f64, sat: f64, hue: f64) -> Dot {
        Dot {
            x,
            y,
            z,
            r: 2.0,
            white: 0.3,
            a: 1.0,
            saturation: sat,
            hue,
        }
    }

    fn sample() -> OrbFrame {
        let mut f = OrbFrame {
            dots: vec![
                dot(50.0, 50.0, -1.0, 0.0, 0.0),
                dot(50.0, 50.0, 1.0, 0.0, 0.0),
                dot(0.0, 50.0, 0.0, 0.0, 0.0),
                dot(50.0, 50.0, 0.0, 0.8, 10.0),
            ],
            ..Default::default()
        };
        f.lines = vec![Line {
            x1: 40.0,
            y1: 50.0,
            x2: 60.0,
            y2: 50.0,
            white: 0.4,
            a: 1.0,
            w: 1.0,
            saturation: 0.0,
            hue: 0.0,
        }];
        f.polylines = vec![Polyline {
            points: vec![Point { x: 0.0, y: 50.0 }, Point { x: 10.0, y: 50.0 }],
            white: 0.5,
            a: 1.0,
            w: 1.0,
            saturation: 0.0,
            hue: 0.0,
            hues: Vec::new(),
        }];
        f.fills = vec![Fill {
            points: vec![
                Point { x: 40.0, y: 40.0 },
                Point { x: 60.0, y: 40.0 },
                Point { x: 60.0, y: 60.0 },
            ],
            white: 0.6,
            a: 1.0,
            gradient: Some(FillGradient {
                stops: vec![GradientStop {
                    offset: 0.0,
                    white: 0.7,
                    a: 1.0,
                    saturation: 0.0,
                    hue: 0.0,
                }],
                ..Default::default()
            }),
            ..Default::default()
        }];
        f
    }

    #[test]
    fn absent_or_zero_strength_is_a_no_op() {
        let f = sample();
        assert_eq!(apply_holo(f.clone(), 100.0, 3.0, &opts(&[])), f);
        assert_eq!(
            apply_holo(f.clone(), 100.0, 3.0, &opts(&[("holoStrength", 0.0)])),
            f
        );
    }

    #[test]
    fn hue_from_depth_facing_and_time_over_kept_lightness() {
        let o = opts(&[("holoStrength", 1.0), ("holoSpeed", 0.0)]);
        let src = sample();
        let out = apply_holo(src.clone(), 100.0, 0.0, &o);
        // Lightness, alpha and geometry untouched on every element kind.
        for (a, b) in src.dots.iter().zip(&out.dots) {
            assert_eq!(
                (a.x, a.y, a.z, a.r, a.white, a.a),
                (b.x, b.y, b.z, b.r, b.white, b.a)
            );
            assert!((b.saturation - 0.7).abs() < 1e-12);
        }
        assert_eq!(out.lines[0].white, 0.4);
        assert_eq!(out.polylines[0].white, 0.5);
        let g = out.fills[0].gradient.as_ref().unwrap();
        assert_eq!((out.fills[0].white, g.stops[0].white), (0.6, 0.7));
        // At t = 0 the view point is (75, 50): the centre is rho 1/3 from it.
        let fc = 1.0 - (1.0 - 1.0_f64 / 9.0).sqrt();
        let hue = |d: f64, f: f64| (180.0 + 300.0 * (0.5 * d + 0.5 * f)).rem_euclid(360.0);
        // Depth: centre dots, far (z -1) vs near (z 1): the span x depth weight apart.
        let (far, near) = (out.dots[0].hue, out.dots[1].hue);
        assert!((far - hue(0.0, fc)).abs() < 1e-9, "{far}");
        assert!((near - hue(1.0, fc)).abs() < 1e-9, "{near}");
        // Facing: a dot 75 away from the view point (rho 1, f 1) vs the centre one.
        assert!((out.dots[2].hue - hue(0.5, 1.0)).abs() < 1e-9);
        // Elements without z take neutral depth; a fill's stops follow its ink.
        assert!((out.lines[0].hue - hue(0.5, fc)).abs() < 1e-9);
        assert!((g.stops[0].hue - out.fills[0].hue).abs() < 1e-9);
        // The polyline sits far from the view point, so it is shifted past the line.
        assert!(out.polylines[0].hue != out.lines[0].hue);
        // The view point circles the centre: a quarter turn later (speed 0.25,
        // t 1) it sits below the centre, so the left dot's facing changes.
        let o_q = opts(&[
            ("holoStrength", 1.0),
            ("holoSpeed", 0.25),
            ("holoSpan", 0.0),
            ("holoDepth", 0.0),
        ]);
        let o_q2 = opts(&[
            ("holoStrength", 1.0),
            ("holoSpeed", 0.25),
            ("holoDepth", 0.0),
        ]);
        let q0 = apply_holo(src.clone(), 100.0, 1.0, &o_q);
        let q1 = apply_holo(src.clone(), 100.0, 1.0, &o_q2);
        assert!(
            (q0.dots[2].hue - (180.0 + 90.0)).abs() < 1e-9,
            "span 0 = drift only"
        );
        let rho = ((50.0_f64.powi(2) + 25.0_f64.powi(2)).sqrt() / 75.0).min(1.0);
        let fq = 1.0 - (1.0 - rho * rho).sqrt();
        assert!((q1.dots[2].hue - (270.0 + 300.0 * 0.5 * fq).rem_euclid(360.0)).abs() < 1e-9);
        // Time: speed 1 turn/s for 2 s = two whole turns -- the view point is
        // back where it started, so only the 720-degree drift (a no-op) remains.
        let o2 = opts(&[("holoStrength", 1.0), ("holoSpeed", 1.0)]);
        let later = apply_holo(src, 100.0, 2.0, &o2);
        for (a, b) in out.dots.iter().zip(&later.dots) {
            assert!((a.hue - b.hue).abs() < 1e-6, "{} {}", a.hue, b.hue);
        }
    }

    #[test]
    fn coloured_elements_ease_along_the_shorter_arc() {
        let src = sample();
        let o = opts(&[
            ("holoStrength", 0.5),
            ("holoSpeed", 0.0),
            ("holoHue", 0.0),
            ("holoDepth", 0.0),
            ("holoFacing", 0.0),
        ]);
        let out = apply_holo(src, 100.0, 0.0, &o);
        // Coloured hue 10 -> target 0 at half strength: 5, not 185.
        assert!((out.dots[3].hue - 5.0).abs() < 1e-9);
        assert!((out.dots[3].saturation - 0.75).abs() < 1e-12);
        // Achromatic snaps to the target at any strength.
        assert!(out.dots[0].hue.abs() < 1e-9);
        assert!((out.dots[0].saturation - 0.35).abs() < 1e-12);
    }
}

#[cfg(test)]
mod glow_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn sample() -> OrbFrame {
        let dots = vec![
            Dot {
                x: 10.0,
                y: 10.0,
                z: -0.5,
                r: 2.0,
                white: 0.2,
                a: 0.9,
                saturation: 0.0,
                hue: 0.0,
            },
            Dot {
                x: 40.0,
                y: 30.0,
                z: 0.5,
                r: 3.0,
                white: 0.1,
                a: 1.0,
                saturation: 0.7,
                hue: 120.0,
            },
        ];
        let lines = vec![Line {
            x1: 10.0,
            y1: 10.0,
            x2: 40.0,
            y2: 30.0,
            white: 0.3,
            a: 0.8,
            w: 1.0,
            saturation: 0.0,
            hue: 0.0,
        }];
        let polylines = vec![Polyline {
            points: vec![Point { x: 5.0, y: 50.0 }, Point { x: 60.0, y: 50.0 }],
            white: 0.15,
            a: 0.9,
            w: 2.0,
            saturation: 0.5,
            hue: 200.0,
            hues: Vec::new(),
        }];
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots,
            lines,
            polylines,
        }
    }

    #[test]
    fn absent_or_zero_strength_is_a_no_op() {
        let f = sample();
        assert_eq!(apply_glow(f.clone(), &opts(&[])), f);
        assert_eq!(apply_glow(f.clone(), &opts(&[("glowStrength", 0.0)])), f);
    }

    #[test]
    fn halos_precede_their_own_element_growing_outward_and_fading() {
        let f = sample();
        let g = apply_glow(f.clone(), &opts(&[("glowStrength", 1.0)]));
        assert_eq!(g.dots.len(), f.dots.len() * 5, "4 halos + the dot, per dot");
        for (i, src) in f.dots.iter().enumerate() {
            let group = &g.dots[i * 5..i * 5 + 5];
            assert_eq!(&group[4], src, "the source dot is drawn last in its group");
            for w in group[..4].windows(2) {
                assert!(w[1].r > w[0].r, "halos grow outward");
                assert!(w[1].a < w[0].a, "and fade");
            }
            assert!(group[0].r > src.r && group[3].r <= src.r * 2.5 + 1e-9);
            assert!(
                group[0].a < src.a * 0.5,
                "the innermost halo never out-inks its dot"
            );
            assert_eq!(group[0].z, src.z);
            assert_eq!(
                (group[0].saturation, group[0].hue),
                (src.saturation, src.hue),
                "keeps the source color"
            );
            let peak: f64 = group[..4].iter().map(|d| d.a).sum();
            assert!(
                peak < src.a * 0.55,
                "stacked halo alpha stays under ~0.5 of the source"
            );
        }
        assert_eq!(g.lines.len(), 5);
        assert!(g.lines[..4]
            .iter()
            .all(|l| l.w > f.lines[0].w && l.a < f.lines[0].a));
        assert_eq!(g.lines[4], f.lines[0]);
        assert_eq!(g.polylines.len(), 5);
        assert!(g.polylines[..4].iter().all(|p| p.w > f.polylines[0].w
            && p.a < f.polylines[0].a
            && p.points == f.polylines[0].points));
        assert_eq!(g.polylines[4], f.polylines[0]);
    }

    #[test]
    fn layers_radius_and_tint_are_honored_and_faint_halos_are_culled() {
        let f = sample();
        let g = apply_glow(
            f.clone(),
            &opts(&[
                ("glowStrength", 1.0),
                ("glowLayers", 2.0),
                ("glowRadius", 4.0),
                ("glowTint", 1.0),
                ("glowHue", 30.0),
            ]),
        );
        assert_eq!(g.dots.len(), f.dots.len() * 3);
        assert!(
            (g.dots[1].r - 2.0 * 4.0).abs() < 1e-9,
            "outermost halo reaches glowRadius x r"
        );
        assert!((g.dots[0].hue - 30.0).abs() < 1e-9 && g.dots[0].saturation > 0.8);
        // A nearly invisible dot doesn't get four invisible halos.
        let faint = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![Dot {
                a: 0.05,
                r: 1.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let g = apply_glow(faint, &opts(&[("glowStrength", 1.0)]));
        assert_eq!(g.dots.len(), 1);
    }
}

#[cfg(test)]
mod noise_material_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn sample() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: (0..20)
                .map(|i| Dot {
                    x: 4.0 + i as f64 * 2.8,
                    y: 8.0 + (i % 5) as f64 * 9.0,
                    z: 0.1 * i as f64,
                    r: 1.5,
                    white: 0.2,
                    a: 0.8,
                    ..Default::default()
                })
                .collect(),
            lines: vec![Line {
                x1: 4.0,
                y1: 8.0,
                x2: 60.0,
                y2: 44.0,
                white: 0.3,
                a: 0.5,
                w: 1.0,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![Polyline {
                points: vec![Point { x: 4.0, y: 8.0 }, Point { x: 30.0, y: 20.0 }],
                a: 0.9,
                w: 2.0,
                ..Default::default()
            }],
        }
    }

    #[test]
    fn absent_or_zero_strength_is_a_no_op() {
        let f = sample();
        assert_eq!(apply_noise(f.clone(), 64.0, 1.0, &opts(&[])), f);
        assert_eq!(
            apply_noise(f.clone(), 64.0, 1.0, &opts(&[("noiseStrength", 0.0)])),
            f
        );
    }

    #[test]
    fn displacement_is_bounded_deterministic_and_time_varying() {
        let f = sample();
        let o = opts(&[("noiseStrength", 1.0), ("noiseAmplitude", 0.05)]);
        let a = apply_noise(f.clone(), 64.0, 1.0, &o);
        let b = apply_noise(f.clone(), 64.0, 1.0, &o);
        let c = apply_noise(f.clone(), 64.0, 2.5, &o);
        assert_eq!(a, b, "pure function of t");
        assert_ne!(a, c, "the field drifts with t");
        let mut moved = 0;
        for (src, dst) in f.dots.iter().zip(&a.dots) {
            let dist = ((dst.x - src.x).powi(2) + (dst.y - src.y).powi(2)).sqrt();
            assert!(
                dist <= 0.05 * 64.0 * 2f64.sqrt() + 1e-9,
                "within amp per axis"
            );
            if dist > 1e-6 {
                moved += 1;
            }
            assert_eq!(
                (dst.z, dst.r, dst.a, dst.white),
                (src.z, src.r, src.a, src.white)
            );
        }
        assert!(moved > 10, "most dots actually move");
        // Half strength halves the displacement of every point.
        let half = apply_noise(
            f.clone(),
            64.0,
            1.0,
            &opts(&[("noiseStrength", 0.5), ("noiseAmplitude", 0.05)]),
        );
        for ((src, full), h) in f.dots.iter().zip(&a.dots).zip(&half.dots) {
            assert!(((h.x - src.x) - 0.5 * (full.x - src.x)).abs() < 1e-9);
        }
    }

    #[test]
    fn shared_vertices_stay_attached() {
        let f = sample();
        let a = apply_noise(f, 64.0, 0.7, &opts(&[("noiseStrength", 1.0)]));
        // dot 0, line start and polyline start all sat at (4, 8).
        let d = &a.dots[0];
        assert!((d.x - a.lines[0].x1).abs() < 1e-9 && (d.y - a.lines[0].y1).abs() < 1e-9);
        assert!(
            (d.x - a.polylines[0].points[0].x).abs() < 1e-9
                && (d.y - a.polylines[0].points[0].y).abs() < 1e-9
        );
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn dot(x: f64, y: f64) -> Dot {
        Dot {
            x,
            y,
            r: 1.0,
            white: 0.2,
            a: 1.0,
            ..Default::default()
        }
    }

    fn frame(dots: Vec<Dot>) -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots,
            ..Default::default()
        }
    }

    #[test]
    fn absent_or_zero_strength_is_a_no_op() {
        let f = frame(vec![dot(0.0, 32.0), dot(64.0, 32.0)]);
        assert_eq!(apply_gradient(f.clone(), 64.0, &opts(&[])), f);
        assert_eq!(
            apply_gradient(f.clone(), 64.0, &opts(&[("gradientStrength", 0.0)])),
            f
        );
    }

    #[test]
    fn ninety_degrees_runs_left_to_right_and_zero_runs_bottom_to_top() {
        let o = opts(&[
            ("gradientStrength", 1.0),
            ("gradientHue", 100.0),
            ("gradientHue2", 200.0),
        ]);
        let g = apply_gradient(
            frame(vec![dot(0.0, 32.0), dot(32.0, 32.0), dot(64.0, 32.0)]),
            64.0,
            &o,
        );
        let hues: Vec<f64> = g.dots.iter().map(|d| d.hue).collect();
        assert!(
            (hues[0] - 100.0).abs() < 1e-9
                && (hues[1] - 150.0).abs() < 1e-9
                && (hues[2] - 200.0).abs() < 1e-9
        );
        assert!(g
            .dots
            .iter()
            .all(|d| (d.saturation - 0.8).abs() < 1e-9 && (d.white - 0.2).abs() < 1e-9));

        let mut up = o.clone();
        up.insert("gradientAngle".into(), 0.0);
        let g = apply_gradient(frame(vec![dot(32.0, 64.0), dot(32.0, 0.0)]), 64.0, &up);
        assert!(
            (g.dots[0].hue - 100.0).abs() < 1e-9,
            "0deg starts at the bottom"
        );
        assert!((g.dots[1].hue - 200.0).abs() < 1e-9, "and ends at the top");
        // 45deg: the bottom-left and top-right corners are the exact ends.
        up.insert("gradientAngle".into(), 45.0);
        let g = apply_gradient(
            frame(vec![dot(0.0, 64.0), dot(64.0, 0.0), dot(0.0, 0.0)]),
            64.0,
            &up,
        );
        assert!((g.dots[0].hue - 100.0).abs() < 1e-9 && (g.dots[1].hue - 200.0).abs() < 1e-9);
        assert!(
            (g.dots[2].hue - 150.0).abs() < 1e-9,
            "the other corners sit at the midpoint"
        );
    }

    #[test]
    fn partial_strength_blends_and_colored_sources_ease_their_hue() {
        let o = opts(&[
            ("gradientStrength", 0.5),
            ("gradientHue", 100.0),
            ("gradientHue2", 100.0),
        ]);
        let mut colored = dot(32.0, 32.0);
        colored.saturation = 0.4;
        colored.hue = 60.0;
        let g = apply_gradient(frame(vec![dot(32.0, 32.0), colored]), 64.0, &o);
        assert!(
            (g.dots[0].hue - 100.0).abs() < 1e-9,
            "achromatic source snaps to the ramp"
        );
        assert!(
            (g.dots[0].saturation - 0.4).abs() < 1e-9,
            "half of the 0.8 default"
        );
        assert!(
            (g.dots[1].hue - 80.0).abs() < 1e-9,
            "colored source eases halfway"
        );
        assert!((g.dots[1].saturation - 0.6).abs() < 1e-9);
    }

    #[test]
    fn polylines_sample_their_mean_vertex_and_lines_their_midpoint() {
        let o = opts(&[
            ("gradientStrength", 1.0),
            ("gradientHue", 0.0),
            ("gradientHue2", 100.0),
        ]);
        let f = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![],
            lines: vec![Line {
                x1: 0.0,
                y1: 0.0,
                x2: 64.0,
                y2: 64.0,
                white: 0.3,
                a: 1.0,
                w: 1.0,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![Polyline {
                points: vec![Point { x: 0.0, y: 10.0 }, Point { x: 32.0, y: 10.0 }],
                a: 1.0,
                w: 1.0,
                ..Default::default()
            }],
        };
        let g = apply_gradient(f.clone(), 64.0, &o);
        // Lines have had colour fields since 2026-09-18 (they used to stay
        // grey): midpoint (32, 32) -> u = 0.5 -> hue 50; lightness kept.
        assert!((g.lines[0].hue - 50.0).abs() < 1e-9);
        assert!((g.lines[0].saturation - 0.8).abs() < 1e-9);
        assert_eq!(g.lines[0].white, f.lines[0].white);
        assert!(
            (g.polylines[0].hue - 25.0).abs() < 1e-9,
            "mean x = 16 of 64 -> u = 0.25"
        );
        assert!((g.polylines[0].saturation - 0.8).abs() < 1e-9);
    }
}

#[cfg(test)]
mod pulse_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn dots_and_a_polyline() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 0.0,
                    y: 0.0,
                    r: 2.0,
                    a: 0.8,
                    ..Default::default()
                },
                Dot {
                    x: 20.0,
                    y: 0.0,
                    r: 2.0,
                    a: 0.8,
                    ..Default::default()
                },
            ],
            lines: vec![],
            polylines: vec![Polyline {
                points: vec![Point { x: 0.0, y: 10.0 }, Point { x: 20.0, y: 10.0 }],
                a: 0.8,
                w: 1.0,
                ..Default::default()
            }],
        }
    }

    #[test]
    fn pulse_wave_peaks_mid_period_and_is_symmetric() {
        assert!(pulse_wave(0.0).abs() < 1e-9);
        assert!(pulse_wave(1.0).abs() < 1e-9);
        assert!((pulse_wave(0.5) - 1.0).abs() < 1e-9);
        let mut prev = 0.0;
        for i in 1..=50 {
            let u = i as f64 / 100.0;
            let v = pulse_wave(u);
            assert!(v >= prev, "rises over the first half");
            assert!((v - pulse_wave(1.0 - u)).abs() < 1e-6, "symmetric");
            prev = v;
        }
    }

    #[test]
    fn no_op_without_the_key_or_at_zero_strength() {
        let frame = dots_and_a_polyline();
        assert_eq!(apply_pulse(frame.clone(), 0.5, &HashMap::new()), frame);
        assert_eq!(
            apply_pulse(frame.clone(), 0.5, &opts(&[("pulseStrength", 0.0)])),
            frame
        );
    }

    #[test]
    fn opacity_dips_mid_period_and_repeats_every_period() {
        let frame = dots_and_a_polyline();
        let o = opts(&[("pulseStrength", 1.0), ("pulsePeriod", 2.0)]);
        let start = apply_pulse(frame.clone(), 0.0, &o);
        let mid = apply_pulse(frame.clone(), 1.0, &o);
        let later = apply_pulse(frame.clone(), 3.0, &o);
        assert_eq!(start, frame, "no dip at the start of a period");
        assert!((mid.dots[0].a - 0.4).abs() < 1e-9, "Tailwind's 50%");
        assert!((mid.polylines[0].a - 0.4).abs() < 1e-9);
        assert_eq!(mid, later, "same pose one period later");
    }

    #[test]
    fn pulse_alone_never_moves_geometry_and_breathe_swells_about_the_centroid() {
        let frame = dots_and_a_polyline();
        let pulse = apply_pulse(frame.clone(), 1.0, &opts(&[("pulseStrength", 1.0)]));
        for (a, b) in pulse.dots.iter().zip(&frame.dots) {
            assert_eq!((a.x, a.y, a.r), (b.x, b.y, b.r));
        }
        let breathe = apply_pulse(
            frame.clone(),
            1.0,
            &opts(&[("pulseStrength", 1.0), ("pulseScale", 0.1)]),
        );
        let (cx, cy) = frame_centroid(&frame).unwrap();
        assert_eq!(frame_centroid(&breathe).map(|c| c.0), Some(cx));
        let dist = |x: f64, y: f64| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        let before = dist(frame.dots[0].x, frame.dots[0].y);
        let after = dist(breathe.dots[0].x, breathe.dots[0].y);
        assert!((after / before - 1.1).abs() < 1e-9);
        assert!((breathe.dots[0].r - 2.2).abs() < 1e-9);
        assert!((breathe.polylines[0].w - 1.1).abs() < 1e-9);
    }

    #[test]
    fn phase_offsets_the_cycle() {
        let frame = dots_and_a_polyline();
        let a = apply_pulse(
            frame.clone(),
            1.0,
            &opts(&[("pulseStrength", 1.0), ("pulsePhase", 0.0)]),
        );
        let b = apply_pulse(
            frame.clone(),
            0.0,
            &opts(&[("pulseStrength", 1.0), ("pulsePhase", 0.5)]),
        );
        assert_eq!(a, b);
    }
}

#[cfg(test)]
mod decay_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn frame() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 0.0,
                    y: 0.0,
                    r: 2.0,
                    a: 1.0,
                    ..Default::default()
                },
                Dot {
                    x: 20.0,
                    y: 0.0,
                    r: 2.0,
                    a: 1.0,
                    ..Default::default()
                },
            ],
            lines: vec![Line {
                x1: 0.0,
                y1: 0.0,
                x2: 20.0,
                y2: 0.0,
                a: 1.0,
                w: 1.0,
                white: 0.0,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![],
        }
    }

    #[test]
    fn every_curve_runs_from_one_to_zero_and_never_rises() {
        for curve in [DECAY_LINEAR, DECAY_QUAD, DECAY_EXP, DECAY_EMPHASIZED] {
            assert_eq!(decay_envelope(-0.1, 1.0, curve), 1.0);
            assert!((decay_envelope(0.0, 1.0, curve) - 1.0).abs() < 1e-9);
            assert_eq!(decay_envelope(1.0, 1.0, curve), 0.0);
            assert_eq!(decay_envelope(5.0, 1.0, curve), 0.0);
            let mut prev = 1.0;
            for i in 0..100 {
                let v = decay_envelope(i as f64 / 100.0, 1.0, curve);
                assert!(v <= prev + 1e-9, "curve {curve} rose at {i}");
                assert!((0.0..=1.0).contains(&v));
                prev = v;
            }
        }
        // The quad curve is exactly the one `apply_interrupt` always used.
        assert_eq!(
            decay_envelope(0.3, 0.45, DECAY_QUAD),
            (1.0 - 0.3 / 0.45f64).powi(2)
        );
        // Emphasized-accelerate holds longer than linear early on.
        assert!(
            decay_envelope(0.3, 1.0, DECAY_EMPHASIZED) > decay_envelope(0.3, 1.0, DECAY_LINEAR)
        );
        // Exponential drops faster than linear early on.
        assert!(decay_envelope(0.3, 1.0, DECAY_EXP) < decay_envelope(0.3, 1.0, DECAY_LINEAR));
    }

    #[test]
    fn no_op_without_the_key_or_before_the_event() {
        let f = frame();
        assert_eq!(apply_decay(f.clone(), &HashMap::new()), f);
        assert_eq!(apply_decay(f.clone(), &opts(&[("decayAge", -0.2)])), f);
        assert_eq!(apply_decay(f.clone(), &opts(&[("decayAge", 0.0)])), f);
    }

    #[test]
    fn alpha_falls_monotonically_and_the_frame_is_empty_after_the_duration() {
        let f = frame();
        let mut prev = 1.0;
        for i in 1..12 {
            let age = i as f64 * 0.05;
            let out = apply_decay(f.clone(), &opts(&[("decayAge", age)]));
            assert!(out.dots[0].a < prev, "fading at {age}");
            assert_eq!(out.dots[0].a, out.lines[0].a);
            assert_eq!(out.dots[0].x, 0.0, "a pure fade doesn't move anything");
            prev = out.dots[0].a;
        }
        let gone = apply_decay(f.clone(), &opts(&[("decayAge", 0.6)]));
        assert_eq!(gone, OrbFrame::default());
    }

    #[test]
    fn decay_scale_shrinks_toward_the_centroid() {
        let f = frame();
        let out = apply_decay(
            f.clone(),
            &opts(&[
                ("decayAge", 0.5),
                ("decayDuration", 1.0),
                ("decayCurve", DECAY_LINEAR as f64),
                ("decayScale", 0.4),
            ]),
        );
        // Linear at the halfway point: e = 0.5, scale = lerp(0.6, 1, 0.5) = 0.8.
        let (cx, _) = frame_centroid(&f).unwrap();
        assert!((out.dots[0].x - (cx + (0.0 - cx) * 0.8)).abs() < 1e-9);
        assert!((out.dots[0].r - 1.6).abs() < 1e-9);
        assert!((out.lines[0].w - 0.8).abs() < 1e-9);
        assert!((out.dots[0].a - 0.5).abs() < 1e-9);
    }
}

#[cfg(test)]
mod interrupt_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn two_dots_and_a_polyline() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 0.0,
                    y: 0.0,
                    r: 2.0,
                    white: 0.4,
                    a: 0.5,
                    ..Default::default()
                },
                Dot {
                    x: 20.0,
                    y: 0.0,
                    r: 2.0,
                    white: 0.4,
                    a: 0.5,
                    ..Default::default()
                },
            ],
            lines: vec![],
            polylines: vec![Polyline {
                points: vec![Point { x: 0.0, y: 10.0 }, Point { x: 20.0, y: 10.0 }],
                white: 0.4,
                a: 0.5,
                w: 1.0,
                ..Default::default()
            }],
        }
    }

    #[test]
    fn no_op_without_the_key_or_once_the_flash_has_passed() {
        let frame = two_dots_and_a_polyline();
        assert_eq!(apply_interrupt(frame.clone(), &HashMap::new()), frame);
        assert_eq!(
            apply_interrupt(frame.clone(), &opts(&[("interruptAge", 0.45)])),
            frame
        );
        assert_eq!(
            apply_interrupt(frame.clone(), &opts(&[("interruptAge", 3.0)])),
            frame
        );
        assert_eq!(
            apply_interrupt(frame.clone(), &opts(&[("interruptAge", -0.1)])),
            frame
        );
    }

    #[test]
    fn peaks_at_age_zero_and_decays_monotonically() {
        let before = two_dots_and_a_polyline();
        let at = |age: f64| apply_interrupt(before.clone(), &opts(&[("interruptAge", age)]));
        let (f0, f1, f2) = (at(0.0), at(0.15), at(0.3));
        assert!(
            f0.dots[0].a > f1.dots[0].a
                && f1.dots[0].a > f2.dots[0].a
                && f2.dots[0].a > before.dots[0].a
        );
        assert!(
            f0.dots[0].r > f1.dots[0].r
                && f1.dots[0].r > f2.dots[0].r
                && f2.dots[0].r > before.dots[0].r
        );
        assert!(
            f0.dots[0].white < f1.dots[0].white && f1.dots[0].white < before.dots[0].white,
            "ink darkens (contrast up)"
        );
        assert!(f0.polylines[0].w > before.polylines[0].w);
    }

    #[test]
    fn kicks_everything_outward_from_the_shared_centroid() {
        let before = two_dots_and_a_polyline(); // centroid: (10, 5)
        let out = apply_interrupt(before.clone(), &opts(&[("interruptAge", 0.0)]));
        assert!(
            out.dots[0].x < 0.0 && out.dots[1].x > 20.0,
            "dots move apart"
        );
        assert!(out.polylines[0].points[0].x < 0.0 && out.polylines[0].points[1].x > 20.0);
        assert!(
            out.polylines[0].points[0].y > 10.0,
            "polyline vertices move away from the centroid's y too"
        );
        // centroid itself stays put: the two dots stay symmetric about x = 10
        assert!((out.dots[0].x + out.dots[1].x - 20.0).abs() < 1e-9);
    }

    #[test]
    fn works_on_a_polyline_only_frame() {
        let mut frame = two_dots_and_a_polyline();
        frame.dots.clear();
        let out = apply_interrupt(frame.clone(), &opts(&[("interruptAge", 0.0)]));
        assert_ne!(
            out, frame,
            "a signal-style frame with no dots must still flash"
        );
        assert!(out.polylines[0].a > frame.polylines[0].a);
    }

    #[test]
    fn tint_and_strength_are_honored() {
        let before = two_dots_and_a_polyline();
        let tinted = apply_interrupt(
            before.clone(),
            &opts(&[
                ("interruptAge", 0.0),
                ("interruptTint", 1.0),
                ("interruptHue", 45.0),
            ]),
        );
        assert!(
            (tinted.dots[0].saturation - 0.9).abs() < 1e-9
                && (tinted.dots[0].hue - 45.0).abs() < 1e-9
        );
        let weak = apply_interrupt(
            before.clone(),
            &opts(&[("interruptAge", 0.0), ("interruptStrength", 0.5)]),
        );
        let full = apply_interrupt(before.clone(), &opts(&[("interruptAge", 0.0)]));
        assert!(before.dots[0].a < weak.dots[0].a && weak.dots[0].a < full.dots[0].a);
    }
}

#[cfg(test)]
mod muted_tests {
    use super::*;

    fn sample_frame() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![Dot {
                x: 5.0,
                y: 5.0,
                r: 2.0,
                white: 0.1,
                a: 0.8,
                saturation: 0.6,
                hue: 200.0,
                ..Default::default()
            }],
            lines: vec![Line {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 0.0,
                white: 0.2,
                a: 0.9,
                w: 1.5,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![Polyline {
                points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 3.0, y: 4.0 }],
                white: 0.15,
                a: 0.95,
                w: 2.0,
                saturation: 0.6,
                hue: 200.0,
                hues: Vec::new(),
            }],
        }
    }

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn no_op_when_absent_or_zero() {
        let frame = sample_frame();
        assert_eq!(apply_muted(frame.clone(), &HashMap::new()), frame);
        assert_eq!(apply_muted(frame.clone(), &opts(&[("muted", 0.0)])), frame);
    }

    #[test]
    fn fully_muted_dims_lightens_desaturates_and_deflates_every_primitive() {
        let before = sample_frame();
        let out = apply_muted(before.clone(), &opts(&[("muted", 1.0)]));
        let (d0, d1) = (&before.dots[0], &out.dots[0]);
        assert!(d1.a < d0.a && d1.r < d0.r && d1.white > d0.white);
        assert!(
            (d1.saturation).abs() < 1e-9,
            "grey mute removes color entirely"
        );
        assert!((d1.x, d1.y) == (d0.x, d0.y), "geometry stays put");
        let (l0, l1) = (&before.lines[0], &out.lines[0]);
        assert!(l1.a < l0.a && l1.w < l0.w && l1.white > l0.white);
        let (p0, p1) = (&before.polylines[0], &out.polylines[0]);
        assert!(p1.a < p0.a && p1.w < p0.w && p1.white > p0.white);
        assert!((p1.saturation).abs() < 1e-9);
        assert_eq!(p1.points, p0.points);
    }

    #[test]
    fn tint_re_aims_hue_and_keeps_some_saturation() {
        let out = apply_muted(
            sample_frame(),
            &opts(&[("muted", 1.0), ("mutedTint", 0.8), ("mutedHue", 8.0)]),
        );
        assert!((out.dots[0].hue - 8.0).abs() < 1e-9);
        assert!(
            (out.dots[0].saturation - 0.68).abs() < 1e-9,
            "0.85 * tint at full mute"
        );
        assert!((out.polylines[0].hue - 8.0).abs() < 1e-9);
    }

    #[test]
    fn half_muted_lands_between_the_two_extremes() {
        let before = sample_frame();
        let half = apply_muted(before.clone(), &opts(&[("muted", 0.5)]));
        let full = apply_muted(before.clone(), &opts(&[("muted", 1.0)]));
        let d = |f: &OrbFrame| f.dots[0].a;
        assert!(full.dots[0].a < d(&half) && d(&half) < before.dots[0].a);
        assert!(
            full.dots[0].saturation < half.dots[0].saturation
                && half.dots[0].saturation < before.dots[0].saturation
        );
    }
}

#[cfg(test)]
mod audio_reactive_tests {
    use super::*;

    fn frame_with_one_dot() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![Dot {
                x: 10.0,
                y: 10.0,
                r: 2.0,
                a: 0.5,
                ..Default::default()
            }],
            lines: vec![],
            polylines: vec![],
        }
    }

    fn frame_with_two_dots() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 0.0,
                    y: 0.0,
                    r: 2.0,
                    a: 0.5,
                    ..Default::default()
                },
                Dot {
                    x: 20.0,
                    y: 0.0,
                    r: 2.0,
                    a: 0.5,
                    ..Default::default()
                },
            ],
            lines: vec![Line {
                x1: 0.0,
                y1: 0.0,
                x2: 20.0,
                y2: 0.0,
                white: 0.5,
                a: 0.5,
                w: 1.0,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![Polyline {
                points: vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 10.0, y: 5.0 },
                    Point { x: 20.0, y: 0.0 },
                ],
                white: 0.5,
                a: 0.5,
                w: 1.0,
                ..Default::default()
            }],
        }
    }

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn a_negative_audio_strength_draws_the_frame_inward_and_still_brightens() {
        // The listening "inhale": same magnitude as the outward swell, the
        // opposite direction, and the glimmer unchanged -- a tightening,
        // not a fade.
        let ring = arc_polyline(32.0, 32.0, 20.0, 0.0, 360.0, 4.0, 0.15, 0.5, 0.0, 0.0);
        let frame = || OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![],
            lines: vec![],
            polylines: vec![ring.clone()],
        };
        let (cx, cy) = frame_centroid(&frame()).unwrap();
        let reach = |f: &OrbFrame| {
            let p = &f.polylines[0].points[5];
            ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
        };
        let rest = reach(&frame());
        let inward =
            apply_audio_reactive(frame(), &o(&[("audioLevel", 1.0), ("audioStrength", -0.5)]));
        let outward =
            apply_audio_reactive(frame(), &o(&[("audioLevel", 1.0), ("audioStrength", 0.5)]));
        assert!(
            (reach(&inward) / rest - 0.8).abs() < 1e-9,
            "expand = 1 - 0.2"
        );
        assert!((reach(&outward) / rest - 1.2).abs() < 1e-9);
        // Brightness and width follow the magnitude, so both directions read.
        assert!((inward.polylines[0].a - outward.polylines[0].a).abs() < 1e-12);
        assert!((inward.polylines[0].w - outward.polylines[0].w).abs() < 1e-12);
        assert!(inward.polylines[0].a > 0.5, "an inhale brightens too");
    }

    #[test]
    fn ink_fades_everything_drawn_and_is_a_no_op_at_one() {
        let base = frame_with_two_dots();
        let untouched = apply_ink(frame_with_two_dots(), &o(&[]));
        let full = apply_ink(frame_with_two_dots(), &o(&[("ink", 1.0)]));
        for (i, d) in base.dots.iter().enumerate() {
            assert_eq!(untouched.dots[i].a, d.a, "no `ink` key changes nothing");
            assert_eq!(full.dots[i].a, d.a, "`ink` 1 changes nothing");
        }
        let half = apply_ink(frame_with_two_dots(), &o(&[("ink", 0.5)]));
        for (i, d) in base.dots.iter().enumerate() {
            assert!((half.dots[i].a - d.a * 0.5).abs() < 1e-12);
        }
        for (i, l) in base.lines.iter().enumerate() {
            assert!((half.lines[i].a - l.a * 0.5).abs() < 1e-12);
        }
        // Geometry and ink value are untouched: `ink` is opacity only.
        assert_eq!(half.dots[0].x, base.dots[0].x);
        assert_eq!(half.dots[0].r, base.dots[0].r);
        assert_eq!(half.dots[0].white, base.dots[0].white);
    }

    #[test]
    fn a_polyline_only_frame_breathes_about_its_own_centroid() {
        let ring = arc_polyline(32.0, 32.0, 20.0, 0.0, 360.0, 4.0, 0.15, 0.5, 0.0, 0.0);
        let frame = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![],
            lines: vec![],
            polylines: vec![ring.clone()],
        };
        let (cx, cy) = frame_centroid(&frame).unwrap();
        let out = apply_audio_reactive(frame, &o(&[("audioLevel", 1.0), ("audioStrength", 0.5)]));
        let p = &out.polylines[0];
        let d0 = ((ring.points[5].x - cx).powi(2) + (ring.points[5].y - cy).powi(2)).sqrt();
        let d1 = ((p.points[5].x - cx).powi(2) + (p.points[5].y - cy).powi(2)).sqrt();
        assert!((d1 / d0 - 1.2).abs() < 1e-9, "expand = 1 + 1 * 0.5 * 0.4");
        assert!((p.w - 4.0 * 1.25).abs() < 1e-9 && (p.a - 0.5 * 1.3).abs() < 1e-9);
    }

    #[test]
    fn frames_with_dots_keep_the_dots_only_centroid() {
        // A far-off polyline must not pull the centroid: dots decide it,
        // exactly as before the polyline path existed.
        let frame = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: frame_with_two_dots().dots,
            lines: vec![],
            polylines: vec![arc_polyline(
                200.0, 200.0, 5.0, 0.0, 90.0, 1.0, 0.1, 0.5, 0.0, 0.0,
            )],
        };
        let out = apply_audio_reactive(frame, &o(&[("audioLevel", 1.0), ("audioStrength", 0.5)]));
        // Dots at x = 0 and 20 around centroid 10, expanded by 1.2.
        assert!((out.dots[0].x - (10.0 - 10.0 * 1.2)).abs() < 1e-9);
        assert!((out.dots[1].x - (10.0 + 10.0 * 1.2)).abs() < 1e-9);
    }

    #[test]
    fn an_empty_frame_is_untouched() {
        let empty = OrbFrame::default();
        assert_eq!(
            apply_audio_reactive(
                empty.clone(),
                &o(&[("audioLevel", 1.0), ("audioStrength", 1.0)])
            ),
            empty
        );
    }

    #[test]
    fn no_op_when_strength_is_absent_or_zero() {
        let frame = frame_with_one_dot();
        assert_eq!(apply_audio_reactive(frame.clone(), &HashMap::new()), frame);

        let mut opts = HashMap::new();
        opts.insert("audioStrength".to_string(), 0.0);
        opts.insert("audioLevel".to_string(), 0.9);
        assert_eq!(apply_audio_reactive(frame.clone(), &opts), frame);
    }

    #[test]
    fn scales_radius_and_boosts_alpha_with_level() {
        let frame = frame_with_one_dot();
        let mut opts = HashMap::new();
        opts.insert("audioStrength".to_string(), 0.6);
        opts.insert("audioLevel".to_string(), 1.0);

        let out = apply_audio_reactive(frame.clone(), &opts);
        assert!(
            out.dots[0].r > frame.dots[0].r,
            "expected radius to grow with audio level"
        );
        assert!(
            out.dots[0].a > frame.dots[0].a,
            "expected alpha to boost with audio level"
        );
        assert!(out.dots[0].a <= 1.0, "alpha must stay clamped to 1.0");
    }

    #[test]
    fn breathes_dots_and_lines_outward_from_the_centroid() {
        let frame = frame_with_two_dots();
        let mut opts = HashMap::new();
        opts.insert("audioStrength".to_string(), 0.5);
        opts.insert("audioLevel".to_string(), 1.0);

        let out = apply_audio_reactive(frame.clone(), &opts);
        // centroid is (10, 0); expand = 1 + 1.0*0.5*0.4 = 1.2 -> dot at x=0 moves to 10 + (0-10)*1.2 = -2
        assert!(
            (out.dots[0].x - (-2.0)).abs() < 1e-9,
            "left dot should move further left: {}",
            out.dots[0].x
        );
        assert!(
            (out.dots[1].x - 22.0).abs() < 1e-9,
            "right dot should move further right: {}",
            out.dots[1].x
        );
        // the line's endpoints must move identically to the dots they connect
        assert!(
            (out.lines[0].x1 - out.dots[0].x).abs() < 1e-9,
            "line start should stay attached to its dot"
        );
        assert!(
            (out.lines[0].x2 - out.dots[1].x).abs() < 1e-9,
            "line end should stay attached to its dot"
        );
        // ...and so must a polyline's vertices
        let pts = &out.polylines[0].points;
        assert!(
            (pts[0].x - out.dots[0].x).abs() < 1e-9,
            "polyline start should stay attached to its dot"
        );
        assert!(
            (pts[2].x - out.dots[1].x).abs() < 1e-9,
            "polyline end should stay attached to its dot"
        );
        assert!(
            (pts[1].y - 6.0).abs() < 1e-9,
            "interior vertex should scale about the centroid too: {}",
            pts[1].y
        );
    }
}

#[cfg(test)]
mod polyline_tests {
    use super::*;

    #[test]
    fn with_polylines_culls_invisible_and_degenerate_paths() {
        let visible = Polyline {
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 1.0, y: 1.0 }],
            a: 0.5,
            ..Default::default()
        };
        let invisible = Polyline {
            a: 0.01,
            ..visible.clone()
        };
        let one_vertex = Polyline {
            points: vec![Point { x: 0.0, y: 0.0 }],
            ..visible.clone()
        };
        let frame = finalize_frame(vec![], vec![], 0.3).with_polylines(vec![
            invisible,
            one_vertex,
            visible.clone(),
        ]);
        assert_eq!(frame.polylines, vec![visible]);
    }

    #[test]
    fn finalize_frame_starts_with_no_polylines() {
        assert!(finalize_frame(vec![], vec![], 0.3).polylines.is_empty());
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;

    fn frame_with_two_dots() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 10.0,
                    y: 10.0,
                    a: 1.0,
                    ..Default::default()
                }, // near the pointer
                Dot {
                    x: 90.0,
                    y: 90.0,
                    a: 1.0,
                    ..Default::default()
                }, // far from the pointer
            ],
            lines: vec![],
            polylines: vec![],
        }
    }

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn pushes_polyline_vertices_near_the_pointer_and_leaves_far_ones() {
        // A ring-like arc with no dots at all (every `ring` state).
        let arc = arc_polyline(32.0, 32.0, 20.0, 0.0, 180.0, 4.0, 0.15, 0.95, 0.0, 0.0);
        let frame = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![],
            lines: vec![],
            polylines: vec![arc.clone()],
        };
        // Pointer just inside 3 o'clock.
        let opts = o(&[
            ("pointerX", 48.0),
            ("pointerY", 32.0),
            ("pointerRadius", 10.0),
            ("pointerStrength", 5.0),
        ]);
        let out = apply_pointer(frame, &opts);
        let p = &out.polylines[0];
        // The vertex nearest 3 o'clock (the arc is sampled every ~4°).
        let d = |q: &Point| (q.x - 52.0).powi(2) + (q.y - 32.0).powi(2);
        let three = (0..arc.points.len())
            .min_by(|&a, &b| d(&arc.points[a]).partial_cmp(&d(&arc.points[b])).unwrap())
            .unwrap();
        assert!(
            p.points[three].x > arc.points[three].x,
            "pushed away from the pointer"
        );
        assert_eq!(p.points[0], arc.points[0], "12 o'clock is out of reach");
        assert_eq!(
            p.a, arc.a,
            "a whole stroke can't fade locally, so it only moves"
        );
    }

    #[test]
    fn a_line_endpoint_on_a_dot_stays_attached() {
        let frame = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![Dot {
                x: 12.0,
                y: 10.0,
                a: 1.0,
                ..Default::default()
            }],
            lines: vec![Line {
                x1: 12.0,
                y1: 10.0,
                x2: 90.0,
                y2: 90.0,
                white: 0.0,
                a: 1.0,
                w: 1.0,
                saturation: 0.0,
                hue: 0.0,
            }],
            polylines: vec![],
        };
        let opts = o(&[
            ("pointerX", 10.0),
            ("pointerY", 10.0),
            ("pointerRadius", 20.0),
            ("pointerStrength", 8.0),
        ]);
        let out = apply_pointer(frame, &opts);
        assert!(out.dots[0].x > 12.0, "the dot moved");
        assert_eq!(
            (out.lines[0].x1, out.lines[0].y1),
            (out.dots[0].x, out.dots[0].y)
        );
        assert_eq!((out.lines[0].x2, out.lines[0].y2), (90.0, 90.0));
    }

    #[test]
    fn a_stroke_out_of_reach_keeps_its_alpha() {
        let arc = arc_polyline(32.0, 32.0, 20.0, 0.0, 90.0, 4.0, 0.15, 0.95, 0.0, 0.0);
        let frame = OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![],
            lines: vec![],
            polylines: vec![arc.clone()],
        };
        let opts = o(&[
            ("pointerX", 5.0),
            ("pointerY", 60.0),
            ("pointerRadius", 5.0),
            ("pointerStrength", 5.0),
        ]);
        assert_eq!(apply_pointer(frame, &opts).polylines[0], arc);
    }

    #[test]
    fn no_op_when_strength_is_absent_or_zero() {
        let frame = frame_with_two_dots();
        let out = apply_pointer(frame.clone(), &HashMap::new());
        assert_eq!(out, frame);

        let mut opts = HashMap::new();
        opts.insert("pointerStrength".to_string(), 0.0);
        opts.insert("pointerRadius".to_string(), 20.0);
        let out = apply_pointer(frame.clone(), &opts);
        assert_eq!(out, frame);
    }

    #[test]
    fn pushes_nearby_dots_away_and_leaves_far_ones_alone() {
        let frame = frame_with_two_dots();
        let mut opts = HashMap::new();
        opts.insert("pointerX".to_string(), 12.0);
        opts.insert("pointerY".to_string(), 10.0);
        opts.insert("pointerRadius".to_string(), 20.0);
        opts.insert("pointerStrength".to_string(), 5.0);

        let out = apply_pointer(frame.clone(), &opts);
        assert_ne!(
            out.dots[0].x, frame.dots[0].x,
            "the near dot should have moved"
        );
        assert!(
            out.dots[0].a < frame.dots[0].a,
            "the near dot should have faded a bit"
        );
        assert_eq!(
            out.dots[1].x, frame.dots[1].x,
            "the far dot is outside pointerRadius, should be untouched"
        );
        assert_eq!(out.dots[1].a, frame.dots[1].a);
    }
}

// --- Real gradient noise (Ken Perlin's "improved noise", 2002) ---
//
// Not a port: no mode ported from `thinking-orbs` uses this, they all use
// `vnoise` above (hash-based *value* noise) and must keep doing so, since
// their golden-vector proof is against upstream's own `vnoise` output.
// `perlin3` exists for `orbs::modes::aurora`, the first mode that wasn't a
// port and had no golden vector to preserve — see `docs/effects-research.md`
// ("Real Perlin / Simplex / Worley noise" was judged the single lowest-risk,
// best-provenance improvement available, since the *algorithm* below is
// Perlin's own published reference structure: fade curve, gradient dot
// products, trilinear interpolation).
//
// The permutation table is NOT Perlin's original published constants —
// hand-transcribing 256 literals from memory risks a silent one-digit error
// with no way to check it here. What the algorithm's correctness properties
// (exact zero at every integer lattice point, C2-continuous interpolation,
// determinism) actually depend on is `PERM` being *some* permutation of
// `0..256`, not which one — so this generates its own, once, deterministically,
// from the existing `hash_d` above via a Fisher-Yates shuffle with a fixed
// seed. Verified by the zero-at-lattice-points property test below, which
// would fail for any wrong implementation regardless of table contents.
fn permutation() -> &'static [u8; 512] {
    use std::sync::OnceLock;
    static PERM: OnceLock<[u8; 512]> = OnceLock::new();
    PERM.get_or_init(|| {
        let mut p: [u8; 256] = [0; 256];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = i as u8;
        }
        for i in (1..256).rev() {
            let j = (hash_d(i as f64, 91.7) * (i as f64 + 1.0)) as usize % (i + 1);
            p.swap(i, j);
        }
        let mut full = [0u8; 512];
        for (i, slot) in full.iter_mut().enumerate() {
            *slot = p[i % 256];
        }
        full
    })
}

/// The 12 cube-edge gradient directions from Perlin's reference implementation.
const GRAD3: [(f64, f64, f64); 12] = [
    (1.0, 1.0, 0.0),
    (-1.0, 1.0, 0.0),
    (1.0, -1.0, 0.0),
    (-1.0, -1.0, 0.0),
    (1.0, 0.0, 1.0),
    (-1.0, 0.0, 1.0),
    (1.0, 0.0, -1.0),
    (-1.0, 0.0, -1.0),
    (0.0, 1.0, 1.0),
    (0.0, -1.0, 1.0),
    (0.0, 1.0, -1.0),
    (0.0, -1.0, -1.0),
];

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn grad3(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    let g = GRAD3[(hash % 12) as usize];
    g.0 * x + g.1 * y + g.2 * z
}

/// Real 3D gradient (Perlin) noise — interpolates pseudorandom *gradient
/// vectors* at lattice corners (dot-producted with the offset to each
/// sample point), unlike `vnoise`'s interpolation of scalar *values*. Looks
/// more isotropic/organic than `vnoise`'s faint axis-aligned grid bias.
/// Typical output range is roughly `[-1, 1]` (not a hard bound for 3D
/// Perlin, but reliably within it in practice for the unit-ish input
/// magnitudes this engine uses).
pub fn perlin3(x: f64, y: f64, z: f64) -> f64 {
    let p = permutation();
    let xi = (x.floor() as i64).rem_euclid(256) as usize;
    let yi = (y.floor() as i64).rem_euclid(256) as usize;
    let zi = (z.floor() as i64).rem_euclid(256) as usize;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let a = p[xi] as usize + yi;
    let aa = p[a] as usize + zi;
    let ab = p[a + 1] as usize + zi;
    let b = p[xi + 1] as usize + yi;
    let ba = p[b] as usize + zi;
    let bb = p[b + 1] as usize + zi;

    let x1 = lerp(grad3(p[aa], xf, yf, zf), grad3(p[ba], xf - 1.0, yf, zf), u);
    let x2 = lerp(
        grad3(p[ab], xf, yf - 1.0, zf),
        grad3(p[bb], xf - 1.0, yf - 1.0, zf),
        u,
    );
    let y1 = lerp(x1, x2, v);

    let x3 = lerp(
        grad3(p[aa + 1], xf, yf, zf - 1.0),
        grad3(p[ba + 1], xf - 1.0, yf, zf - 1.0),
        u,
    );
    let x4 = lerp(
        grad3(p[ab + 1], xf, yf - 1.0, zf - 1.0),
        grad3(p[bb + 1], xf - 1.0, yf - 1.0, zf - 1.0),
        u,
    );
    let y2 = lerp(x3, x4, v);

    lerp(y1, y2, w)
}

#[cfg(test)]
mod noise_tests {
    use super::perlin3;

    /// The one property that holds regardless of which permutation backs
    /// the table: at an exact integer lattice point, every corner's offset
    /// vector to that point is zero-length except the corner's own (also
    /// zero), so every gradient dot-product is exactly 0.0 and every
    /// interpolation weight is exactly 0.0 or 1.0 -- the result is exactly
    /// 0.0. If this fails, the interpolation/indexing logic is wrong, not
    /// the table.
    #[test]
    fn zero_at_integer_lattice_points() {
        for (x, y, z) in [
            (0.0, 0.0, 0.0),
            (3.0, 5.0, -2.0),
            (100.0, 0.0, 7.0),
            (-4.0, -4.0, -4.0),
        ] {
            let n = perlin3(x, y, z);
            assert!(n.abs() < 1e-9, "perlin3({x},{y},{z}) = {n}, expected ~0");
        }
    }

    #[test]
    fn deterministic() {
        assert_eq!(perlin3(1.23, 4.56, 7.89), perlin3(1.23, 4.56, 7.89));
    }

    #[test]
    fn stays_in_practical_bounds() {
        for i in 0..500 {
            let t = i as f64 * 0.037;
            let n = perlin3(t, t * 1.3 + 0.5, t * 0.7 - 1.1);
            assert!(
                n.abs() <= 1.5,
                "perlin3 produced {n} at t={t}, outside expected range"
            );
        }
    }

    /// Prints, doesn't assert -- there's no established perf budget to gate
    /// on, this only exists to answer "is perlin3 meaningfully more
    /// expensive than vnoise" with real numbers instead of a guess. Ignored
    /// by default; run explicitly (and in release, debug timings aren't
    /// representative):
    /// `cargo test -p core_engine --release -- --ignored --nocapture perf_vnoise_vs_perlin3`
    #[test]
    #[ignore = "prints timing, not a regression gate -- see doc comment"]
    fn perf_vnoise_vs_perlin3() {
        use super::vnoise;
        use crate::orbs::modes::web::frame_web;
        use crate::orbs::modes::webflow::frame_webflow;
        use crate::orbs::profiles::opts;
        use std::hint::black_box;
        use std::time::Instant;

        const N: u64 = 2_000_000;

        let start = Instant::now();
        let mut acc = 0.0;
        for i in 0..N {
            acc += vnoise(i as f64 * 0.013, i as f64 * 0.027);
        }
        let vnoise_dt = start.elapsed();
        black_box(acc);

        let start = Instant::now();
        let mut acc = 0.0;
        for i in 0..N {
            acc += perlin3(i as f64 * 0.013, i as f64 * 0.027, i as f64 * 0.019);
        }
        let perlin_dt = start.elapsed();
        black_box(acc);

        println!(
            "raw noise call, {N} iterations:\n  vnoise:  {vnoise_dt:?} ({:.2} ns/call)\n  perlin3: {perlin_dt:?} ({:.2} ns/call)\n  ratio:   perlin3 is {:.2}x vnoise's cost",
            vnoise_dt.as_nanos() as f64 / N as f64,
            perlin_dt.as_nanos() as f64 / N as f64,
            perlin_dt.as_secs_f64() / vnoise_dt.as_secs_f64()
        );

        // Whole-frame comparison at the engine's actual scale (30 nodes,
        // same opts for both -- see webflow.rs), not just the raw function.
        const FRAMES: u64 = 100_000;
        let o = opts(&[
            ("nodeN", 30.0),
            ("thr", 0.72),
            ("signals", 5.0),
            ("nodeR", 1.4),
            ("nodeRDepth", 1.8),
            ("lineW", 0.8),
            ("rMin", 0.3),
        ]);

        let start = Instant::now();
        for i in 0..FRAMES {
            black_box(frame_web(64.0, i as f64 * 0.001, &o));
        }
        let web_dt = start.elapsed();

        let start = Instant::now();
        for i in 0..FRAMES {
            black_box(frame_webflow(64.0, i as f64 * 0.001, &o));
        }
        let webflow_dt = start.elapsed();

        println!(
            "whole frame, {FRAMES} frames at size=64/nodeN=30:\n  connecting (vnoise):  {web_dt:?} ({:.2} µs/frame)\n  drifting (perlin3):   {webflow_dt:?} ({:.2} µs/frame)\n  ratio:                drifting is {:.2}x connecting's cost",
            web_dt.as_secs_f64() * 1e6 / FRAMES as f64,
            webflow_dt.as_secs_f64() * 1e6 / FRAMES as f64,
            webflow_dt.as_secs_f64() / web_dt.as_secs_f64()
        );
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn mixed() -> OrbFrame {
        OrbFrame {
            color_mode: ColorMode::Ink,
            fills: Vec::new(),
            effects: Vec::new(),
            dots: vec![
                Dot {
                    x: 10.0,
                    y: 10.0,
                    r: 2.0,
                    white: 0.2,
                    a: 1.0,
                    ..Default::default()
                },
                Dot {
                    x: 20.0,
                    y: 10.0,
                    r: 2.0,
                    white: 0.6,
                    a: 1.0,
                    saturation: 0.5,
                    hue: 350.0,
                    ..Default::default()
                },
            ],
            lines: vec![Line {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 10.0,
                white: 0.42,
                a: 1.0,
                w: 1.0,
                ..Default::default()
            }],
            polylines: vec![Polyline {
                points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 5.0, y: 5.0 }],
                white: 0.15,
                a: 1.0,
                w: 2.0,
                ..Default::default()
            }],
        }
    }

    #[test]
    fn no_op_without_keys() {
        let f = mixed();
        assert_eq!(apply_color(f.clone(), &HashMap::new()), f);
        assert_eq!(apply_color(f.clone(), &o(&[("colorMix", 0.0)])), f);
    }

    #[test]
    fn full_mix_colours_every_primitive_and_keeps_lightness() {
        let f = mixed();
        let g = apply_color(
            f.clone(),
            &o(&[
                ("colorMix", 1.0),
                ("colorHue", 30.0),
                ("colorSaturation", 0.7),
            ]),
        );
        assert!(g
            .dots
            .iter()
            .all(|d| d.hue == 30.0 && (d.saturation - 0.7).abs() < 1e-12));
        assert_eq!(
            (g.lines[0].hue, g.lines[0].saturation),
            (30.0, 0.7),
            "lines are coloured too"
        );
        assert_eq!((g.polylines[0].hue, g.polylines[0].saturation), (30.0, 0.7));
        for (a, b) in g.dots.iter().zip(&f.dots) {
            assert_eq!(a.white, b.white, "no lightness bias -> depth ink untouched");
        }
    }

    #[test]
    fn half_mix_takes_the_shorter_hue_arc_and_grey_snaps() {
        let g = apply_color(
            mixed(),
            &o(&[
                ("colorMix", 0.5),
                ("colorHue", 30.0),
                ("colorSaturation", 0.9),
            ]),
        );
        // Coloured dot: 350 -> 30 the short way (through 0), halfway = 10.
        assert!((g.dots[1].hue - 10.0).abs() < 1e-9);
        assert!((g.dots[1].saturation - 0.7).abs() < 1e-12);
        // Grey dot: its hue meant nothing, so it snaps; saturation halfway.
        assert_eq!(g.dots[0].hue, 30.0);
        assert!((g.dots[0].saturation - 0.45).abs() < 1e-12);
    }

    #[test]
    fn lightness_bias_preserves_order_and_spread() {
        let ws = [0.05, 0.2, 0.42, 0.6, 0.72, 0.95];
        for b in [-0.8, -0.5, -0.1, 0.1, 0.5, 0.8] {
            let out: Vec<f64> = ws.iter().map(|w| lightness_bias(*w, b)).collect();
            assert!(out.windows(2).all(|p| p[0] < p[1]), "b = {b}: order kept");
            let spread = out[5] - out[0];
            assert!(spread > 0.1, "b = {b}: the ramp didn't collapse ({spread})");
        }
        assert_eq!(lightness_bias(0.3, 0.0), 0.3);
        assert_eq!(lightness_bias(0.3, 1.0), 1.0);
        assert_eq!(lightness_bias(0.3, -1.0), 0.0);
    }

    #[test]
    fn a_real_sphere_keeps_its_depth_ramp_under_a_bias() {
        // `working` at t = 1: its dots' ink encodes depth. A bias must keep
        // every pairwise order (no flattening into a disc).
        let base = crate::frame("working".into(), 64, 1.0).unwrap();
        for b in [-0.5, 0.5] {
            let g = apply_color(
                base.clone(),
                &o(&[("colorMix", 1.0), ("colorLightness", b)]),
            );
            let mut pairs = base.dots.iter().zip(&g.dots).collect::<Vec<_>>();
            pairs.sort_by(|x, y| x.0.white.partial_cmp(&y.0.white).unwrap());
            assert!(pairs
                .windows(2)
                .all(|p| p[0].1.white <= p[1].1.white + 1e-12));
            let (lo, hi) = (pairs[0].1.white, pairs[pairs.len() - 1].1.white);
            let (blo, bhi) = (pairs[0].0.white, pairs[pairs.len() - 1].0.white);
            assert!(hi - lo > 0.4 * (bhi - blo), "b = {b}: spread kept");
        }
    }

    #[test]
    fn color_mode_travels_in_the_frame_even_without_mix() {
        assert_eq!(mixed().color_mode, ColorMode::Ink);
        let fixed = apply_color(mixed(), &o(&[("colorMode", 1.0)]));
        assert_eq!(fixed.color_mode, ColorMode::Fixed);
        assert_eq!(fixed.dots, mixed().dots, "mode alone recolours nothing");
        let ink = apply_color(fixed, &o(&[("colorMode", 0.0)]));
        assert_eq!(ink.color_mode, ColorMode::Ink);
        // Through the public render chain as well.
        let f = crate::frame_with_overrides("glowing".into(), 64, 1.0, o(&[("colorMode", 1.0)]))
            .unwrap();
        assert_eq!(f.color_mode, ColorMode::Fixed);
        assert_eq!(
            crate::frame("glowing".into(), 64, 1.0).unwrap().color_mode,
            ColorMode::Ink
        );
    }

    #[test]
    fn gradient_third_stop_is_optional_and_places_the_middle() {
        let base = o(&[
            ("gradientStrength", 1.0),
            ("gradientHue", 0.0),
            ("gradientHue2", 100.0),
        ]);
        let f = mixed();
        // Absent third stop: bit-identical to an explicit two-stop call.
        let two = apply_gradient(f.clone(), 64.0, &base);
        let mut with_mid_only = base.clone();
        with_mid_only.insert("gradientMid".into(), 0.3);
        assert_eq!(
            apply_gradient(f.clone(), 64.0, &with_mid_only),
            two,
            "gradientMid alone changes nothing"
        );
        // Three stops, mid at 0.5: the element at u = 0.5 gets exactly h2.
        let mut three = base.clone();
        three.insert("gradientHue3".into(), 250.0);
        let centre = OrbFrame {
            dots: vec![Dot {
                x: 32.0,
                y: 32.0,
                r: 1.0,
                a: 1.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!((apply_gradient(centre.clone(), 64.0, &three).dots[0].hue - 100.0).abs() < 1e-9);
        // u = 0.75 is halfway between h2 = 100 and h3 = 250.
        let right = OrbFrame {
            dots: vec![Dot {
                x: 48.0,
                y: 32.0,
                r: 1.0,
                a: 1.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!((apply_gradient(right, 64.0, &three).dots[0].hue - 175.0).abs() < 1e-9);
    }

    #[test]
    fn glow_and_mute_carry_line_colour() {
        let coloured = apply_color(mixed(), &o(&[("colorMix", 1.0), ("colorHue", 120.0)]));
        let glow = apply_glow(coloured.clone(), &o(&[("glowStrength", 1.0)]));
        assert!(
            glow.lines.len() > 1 && glow.lines.iter().all(|l| l.hue == 120.0),
            "halos copy line colour"
        );
        let tinted = apply_glow(
            coloured.clone(),
            &o(&[("glowStrength", 1.0), ("glowTint", 1.0), ("glowHue", 40.0)]),
        );
        assert!(
            tinted.lines.iter().filter(|l| l.hue == 40.0).count() >= 1,
            "glow tint reaches line halos"
        );
        let muted = apply_muted(coloured, &o(&[("muted", 1.0)]));
        assert!(
            muted.lines[0].saturation < 0.8,
            "a grey mute desaturates lines too"
        );
    }
}

#[cfg(test)]
mod per_vertex_hue_tests {
    use super::*;

    fn opts(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    /// A 24-vertex ring (a track) plus one zero-length stroke (a head).
    fn ring_frame() -> OrbFrame {
        let points: Vec<Point> = (0..24)
            .map(|k| {
                let a = k as f64 / 24.0 * std::f64::consts::TAU;
                Point {
                    x: 32.0 + 20.0 * a.cos(),
                    y: 32.0 + 20.0 * a.sin(),
                }
            })
            .collect();
        OrbFrame {
            polylines: vec![
                Polyline {
                    points,
                    white: 0.4,
                    a: 0.5,
                    w: 3.0,
                    ..Default::default()
                },
                Polyline {
                    points: vec![Point { x: 32.0, y: 12.0 }; 2],
                    white: 0.3,
                    a: 1.0,
                    w: 4.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    /// A dot at the vertex's position with `z = 0` gets holo's `d = 0.5`,
    /// the same depth term strokes use -- so it's an independent oracle
    /// for the vertex hue. Gradient has no depth term at all.
    fn dot_at(q: &Point) -> Dot {
        Dot {
            x: q.x,
            y: q.y,
            r: 1.0,
            white: 0.4,
            a: 1.0,
            ..Default::default()
        }
    }

    fn check_vertices_match_dots(apply: impl Fn(OrbFrame) -> OrbFrame) {
        let mut f = ring_frame();
        let track = f.polylines[0].points.clone();
        f.dots = track.iter().map(dot_at).collect();
        let out = apply(f);
        let p = &out.polylines[0];
        assert_eq!(p.hues.len(), p.points.len());
        for (h, d) in p.hues.iter().zip(&out.dots) {
            assert!((h - d.hue).abs() < 1e-9, "vertex {h} vs dot {}", d.hue);
        }
        // The zero-length head: every vertex has one hue, so no `hues`.
        assert!(out.polylines[1].hues.is_empty());
    }

    #[test]
    fn holo_and_gradient_colour_each_vertex_at_its_own_position() {
        check_vertices_match_dots(|f| apply_holo(f, 64.0, 0.0, &opts(&[("holoStrength", 1.0)])));
        check_vertices_match_dots(|f| apply_gradient(f, 64.0, &opts(&[("gradientStrength", 1.0)])));
    }

    #[test]
    fn single_hue_stays_the_vertex_mean_sample() {
        // `hue` must be what it was before per-vertex hues existed: the
        // colour at the vertex mean -- here a dot at the ring's centre.
        let mut f = ring_frame();
        f.polylines.truncate(1);
        f.dots = vec![dot_at(&Point { x: 32.0, y: 32.0 })];
        let out = apply_holo(f, 64.0, 0.0, &opts(&[("holoStrength", 1.0)]));
        let mean = out.polylines[0]
            .points
            .iter()
            .fold((0.0, 0.0), |a, q| (a.0 + q.x, a.1 + q.y));
        assert!((mean.0 / 24.0 - 32.0).abs() < 1e-9 && (mean.1 / 24.0 - 32.0).abs() < 1e-9);
        assert!((out.polylines[0].hue - out.dots[0].hue).abs() < 1e-9);
    }

    #[test]
    fn holo_over_gradient_eases_each_vertex_from_its_own_hue() {
        let g = apply_gradient(ring_frame(), 64.0, &opts(&[("gradientStrength", 1.0)]));
        let before = g.polylines[0].hues.clone();
        let sat = g.polylines[0].saturation;
        let out = apply_holo(g, 64.0, 0.0, &opts(&[("holoStrength", 0.5)]));
        let full = apply_holo(ring_frame(), 64.0, 0.0, &opts(&[("holoStrength", 1.0)]));
        assert!(sat > 0.0);
        for ((h, b), t) in out.polylines[0]
            .hues
            .iter()
            .zip(&before)
            .zip(&full.polylines[0].hues)
        {
            assert!((h - lerp_hue(*b, *t, 0.5)).abs() < 1e-9);
        }
    }

    #[test]
    fn muted_interrupt_and_glow_carry_or_drop_the_hues() {
        let holo = || apply_holo(ring_frame(), 64.0, 0.0, &opts(&[("holoStrength", 1.0)]));
        let n = holo().polylines[0].points.len();
        // Muted keeps the sweep (its colour rule is applied per vertex).
        let m = apply_muted(holo(), &opts(&[("muted", 1.0)]));
        assert!(m.polylines[0].hues.is_empty() || m.polylines[0].hues.len() == n);
        // An interrupt flash at its peak snaps every vertex to its hue.
        let i = apply_interrupt(
            holo(),
            &opts(&[("interruptAge", 0.0), ("interruptTint", 1.0)]),
        );
        let p = &i.polylines[0];
        assert!(p.hues.is_empty() || p.hues.iter().all(|&h| (h - p.hue).abs() < 1e-9));
        // Glow halos: tinted = one hue (no `hues`), untinted = the sweep.
        let tinted = apply_glow(holo(), &opts(&[("glowStrength", 1.0), ("glowTint", 1.0)]));
        let plain = apply_glow(holo(), &opts(&[("glowStrength", 1.0)]));
        let halo_hues = |f: &OrbFrame| {
            f.polylines
                .iter()
                .filter(|p| p.points.len() == n)
                .map(|p| p.hues.len())
                .collect::<Vec<_>>()
        };
        let t = halo_hues(&tinted);
        assert!(t[..t.len() - 1].iter().all(|&k| k == 0) && t[t.len() - 1] == n);
        assert!(halo_hues(&plain).iter().all(|&k| k == n));
        let blurred = apply_glow(holo(), &opts(&[("glowStrength", 1.0), ("glowMode", 1.0)]));
        assert!(halo_hues(&blurred).iter().all(|&k| k == n));
    }
}
