//! FX Spec v1 -- one versioned, human-readable JSON file that describes one
//! object in one state (object, state, size, speed, color, gradient,
//! materials, params), loaded identically on every platform. Rust owns the
//! whole contract: parse, validate, migrate, color conversion and the
//! resolution to engine opts; wasm/UniFFI export it and TS/Swift/Kotlin/RN
//! only wrap it. See `docs/fx-spec.md` and `spec/fx-spec-1.schema.json`.
//!
//! Design notes :
//! - **Versioning** follows glTF 2.0's `asset.version`: `"major.minor"`; a
//!   major bump may break, minor versions are backward *and* forward
//!   compatible. So a 1.0 runtime errors on a 2.x file, but reads a newer
//!   1.x file, reporting keys it doesn't know as *warnings* (that's how
//!   v1.1's per-state map and bindings arrive without breaking 1.0 files
//!   or 1.0 runtimes). In a file that claims `1.0`, unknown keys are
//!   errors. Never silently ignored either way.
//! - **Color** accepts `"#RRGGBB"` / `"#RGB"` or a W3C Design Tokens
//!   (DTCG 2025.10) color value `{colorSpace, components, alpha?, hex?}`.
//!   `srgb` and `hsl` convert exactly; any other space falls back to its
//!   `hex` (with a warning), which leaves the door open to OKLCH later.
//! - **Hex -> engine keys** is a port of the Studio's `kit/color.ts` (the
//!   CSS Color 4 sample code) including its rounding, so a spec exported by
//!   the Studio renders exactly what the Studio showed; a parity test
//!   (`packages/core/test/fx-color-parity.test.mjs`) holds the two together.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};

pub const RUNTIME_MAJOR: u64 = 1;
pub const RUNTIME_MINOR: u64 = 8;
/// The oldest minor this runtime reads. 1.0–1.7 were never published, so their
/// acceptance was dropped before the first release instead of becoming a
/// compatibility promise (release decision 0.1).
pub const FLOOR_MINOR: u64 = 8;
pub const SIZES: [u32; 3] = [20, 32, 64];

/// One problem found in a spec: `severity` is `"error"` or `"warning"`,
/// `path` a JSON Pointer (RFC 6901, e.g. `/params/colorHeu`).
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxDiagnostic {
    pub severity: String,
    pub path: String,
    pub message: String,
}

/// A spec resolved to what the engine understands. `ok` is false when any
/// diagnostic is an error; `overrides` then holds whatever did resolve, but
/// callers should not render it (`frame_from_fx_spec` returns `None`).
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxSpecResolved {
    pub ok: bool,
    pub state: String,
    pub size: u32,
    /// The spec's own speed multiplier (1 = the preset's tuned speed).
    pub speed: f64,
    pub overrides: HashMap<String, f64>,
    pub diagnostics: Vec<FxDiagnostic>,
    /// v1.1: the `states` key that rendered (`""` = the base design).
    pub state_key: String,
    /// v1.1: every key of `states`, for a state picker.
    pub state_keys: Vec<String>,
    /// v1.1: bound targets whose input wasn't passed (left at their static /
    /// engine value).
    pub inactive_bindings: Vec<String>,
    /// v1.2: the frame-rate cap the host should honour -- `performance.
    /// lowPower.maxFps` under low power (when set), else `performance.maxFps`;
    /// `None` = no preference (the host's own default).
    pub max_fps: Option<f64>,
    /// v1.2: materials shed because the host reported low power.
    pub disabled_materials: Vec<String>,
}

/// CSS Color 4 HSL of a color: `h` in degrees (`0` and `achromatic: true`
/// for greys, where CSS's hue is NaN), `s`/`l` in `0..1`, plus the
/// normalized 6-digit hex.
#[cfg_attr(not(target_arch = "wasm32"), derive(uniffi::Record))]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct FxHsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
    pub achromatic: bool,
    pub hex: String,
}

struct Diag(Vec<FxDiagnostic>);

impl Diag {
    fn error(&mut self, path: &str, msg: impl Into<String>) {
        self.0.push(FxDiagnostic {
            severity: "error".into(),
            path: path.into(),
            message: msg.into(),
        });
    }
    fn warn(&mut self, path: &str, msg: impl Into<String>) {
        self.0.push(FxDiagnostic {
            severity: "warning".into(),
            path: path.into(),
            message: msg.into(),
        });
    }
    /// An unknown key: an error in a file this runtime fully knows, a
    /// warning in a newer-minor file (glTF's forward-compatible minor rule).
    fn unknown(&mut self, strict: bool, path: &str, key: &str, known: &[&str]) {
        let hint = suggest(key, known)
            .map(|s| format!(" -- did you mean `{s}`?"))
            .unwrap_or_default();
        if strict {
            self.error(path, format!("unknown key `{key}`{hint}"));
        } else {
            self.warn(
                path,
                format!("unknown key `{key}` (newer 1.x spec? ignored){hint}"),
            );
        }
    }
}

fn ptr(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

/// Levenshtein distance, for "did you mean" hints.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

pub(crate) fn suggest<'a>(key: &str, known: &[&'a str]) -> Option<&'a str> {
    known
        .iter()
        .map(|k| (edit_distance(&key.to_lowercase(), &k.to_lowercase()), *k))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

// ---------------------------------------------------------------- color --

/// `kit/color.ts`'s `normalizeHex`: `#abc` / `abc` / `#AABBCC` -> `#aabbcc`.
pub fn normalize_hex(input: &str) -> Option<String> {
    let m = input.trim().trim_start_matches('#').to_lowercase();
    if !m.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match m.len() {
        3 => {
            let c: Vec<char> = m.chars().collect();
            Some(format!("#{0}{0}{1}{1}{2}{2}", c[0], c[1], c[2]))
        }
        6 => Some(format!("#{m}")),
        _ => None,
    }
}

/// CSS Color 4 `better-rgbToHsl` (the exact algorithm `kit/color.ts`
/// ports), on `0..1` channels. `h` is `None` for achromatic colors.
pub fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (Option<f64>, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let mut h = f64::NAN;
    let mut s = 0.0;
    let l = (min + max) / 2.0;
    let d = max - min;
    if d != 0.0 {
        s = if l == 0.0 || l == 1.0 {
            0.0
        } else {
            (max - l) / l.min(1.0 - l)
        };
        h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h *= 60.0;
    }
    if s < 0.0 {
        h += 180.0;
        s = s.abs();
    }
    if h >= 360.0 {
        h -= 360.0;
    }
    if s <= 1e-5 {
        h = f64::NAN;
    }
    (if h.is_nan() { None } else { Some(h) }, s, l)
}

/// CSS Color 4 `hslToRgb` -> `#rrggbb` (`kit/color.ts`'s `hslToHex`).
pub fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let hue = h.rem_euclid(360.0);
    let f = |n: f64| {
        let k = (n + hue / 30.0) % 12.0;
        let a = s * l.min(1.0 - l);
        l - a * (-1.0f64).max((k - 3.0).min(9.0 - k).min(1.0))
    };
    let to = |v: f64| (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
    format!("#{:02x}{:02x}{:02x}", to(f(0.0)), to(f(8.0)), to(f(4.0)))
}

fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
    let v = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
    (v(1), v(3), v(5))
}

/// A color's HSL from a hex string (the parity-tested entry point).
pub fn hex_to_hsl(hex: &str) -> Option<FxHsl> {
    let n = normalize_hex(hex)?;
    let (r, g, b) = hex_to_rgb(&n);
    let (h, s, l) = rgb_to_hsl(r, g, b);
    Some(FxHsl {
        h: h.unwrap_or(0.0),
        s,
        l,
        achromatic: h.is_none(),
        hex: n,
    })
}

/// JavaScript's `Math.round` (half toward +infinity), so rounded engine
/// keys equal the Studio's bit for bit -- Rust's `round` goes half away
/// from zero, which differs on negative unwrapped hues.
fn js_round(v: f64, places: i32) -> f64 {
    let p = 10f64.powi(places);
    (v * p + 0.5).floor() / p
}

/// `kit/color.ts`'s `shortestHueDelta`: signed shortest turn, in (-180, 180].
pub fn shortest_hue_delta(from: f64, to: f64) -> f64 {
    let d = ((to - from) % 360.0 + 540.0) % 360.0 - 180.0;
    if d == -180.0 {
        180.0
    } else {
        d
    }
}

/// `kit/color.ts`'s `unwrapStops`: gradient stop hues placed relative to the
/// previous one (the engine's hue ramp doesn't wrap), the short way or the
/// long way round.
pub fn unwrap_stops(hues: &[f64], long_way: bool) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(hues.len());
    for (i, h) in hues.iter().enumerate() {
        if i == 0 {
            out.push(h.rem_euclid(360.0));
            continue;
        }
        let mut d = shortest_hue_delta(out[i - 1], *h);
        if long_way && d != 0.0 {
            d = if d > 0.0 { d - 360.0 } else { d + 360.0 };
        }
        out.push(out[i - 1] + d);
    }
    out
}

/// `fx_color_to_hsl`'s entry point: a hex string, or a DTCG color object
/// as JSON. `None` if it doesn't parse; warnings (e.g. the hex fallback)
/// are dropped here -- `resolve` is where they're reported.
pub fn color_to_hsl(input: &str) -> Option<FxHsl> {
    let t = input.trim();
    let v = if t.starts_with('{') {
        serde_json::from_str(t).ok()?
    } else {
        Value::String(t.to_string())
    };
    let mut diag = Diag(Vec::new());
    let out = parse_color(&v, "", &mut diag)?;
    (!diag.0.iter().any(|d| d.severity == "error")).then_some(out)
}

/// Parse a spec color value (hex string or DTCG object) into HSL.
fn parse_color(v: &Value, path: &str, diag: &mut Diag) -> Option<FxHsl> {
    match v {
        Value::String(s) => {
            let hsl = hex_to_hsl(s);
            if hsl.is_none() {
                diag.error(path, format!("`{s}` is not a hex color (#RRGGBB or #RGB)"));
            }
            hsl
        }
        Value::Object(o) => parse_dtcg(o, path, diag),
        _ => {
            diag.error(path, "a color is a hex string or a DTCG color object");
            None
        }
    }
}

const DTCG_KEYS: [&str; 4] = ["colorSpace", "components", "alpha", "hex"];

fn parse_dtcg(o: &Map<String, Value>, path: &str, diag: &mut Diag) -> Option<FxHsl> {
    for k in o.keys() {
        if !DTCG_KEYS.contains(&k.as_str()) {
            diag.unknown(true, &ptr(path, k), k, &DTCG_KEYS);
        }
    }
    if let Some(a) = o.get("alpha").and_then(Value::as_f64) {
        if (a - 1.0).abs() > 1e-9 {
            diag.warn(
                &ptr(path, "alpha"),
                "alpha is ignored: engine colors are opaque (element alpha comes from the mode)",
            );
        }
    }
    let space = o.get("colorSpace").and_then(Value::as_str).unwrap_or("");
    let comps: Option<Vec<Option<f64>>> = o.get("components").and_then(Value::as_array).map(|a| {
        a.iter()
            .map(|c| {
                if c.as_str() == Some("none") {
                    None
                } else {
                    c.as_f64()
                }
            })
            .collect()
    });
    let fallback = |diag: &mut Diag| -> Option<FxHsl> {
        match o.get("hex").and_then(Value::as_str).and_then(hex_to_hsl) {
            Some(h) => {
                diag.warn(
                    &ptr(path, "colorSpace"),
                    format!("colorSpace `{space}` isn't converted in FX Spec v1; used the `hex` fallback"),
                );
                Some(h)
            }
            None => {
                diag.error(
                    &ptr(path, "colorSpace"),
                    format!("colorSpace `{space}` isn't supported in v1 (srgb, hsl) and there's no `hex` fallback"),
                );
                None
            }
        }
    };
    let comps = match comps {
        Some(c) if c.len() == 3 => c,
        _ if space != "srgb" && space != "hsl" => return fallback(diag),
        _ => {
            diag.error(
                &ptr(path, "components"),
                "components must be an array of 3 numbers (or \"none\")",
            );
            return None;
        }
    };
    match space {
        "srgb" => {
            let [r, g, b] =
                [comps[0], comps[1], comps[2]].map(|c| c.unwrap_or(0.0).clamp(0.0, 1.0));
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (r * 255.0 + 0.5).floor() as u8,
                (g * 255.0 + 0.5).floor() as u8,
                (b * 255.0 + 0.5).floor() as u8
            );
            Some(FxHsl {
                h: h.unwrap_or(0.0),
                s,
                l,
                achromatic: h.is_none(),
                hex,
            })
        }
        "hsl" => {
            let s = comps[1].unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
            let l = comps[2].unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
            let achromatic = comps[0].is_none() || s <= 1e-5;
            let h = comps[0].unwrap_or(0.0).rem_euclid(360.0);
            Some(FxHsl {
                h: if achromatic { 0.0 } else { h },
                s,
                l,
                achromatic,
                hex: hsl_to_hex(h, s, l),
            })
        }
        _ => fallback(diag),
    }
}

// ------------------------------------------------------------- registry --

/// Opt keys each mode reads (grepped from its `get(o, "...")` calls, plus
/// `arc::layout`'s `strokeWidth`/`gap` for the modes that share it). The
/// resolved preset's own opts are allowed on top (ported orb keys). Kept
/// in sync with the sources by `params_table_matches_mode_sources`.
pub(crate) fn mode_params(mode: &str) -> &'static [&'static str] {
    match mode {
        "aurora" => &[
            "depthTone",
            "hueOffset",
            "hueSpeed",
            "hueSpread",
            "nodeCount",
            "nodeSize",
            "saturation",
            "surfaceScale",
            "surfaceSpeed",
        ],
        "braid" => &["ghostN", "rBase", "rDepth", "strandN", "turns"],
        "chladni" => &["holdDuration", "nodeCount", "nodeSize"],
        "crystallize" => &["dotSize", "driftAmplitude", "lineWidth", "period"],
        "eclipse" => &["nodeCount", "nodeSize", "progress"],
        "globe" => &[
            "dimBase",
            "inkFar",
            "inkSpan",
            "latRings",
            "lonDensity",
            "rBase",
            "rBoost",
            "rDepth",
            "scanMul",
        ],
        "hush" => &[
            "dim",
            "hue",
            "nodeCount",
            "nodeSize",
            "period",
            "pulseAmplitude",
            "saturation",
            "yaw",
        ],
        "morph" => &["iconD", "rDot", "spread"],
        "orbits" => &[
            "ghostA",
            "ghostN",
            "ghostR",
            "orbitN",
            "partR",
            "partRDepth",
            "particles",
        ],
        "ribbon" | "ring" => &[
            "bandMul", "faceOn", "ghostN", "lanes", "rBase", "rDepth", "segs", "spin", "wobMul",
        ],
        "rubik" => &[
            "inkFar",
            "inkSpan",
            "latRings",
            "lonDensity",
            "moveCount",
            "rActive",
            "rBase",
            "rDepth",
        ],
        "sonar" => &[
            "coreSize",
            "echoCount",
            "echoSpacing",
            "period",
            "ringCount",
        ],
        "spectrum" => &[
            "barCount",
            "barDotCount",
            "dotSize",
            "hue",
            "jumpSpeed",
            "saturation",
        ],
        "warp" => &["decay", "period", "starCount", "warpSpeed"],
        "wave" => &["lonDensity", "rBase", "rDepth", "rings"],
        "web" | "webflow" => &[
            "lineW",
            "nodeN",
            "nodeR",
            "nodeRDepth",
            "signals",
            "spread",
            "thr",
        ],
        "bar" => &["barCount", "barWidth", "hue", "minHeight", "saturation"],
        "matrix" => &[
            "columnCount",
            "hue",
            "ledCount",
            "ledSize",
            "minLevel",
            "mirror",
            "saturation",
        ],
        "scroll" => &["barWidth", "fadeWidth", "hue", "minHeight", "saturation"],
        "waveform" => &[
            "amplitude",
            "hue",
            "layerCount",
            "lineWidth",
            "pointCount",
            "saturation",
        ],
        "arc" => &[
            "gap",
            "hue",
            "progress",
            "saturation",
            "strokeWidth",
            "trackOpacity",
        ],
        "gauge" => &[
            "fill",
            "gap",
            "hue",
            "marker",
            "progress",
            "saturation",
            "strokeWidth",
            "sweep",
            "trackOpacity",
        ],
        "nested" => &[
            "hue",
            "hueStep",
            "maxLaps",
            "ringCount",
            "saturation",
            "spacing",
            "strokeWidth",
            "trackOpacity",
        ],
        "segmented" => &[
            "gap",
            "hue",
            "progress",
            "saturation",
            "segmentCount",
            "strokeWidth",
            "trackOpacity",
        ],
        "spinner" => &["gap", "hue", "saturation", "strokeWidth", "trackOpacity"],
        "broadcast" => &[
            "cumulative",
            "dotSize",
            "hue",
            "inactiveOpacity",
            "level",
            "period",
            "reversing",
            "saturation",
            "sides",
            "strokeWidth",
            "waveCount",
            "waveSweep",
        ],
        "halo" => &[
            "accuracy",
            "dotSize",
            "haloOpacity",
            "hue",
            "period",
            "ringWidth",
            "saturation",
        ],
        "ping" => &[
            "dotSize",
            "hue",
            "once",
            "period",
            "ringCount",
            "ringReach",
            "ringWidth",
            "saturation",
        ],
        "pulse" => &[
            "dotSize",
            "hue",
            "period",
            "quality",
            "saturation",
            "segmentGap",
            "segmentRadius",
            "segmentWidth",
        ],
        "radar" => &[
            "blipCount",
            "dotSize",
            "hue",
            "period",
            "ringCount",
            "saturation",
            "seed",
            "trailFill",
            "trailLength",
        ],
        "dots" => &[
            "bounceAmplitude",
            "delay",
            "dotCount",
            "dotSize",
            "hue",
            "period",
            "saturation",
            "spacing",
        ],
        "shimmer" => &[
            "highlightFill",
            "highlightLength",
            "hue",
            "length",
            "period",
            "saturation",
            "thickness",
            "trackOpacity",
        ],
        _ => &[],
    }
}

/// Indexed params a mode accepts as design values (`segment0`, ...).
pub(crate) fn indexed_param(mode: &str, key: &str) -> bool {
    let idx = |prefix: &str, max: usize| {
        key.strip_prefix(prefix)
            .and_then(|n| n.parse::<usize>().ok())
            .is_some_and(|n| n < max)
    };
    match mode {
        "nested" => idx("progress", 4),
        "segmented" => idx("segment", 24),
        _ => false,
    }
}

/// Keys every mode reads through `finalize_frame` / the orb scaling.
pub(crate) const SHARED_PARAMS: [&str; 2] = ["rMin", "rsPow"];

/// Runtime inputs and cues -- live data, not design. Rejected in `params`
/// with a reason; bindings (data -> visual) arrive in FX Spec v1.1.
pub(crate) fn runtime_key(key: &str) -> Option<&'static str> {
    let indexed = |p: &str| {
        key.strip_prefix(p)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    if key.starts_with("pointer") {
        Some("pointer position is a live input (apply_pointer)")
    } else if key.starts_with("audio") {
        Some("audio levels/bands are live inputs (the voice pipeline)")
    } else if key == "voiceStateCode" {
        Some("the voice lifecycle is a live input")
    } else if key.starts_with("history") {
        Some("the scrolling history buffer is caller-owned live data")
    } else if key.starts_with("interrupt") {
        Some("the barge-in flash is a live event (apply_interrupt)")
    } else if key == "muted" || key.starts_with("mutedT") || key == "mutedHue" {
        Some("the mute cue is live app state (apply_muted)")
    } else if key.starts_with("decay") && key != "decay" {
        Some("the one-shot fade is a live event (apply_decay)")
    } else if indexed("peak") {
        Some("matrix peaks are caller-owned live data")
    } else {
        None
    }
}

/// Engine keys owned by a spec section -- in `params` they're a mistake.
fn section_owner(key: &str) -> Option<&'static str> {
    match key {
        "colorMix" | "colorHue" | "colorSaturation" | "colorLightness" | "colorMode" => {
            Some("/color")
        }
        k if k.starts_with("gradient") => Some("/gradient"),
        "glowStrength" | "glowRadius" | "glowLayers" | "glowTint" | "glowHue" => {
            Some("/materials/glow")
        }
        "noiseStrength" | "noiseAmplitude" | "noiseScale" | "noiseSpeed" | "noiseSeed" => {
            Some("/materials/noise")
        }
        "pulseStrength" | "pulsePeriod" | "pulseOpacity" | "pulseScale" | "pulsePhase" => {
            Some("/materials/pulse")
        }
        k if k.starts_with("liquid") => Some("/materials/liquid"),
        k if k.starts_with("particle") => Some("/materials/particles"),
        k if k.starts_with("holo") => Some("/materials/holographic"),
        _ => None,
    }
}

fn family_of(state: &str) -> Option<&'static str> {
    if crate::orbs::presets::resolve_preset(state, 64).is_some() {
        Some("orb")
    } else if crate::signal::presets::resolve_preset(state, 64).is_some() {
        Some("signal")
    } else if crate::ring::presets::resolve_preset(state, 64).is_some() {
        Some("ring")
    } else if crate::beacon::presets::resolve_preset(state, 64).is_some() {
        Some("beacon")
    } else if crate::core_fx::presets::resolve_preset(state, 64).is_some() {
        Some("core")
    } else {
        None
    }
}

// ------------------------------------------------------------ migration --

/// 1.7 names (the parameter catalog's `path`s) -> the engine keys the rest
/// of the resolver works in. Bindable material masters only; `progress[i]`
/// is handled by `binding_engine_key`.
const BINDING_PATHS: [(&str, &str); 5] = [
    ("glow.strength", "glowStrength"),
    ("noise.strength", "noiseStrength"),
    ("gradient.strength", "gradientStrength"),
    ("pulse.strength", "pulseStrength"),
    ("color.mix", "colorMix"),
];

/// A 1.7 binding target (`glow.strength`, `progress[1]`) -> its engine key.
fn binding_engine_key(target: &str) -> Option<String> {
    if let Some((_, k)) = BINDING_PATHS.iter().find(|(p, _)| *p == target) {
        return Some(k.to_string());
    }
    let i = target.strip_prefix("progress[")?.strip_suffix(']')?;
    (i.len() == 1 && i.as_bytes()[0].is_ascii_digit()).then(|| format!("progress{i}"))
}

/// An engine-key binding target that 1.7 names differently, with its new name.
fn binding_new_name(target: &str) -> Option<String> {
    if let Some((p, _)) = BINDING_PATHS.iter().find(|(_, k)| *k == target) {
        return Some(p.to_string());
    }
    let i = target.strip_prefix("progress")?;
    (i.len() == 1 && i.as_bytes()[0].is_ascii_digit()).then(|| format!("progress[{i}]"))
}

/// Array params (1.7): `"progress": [..]` on tracking, `"segment": [..]` on
/// stepping -> the indexed engine keys (`progress0..3`, `segment0..23`).
const ARRAY_PARAMS: [(&str, usize); 2] = [("progress", 4), ("segment", 24)];

/// Reads one block (the base or a `states` entry) into the engine names the
/// resolver works in: catalog-path binding targets (`glow.strength`,
/// `progress[0]`) -> engine keys, array params (`"progress": [..]`) -> indexed
/// keys. The names FX Spec 1.7 replaced are errors that name the replacement --
/// 1.0–1.7 were never published, so there is no alias to keep (FLOOR_MINOR).
fn migrate_block(b: &mut Map<String, Value>, prefix: &str, diag: &mut Diag) {
    let at = |k: &str| ptr(prefix, k);
    if b.remove("state").is_some() {
        diag.error(
            &at("state"),
            "`state` is `pattern` (the visual; the lifecycle key is the `states` map)",
        );
    }
    // Binding targets: dotted paths / progress[i] in, engine keys out.
    if let Some(Value::Object(bm)) = b.get_mut("bindings") {
        let keys: Vec<String> = bm.keys().cloned().collect();
        for k in keys {
            if let Some(engine) = binding_engine_key(&k) {
                if let Some(v) = bm.remove(&k) {
                    bm.insert(engine, v);
                }
            } else if let Some(new) = binding_new_name(&k) {
                diag.error(
                    &ptr(&ptr(prefix, "bindings"), &k),
                    format!("`{k}` is `{new}`"),
                );
                bm.remove(&k);
            }
        }
    }
    // Array params: expand.
    if let Some(Value::Object(pm)) = b.get_mut("params") {
        for (name, max) in ARRAY_PARAMS {
            let Some(Value::Array(items)) = pm.get(name).cloned() else {
                continue;
            };
            let path = ptr(&ptr(prefix, "params"), name);
            pm.remove(name);
            if items.is_empty() || items.len() > max {
                diag.error(&path, format!("`{name}` takes 1 to {max} values"));
                continue;
            }
            for (i, v) in items.into_iter().enumerate() {
                pm.insert(format!("{name}{i}"), v);
            }
        }
    }
}

/// Lifts a document to the names the resolver works in (`migrate_block`, for
/// the base and every `states` entry).
fn migrate(mut doc: Value, diag: &mut Diag) -> Value {
    if let Some(root) = doc.as_object_mut() {
        migrate_block(root, "", diag);
        if let Some(Value::Object(states)) = root.get_mut("states") {
            for (key, entry) in states.iter_mut() {
                if let Some(e) = entry.as_object_mut() {
                    migrate_block(e, &ptr("/states", key), diag);
                }
            }
        }
    }
    doc
}

// ------------------------------------------------------------ resolving --

const TOP_KEYS: [&str; 16] = [
    "$schema",
    "fxSpec",
    "name",
    "description",
    "object",
    "pattern",
    "size",
    "speed",
    "ink",
    "color",
    "gradient",
    "materials",
    "params",
    "bindings",
    "states",
    "performance",
];
/// The design keys of a block: the base (top level) and each `states` entry.
const ENTRY_KEYS: [&str; 8] = [
    "pattern",
    "speed",
    "ink",
    "color",
    "gradient",
    "materials",
    "params",
    "bindings",
];
const BINDING_KEYS: [&str; 4] = ["input", "inputRange", "outputRange", "curve"];
const COLOR_KEYS: [&str; 4] = ["value", "mix", "lightness", "mode"];
const GRADIENT_KEYS: [&str; 6] = ["stops", "angle", "strength", "saturation", "mid", "path"];
pub(crate) const MATERIAL_SECTIONS: [&str; 6] = [
    "glow",
    "noise",
    "pulse",
    "liquid",
    "particles",
    "holographic",
];
/// 1.5: particles' word key -> `particleStyle`.
const PARTICLE_STYLES: [&str; 4] = ["drift", "attract", "orbit", "rise"];
/// 1.4: liquid's word/boolean keys -> `liquidStyle` / `liquidKeep`.
const LIQUID_ENUM_KEYS: [&str; 2] = ["style", "keep"];

/// Every key a material section accepts: its numeric keys plus its word keys.
/// The resolver reads its allowlist from here, and so does
/// `accepted_key_paths`, so the version gate can't miss a key.
fn section_key_names(section: &str) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = material_keys(section).iter().map(|k| k.0).collect();
    match section {
        "glow" => names.extend(GLOW_ENUM_KEYS),
        "liquid" => names.extend(LIQUID_ENUM_KEYS),
        "particles" => names.push("style"),
        _ => {}
    }
    names
}

// --------------------------------------------------------- version gate --
//
// What a file may say depends on the minor it declares: a key added in 1.9 is an
// error in a file that says 1.8, and isn't honoured -- otherwise a writer could
// emit a file that claims an old version and needs a new runtime, and the old
// runtime would draw it without the feature, silently.
// Two pieces make that hold without anyone
// remembering it at the next bump:
//
// - `accepted_key_paths()` is built from the same tables the resolver accepts
//   keys with, so a new key shows up in it by itself;
// - `spec/fx-spec-1.8-keys.json` freezes what a 1.8 file may say, and a test
//   requires every accepted path to be either in it or in `SINCE` with a later
//   minor. A key added without a `SINCE` row fails that test by name.

/// Keys added after the 1.8 floor, with the minor that added them. A file that
/// declares an older minor gets an error for each and the key is dropped.
const SINCE: &[(&str, u64)] = &[];

/// Every key path a file can use, in the form `SINCE` and the gate use:
/// `ink`, `color.mode`, `materials.glow`, `materials.glow.mode`,
/// `bindings.curve` (a binding body key), `bindings:glowStrength` (a target),
/// `performance.lowPower.maxFps`, `performance.lowPower.disable:blur`.
#[cfg(test)]
pub(crate) fn accepted_key_paths() -> Vec<String> {
    let mut out: Vec<String> = TOP_KEYS
        .iter()
        .chain(ENTRY_KEYS.iter())
        .map(|k| k.to_string())
        .collect();
    out.extend(COLOR_KEYS.iter().map(|k| format!("color.{k}")));
    out.extend(GRADIENT_KEYS.iter().map(|k| format!("gradient.{k}")));
    out.extend(BINDING_KEYS.iter().map(|k| format!("bindings.{k}")));
    out.extend(
        crate::reactive::REACTIVE_TARGETS
            .iter()
            .map(|t| format!("bindings:{}", t.name)),
    );
    for s in MATERIAL_SECTIONS {
        out.push(format!("materials.{s}"));
        out.extend(
            section_key_names(s)
                .iter()
                .map(|k| format!("materials.{s}.{k}")),
        );
    }
    out.extend(PERFORMANCE_KEYS.iter().map(|k| format!("performance.{k}")));
    out.extend(
        LOW_POWER_KEYS
            .iter()
            .map(|k| format!("performance.lowPower.{k}")),
    );
    out.extend(
        SHEDDABLE
            .iter()
            .map(|m| format!("performance.lowPower.disable:{m}")),
    );
    out.sort();
    out.dedup();
    out
}

/// Checks one object's keys: (object, its JSON Pointer, key -> gate path).
type GateCheck<'a> = dyn FnMut(&mut Map<String, Value>, &str, &dyn Fn(&str) -> String) + 'a;

/// The gate: drops, with an error, every key `since` says is newer than `minor`.
/// Runs on the migrated document (engine-key binding targets), before resolving.
fn version_gate(root: &mut Map<String, Value>, minor: u64, since: &[(&str, u64)], diag: &mut Diag) {
    if since.is_empty() {
        return;
    }
    let newer = |path: &str| {
        since
            .iter()
            .find(|(p, m)| *p == path && *m > minor)
            .map(|(_, m)| *m)
    };
    let mut check =
        |obj: &mut Map<String, Value>, ptr_prefix: &str, key_path: &dyn Fn(&str) -> String| {
            let keys: Vec<String> = obj.keys().cloned().collect();
            for k in keys {
                if let Some(m) = newer(&key_path(&k)) {
                    diag.error(
                        &ptr(ptr_prefix, &k),
                        format!(
                            "`{}` needs \"fxSpec\": \"1.{m}\" (this file says 1.{minor})",
                            key_path(&k)
                        ),
                    );
                    obj.remove(&k);
                }
            }
        };
    fn block(b: &mut Map<String, Value>, prefix: &str, check: &mut GateCheck) {
        check(b, prefix, &|k: &str| k.to_string());
        for (sec, sub) in [("color", "color"), ("gradient", "gradient")] {
            if let Some(Value::Object(o)) = b.get_mut(sec) {
                check(o, &ptr(prefix, sec), &|k: &str| format!("{sub}.{k}"));
            }
        }
        if let Some(Value::Object(m)) = b.get_mut("materials") {
            let mp = ptr(prefix, "materials");
            check(m, &mp, &|k: &str| format!("materials.{k}"));
            for (sec, body) in m.iter_mut() {
                if let Value::Object(o) = body {
                    let sec = sec.clone();
                    check(o, &ptr(&mp, &sec), &|k: &str| {
                        format!("materials.{sec}.{k}")
                    });
                }
            }
        }
        if let Some(Value::Object(bs)) = b.get_mut("bindings") {
            let bp = ptr(prefix, "bindings");
            check(bs, &bp, &|k: &str| format!("bindings:{k}"));
            for (t, body) in bs.iter_mut() {
                if let Value::Object(o) = body {
                    check(o, &ptr(&bp, t), &|k: &str| format!("bindings.{k}"));
                }
            }
        }
    }
    block(root, "", &mut check);
    if let Some(Value::Object(states)) = root.get_mut("states") {
        for (key, entry) in states.iter_mut() {
            if let Value::Object(e) = entry {
                block(e, &ptr("/states", key), &mut check);
            }
        }
    }
    if let Some(Value::Object(perf)) = root.get_mut("performance") {
        check(perf, "/performance", &|k: &str| format!("performance.{k}"));
        if let Some(Value::Object(lp)) = perf.get_mut("lowPower") {
            check(lp, "/performance/lowPower", &|k: &str| {
                format!("performance.lowPower.{k}")
            });
            if let Some(Value::Array(d)) = lp.get_mut("disable") {
                let mut i = 0;
                d.retain(|v| {
                    let keep = match v.as_str().and_then(|m| newer(&format!("performance.lowPower.disable:{m}"))) {
                        Some(need) => {
                            diag.error(
                                &format!("/performance/lowPower/disable/{i}"),
                                format!("shedding `{}` needs \"fxSpec\": \"1.{need}\" (this file says 1.{minor})", v.as_str().unwrap()),
                            );
                            false
                        }
                        None => true,
                    };
                    i += 1;
                    keep
                });
            }
        }
    }
}

/// (spec key, engine key, min, max) per material.
pub(crate) fn material_keys(section: &str) -> &'static [(&'static str, &'static str, f64, f64)] {
    match section {
        "glow" => &[
            ("strength", "glowStrength", 0.0, 1.0),
            ("radius", "glowRadius", 1.0, 8.0),
            ("layers", "glowLayers", 1.0, 8.0),
            ("tint", "glowTint", 0.0, 1.0),
            ("hue", "glowHue", 0.0, 360.0),
        ],
        "noise" => &[
            ("strength", "noiseStrength", 0.0, 1.0),
            ("amplitude", "noiseAmplitude", 0.0, 1.0),
            ("scale", "noiseScale", 0.0, 64.0),
            ("speed", "noiseSpeed", 0.0, 16.0),
            ("seed", "noiseSeed", -1e9, 1e9),
        ],
        "pulse" => &[
            ("strength", "pulseStrength", 0.0, 1.0),
            ("period", "pulsePeriod", 0.05, 60.0),
            ("opacity", "pulseOpacity", 0.0, 1.0),
            ("scale", "pulseScale", 0.0, 1.0),
            ("phase", "pulsePhase", -1e9, 1e9),
        ],
        "liquid" => &[
            ("strength", "liquidStrength", 0.0, 1.0),
            ("reach", "liquidReach", 1.0, 12.0),
            ("threshold", "liquidThreshold", 0.05, 4.0),
            ("cells", "liquidCells", 8.0, 96.0),
            ("spacing", "liquidSpacing", 0.5, 12.0),
            ("width", "liquidWidth", 0.05, 8.0),
            ("blur", "liquidBlur", 0.0, 32.0),
        ],
        "particles" => &[
            ("strength", "particleStrength", 0.0, 1.0),
            ("count", "particleCount", 0.0, 200.0),
            ("size", "particleSize", 0.05, 4.0),
            ("spread", "particleSpread", 0.0, 1.0),
            ("life", "particleLife", 0.1, 30.0),
            ("seed", "particleSeed", -1e9, 1e9),
            // 1.6: burst sync and audio coupling.
            ("sync", "particleSync", 0.0, 1.0),
            ("audio", "particleAudio", 0.0, 1.0),
        ],
        // 1.6: holographic-lite, a hue sweep over the kept lightness.
        "holographic" => &[
            ("strength", "holoStrength", 0.0, 1.0),
            ("hue", "holoHue", 0.0, 360.0),
            ("span", "holoSpan", 0.0, 720.0),
            ("saturation", "holoSaturation", 0.0, 1.0),
            ("depth", "holoDepth", 0.0, 1.0),
            ("facing", "holoFacing", 0.0, 1.0),
            ("speed", "holoSpeed", -4.0, 4.0),
        ],
        _ => &[],
    }
}

fn number(v: &Value, path: &str, lo: f64, hi: f64, diag: &mut Diag) -> Option<f64> {
    match v.as_f64() {
        Some(x) if x.is_finite() && (lo..=hi).contains(&x) => Some(x),
        Some(x) => {
            diag.error(path, format!("{x} is out of range [{lo}, {hi}]"));
            None
        }
        None => {
            diag.error(path, "expected a number");
            None
        }
    }
}

fn as_object<'a>(v: &'a Value, path: &str, diag: &mut Diag) -> Option<&'a Map<String, Value>> {
    let o = v.as_object();
    if o.is_none() {
        diag.error(path, "expected an object");
    }
    o
}

/// One resolved design block (the base, or a `states` entry merged over
/// it): what `resolve_block` hands back besides its diagnostics.
struct Block {
    state: String,
    speed: f64,
    /// Whether this block wrote `speed` itself. The voice-state profile's
    /// speed multiplier applies only when it didn't (the author wins).
    speed_set: bool,
    overrides: BTreeMap<String, f64>,
    bindings: Vec<(String, String, crate::reactive::Mapper)>,
}

/// Validate and resolve one design block -- the v1.0 resolver, unchanged
/// in behaviour, plus `bindings`. `prefix` is the block's JSON Pointer
/// (`""` for the base, `/states/<key>` for an entry).
fn resolve_block(
    b: &Map<String, Value>,
    prefix: &str,
    strict: bool,
    object: Option<&str>,
    size: u32,
    diag: &mut Diag,
) -> Block {
    let at = |k: &str| ptr(prefix, k);
    let mut out: BTreeMap<String, f64> = BTreeMap::new();
    // Whether `colorMix` was *explicitly* set to a non-zero amount. A bare
    // `value` (implicit mix 1) or `mix: 0` only supplies the colour a
    // `colorMix` binding mixes toward -- not a conflicting static amount.
    let mut static_color_mix = false;

    // The pattern (the look; the object and size are file-wide).
    let state = b.get("pattern").and_then(Value::as_str).unwrap_or("");
    let family = family_of(state);
    if state.is_empty() {
        diag.error(&at("pattern"), "missing `pattern`");
    } else if family.is_none() {
        diag.error(&at("pattern"), format!("unknown pattern `{state}`"));
    }
    if let (Some(o), Some(f)) = (object, family) {
        if o != f {
            // The base reports at `/object`; an entry points at its own `pattern`.
            let path = if prefix.is_empty() {
                "/object".to_string()
            } else {
                at("pattern")
            };
            let article = if f.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            diag.error(
                &path,
                format!("`{state}` is {article} {f} pattern; this file's object is `{o}`"),
            );
        }
    }
    let speed = match b.get("speed") {
        None => 1.0,
        Some(v) => number(v, &at("speed"), 0.0, 100.0, diag).unwrap_or(1.0),
    };
    // 1.8: `ink` is the whole visual's opacity (`primitives::apply_ink`) --
    // a design key like `speed`, so it inherits from the base and a state
    // patches it (`null` in a patch drops back to the base / full ink).
    if let Some(v) = b.get("ink") {
        if let Some(x) = number(v, &at("ink"), 0.0, 1.0, diag) {
            out.insert("ink".to_string(), x);
        }
    }
    let resolved = if family.is_some() {
        crate::resolved_opts(state.to_string(), size)
    } else {
        None
    };
    let mode = resolved
        .as_ref()
        .map(|r| r.mode.clone())
        .unwrap_or_default();

    // Color.
    if let Some(c) = b.get("color") {
        let cp = at("color");
        let (value, mix, lightness, cmode) = match c {
            Value::String(_) => (Some(c), Some(1.0), None, None),
            Value::Object(o) => {
                for k in o.keys() {
                    if !COLOR_KEYS.contains(&k.as_str()) {
                        diag.unknown(strict, &ptr(&cp, k), k, &COLOR_KEYS);
                    }
                }
                let mix = match (o.get("mix"), o.get("value")) {
                    (Some(m), _) => {
                        let v = number(m, &ptr(&cp, "mix"), 0.0, 1.0, diag);
                        static_color_mix = v.is_some_and(|x| x != 0.0);
                        v
                    }
                    (None, Some(_)) => Some(1.0),
                    (None, None) => None,
                };
                let lightness = o
                    .get("lightness")
                    .and_then(|l| number(l, &ptr(&cp, "lightness"), -1.0, 1.0, diag));
                (o.get("value"), mix, lightness, o.get("mode"))
            }
            _ => {
                diag.error(
                    &cp,
                    "`color` is a hex string or an object {value, mix, lightness, mode}",
                );
                (None, None, None, None)
            }
        };
        if let Some(v) = value {
            let path = if c.is_string() {
                cp.clone()
            } else {
                ptr(&cp, "value")
            };
            if let Some(hsl) = parse_color(v, &path, diag) {
                out.insert("colorHue".into(), js_round(hsl.h, 1));
                out.insert("colorSaturation".into(), js_round(hsl.s, 3));
            }
        }
        if let Some(m) = mix {
            if value.is_some() {
                out.insert("colorMix".into(), m);
            } else {
                diag.error(&ptr(&cp, "mix"), "`mix` needs a `value` color");
            }
        }
        if let Some(l) = lightness {
            if l != 0.0 {
                out.insert("colorLightness".into(), l);
            }
        }
        if let Some(m) = cmode {
            match m.as_str() {
                Some("ink") => {
                    out.insert("colorMode".into(), 0.0);
                }
                Some("fixed") => {
                    out.insert("colorMode".into(), 1.0);
                }
                _ => diag.error(
                    &ptr(&cp, "mode"),
                    "mode is \"ink\" (follows the theme) or \"fixed\" (same in both)",
                ),
            }
        }
    }

    // Gradient.
    if let Some(g) = b.get("gradient") {
        let gp = at("gradient");
        if let Some(o) = as_object(g, &gp, diag) {
            for k in o.keys() {
                if !GRADIENT_KEYS.contains(&k.as_str()) {
                    diag.unknown(strict, &ptr(&gp, k), k, &GRADIENT_KEYS);
                }
            }
            let long_way = match o.get("path").map(|p| p.as_str()) {
                None | Some(Some("short")) => false,
                Some(Some("long")) => true,
                _ => {
                    diag.error(&ptr(&gp, "path"), "path is \"short\" or \"long\"");
                    false
                }
            };
            match o.get("stops").and_then(Value::as_array) {
                Some(stops) if (2..=3).contains(&stops.len()) => {
                    let hues: Vec<Option<f64>> = stops
                        .iter()
                        .enumerate()
                        .map(|(i, s)| parse_color(s, &format!("{gp}/stops/{i}"), diag).map(|h| h.h))
                        .collect();
                    if hues.iter().all(Option::is_some) {
                        let hs: Vec<f64> = hues.into_iter().map(Option::unwrap).collect();
                        let un = unwrap_stops(&hs, long_way);
                        for (i, key) in ["gradientHue", "gradientHue2", "gradientHue3"]
                            .iter()
                            .enumerate()
                            .take(un.len())
                        {
                            out.insert((*key).into(), js_round(un[i], 1));
                        }
                    }
                }
                _ => diag.error(&ptr(&gp, "stops"), "stops is an array of 2 or 3 colors"),
            }
            let strength = o.get("strength").map_or(Some(1.0), |v| {
                number(v, &ptr(&gp, "strength"), 0.0, 1.0, diag)
            });
            if let Some(s) = strength {
                out.insert("gradientStrength".into(), s);
            }
            for (k, e, lo, hi) in [
                ("angle", "gradientAngle", -360.0, 720.0),
                ("saturation", "gradientSaturation", 0.0, 1.0),
                ("mid", "gradientMid", 0.05, 0.95),
            ] {
                if let Some(v) = o.get(k).and_then(|v| number(v, &ptr(&gp, k), lo, hi, diag)) {
                    out.insert(e.into(), v);
                }
            }
        }
    }

    // Materials.
    if let Some(m) = b.get("materials") {
        let mp = at("materials");
        if let Some(o) = as_object(m, &mp, diag) {
            for (section, body) in o {
                let path = ptr(&mp, section);
                // The catalog lists color and gradient among its eight materials, but
                // in a file they're top-level sections: say where they go instead of
                // a bare "unknown key".
                if section == "color" || section == "gradient" {
                    diag.error(
                        &path,
                        format!("`{section}` is a top-level section in an FX Spec file: move it to `/{section}`"),
                    );
                    continue;
                }
                if !MATERIAL_SECTIONS.contains(&section.as_str()) {
                    diag.unknown(strict, &path, section, &MATERIAL_SECTIONS);
                    continue;
                }
                let Some(bo) = as_object(body, &path, diag) else {
                    continue;
                };
                let keys = material_keys(section);
                let names = section_key_names(section);
                for (k, v) in bo {
                    // 1.5: particles' style is a word.
                    if section == "particles" && k == "style" {
                        match v
                            .as_str()
                            .and_then(|w| PARTICLE_STYLES.iter().position(|x| *x == w))
                        {
                            Some(i) => {
                                out.insert("particleStyle".into(), i as f64);
                            }
                            None => diag.error(
                                &ptr(&path, k),
                                format!("`style` is one of {}", PARTICLE_STYLES.join(", ")),
                            ),
                        }
                        continue;
                    }
                    // 1.4: liquid's style is a word, keep a boolean.
                    if section == "liquid" && LIQUID_ENUM_KEYS.contains(&k.as_str()) {
                        let kp = ptr(&path, k);
                        if k == "style" {
                            match v.as_str() {
                                Some("fill") => {
                                    out.insert("liquidStyle".into(), 0.0);
                                }
                                Some("outline") => {
                                    out.insert("liquidStyle".into(), 1.0);
                                }
                                Some("dots") => {
                                    out.insert("liquidStyle".into(), 2.0);
                                }
                                _ => {
                                    diag.error(&kp, "`style` is \"outline\", \"dots\" or \"fill\"")
                                }
                            }
                        } else {
                            match v.as_bool() {
                                Some(b) => {
                                    out.insert("liquidKeep".into(), if b { 1.0 } else { 0.0 });
                                }
                                None => diag.error(&kp, "`keep` is true or false"),
                            }
                        }
                        continue;
                    }
                    // 1.3: glow's paint mode and blend are words, not numbers.
                    if section == "glow" && GLOW_ENUM_KEYS.contains(&k.as_str()) {
                        let kp = ptr(&path, k);
                        let (engine, words): (&str, [&str; 2]) = if k == "mode" {
                            ("glowMode", ["stacked", "blur"])
                        } else {
                            ("glowBlend", ["normal", "additive"])
                        };
                        match v.as_str().and_then(|w| words.iter().position(|x| *x == w)) {
                            Some(i) => {
                                out.insert(engine.into(), i as f64);
                            }
                            None => diag.error(
                                &kp,
                                format!("`{k}` is \"{}\" or \"{}\"", words[0], words[1]),
                            ),
                        }
                        continue;
                    }
                    match keys.iter().find(|x| x.0 == k) {
                        Some((_, engine, lo, hi)) => {
                            if let Some(x) = number(v, &ptr(&path, k), *lo, *hi, diag) {
                                out.insert((*engine).into(), x);
                            }
                        }
                        None => diag.unknown(strict, &ptr(&path, k), k, &names),
                    }
                }
            }
        }
    }

    // Params.
    if let Some(p) = b.get("params") {
        let pp = at("params");
        if let Some(o) = as_object(p, &pp, diag) {
            let preset_keys: Vec<String> = resolved
                .as_ref()
                .map(|r| r.opts.keys().cloned().collect())
                .unwrap_or_default();
            let mut allowed: Vec<&str> = mode_params(&mode).to_vec();
            allowed.extend(SHARED_PARAMS);
            allowed.extend(preset_keys.iter().map(String::as_str));
            for (k, v) in o {
                let path = ptr(&pp, k);
                let key: &str = k;
                if let Some(owner) = section_owner(key) {
                    diag.error(
                        &path,
                        format!("`{key}` belongs in `{owner}`, not in params"),
                    );
                    continue;
                }
                if let Some(why) = runtime_key(key) {
                    let hint = if crate::reactive::target(key).is_some() {
                        format!("; bind it under `bindings.{key}` instead")
                    } else {
                        String::new()
                    };
                    diag.error(
                        &path,
                        format!("`{key}` is a runtime input, not design: {why}{hint}"),
                    );
                    continue;
                }
                if family.is_some() && !allowed.contains(&key) && !indexed_param(&mode, key) {
                    diag.unknown(strict, &path, key, &allowed);
                    continue;
                }
                if let Some(x) = number(v, &path, -1e9, 1e9, diag) {
                    out.insert(key.into(), x);
                }
            }
        }
    }

    // Bindings (v1.1): app input -> one curated Reactive target.
    let mut bindings = Vec::new();
    if let Some(bv) = b.get("bindings") {
        let bp = at("bindings");
        if let Some(o) = as_object(bv, &bp, diag) {
            let targets: Vec<&str> = crate::reactive::REACTIVE_TARGETS
                .iter()
                .map(|t| t.name)
                .collect();
            // Suggestions use the file's names (`glow.strength`), not the engine keys.
            let shown: Vec<String> = targets
                .iter()
                .map(|t| binding_new_name(t).unwrap_or_else(|| t.to_string()))
                .collect();
            let shown_refs: Vec<&str> = shown.iter().map(String::as_str).collect();
            for (target, body) in o {
                let path = ptr(&bp, target);
                if crate::reactive::target(target).is_none() {
                    let hint = suggest(target, &shown_refs)
                        .map(|s| format!(" -- did you mean `{s}`?"))
                        .unwrap_or_default();
                    diag.error(
                        &path,
                        format!(
                            "`{target}` isn't a bindable target; bindings drive only the curated Reactive Inputs ({}){hint}",
                            shown.join(", ")
                        ),
                    );
                    continue;
                }
                let Some(bo) = as_object(body, &path, diag) else {
                    continue;
                };
                for k in bo.keys() {
                    if !BINDING_KEYS.contains(&k.as_str()) {
                        diag.unknown(strict, &ptr(&path, k), k, &BINDING_KEYS);
                    }
                }
                let input = match bo.get("input").and_then(Value::as_str) {
                    Some(i) if !i.trim().is_empty() => i.to_string(),
                    _ => {
                        diag.error(
                            &ptr(&path, "input"),
                            "`input` is the app's own input name (a non-empty string)",
                        );
                        continue;
                    }
                };
                let range = |k: &str, diag: &mut Diag| -> Result<Option<Vec<f64>>, ()> {
                    match bo.get(k) {
                        None => Ok(None),
                        Some(Value::Array(a)) if a.iter().all(Value::is_number) => {
                            Ok(Some(a.iter().filter_map(Value::as_f64).collect()))
                        }
                        Some(_) => {
                            diag.error(&ptr(&path, k), "expected an array of numbers");
                            Err(())
                        }
                    }
                };
                let (Ok(inr), Ok(outr)) = (range("inputRange", diag), range("outputRange", diag))
                else {
                    continue;
                };
                let curves: Vec<String> = match bo.get("curve") {
                    None => Vec::new(),
                    Some(Value::String(c)) => vec![c.clone()],
                    Some(Value::Array(a)) if a.iter().all(Value::is_string) => a
                        .iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect(),
                    Some(_) => {
                        diag.error(
                            &ptr(&path, "curve"),
                            format!(
                                "curve is one of {} or an array of them (one per segment)",
                                crate::reactive::CURVES.join(", ")
                            ),
                        );
                        continue;
                    }
                };
                match crate::reactive::compile(target, inr.as_deref(), outr.as_deref(), &curves) {
                    Ok(m) => {
                        let mode_specific = matches!(target.as_str(), "quality" | "accuracy")
                            || target.starts_with("progress");
                        if mode_specific
                            && !mode.is_empty()
                            && !mode_params(&mode).contains(&target.as_str())
                            && !indexed_param(&mode, target)
                        {
                            diag.warn(
                                &path,
                                format!("`{state}` (mode `{mode}`) doesn't read `{target}`; this binding has no visible effect"),
                            );
                        }
                        let neutral = target == "colorMix" && !static_color_mix;
                        if out.contains_key(target.as_str()) && !neutral {
                            diag.warn(
                                &path,
                                format!("`{target}` is also set statically in this block; the binding wins while its input is present"),
                            );
                        }
                        bindings.push((target.clone(), input, m));
                    }
                    Err(e) => diag.error(&path, e),
                }
            }
        }
    }

    Block {
        state: state.to_string(),
        speed,
        speed_set: b.contains_key("speed"),
        overrides: out,
        bindings,
    }
}

const PERFORMANCE_KEYS: [&str; 2] = ["maxFps", "lowPower"];
const LOW_POWER_KEYS: [&str; 2] = ["maxFps", "disable"];
/// The materials a low-power host may shed (their `*Strength` goes to 0).
const SHEDDABLE: [&str; 7] = [
    "glow",
    "noise",
    "pulse",
    "gradient",
    "blur",
    "liquid",
    "particles",
];
/// 1.3: glow's word-valued keys (`mode`: stacked | blur, `blend`: normal |
/// additive) -> `glowMode` / `glowBlend`.
const GLOW_ENUM_KEYS: [&str; 2] = ["mode", "blend"];

#[derive(Default)]
struct Performance {
    max_fps: Option<f64>,
    low_power_fps: Option<f64>,
    disable: Vec<String>,
}

/// The 1.2 `performance` block (file-wide). The engine doesn't pace frames
/// or detect power state -- the host does both (iOS Low Power Mode, Android
/// Battery Saver, or an app policy) and passes `low_power`; the spec only
/// declares the cap and what to shed.
fn performance(v: &Value, strict: bool, diag: &mut Diag) -> Performance {
    let mut out = Performance::default();
    let Some(o) = as_object(v, "/performance", diag) else {
        return out;
    };
    for k in o.keys() {
        if !PERFORMANCE_KEYS.contains(&k.as_str()) {
            diag.unknown(strict, &ptr("/performance", k), k, &PERFORMANCE_KEYS);
        }
    }
    out.max_fps = o
        .get("maxFps")
        .and_then(|f| number(f, "/performance/maxFps", 1.0, 120.0, diag));
    if let Some(lp) = o.get("lowPower") {
        if let Some(lo) = as_object(lp, "/performance/lowPower", diag) {
            for k in lo.keys() {
                if !LOW_POWER_KEYS.contains(&k.as_str()) {
                    diag.unknown(strict, &ptr("/performance/lowPower", k), k, &LOW_POWER_KEYS);
                }
            }
            out.low_power_fps = lo
                .get("maxFps")
                .and_then(|f| number(f, "/performance/lowPower/maxFps", 1.0, 120.0, diag));
            if let Some(d) = lo.get("disable") {
                match d.as_array() {
                    Some(a) => {
                        for (i, m) in a.iter().enumerate() {
                            let path = format!("/performance/lowPower/disable/{i}");
                            match m.as_str() {
                                Some("holographic") => diag.error(
                                    &path,
                                    "`holographic` only recolours (no elements, no blur), so there is nothing to shed",
                                ),
                                Some(m) if SHEDDABLE.contains(&m) => {
                                    if !out.disable.iter().any(|x| x == m) {
                                        out.disable.push(m.to_string());
                                    }
                                }
                                Some(m) => {
                                    let hint = suggest(m, &SHEDDABLE)
                                        .map(|s| format!(" -- did you mean `{s}`?"))
                                        .unwrap_or_default();
                                    diag.error(
                                        &path,
                                        format!(
                                            "`{m}` can't be shed; one of {}{hint}",
                                            SHEDDABLE.join(", ")
                                        ),
                                    );
                                }
                                None => diag.error(&path, "expected a material name"),
                            }
                        }
                    }
                    None => diag.error(
                        "/performance/lowPower/disable",
                        "expected an array of material names",
                    ),
                }
            }
            if let (Some(lp), Some(mx)) = (out.low_power_fps, out.max_fps) {
                if lp > mx {
                    diag.warn(
                        "/performance/lowPower/maxFps",
                        format!("{lp} is above the normal cap ({mx}); low power should cap lower"),
                    );
                }
            }
        }
    }
    out
}

/// RFC 7396 JSON Merge Patch: objects merge recursively, `null` deletes,
/// anything else (arrays included) replaces the target whole.
fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let t = target.as_object_mut().unwrap();
    for (k, v) in p {
        if v.is_null() {
            t.remove(k);
        } else {
            merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}

/// Parse, migrate, validate and resolve an FX Spec document -- the v1.0
/// entry point: the base design, no inputs.
pub fn resolve(json: &str) -> FxSpecResolved {
    resolve_full(json, None, &HashMap::new(), false)
}

/// `resolve_full` with the host at normal power (the tests' shorthand).
#[cfg(test)]
pub fn resolve_with(
    json: &str,
    state_key: Option<&str>,
    inputs: &HashMap<String, f64>,
) -> FxSpecResolved {
    resolve_full(json, state_key, inputs, false)
}

/// Resolve an FX Spec for the caller's current lifecycle `state` (a key of
/// `states`; `None` or an unknown key renders the base design) and app
/// `inputs` (drive `bindings`; a binding whose input is absent is inactive
/// and listed in `inactive_bindings`). The engine stays stateless: easing
/// and cross-fades are the caller's (docs/fx-spec.md, *Caller loop*).
pub fn resolve_full(
    json: &str,
    state_key: Option<&str>,
    inputs: &HashMap<String, f64>,
    low_power: bool,
) -> FxSpecResolved {
    let mut diag = Diag(Vec::new());
    let mut res = FxSpecResolved {
        ok: false,
        state: String::new(),
        size: 64,
        speed: 1.0,
        overrides: HashMap::new(),
        diagnostics: Vec::new(),
        state_key: String::new(),
        state_keys: Vec::new(),
        inactive_bindings: Vec::new(),
        max_fps: None,
        disabled_materials: Vec::new(),
    };
    let doc: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => {
            diag.error("", format!("not valid JSON: {e}"));
            res.diagnostics = diag.0;
            return res;
        }
    };
    let Some(root) = doc.as_object() else {
        diag.error("", "an FX Spec is a JSON object");
        res.diagnostics = diag.0;
        return res;
    };

    // Version (glTF-style major.minor).
    // A missing or malformed version is reported and the rest read as the
    // current minor, so the file's other problems still show.
    let minor = match root.get("fxSpec").and_then(Value::as_str) {
        None => {
            diag.error(
                "/fxSpec",
                format!("missing `fxSpec` version (e.g. \"1.{RUNTIME_MINOR}\")"),
            );
            RUNTIME_MINOR
        }
        Some(v) => {
            let parts: Vec<Option<u64>> = v.split('.').map(|p| p.parse().ok()).collect();
            match parts.as_slice() {
                [Some(major), Some(minor)] if *major == RUNTIME_MAJOR && *minor < FLOOR_MINOR => {
                    diag.error(
                        "/fxSpec",
                        format!(
                            "FX Spec {major}.{minor} isn't supported; this runtime reads 1.{FLOOR_MINOR} and later"
                        ),
                    );
                    res.diagnostics = diag.0;
                    return res;
                }
                [Some(major), Some(minor)] if *major == RUNTIME_MAJOR => *minor,
                [Some(major), Some(_)] => {
                    diag.error(
                        "/fxSpec",
                        format!("FX Spec {major}.x isn't supported by this runtime (1.x)"),
                    );
                    res.diagnostics = diag.0;
                    return res;
                }
                _ => {
                    diag.error("/fxSpec", format!("`{v}` isn't a \"major.minor\" version"));
                    RUNTIME_MINOR
                }
            }
        }
    };
    let strict = minor <= RUNTIME_MINOR;
    let mut doc = migrate(doc.clone(), &mut diag);
    let root = doc.as_object_mut().unwrap();
    version_gate(root, minor, SINCE, &mut diag);

    for k in root.keys() {
        if !TOP_KEYS.contains(&k.as_str()) {
            diag.unknown(strict, &ptr("", k), k, &TOP_KEYS);
        }
    }
    for k in ["name", "description", "$schema"] {
        if let Some(v) = root.get(k) {
            if !v.is_string() {
                diag.error(&ptr("", k), "expected a string");
            }
        }
    }

    // File-wide: object and size.
    let object = root.get("object").and_then(Value::as_str);
    match object {
        None => diag.error(
            "/object",
            "missing `object` (orb, signal, ring, beacon or core)",
        ),
        Some(o) if !["orb", "signal", "ring", "beacon", "core"].contains(&o) => {
            diag.error("/object", format!("unknown object `{o}`"))
        }
        _ => {}
    }
    let size = match root.get("size") {
        None => 64,
        Some(v) => match v.as_u64() {
            Some(s) if SIZES.contains(&(s as u32)) => s as u32,
            _ => {
                diag.error("/size", "size must be 20, 32 or 64");
                64
            }
        },
    };

    // The base design block, then every `states` entry merged over it.
    let mut base = Map::new();
    for k in ENTRY_KEYS {
        if let Some(v) = root.get(k) {
            base.insert(k.to_string(), v.clone());
        }
    }
    let base_block = resolve_block(&base, "", strict, object, size, &mut diag);
    // Each entry, with the engine keys its patch removed from the base design
    // (a `null` for a key or a whole section): the voice-state profile below
    // must not fill those back in.
    let mut entries: Vec<(String, Block, Vec<String>)> = Vec::new();
    if let Some(sv) = root.get("states") {
        if let Some(states) = as_object(sv, "/states", &mut diag) {
            for (key, entry) in states {
                let prefix = ptr("/states", key);
                let Some(eo) = as_object(entry, &prefix, &mut diag) else {
                    continue;
                };
                for k in eo.keys() {
                    if k == "object" || k == "size" || k == "performance" {
                        diag.error(
                            &ptr(&prefix, k),
                            format!("`{k}` is file-wide; set it at the top level"),
                        );
                    } else if !ENTRY_KEYS.contains(&k.as_str()) {
                        diag.unknown(strict, &ptr(&prefix, k), k, &ENTRY_KEYS);
                    }
                }
                let mut merged = Value::Object(base.clone());
                let mut patch = entry.clone();
                if let Some(p) = patch.as_object_mut() {
                    p.remove("object");
                    p.remove("size");
                }
                merge_patch(&mut merged, &patch);
                // Validate the merged block, keeping only what the entry
                // itself wrote: inherited keys were checked on the base, and
                // an inherited param the entry's mode doesn't read is simply
                // not applied there.
                let mut local = Diag(Vec::new());
                let block = resolve_block(
                    merged.as_object().unwrap(),
                    &prefix,
                    strict,
                    object,
                    size,
                    &mut local,
                );
                for d in local.0 {
                    let rest = &d.path[prefix.len()..];
                    let own = rest.is_empty()
                        || entry.pointer(rest).is_some()
                        || (rest == "/pattern" && eo.contains_key("pattern"));
                    if own {
                        diag.0.push(d);
                    }
                }
                let removed = base_block
                    .overrides
                    .keys()
                    .filter(|k| !block.overrides.contains_key(*k))
                    .cloned()
                    .collect();
                entries.push((key.clone(), block, removed));
            }
        }
    }

    // Which block renders: the requested entry, else the base.
    let (key, block, removed) = match state_key {
        Some(k) => match entries.iter().position(|(ek, _, _)| ek == k) {
            Some(i) => {
                let (_, b, r) = entries.swap_remove(i);
                (k.to_string(), b, r)
            }
            None => {
                if !entries.is_empty() || root.contains_key("states") {
                    diag.warn(
                        "/states",
                        format!("state `{k}` isn't in `states`; rendering the base design"),
                    );
                }
                (String::new(), base_block, Vec::new())
            }
        },
        None => (String::new(), base_block, Vec::new()),
    };
    res.state_keys = root
        .get("states")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    let mut out = block.overrides;
    let mut speed = block.speed;
    // v1.8: the built-in voice-state profile. For a `states` key that is one
    // of the agent states, the engine knows what that state does to any
    // pattern (`voice_state.rs`) -- so a file patches the language instead
    // of restating it. **Fill-only**: every key the file set, in the base or
    // in the entry, wins; a `null` in the patch removed the key and opts it
    // out here too. Runs before the bindings below, whose companions also
    // `or_insert`, so a profile `audioStrength` (negative while listening)
    // survives the `audioLevel` companion's positive default.
    if !key.is_empty() {
        if let Some(p) = crate::voice_state::profile(&block.state, &key) {
            for (k, v) in p.overrides {
                if !removed.contains(&k) {
                    out.entry(k).or_insert(v);
                }
            }
            if !block.speed_set {
                speed *= p.speed;
            }
        }
    }
    for (target, input, mapper) in &block.bindings {
        match inputs.get(input) {
            Some(v) => {
                let t = crate::reactive::target(target).unwrap();
                for (c, dv) in t.companions {
                    out.entry((*c).to_string()).or_insert(*dv);
                }
                out.insert(target.clone(), mapper.map(*v));
            }
            None => res.inactive_bindings.push(target.clone()),
        }
    }

    // Performance (1.2): the effective frame cap, and under low power the
    // materials the spec sheds -- forced off *after* bindings, so a bound
    // `glowStrength` is shed too (and isn't reported as an inactive binding).
    let perf = root
        .get("performance")
        .map(|p| performance(p, strict, &mut diag))
        .unwrap_or_default();
    res.max_fps = match (low_power, perf.low_power_fps) {
        (true, Some(f)) => Some(f),
        _ => perf.max_fps,
    };
    if low_power {
        for m in &perf.disable {
            if m == "blur" {
                // Every blur sigma to 0; glow's blur mode falls back to its
                // stacked copies (`apply_glow`, `apply_blur_scale`).
                out.insert("blurScale".into(), 0.0);
                continue;
            }
            // The engine key is singular for particles (`particleStrength`).
            let key = if m == "particles" {
                "particleStrength".to_string()
            } else {
                format!("{m}Strength")
            };
            if out.contains_key(&key) {
                out.insert(key.clone(), 0.0);
            }
            res.inactive_bindings.retain(|t| *t != key);
        }
        res.disabled_materials = perf.disable;
    }

    res.ok = !diag.0.iter().any(|d| d.severity == "error");
    res.state = block.state;
    res.size = size;
    res.speed = speed;
    res.overrides = out.into_iter().collect();
    res.diagnostics = diag.0;
    res.state_key = key;
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn errors(r: &FxSpecResolved) -> Vec<&FxDiagnostic> {
        r.diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .collect()
    }

    fn warnings(r: &FxSpecResolved) -> Vec<&FxDiagnostic> {
        r.diagnostics
            .iter()
            .filter(|d| d.severity == "warning")
            .collect()
    }

    #[test]
    fn a_full_spec_resolves_to_engine_keys() {
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "size": 64, "speed": 1.5,
              "color": { "value": "#00FFB2", "mix": 1, "lightness": 0.3, "mode": "fixed" },
              "gradient": { "stops": ["#6E56CF", "#E54666"], "angle": 45, "strength": 0.5 },
              "materials": { "glow": { "strength": 0.6 } },
              "params": { "orbitN": 10 } }"##,
        );
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!((r.state.as_str(), r.size, r.speed), ("working", 64, 1.5));
        let o = &r.overrides;
        assert_eq!(o["colorMix"], 1.0);
        assert_eq!(o["colorMode"], 1.0);
        assert_eq!(o["colorLightness"], 0.3);
        assert_eq!(
            o["colorHue"], 161.9,
            "#00FFB2 -> h 161.88 rounded like the Studio"
        );
        assert_eq!(o["colorSaturation"], 1.0);
        assert_eq!(o["gradientStrength"], 0.5);
        assert_eq!(o["gradientAngle"], 45.0);
        assert_eq!(o["glowStrength"], 0.6);
        assert_eq!(o["orbitN"], 10.0, "ported orb keys come from the preset");
        assert!(o.contains_key("gradientHue") && o.contains_key("gradientHue2"));
    }

    #[test]
    fn shorthand_color_and_defaults() {
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "ring", "pattern": "completing", "color": "#e54666" }"##,
        );
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(r.size, 64);
        assert_eq!(r.overrides["colorMix"], 1.0);
        assert!(!r.overrides.contains_key("colorMode"));
    }

    #[test]
    fn typos_are_errors_with_a_hint_and_a_path() {
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "colr": "#fff", "params": { "orbitn": 3 } }"##,
        );
        assert!(!r.ok);
        let e = errors(&r);
        assert!(e
            .iter()
            .any(|d| d.path == "/colr" && d.message.contains("did you mean `color`")));
        assert!(e
            .iter()
            .any(|d| d.path == "/params/orbitn" && d.message.contains("`orbitN`")));
    }

    #[test]
    fn version_rules_follow_gltf() {
        let two = resolve(r##"{ "fxSpec": "2.0", "object": "orb", "pattern": "working" }"##);
        assert!(!two.ok && errors(&two)[0].path == "/fxSpec");
        // A missing version is an error, and the rest is still read as the
        // current minor, so the file's other problems show too.
        let missing = resolve(r##"{ "object": "orb", "pattern": "wroking" }"##);
        assert!(!missing.ok);
        assert_eq!(errors(&missing)[0].path, "/fxSpec");
        assert!(errors(&missing).iter().any(|d| d.path == "/pattern"));
        // A newer 1.x file: unknown keys are warnings, the rest renders.
        let newer = resolve(
            r##"{ "fxSpec": "1.9", "object": "orb", "pattern": "working", "timeline": {}, "layers": [] }"##,
        );
        assert!(newer.ok, "{:?}", newer.diagnostics);
        assert_eq!(warnings(&newer).len(), 2);
        // Unknown keys in a file this runtime fully knows are errors.
        let typo = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "stattes": {} }"##,
        );
        assert!(errors(&typo).iter().any(|d| d.message.contains("`states`")));
    }

    #[test]
    fn the_floor_is_1_8() {
        // 1.0-1.7 were never published: one error, nothing resolved, whatever
        // the rest of the file says (release decision 0.1).
        for v in ["1.0", "1.7"] {
            let r = resolve(&format!(
                r##"{{ "fxSpec": "{v}", "object": "orb", "state": "working", "params": {{ "orbitN": 3 }} }}"##
            ));
            assert!(!r.ok, "{v}");
            assert_eq!(r.diagnostics.len(), 1, "{v}: {:?}", r.diagnostics);
            assert_eq!(r.diagnostics[0].path, "/fxSpec");
            assert!(
                r.diagnostics[0].message.contains("reads 1.8 and later"),
                "{:?}",
                r.diagnostics
            );
            assert!(r.overrides.is_empty(), "{v}");
        }
    }

    #[test]
    fn names_1_7_replaced_are_errors_that_name_the_replacement() {
        // No alias survives the floor: the old name is an error that says the
        // new one, in the base and in an entry, and it isn't honoured.
        let r = resolve_with(
            r##"{ "fxSpec": "1.8", "object": "orb", "state": "working",
                  "bindings": { "glowStrength": { "input": "x" } },
                  "states": { "a": { "state": "glowing" } } }"##,
            None,
            &HashMap::from([("x".to_string(), 1.0)]),
        );
        assert!(!r.ok);
        let e = errors(&r);
        assert!(e
            .iter()
            .any(|d| d.path == "/state" && d.message.contains("`pattern`")));
        assert!(e
            .iter()
            .any(|d| d.path == "/states/a/state" && d.message.contains("`pattern`")));
        assert!(e
            .iter()
            .any(|d| d.path == "/bindings/glowStrength" && d.message.contains("`glow.strength`")));
        assert!(!r.overrides.contains_key("glowStrength"));
        // The taxonomy retrofit's per-mode renames are gone too: aurora's old
        // `nodeN` is an unknown key, not a silent `nodeCount`...
        let aurora = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing", "params": { "nodeN": 120 } }"##,
        );
        assert!(errors(&aurora).iter().any(|d| d.path == "/params/nodeN"));
        assert!(!aurora.overrides.contains_key("nodeCount"));
        // ...while `web` (ported) still reads its own, current nodeN.
        let web = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "connecting", "params": { "nodeN": 30 } }"##,
        );
        assert!(web.ok && web.overrides["nodeN"] == 30.0 && web.diagnostics.is_empty());
    }

    #[test]
    fn object_state_and_size_are_checked() {
        let r = resolve(r##"{ "fxSpec": "1.8", "object": "ring", "pattern": "working" }"##);
        assert!(errors(&r)
            .iter()
            .any(|d| d.path == "/object" && d.message.contains("an orb pattern")));
        let r = resolve(r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "wroking" }"##);
        assert!(errors(&r).iter().any(|d| d.path == "/pattern"));
        let r =
            resolve(r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "size": 48 }"##);
        assert!(errors(&r).iter().any(|d| d.path == "/size"));
    }

    #[test]
    fn runtime_and_section_keys_are_rejected_in_params() {
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "signal", "pattern": "signaling",
              "params": { "audioLevel": 0.5, "pointerX": 3, "colorHue": 10, "glowStrength": 1, "barCount": 12 } }"##,
        );
        let e = errors(&r);
        assert!(e
            .iter()
            .any(|d| d.path == "/params/audioLevel" && d.message.contains("runtime input")));
        assert!(e.iter().any(|d| d.path == "/params/pointerX"));
        assert!(e
            .iter()
            .any(|d| d.path == "/params/colorHue" && d.message.contains("/color")));
        assert!(e
            .iter()
            .any(|d| d.path == "/params/glowStrength" && d.message.contains("/materials/glow")));
        assert_eq!(r.overrides["barCount"], 12.0);
    }

    #[test]
    fn dtcg_colors() {
        let srgb = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "color": { "value": { "colorSpace": "srgb", "components": [1, 0, 1], "hex": "#ff00ff" } } }"##,
        );
        assert!(srgb.ok && srgb.overrides["colorHue"] == 300.0);
        let hsl = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "color": { "value": { "colorSpace": "hsl", "components": [200, 80, 50] } } }"##,
        );
        assert!(
            hsl.ok && hsl.overrides["colorHue"] == 200.0 && hsl.overrides["colorSaturation"] == 0.8
        );
        let oklch = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "color": { "value": { "colorSpace": "oklch", "components": [0.7, 0.1, 30], "hex": "#e54666" } } }"##,
        );
        assert!(oklch.ok, "hex fallback");
        assert!(warnings(&oklch)
            .iter()
            .any(|d| d.message.contains("fallback")));
        let bare = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "color": { "value": { "colorSpace": "oklch", "components": [0.7, 0.1, 30] } } }"##,
        );
        assert!(!bare.ok);
        let grey = hex_to_hsl("#808080").unwrap();
        assert!(grey.achromatic && grey.h == 0.0 && grey.s == 0.0);
    }

    #[test]
    fn hue_helpers_match_the_studio() {
        assert_eq!(shortest_hue_delta(350.0, 10.0), 20.0);
        assert_eq!(shortest_hue_delta(0.0, 180.0), 180.0);
        assert_eq!(unwrap_stops(&[300.0, 40.0], false), vec![300.0, 400.0]);
        assert_eq!(unwrap_stops(&[300.0, 40.0], true), vec![300.0, 40.0]);
        assert_eq!(js_round(-10.05, 1), -10.0, "JS Math.round semantics");
        assert_eq!(hsl_to_hex(200.0, 0.8, 0.5), "#19a1e6");
    }

    #[test]
    fn params_table_matches_mode_sources() {
        // Re-scan the mode files so the allowlist can't drift from what the
        // modes actually read.
        let sources: &[(&str, &str)] = &[
            ("aurora", include_str!("orbs/modes/aurora.rs")),
            ("spectrum", include_str!("orbs/modes/spectrum.rs")),
            ("sonar", include_str!("orbs/modes/sonar.rs")),
            ("warp", include_str!("orbs/modes/warp.rs")),
            ("chladni", include_str!("orbs/modes/chladni.rs")),
            ("eclipse", include_str!("orbs/modes/eclipse.rs")),
            ("crystallize", include_str!("orbs/modes/crystallize.rs")),
            ("hush", include_str!("orbs/modes/hush.rs")),
            ("bar", include_str!("signal/modes/bar.rs")),
            ("waveform", include_str!("signal/modes/waveform.rs")),
            ("scroll", include_str!("signal/modes/scroll.rs")),
            ("matrix", include_str!("signal/modes/matrix.rs")),
            ("arc", include_str!("ring/modes/arc.rs")),
            ("gauge", include_str!("ring/modes/gauge.rs")),
            ("nested", include_str!("ring/modes/nested.rs")),
            ("segmented", include_str!("ring/modes/segmented.rs")),
            ("spinner", include_str!("ring/modes/spinner.rs")),
            ("ping", include_str!("beacon/modes/ping.rs")),
            ("pulse", include_str!("beacon/modes/pulse.rs")),
            ("halo", include_str!("beacon/modes/halo.rs")),
            ("radar", include_str!("beacon/modes/radar.rs")),
            ("broadcast", include_str!("beacon/modes/broadcast.rs")),
            ("shimmer", include_str!("core_fx/modes/shimmer.rs")),
            ("dots", include_str!("core_fx/modes/dots.rs")),
        ];
        for (mode, src) in sources {
            let code = src.split("#[cfg(test)]").next().unwrap();
            for part in code.split("get(o, \"").skip(1) {
                let key = part.split('"').next().unwrap();
                if runtime_key(key).is_some()
                    || matches!(key, "rMin" | "rsPow" | "audioBandCount" | "voiceStateCode")
                {
                    continue;
                }
                assert!(
                    mode_params(mode).contains(&key),
                    "{mode} reads `{key}` but the FX Spec params table doesn't list it"
                );
            }
        }
    }

    const EXAMPLES: [(&str, &str); 13] = [
        (
            "voice-assistant-glowing",
            include_str!("../../../spec/examples/voice-assistant-glowing.fxspec.json"),
        ),
        (
            "orb-brand-fixed",
            include_str!("../../../spec/examples/orb-brand-fixed.fxspec.json"),
        ),
        (
            "signal-matrix",
            include_str!("../../../spec/examples/signal-matrix.fxspec.json"),
        ),
        (
            "ring-tracking-gradient",
            include_str!("../../../spec/examples/ring-tracking-gradient.fxspec.json"),
        ),
        (
            "beacon-radar-glow",
            include_str!("../../../spec/examples/beacon-radar-glow.fxspec.json"),
        ),
        (
            "voice-assistant",
            include_str!("../../../spec/examples/voice-assistant.fxspec.json"),
        ),
        (
            "fitness-rings",
            include_str!("../../../spec/examples/fitness-rings.fxspec.json"),
        ),
        (
            "status-beacon-power",
            include_str!("../../../spec/examples/status-beacon-power.fxspec.json"),
        ),
        (
            "shimmer-gradient",
            include_str!("../../../spec/examples/shimmer-gradient.fxspec.json"),
        ),
        (
            "radar-wedge-blur",
            include_str!("../../../spec/examples/radar-wedge-blur.fxspec.json"),
        ),
        (
            "liquid-orb",
            include_str!("../../../spec/examples/liquid-orb.fxspec.json"),
        ),
        (
            "particles-orb",
            include_str!("../../../spec/examples/particles-orb.fxspec.json"),
        ),
        (
            "holo-orb",
            include_str!("../../../spec/examples/holo-orb.fxspec.json"),
        ),
    ];

    /// The fixed app inputs the parity tests pass (every platform uses the
    /// same map; unused names are ignored).
    fn test_inputs() -> HashMap<String, f64> {
        [
            ("micMuted", 0.0),
            ("micLevel", 0.6),
            ("agentVolume", 0.5),
            ("steps", 6200.0),
            ("waterMl", 1800.0),
            ("activeMinutes", 12.0),
            ("heartRate", 128.0),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }

    #[test]
    fn examples_resolve_and_render_like_overrides() {
        for (name, json) in EXAMPLES {
            let base = resolve(json);
            let mut keys: Vec<Option<String>> = vec![None];
            keys.extend(base.state_keys.iter().cloned().map(Some));
            for key in keys {
                let r = resolve_with(json, key.as_deref(), &test_inputs());
                assert!(r.ok, "{name} {key:?}: {:?}", r.diagnostics);
                assert!(
                    warnings(&r).is_empty(),
                    "{name} {key:?}: {:?}",
                    r.diagnostics
                );
                assert!(
                    r.inactive_bindings.is_empty(),
                    "{name} {key:?}: all inputs passed"
                );
                let preset = crate::resolved_opts(r.state.clone(), r.size).unwrap().speed;
                let got = crate::frame_from_fx_spec_with(
                    json.to_string(),
                    1.3,
                    key.clone(),
                    test_inputs(),
                    false,
                )
                .unwrap();
                let want = crate::frame_with_overrides(
                    r.state.clone(),
                    r.size,
                    1.3 * preset * r.speed,
                    r.overrides.clone(),
                )
                .unwrap();
                assert_eq!(got, want, "{name} {key:?}");
            }
        }
    }

    #[test]
    fn v1_8_fills_a_voice_state_from_the_profile_without_overruling_the_file() {
        // The state language ships with the engine: a 1.8 file naming a
        // voice state gets `ink`, the audio direction and the tempo without
        // restating them -- but anything the file writes wins.
        let json = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
              "states": { "idle": {}, "listening": { "ink": 0.5 },
                          "speaking": {}, "recording": {} } }"##;
        let idle = resolve_with(json, Some("idle"), &HashMap::new());
        assert!(idle.ok, "{:?}", idle.diagnostics);
        assert_eq!(idle.overrides["ink"], 0.8, "the profile fills idle's ink");
        assert!(idle.speed < 1.0, "and idle's slower tempo");

        let listening = resolve_with(json, Some("listening"), &HashMap::new());
        assert_eq!(listening.overrides["ink"], 0.5, "the file's own ink wins");
        assert!(
            listening.overrides["audioStrength"] < 0.0,
            "listening draws inward"
        );
        assert!(
            resolve_with(json, Some("speaking"), &HashMap::new()).overrides["audioStrength"] > 0.0,
            "speaking swells outward"
        );

        // A key outside the voice language is left alone entirely.
        let own = resolve_with(json, Some("recording"), &HashMap::new());
        assert!(!own.overrides.contains_key("ink"));
        assert_eq!(own.speed, 1.0);
    }

    #[test]
    fn the_profile_does_not_reach_the_base_design() {
        // A voice state's profile applies to that state's entry only; the
        // base design (no state key) never gets one.
        let json = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
              "states": { "idle": {} } }"##;
        let base = resolve(json);
        assert!(!base.overrides.contains_key("ink"), "the base is untouched");
        assert_eq!(base.speed, 1.0);
    }

    #[test]
    fn a_speed_the_file_sets_beats_the_profiles_multiplier() {
        let json = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
              "states": { "idle": { "speed": 2 } } }"##;
        assert_eq!(resolve_with(json, Some("idle"), &HashMap::new()).speed, 2.0);
    }

    #[test]
    fn v1_8_examples_resolve_identically() {
        // The lock for the runtime that ships, and since FLOOR_MINOR the only
        // one: 1.8 is the oldest minor this runtime reads, so there are no older
        // wordings left to hold (release decision 0.1). All 13
        // examples declare 1.8. Captured once 1.8 was stable;
        // a later minor adds its own.
        resolves_like_snapshot(include_str!("../../../spec/fx-spec-1.8-resolved.json"), 13);
    }

    const HOLO: &str = include_str!("../../../spec/examples/holo-orb.fxspec.json");

    #[test]
    fn catalog_path_targets_and_array_params_resolve_to_engine_keys() {
        let x = HashMap::from([("x".to_string(), 0.5)]);
        let r = resolve_with(
            r##"{ "fxSpec": "1.8", "object": "ring", "pattern": "tracking",
                  "params": { "progress": [0.2] },
                  "bindings": { "glow.strength": { "input": "x" }, "progress[1]": { "input": "x" } } }"##,
            None,
            &x,
        );
        assert!(r.ok && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        assert_eq!(r.overrides["glowStrength"], 0.5);
        assert_eq!(r.overrides["progress0"], 0.2);
        assert_eq!(r.overrides["progress1"], 0.5);
        let long = resolve(
            r##"{ "fxSpec": "1.8", "object": "ring", "pattern": "tracking", "params": { "progress": [1, 1, 1, 1, 1] } }"##,
        );
        assert!(!long.ok && errors(&long)[0].message.contains("1 to 4 values"));
    }

    #[test]
    fn v1_6_holographic_material() {
        let none = HashMap::new();
        let r = resolve_full(HOLO, None, &none, false);
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(
            (
                r.overrides["holoStrength"],
                r.overrides["holoHue"],
                r.overrides["holoSpeed"]
            ),
            (1.0, 200.0, 0.08)
        );
        // Holographic keeps geometry and lightness: same frame minus hue/sat.
        let f =
            crate::frame_from_fx_spec_with(HOLO.into(), 1.0, None, none.clone(), false).unwrap();
        let off = HOLO.replacen("\"strength\": 1", "\"strength\": 0", 1);
        let plain = crate::frame_from_fx_spec_with(off, 1.0, None, none.clone(), false).unwrap();
        assert_eq!(f.dots.len(), plain.dots.len());
        for (a, b) in f.dots.iter().zip(&plain.dots) {
            assert_eq!((a.x, a.y, a.r, a.white, a.a), (b.x, b.y, b.r, b.white, b.a));
        }
        assert!(f.dots.iter().any(|d| d.saturation > 0.0));
        // States merge; low power sheds the glow but keeps the holographic.
        let sp = resolve_full(HOLO, Some("speaking"), &none, true);
        assert_eq!(
            (sp.overrides["holoFacing"], sp.overrides["holoStrength"]),
            (0.8, 1.0)
        );
        assert_eq!(sp.overrides["glowStrength"], 0.0);
        // Can't-shed, validation, and holo keys don't belong in params.
        // 1.6 particle keys resolve.
        let p = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "speaking",
                  "materials": { "particles": { "strength": 1, "sync": 0.5, "audio": 1 } } }"##;
        let ok = resolve(p);
        assert!(ok.ok, "{:?}", ok.diagnostics);
        assert_eq!(
            (ok.overrides["particleSync"], ok.overrides["particleAudio"]),
            (0.5, 1.0)
        );
        let bad = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
                  "materials": { "holographic": { "span": 900, "hue2": 3 } },
                  "params": { "holoHue": 30 },
                  "performance": { "lowPower": { "disable": ["holographic"] } } }"##,
        );
        let e = errors(&bad);
        assert!(e.iter().any(|d| d.path == "/materials/holographic/span"));
        assert!(e
            .iter()
            .any(|d| d.path == "/materials/holographic/hue2" && d.message.contains("`hue`")));
        assert!(e
            .iter()
            .any(|d| d.path == "/params/holoHue" && d.message.contains("/materials/holographic")));
        assert!(e.iter().any(|d| d.path == "/performance/lowPower/disable/0"
            && d.message.contains("nothing to shed")));
    }

    const PARTICLES: &str = include_str!("../../../spec/examples/particles-orb.fxspec.json");

    #[test]
    fn v1_5_particles_material_and_shedding() {
        let none = HashMap::new();
        let r = resolve_full(PARTICLES, None, &none, false);
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(
            (
                r.overrides["particleStrength"],
                r.overrides["particleCount"],
                r.overrides["particleStyle"]
            ),
            (1.0, 32.0, 0.0)
        );
        // Same spec with particles shed (low power) has exactly the orb's own dots.
        let f = crate::frame_from_fx_spec_with(PARTICLES.into(), 1.0, None, none.clone(), false)
            .unwrap();
        let shed = crate::frame_from_fx_spec_with(PARTICLES.into(), 1.0, None, none.clone(), true)
            .unwrap();
        assert!(f.dots.len() > shed.dots.len(), "particles add dots");
        assert_eq!(
            &f.dots[..shed.dots.len()],
            &shed.dots[..],
            "the orb's own dots are untouched"
        );
        let l = resolve_full(PARTICLES, Some("listening"), &none, false);
        assert_eq!(
            (l.overrides["particleStyle"], l.overrides["particleSpread"]),
            (1.0, 0.3)
        );
        // Low power sheds particles (and blur): strength 0.
        let low = resolve_full(PARTICLES, Some("speaking"), &none, true);
        assert_eq!(low.overrides["particleStrength"], 0.0);
        assert_eq!(low.overrides["blurScale"], 0.0);
        assert_eq!(low.disabled_materials, ["particles", "blur"]);
        // Validation, and particle keys don't belong in params.
        let bad = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
                  "materials": { "particles": { "style": "swirl", "cuont": 3 } },
                  "params": { "particleCount": 3 } }"##,
        );
        let e = errors(&bad);
        assert!(e.iter().any(|d| d.path == "/materials/particles/style"));
        assert!(e
            .iter()
            .any(|d| d.path == "/materials/particles/cuont" && d.message.contains("`count`")));
        assert!(e.iter().any(
            |d| d.path == "/params/particleCount" && d.message.contains("/materials/particles")
        ));
    }

    fn resolves_like_snapshot(snapshot: &str, count: usize) {
        let snap: Value = serde_json::from_str(snapshot).unwrap();
        let inputs: HashMap<String, f64> = serde_json::from_value(snap["inputs"].clone()).unwrap();
        let examples = snap["examples"].as_object().unwrap();
        assert_eq!(examples.len(), count);
        for (file, rows) in examples {
            let (_, json) = EXAMPLES
                .iter()
                .find(|(n, _)| format!("{n}.fxspec.json") == *file)
                .unwrap();
            for (key, want) in rows.as_object().unwrap() {
                let (st, low) = match key.strip_suffix("|lowPower") {
                    Some(k) => (k, true),
                    None => (key.as_str(), false),
                };
                let st = (!st.is_empty()).then_some(st);
                let r = resolve_full(json, st, &inputs, low);
                let label = format!("{file} {key:?}");
                let got: BTreeMap<String, f64> = r.overrides.clone().into_iter().collect();
                let exp: BTreeMap<String, f64> =
                    serde_json::from_value(want["overrides"].clone()).unwrap();
                assert_eq!(got, exp, "{label}");
                assert_eq!(json!(r.ok), want["ok"], "{label}");
                assert_eq!(json!(r.state), want["state"], "{label}");
                assert_eq!(r.speed, want["speed"].as_f64().unwrap(), "{label}");
                let diags: Vec<Value> = r
                    .diagnostics
                    .iter()
                    .map(
                        |d| json!({ "severity": d.severity, "path": d.path, "message": d.message }),
                    )
                    .collect();
                assert_eq!(json!(diags), want["diagnostics"], "{label}");
                assert_eq!(json!(r.state_key), want["stateKey"], "{label}");
                assert_eq!(
                    json!(r.inactive_bindings),
                    want["inactiveBindings"],
                    "{label}"
                );
                assert_eq!(r.max_fps, want["maxFps"].as_f64(), "{label}");
                assert_eq!(
                    json!(r.disabled_materials),
                    want["disabledMaterials"],
                    "{label}"
                );
            }
        }
    }

    const LIQUID: &str = include_str!("../../../spec/examples/liquid-orb.fxspec.json");

    #[test]
    fn v1_4_liquid_material_and_shedding() {
        let none = HashMap::new();
        let r = resolve_full(LIQUID, None, &none, false);
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(
            (r.overrides["liquidStrength"], r.overrides["liquidStyle"]),
            (1.0, 1.0)
        );
        let f =
            crate::frame_from_fx_spec_with(LIQUID.into(), 1.0, None, none.clone(), false).unwrap();
        assert!(
            !f.polylines.is_empty() && f.dots.is_empty(),
            "outline contours replace the dots"
        );
        // The speaking state: a liquid ring -- a filled band with a hole,
        // source dots kept on top.
        let sp = resolve_full(LIQUID, Some("speaking"), &none, false);
        assert_eq!(
            (sp.overrides["liquidStyle"], sp.overrides["liquidKeep"]),
            (0.0, 1.0)
        );
        let sf = crate::frame_from_fx_spec_with(
            LIQUID.into(),
            1.0,
            Some("speaking".into()),
            none.clone(),
            false,
        )
        .unwrap();
        assert!(
            sf.fills.iter().any(|x| !x.holes.is_empty()),
            "a band with a hole"
        );
        assert!(!sf.dots.is_empty());
        // Low power sheds liquid: plain dots, 24 fps.
        let low = resolve_full(LIQUID, None, &none, true);
        assert_eq!(low.overrides["liquidStrength"], 0.0);
        assert_eq!(
            (low.max_fps, low.disabled_materials.clone()),
            (Some(24.0), vec!["liquid".to_string()])
        );
        let lf =
            crate::frame_from_fx_spec_with(LIQUID.into(), 1.0, None, none.clone(), true).unwrap();
        assert!(lf.polylines.is_empty() && !lf.dots.is_empty());
        // Validation, and liquid keys don't belong in params.
        let bad = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "glowing",
                  "materials": { "liquid": { "style": "goo", "keep": 1, "reech": 2 } },
                  "params": { "liquidReach": 3 } }"##,
        );
        let e = errors(&bad);
        assert!(e.iter().any(|d| d.path == "/materials/liquid/style"));
        assert!(e.iter().any(|d| d.path == "/materials/liquid/keep"));
        assert!(e
            .iter()
            .any(|d| d.path == "/materials/liquid/reech" && d.message.contains("`reach`")));
        assert!(e
            .iter()
            .any(|d| d.path == "/params/liquidReach" && d.message.contains("/materials/liquid")));
    }

    const MATERIALS: &str = include_str!("../../../spec/examples/radar-wedge-blur.fxspec.json");

    #[test]
    fn v1_3_glow_mode_blend_and_blur_shedding() {
        let none = HashMap::new();
        let r = resolve_full(MATERIALS, None, &none, false);
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(
            (r.overrides["glowMode"], r.overrides["glowBlend"]),
            (1.0, 1.0)
        );
        assert_eq!(r.overrides["trailFill"], 1.0);
        assert!(!r.overrides.contains_key("blurScale"));
        let f = crate::frame_from_fx_spec_with(MATERIALS.into(), 1.0, None, none.clone(), false)
            .unwrap();
        assert_eq!(f.fills.len(), 1, "the wedge");
        assert!(f.effects.iter().any(|e| e.blur > 0.0 && e.blend == 1));
        // Low power sheds blur: blurScale 0, glow back to stacked copies,
        // no blur left anywhere; the wedge (no blur) stays.
        let low = resolve_full(MATERIALS, None, &none, true);
        assert_eq!(low.overrides["blurScale"], 0.0);
        assert_eq!(low.max_fps, Some(20.0));
        assert_eq!(low.disabled_materials, ["blur"]);
        let lf = crate::frame_from_fx_spec_with(MATERIALS.into(), 1.0, None, none.clone(), true)
            .unwrap();
        assert!(lf.effects.is_empty(), "stacked glow: no effect runs left");
        assert_eq!(lf.fills.len(), 1);
        assert!(lf.fills.iter().all(|x| x.blur == 0.0));
        let bad = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
                  "materials": { "glow": { "mode": "soft", "blend": 1 } } }"##,
        );
        assert_eq!(errors(&bad).len(), 2);
    }

    const POWER: &str = include_str!("../../../spec/examples/status-beacon-power.fxspec.json");

    #[test]
    fn performance_caps_fps_and_sheds_materials_under_low_power() {
        let none = HashMap::new();
        let normal = resolve_full(POWER, None, &none, false);
        assert!(normal.ok, "{:?}", normal.diagnostics);
        assert_eq!(normal.max_fps, Some(30.0));
        assert!(normal.disabled_materials.is_empty());
        assert_eq!(normal.overrides["glowStrength"], 0.6);
        let low = resolve_full(POWER, None, &none, true);
        assert_eq!(low.max_fps, Some(15.0));
        assert_eq!(low.disabled_materials, ["glow", "noise"]);
        assert_eq!(
            (
                low.overrides["glowStrength"],
                low.overrides["noiseStrength"]
            ),
            (0.0, 0.0)
        );
        assert_eq!(
            low.overrides["glowRadius"], 3.0,
            "only the master strength is shed"
        );
        // Shed after bindings: a bound glow goes too, and isn't "inactive".
        let bound = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "bindings": { "glow.strength": { "input": "hr" }, "noise.strength": { "input": "n" } },
            "performance": { "lowPower": { "disable": ["glow", "noise"] } } }"##;
        let r = resolve_full(bound, None, &HashMap::from([("hr".to_string(), 1.0)]), true);
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(r.overrides["glowStrength"], 0.0);
        assert!(r.inactive_bindings.is_empty(), "{:?}", r.inactive_bindings);
        assert_eq!(r.max_fps, None, "no maxFps anywhere = host default");
        // lowPower.maxFps absent -> the normal cap still applies.
        let only = r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "performance": { "maxFps": 24, "lowPower": { "disable": ["glow"] } } }"##;
        assert_eq!(resolve_full(only, None, &none, true).max_fps, Some(24.0));
        // Low-power frame really has no glow copies.
        let lf =
            crate::frame_from_fx_spec_with(POWER.into(), 1.0, None, none.clone(), true).unwrap();
        let nf =
            crate::frame_from_fx_spec_with(POWER.into(), 1.0, None, none.clone(), false).unwrap();
        assert!(lf.dots.len() < nf.dots.len());
    }

    #[test]
    fn performance_is_validated() {
        let e = |s: &str| {
            let r = resolve(s);
            r.diagnostics
                .into_iter()
                .filter(|d| d.severity == "error")
                .map(|d| (d.path, d.message))
                .collect::<Vec<_>>()
        };
        let bad = e(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "performance": { "maxFps": 500, "lowPower": { "disable": ["glwo", "dots"], "maxfs": 10 } } }"##,
        );
        assert!(bad.iter().any(|(p, _)| p == "/performance/maxFps"));
        assert!(bad
            .iter()
            .any(|(p, m)| p == "/performance/lowPower/disable/0" && m.contains("`glow`")));
        assert!(bad
            .iter()
            .any(|(p, _)| p == "/performance/lowPower/disable/1"));
        assert!(bad
            .iter()
            .any(|(p, m)| p == "/performance/lowPower/maxfs" && m.contains("`maxFps`")));
        let entry = e(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "states": { "a": { "performance": { "maxFps": 10 } } } }"##,
        );
        assert!(entry
            .iter()
            .any(|(p, m)| p == "/states/a/performance" && m.contains("file-wide")));
        let up = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working",
            "performance": { "maxFps": 20, "lowPower": { "maxFps": 30 } } }"##,
        );
        assert!(
            up.ok
                && up.diagnostics[0]
                    .message
                    .contains("low power should cap lower")
        );
    }

    const VOICE: &str = include_str!("../../../spec/examples/voice-assistant.fxspec.json");
    const FIT: &str = include_str!("../../../spec/examples/fitness-rings.fxspec.json");

    #[test]
    fn states_merge_over_the_base_per_rfc_7396() {
        let none = HashMap::new();
        let base = resolve_with(VOICE, None, &none);
        assert_eq!(
            (base.state.as_str(), base.state_key.as_str()),
            ("breathing", "")
        );
        assert_eq!(base.overrides["glowStrength"], 0.25);
        assert_eq!(base.state_keys.len(), 5);
        // Entry values win; untouched base keys are inherited.
        let sp = resolve_with(VOICE, Some("speaking"), &none);
        assert_eq!(
            (sp.state.as_str(), sp.state_key.as_str(), sp.speed),
            ("speaking", "speaking", 1.1)
        );
        assert_eq!(sp.overrides["glowStrength"], 0.4);
        assert_eq!(
            sp.overrides["glowRadius"], 3.0,
            "inherited from the base glow"
        );
        assert_eq!(sp.overrides["colorMix"], 0.7);
        // `null` deletes: no glow at all while thinking -- and the 1.8 voice
        // profile, which has glow for thinking, doesn't put it back (a `null`
        // opts the key out of the profile; docs/fx-spec.md).
        let th = resolve_with(VOICE, Some("thinking"), &none);
        assert_eq!(th.state, "working");
        assert!(!th.overrides.keys().any(|k| k.starts_with("glow")));
        assert!(crate::voice_state::profile("working", "thinking")
            .unwrap()
            .overrides
            .contains_key("glowStrength"));
        // An empty entry is the base under a named key, plus what the voice
        // profile fills in for that key -- never over a base value.
        let idle = resolve_with(VOICE, Some("idle"), &none);
        assert_eq!(idle.state_key, "idle");
        let prof = crate::voice_state::profile(&idle.state, "idle").unwrap();
        for (k, v) in &idle.overrides {
            match base.overrides.get(k) {
                Some(b) => assert_eq!(v, b, "{k}: the base value is kept"),
                None => assert_eq!(Some(v), prof.overrides.get(k), "{k}: from the profile"),
            }
        }
        assert!(base
            .overrides
            .keys()
            .all(|k| idle.overrides.contains_key(k)));
    }

    #[test]
    fn unknown_or_missing_state_falls_back_to_the_base() {
        let none = HashMap::new();
        let r = resolve_with(VOICE, Some("dreaming"), &none);
        assert!(r.ok);
        assert_eq!((r.state.as_str(), r.state_key.as_str()), ("breathing", ""));
        assert!(warnings(&r)
            .iter()
            .any(|d| d.path == "/states" && d.message.contains("`dreaming`")));
        // A file without `states` ignores a requested state silently.
        let plain = resolve_with(
            include_str!("../../../spec/examples/orb-brand-fixed.fxspec.json"),
            Some("speaking"),
            &none,
        );
        assert!(plain.ok && plain.diagnostics.is_empty() && plain.state == "working");
    }

    #[test]
    fn bindings_map_inputs_and_report_missing_ones() {
        let r = resolve_with(FIT, None, &HashMap::from([("steps".to_string(), 5000.0)]));
        assert!(r.ok, "{:?}", r.diagnostics);
        let want = crate::reactive::compile(
            "progress0",
            Some(&[0.0, 10000.0, 30000.0]),
            Some(&[0.0, 1.0, 3.0]),
            &["easeOut".into(), "linear".into()],
        )
        .unwrap()
        .map(5000.0);
        assert_eq!(r.overrides["progress0"], want);
        assert!(want > 0.5, "easeOut");
        // Goal = one lap (the orchestrator's 20:55 bug: the default output
        // used to be the full 0..3 lap range, so the goal read as 3 laps).
        let fit = |k: &str, v: f64| {
            resolve_with(FIT, None, &HashMap::from([(k.to_string(), v)])).overrides[&k
                .replace("steps", "progress0")
                .replace("waterMl", "progress1")
                .replace("activeMinutes", "progress2")]
        };
        assert_eq!(fit("steps", 10_000.0), 1.0);
        assert_eq!(fit("steps", 20_000.0), 2.0);
        assert_eq!(fit("steps", 90_000.0), 3.0);
        assert_eq!(fit("waterMl", 1_800.0), 0.72);
        assert_eq!(
            fit("activeMinutes", 45.0),
            1.0,
            "past the goal: clamped at one lap by default"
        );
        let mut inactive = r.inactive_bindings.clone();
        inactive.sort();
        assert_eq!(inactive, ["glowStrength", "progress1", "progress2"]);
        assert!(
            !r.overrides.contains_key("progress1"),
            "missing input = inactive, not guessed"
        );
        // Multi-stop, per-segment curves, clamped.
        let hr = |v: f64| {
            resolve_with(FIT, None, &HashMap::from([("heartRate".to_string(), v)])).overrides
                ["glowStrength"]
        };
        assert_eq!(hr(40.0), 0.0);
        assert_eq!(hr(80.0), 0.1);
        assert_eq!(hr(200.0), 0.8);
        // Companions ride along (audioLevel needs audioStrength).
        let v = resolve_with(
            VOICE,
            Some("speaking"),
            &HashMap::from([("agentVolume".to_string(), 0.4)]),
        );
        assert_eq!(v.overrides["audioLevel"], 0.5);
        // The companion's default (0.18) would fill audioStrength, but in 1.8
        // the speaking profile runs first and its value stays.
        assert_eq!(
            v.overrides["audioStrength"],
            crate::voice_state::profile(&v.state, "speaking")
                .unwrap()
                .overrides["audioStrength"]
        );
        // A deleted binding is gone, not inactive, in that state.
        let g = resolve_with(FIT, Some("goalReached"), &HashMap::new());
        assert!(g.ok, "{:?}", g.diagnostics);
        assert_eq!(g.state, "completing");
        assert!(g.inactive_bindings.is_empty());
        assert_eq!(g.overrides["progress"], 1.0);
        assert!(
            !g.overrides.contains_key("ringCount"),
            "inherited param the arc doesn't read"
        );
    }

    #[test]
    fn binding_targets_and_ranges_are_validated() {
        let spec = |b: &str| {
            resolve(&format!(
                r##"{{ "fxSpec": "1.8", "object": "ring", "pattern": "completing", "bindings": {b} }}"##
            ))
        };
        let r = spec(r##"{ "dotSize": { "input": "x" }, "progres": { "input": "x" } }"##);
        assert!(errors(&r).iter().any(
            |d| d.path == "/bindings/dotSize" && d.message.contains("isn't a bindable target")
        ));
        assert!(errors(&r)
            .iter()
            .any(|d| d.path == "/bindings/progres" && d.message.contains("`progress`")));
        let r = spec(r##"{ "progress": { "input": "" } }"##);
        assert!(errors(&r)
            .iter()
            .any(|d| d.path == "/bindings/progress/input"));
        let r = spec(
            r##"{ "progress": { "input": "x", "inputRange": [0, 1, 0.5], "outputRange": [0, 1, 1] } }"##,
        );
        assert!(errors(&r)
            .iter()
            .any(|d| d.message.contains("strictly ascending")));
        let r = spec(r##"{ "progress": { "input": "x", "curve": "bounce" } }"##);
        assert!(errors(&r)
            .iter()
            .any(|d| d.message.contains("unknown curve")));
        let r = spec(
            r##"{ "progress": { "input": "x", "inputRange": [0, 1], "outputRange": [0, 1, 1] } }"##,
        );
        assert!(errors(&r).iter().any(|d| d.message.contains("same length")));
        // A mode-specific target the mode doesn't read: warning.
        let r = spec(r##"{ "quality": { "input": "x" } }"##);
        assert!(r.ok && warnings(&r)[0].message.contains("doesn't read `quality`"));
        // Bound and static at once: warning, binding wins.
        let r = resolve_with(
            r##"{ "fxSpec": "1.8", "object": "ring", "pattern": "completing", "params": { "progress": 0.2 },
                  "bindings": { "progress": { "input": "x" } } }"##,
            None,
            &HashMap::from([("x".to_string(), 0.9)]),
        );
        assert!(r.ok && warnings(&r)[0].message.contains("also set statically"));
        assert_eq!(r.overrides["progress"], 0.9);
        // colorMix: a colour to mix toward (bare value or `mix: 0`) is not a
        // conflict -- the Studios export exactly this; an explicit non-zero
        // mix is.
        let cm = |color: &str| {
            resolve_with(
                &format!(
                    r##"{{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "color": {color},
                          "bindings": {{ "color.mix": {{ "input": "x" }} }} }}"##
                ),
                None,
                &HashMap::from([("x".to_string(), 0.4)]),
            )
        };
        for neutral in [r##"{ "value": "#e5484d", "mix": 0 }"##, r##""#e5484d""##] {
            let r = cm(neutral);
            assert!(
                r.ok && r.diagnostics.is_empty(),
                "{neutral}: {:?}",
                r.diagnostics
            );
            assert_eq!(
                (r.overrides["colorMix"], r.overrides["colorHue"]),
                (0.4, 358.1)
            );
        }
        let r = cm(r##"{ "value": "#e5484d", "mix": 0.6 }"##);
        assert!(warnings(&r)[0].message.contains("also set statically"));
        assert_eq!(r.overrides["colorMix"], 0.4);
        // Runtime keys in params point at bindings when they're bindable.
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "params": { "audioLevel": 1 } }"##,
        );
        assert!(errors(&r)[0].message.contains("bindings.audioLevel"));
    }

    #[test]
    fn entries_are_validated_with_their_own_paths() {
        let r = resolve(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "params": { "orbitN": 4 },
                  "states": {
                    "a": { "pattern": "loading" },
                    "b": { "size": 20, "params": { "orbitn": 3 } },
                    "c": { "pattern": "speaking" },
                    "d": 5 } }"##,
        );
        let e = errors(&r);
        assert!(e
            .iter()
            .any(|d| d.path == "/states/a/pattern" && d.message.contains("a ring pattern")));
        assert!(e
            .iter()
            .any(|d| d.path == "/states/b/size" && d.message.contains("file-wide")));
        assert!(e
            .iter()
            .any(|d| d.path == "/states/b/params/orbitn" && d.message.contains("`orbitN`")));
        assert!(e.iter().any(|d| d.path == "/states/d"));
        // `c` inherits orbitN, which `speaking` doesn't read: not reported
        // against the entry, and not applied there.
        assert!(!r
            .diagnostics
            .iter()
            .any(|d| d.path.starts_with("/states/c")));
        let c = resolve_with(
            r##"{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "params": { "orbitN": 4 },
                  "states": { "c": { "pattern": "speaking" } } }"##,
            Some("c"),
            &HashMap::new(),
        );
        assert!(c.ok && !c.overrides.contains_key("orbitN"));
    }

    #[test]
    fn schema_lists_the_same_top_level_and_section_keys() {
        let schema: Value =
            serde_json::from_str(include_str!("../../../spec/fx-spec-1.schema.json")).unwrap();
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            k.sort();
            k
        };
        let sorted = |a: &[&str]| -> Vec<String> {
            let mut k: Vec<String> = a.iter().map(|s| s.to_string()).collect();
            k.sort();
            k
        };
        assert_eq!(keys(&schema), sorted(&TOP_KEYS));
        let defs = &schema["$defs"];
        assert_eq!(keys(&defs["colorSection"]), sorted(&COLOR_KEYS));
        assert_eq!(keys(&defs["gradient"]), sorted(&GRADIENT_KEYS));
        assert_eq!(keys(&defs["dtcgColor"]), sorted(&DTCG_KEYS));
        for m in MATERIAL_SECTIONS {
            let mut names: Vec<&str> = material_keys(m).iter().map(|k| k.0).collect();
            if m == "glow" {
                names.extend(GLOW_ENUM_KEYS);
            }
            if m == "liquid" {
                names.extend(LIQUID_ENUM_KEYS);
            }
            if m == "particles" {
                names.push("style");
            }
            assert_eq!(keys(&defs[m]), sorted(&names), "materials.{m}");
        }
        // v1.1
        assert_eq!(keys(&defs["stateEntry"]), sorted(&ENTRY_KEYS));
        assert_eq!(keys(&defs["binding"]), sorted(&BINDING_KEYS));
        let names = |v: &Value| -> Vec<String> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        };
        let targets: Vec<&str> = crate::reactive::REACTIVE_TARGETS
            .iter()
            .map(|t| t.name)
            .collect();
        let mut targets: Vec<String> = targets.iter().map(|s| s.to_string()).collect();
        // 1.7: the catalog-path names of the same targets.
        targets.extend((0..4).map(|i| format!("progress[{i}]")));
        targets.extend(BINDING_PATHS.iter().map(|(p, _)| p.to_string()));
        assert_eq!(names(&defs["bindings"]["propertyNames"]["enum"]), targets);
        assert_eq!(
            names(&defs["curve"]["enum"]),
            crate::reactive::CURVES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        // v1.2
        assert_eq!(keys(&defs["performance"]), sorted(&PERFORMANCE_KEYS));
        assert_eq!(
            keys(&defs["performance"]["properties"]["lowPower"]),
            sorted(&LOW_POWER_KEYS)
        );
        assert_eq!(
            names(
                &defs["performance"]["properties"]["lowPower"]["properties"]["disable"]["items"]
                    ["enum"]
            ),
            SHEDDABLE.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn material_and_section_keys_exist_in_the_engine() {
        // Every engine key a section writes is one the post-processes read.
        let src = concat!(
            include_str!("primitives.rs"),
            include_str!("lib.rs"),
            include_str!("liquid.rs"),
            include_str!("particles.rs")
        );
        for m in MATERIAL_SECTIONS {
            for (_, engine, _, _) in material_keys(m) {
                assert!(
                    src.contains(&format!("\"{engine}\"")),
                    "{engine} isn't read by the engine"
                );
            }
        }
        for k in [
            "colorMix",
            "colorHue",
            "colorSaturation",
            "colorLightness",
            "colorMode",
            "gradientHue3",
            "gradientMid",
        ] {
            assert!(src.contains(&format!("\"{k}\"")), "{k}");
        }
    }

    const KEYS_1_8: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/fx-spec-1.8-keys.json"
    );

    /// A1: every key a file can use is either one a 1.8 file could already use
    /// (frozen in spec/fx-spec-1.8-keys.json) or in `SINCE` with the later minor
    /// that added it. Adding a material, a section key, a binding target or a
    /// sheddable without a `SINCE` row fails here by name; so does removing a 1.8
    /// key, which would break 1.8 files (a major version).
    #[test]
    // While the runtime is the floor (1.8), the valid SINCE range 1.9..=1.8 is
    // empty -- correctly: no key may be newer yet. It becomes 1.9..=1.9 at the bump.
    #[allow(clippy::reversed_empty_ranges)]
    fn every_key_is_a_1_8_key_or_has_a_since_minor() {
        let frozen: Value = serde_json::from_str(&std::fs::read_to_string(KEYS_1_8).unwrap_or_else(|_| {
            panic!("spec/fx-spec-1.8-keys.json is missing -- capture it with FX_SPEC_KEYS_WRITE=1 cargo test -p core_engine --lib -- --ignored write_the_1_8_keys")
        }))
        .unwrap();
        let frozen: Vec<&str> = frozen["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            frozen.len() > 100,
            "a non-trivial denominator ({} keys)",
            frozen.len()
        );
        let accepted = accepted_key_paths();
        for k in &accepted {
            let since = SINCE.iter().find(|(p, _)| p == k).map(|(_, m)| *m);
            match (frozen.contains(&k.as_str()), since) {
                (true, None) => {}
                (true, Some(m)) => panic!("`{k}` is a 1.8 key but SINCE says 1.{m}: a 1.8 file may use it"),
                (false, None) => panic!(
                    "`{k}` is new since the 1.8 floor and has no SINCE row: add (\"{k}\", <the minor that adds it>) \
                     so a file declaring an older minor gets an error instead of a silently different drawing"
                ),
                (false, Some(m)) => assert!((FLOOR_MINOR + 1..=RUNTIME_MINOR).contains(&m), "`{k}`: SINCE 1.{m} is outside 1.{}..=1.{RUNTIME_MINOR}", FLOOR_MINOR + 1),
            }
        }
        for k in &frozen {
            assert!(
                accepted.iter().any(|a| a == k),
                "`{k}` was a 1.8 key and is no longer accepted: 1.8 files using it break"
            );
        }
        for (p, _) in SINCE {
            assert!(
                accepted.iter().any(|a| a == p),
                "SINCE lists `{p}`, which the resolver doesn't accept"
            );
        }
    }

    #[test]
    #[ignore = "writes spec/fx-spec-1.8-keys.json; run deliberately with FX_SPEC_KEYS_WRITE=1"]
    fn write_the_1_8_keys() {
        if std::env::var("FX_SPEC_KEYS_WRITE").as_deref() != Ok("1") {
            return;
        }
        let keys: Vec<String> = accepted_key_paths()
            .into_iter()
            .filter(|k| !SINCE.iter().any(|(p, _)| p == k))
            .collect();
        let doc = json!({
            "note": "Every key path an FX Spec 1.8 file may use (fx_spec.rs accepted_key_paths). Frozen: a key added later needs a SINCE row with its minor, and removing one of these breaks 1.8 files. Never regenerate to paper over a failure (docs/fx-spec.md, Versioning).",
            "fxSpec": "1.8",
            "keys": keys,
        });
        std::fs::write(KEYS_1_8, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }

    /// The gate itself, with a stand-in `SINCE` (the real one is empty until a
    /// minor adds a key): each kind of path, in the base, an entry and
    /// performance, is an error in an older file and is dropped.
    #[test]
    fn the_version_gate_drops_newer_keys_with_an_error() {
        let since: &[(&str, u64)] = &[
            ("ink", 9),
            ("materials.holographic", 9),
            ("materials.glow.mode", 9),
            ("bindings:glowStrength", 9),
            ("bindings.curve", 9),
            ("performance.lowPower.disable:blur", 9),
        ];
        let doc = json!({
            "ink": 0.5,
            "materials": { "holographic": { "strength": 1 }, "glow": { "strength": 1, "mode": "blur" } },
            "bindings": { "glowStrength": { "input": "x" }, "audioLevel": { "input": "y", "curve": "easeOut" } },
            "states": { "a": { "ink": 0.3 } },
            "performance": { "lowPower": { "disable": ["glow", "blur"] } }
        });
        for (minor, want) in [(8u64, 7usize), (9, 0)] {
            let mut d = doc.clone();
            let mut diag = Diag(Vec::new());
            version_gate(d.as_object_mut().unwrap(), minor, since, &mut diag);
            assert_eq!(diag.0.len(), want, "1.{minor}: {:?}", diag.0);
            if minor == 8 {
                let paths: Vec<&str> = diag.0.iter().map(|x| x.path.as_str()).collect();
                for p in [
                    "/ink",
                    "/materials/holographic",
                    "/materials/glow/mode",
                    "/bindings/glowStrength",
                    "/bindings/audioLevel/curve",
                    "/states/a/ink",
                    "/performance/lowPower/disable/1",
                ] {
                    assert!(paths.contains(&p), "{p} not reported: {paths:?}");
                }
                assert!(diag.0[0]
                    .message
                    .contains("needs \"fxSpec\": \"1.9\" (this file says 1.8)"));
                assert_eq!(
                    d["materials"],
                    json!({ "glow": { "strength": 1 } }),
                    "dropped, not honoured"
                );
                assert_eq!(d["performance"]["lowPower"]["disable"], json!(["glow"]));
            } else {
                assert_eq!(d, doc, "a 1.9 file keeps them all");
            }
        }
    }

    #[test]
    fn color_and_gradient_under_materials_say_where_they_go() {
        for section in ["color", "gradient"] {
            let r = resolve(&format!(
                r##"{{ "fxSpec": "1.8", "object": "orb", "pattern": "working", "materials": {{ "{section}": {{ "mix": 0.5 }} }} }}"##
            ));
            assert!(!r.ok);
            let e = errors(&r);
            assert_eq!(e.len(), 1, "{section}: {:?}", r.diagnostics);
            assert_eq!(e[0].path, format!("/materials/{section}"));
            assert!(
                e[0].message.contains(&format!("move it to `/{section}`")),
                "{}",
                e[0].message
            );
        }
    }
}
