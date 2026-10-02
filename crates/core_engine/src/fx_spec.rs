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
pub const RUNTIME_MINOR: u64 = 13;
/// The first minor that knows the `character` object (1.11).
const CHARACTER_SINCE: u64 = 11;
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

pub fn suggest<'a>(key: &str, known: &[&'a str]) -> Option<&'a str> {
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
/// The keys every character mode reads (the rig and the character options);
/// a character with extra motion lists its own on top (`HUM_PARAMS`).
const CHARACTER_PARAMS: &[&str] = &[
    "accessories",
    "bounceGain",
    "breath",
    "earGain",
    "eyeAsym",
    "eyeH",
    "eyeR",
    "eyeSmile",
    "eyeTilt",
    "eyeW",
    "gazeX",
    "gazeY",
    "hue",
    "lean",
    "lid",
    "look",
    "mouth",
    "mouthDots",
    "mouthGain",
    "mouthTalk",
    "seed",
    "squashGain",
    "tilt",
    "turn",
    "turnBlink",
    "turnNod",
    "turnPitch",
    "turnWander",
    "turnYaw",
];

/// BEEP: the shared character keys plus `arms` (design note 17).
const BEEP_PARAMS: [&str; 30] = [
    "accessories",
    "arms",
    "bounceGain",
    "breath",
    "earGain",
    "eyeAsym",
    "eyeH",
    "eyeR",
    "eyeSmile",
    "eyeTilt",
    "eyeW",
    "gazeX",
    "gazeY",
    "hue",
    "lean",
    "lid",
    "look",
    "mouth",
    "mouthDots",
    "mouthGain",
    "mouthTalk",
    "seed",
    "squashGain",
    "tilt",
    "turn",
    "turnBlink",
    "turnNod",
    "turnPitch",
    "turnWander",
    "turnYaw",
];

/// HUM: the shared character keys plus its speaking sway.
const HUM_PARAMS: [&str; 30] = [
    "accessories",
    "bounceGain",
    "breath",
    "earGain",
    "eyeAsym",
    "eyeH",
    "eyeR",
    "eyeSmile",
    "eyeTilt",
    "eyeW",
    "gazeX",
    "gazeY",
    "hue",
    "lean",
    "lid",
    "look",
    "mouth",
    "mouthDots",
    "mouthGain",
    "mouthTalk",
    "seed",
    "squashGain",
    "swayGain",
    "tilt",
    "turn",
    "turnBlink",
    "turnNod",
    "turnPitch",
    "turnWander",
    "turnYaw",
];

/// WISP: the shared character keys plus its tail curl.
const WISP_PARAMS: [&str; 30] = [
    "accessories",
    "bounceGain",
    "breath",
    "curlGain",
    "earGain",
    "eyeAsym",
    "eyeH",
    "eyeR",
    "eyeSmile",
    "eyeTilt",
    "eyeW",
    "gazeX",
    "gazeY",
    "hue",
    "lean",
    "lid",
    "look",
    "mouth",
    "mouthDots",
    "mouthGain",
    "mouthTalk",
    "seed",
    "squashGain",
    "tilt",
    "turn",
    "turnBlink",
    "turnNod",
    "turnPitch",
    "turnWander",
    "turnYaw",
];

/// CHIRP: the shared character keys plus its wing flutter.
const CHIRP_PARAMS: [&str; 30] = [
    "accessories",
    "bounceGain",
    "breath",
    "earGain",
    "eyeAsym",
    "eyeH",
    "eyeR",
    "eyeSmile",
    "eyeTilt",
    "eyeW",
    "flutterGain",
    "gazeX",
    "gazeY",
    "hue",
    "lean",
    "lid",
    "look",
    "mouth",
    "mouthDots",
    "mouthGain",
    "mouthTalk",
    "seed",
    "squashGain",
    "tilt",
    "turn",
    "turnBlink",
    "turnNod",
    "turnPitch",
    "turnWander",
    "turnYaw",
];

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
        "playback" => &[
            "barCount",
            "barWidth",
            "hue",
            "minHeight",
            "playhead",
            "progress",
            "saturation",
            "unplayedOpacity",
        ],
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
        "speaker" => &[
            "avatarGap",
            "flow",
            "hue",
            "idleOpacity",
            "innerRadius",
            "reach",
            "rippleCount",
            "saturation",
            "shimmer",
            "thickness",
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
        "buzzy" => CHARACTER_PARAMS,
        "hum" => &HUM_PARAMS,
        "wisp" => &WISP_PARAMS,
        "chirp" => &CHIRP_PARAMS,
        "cuppa" | "bean" => CHARACTER_PARAMS,
        "beep" => &BEEP_PARAMS,
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
        // A recipe from the file (1.12) reads the shared keys; its own gains
        // are allowed on top where `params` is checked.
        m if m.starts_with(crate::character::registry::PREFIX) => CHARACTER_PARAMS,
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
        "playback" => idx("envelope", 64),
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
    } else if key == "aspect" {
        Some("the box ratio of a wide pattern is set by the view")
    } else if key == "stateAge" {
        Some("the time since the state changed is set by the view")
    } else if key == "tapX" || key == "tapY" {
        Some("where the view was tapped is set by the view, during the hop")
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
    } else if crate::character::presets::resolve_preset(state, 64).is_some() {
        Some("character")
    } else {
        None
    }
}

/// Registers the file's `recipe` (`character/registry.rs`) and points every
/// `pattern` that names its `id` (the base and each `states` entry) at the
/// registered key, which the views draw like any other mode. Recipe errors
/// keep their place: `/recipe/parts/2/lift/ears: expected 2 values`.
fn use_recipe(root: &mut Map<String, Value>, r: &Value, object: Option<&str>, diag: &mut Diag) {
    if object != Some("character") {
        diag.error("/recipe", "`recipe` is for `object: character`");
        return;
    }
    if !r.is_object() {
        diag.error("/recipe", "expected an object");
        return;
    }
    let id = r.get("id").and_then(Value::as_str).unwrap_or_default();
    if crate::character::presets::STATES.contains(&id) {
        diag.error(
            "/recipe/id",
            format!("`{id}` is a built-in character; give the recipe its own id"),
        );
        return;
    }
    let Some((key, _)) = register(r, diag) else {
        return;
    };
    let mut used = false;
    let mut point = |b: &mut Map<String, Value>| {
        if b.get("pattern").and_then(Value::as_str) == Some(id) {
            b.insert("pattern".into(), Value::String(key.into()));
            used = true;
        }
    };
    point(root);
    if let Some(Value::Object(states)) = root.get_mut("states") {
        for e in states.values_mut() {
            if let Value::Object(e) = e {
                point(e);
            }
        }
    }
    if !used {
        diag.warn(
            "/recipe/id",
            format!("no `pattern` names `{id}`, so the recipe isn't drawn"),
        );
    }
    if let Some(c) = crate::cost::estimate(key, 64, &HashMap::new()) {
        if c.class == "heavy" {
            diag.warn(
                "/recipe",
                format!(
                    "this character is heavy to draw ({} elements); expect a cost on low-end devices",
                    c.elements
                ),
            );
        }
    }
}

/// Registers a recipe's JSON: its key and how many cosmetics didn't fit (each
/// warns), or the error reported (a `/cosmetics` pointer as it is, the rest
/// under `/recipe`).
#[inline(never)]
fn register(r: &Value, diag: &mut Diag) -> Option<(&'static str, usize)> {
    let mut skipped = Vec::new();
    match crate::character::registry::register_with(&r.to_string(), &mut skipped) {
        Ok(k) => {
            let n = skipped.len();
            for (at, why) in skipped {
                diag.warn(&at, why);
            }
            Some((k, n))
        }
        Err(e) => {
            let (at, what) = e.split_once(": ").unwrap_or(("", &e));
            let pre = if at.starts_with("/cosmetics") {
                ""
            } else {
                "/recipe"
            };
            diag.error(&format!("{pre}{at}"), what.to_string());
            None
        }
    }
}

/// 1.13: `cosmetics` (design note 21) go into the file's `recipe`, and into
/// each built-in character a `pattern` names, which then draws that recipe.
fn use_cosmetics(root: &mut Map<String, Value>, c: &Value, diag: &mut Diag) {
    if root.get("object").and_then(Value::as_str) != Some("character") {
        diag.error("/cosmetics", "`cosmetics` is for `object: character`");
        return;
    }
    if let Some(Value::Object(r)) = root.get_mut("recipe") {
        r.insert("cosmetics".into(), c.clone());
        return;
    }
    let mut done: Vec<(String, Option<&'static str>)> = Vec::new();
    wear(root, c, &mut done, diag);
    if let Some(Value::Object(states)) = root.get_mut("states") {
        for e in states.values_mut() {
            if let Value::Object(e) = e {
                wear(e, c, &mut done, diag);
            }
        }
    }
}

/// Points `b`'s built-in `pattern` at that character wearing `c`.
#[inline(never)]
fn wear(
    b: &mut Map<String, Value>,
    c: &Value,
    done: &mut Vec<(String, Option<&'static str>)>,
    diag: &mut Diag,
) {
    let Some(p) = b.get("pattern").and_then(Value::as_str) else {
        return;
    };
    let key = match done.iter().find(|(n, _)| n == p) {
        Some(d) => d.1,
        None => {
            let mut k = None;
            if let Some((id, text)) = crate::character::recipe::RECIPES
                .iter()
                .find(|(id, _)| *id == p)
            {
                if let Ok(Value::Object(mut r)) = serde_json::from_str(text) {
                    r.insert("cosmetics".into(), c.clone());
                    r.insert("profile".into(), Value::String(id.to_string()));
                    // Nothing worn (none fits): the built-in draws as it is.
                    let worn = c.as_array().map_or(0, Vec::len);
                    k = register(&Value::Object(r), diag)
                        .filter(|(_, skipped)| *skipped < worn)
                        .map(|(k, _)| k);
                }
            }
            done.push((p.to_string(), k));
            k
        }
    };
    if let Some(k) = key {
        b.insert("pattern".into(), Value::String(k.into()));
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
/// stepping, `"envelope": [..]` on playing (1.9) -> the indexed engine keys
/// (`progress0..3`, `segment0..23`, `envelope0..63`).
const ARRAY_PARAMS: [(&str, usize); 3] = [("progress", 4), ("segment", 24), ("envelope", 64)];

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

const TOP_KEYS: [&str; 23] = [
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
    "transitions",
    "rules",
    "accessibility",
    "recipe",
    "expression",
    "palette",
    "cosmetics",
];
/// The design keys of a block: the base (top level) and each `states` entry.
const ENTRY_KEYS: [&str; 10] = [
    "pattern",
    "speed",
    "ink",
    "expression",
    "palette",
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
const SINCE: &[(&str, u64)] = &[
    ("transitions", 9),
    ("transitions.duration", 9),
    ("transitions.curve", 9),
    ("rules", 9),
    ("rules.when", 9),
    ("rules.state", 9),
    ("rules.hysteresis", 9),
    ("rules.when.input", 9),
    ("rules.when.gt", 9),
    ("rules.when.gte", 9),
    ("rules.when.lt", 9),
    ("rules.when.lte", 9),
    ("rules.when.between", 9),
    ("accessibility", 9),
    ("accessibility.name", 9),
    ("accessibility.states", 9),
    ("accessibility.announce", 9),
    ("recipe", 12),
    ("expression", 12),
    ("palette", 12),
    ("cosmetics", 13),
];

/// A `transitions` entry's keys (1.9).
const TRANSITION_KEYS: [&str; 2] = ["duration", "curve"];

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
    out.extend(TRANSITION_KEYS.iter().map(|k| format!("transitions.{k}")));
    out.extend(crate::rules::RULE_KEYS.iter().map(|k| format!("rules.{k}")));
    out.extend(
        crate::rules::WHEN_KEYS
            .iter()
            .map(|k| format!("rules.when.{k}")),
    );
    out.extend(
        crate::a11y::KEYS
            .iter()
            .map(|k| format!("accessibility.{k}")),
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
    // A character's palette is drawn, not tinted: the frame-wide colour and
    // gradient would repaint its eyes and screen too. `params.hue` turns the shell.
    if object == Some("character") {
        for sec in ["color", "gradient"] {
            if b.contains_key(sec) {
                diag.error(
                    &at(sec),
                    format!(
                        "`{sec}` doesn't apply to a character; set `params.hue` to turn its colour"
                    ),
                );
            }
        }
    }
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
    // 1.12: a character's expression (design note 16), as its five weights.
    if let Some(v) = b.get("expression") {
        let names: Vec<&str> = crate::character::rig::EXPRESSIONS
            .iter()
            .map(|(n, _)| *n)
            .collect();
        match (object, v.as_str()) {
            (Some(o), _) if o != "character" => {
                diag.error(&at("expression"), "`expression` is for `object: character`")
            }
            (_, Some(name)) if name == "none" || names.contains(&name) => {
                for (n, key) in crate::character::rig::EXPRESSIONS {
                    out.insert(key.to_string(), if n == name { 1.0 } else { 0.0 });
                }
            }
            (_, Some(name)) => {
                let mut known = names.clone();
                known.push("none");
                let hint = suggest(name, &known)
                    .map(|s| format!(" -- did you mean `{s}`?"))
                    .unwrap_or_default();
                diag.error(
                    &at("expression"),
                    format!(
                        "unknown expression `{name}` ({}, none){hint}",
                        names.join(", ")
                    ),
                )
            }
            (_, None) => diag.error(&at("expression"), "expected an expression name"),
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

    // 1.12: a character's palette, in part (design note 19).
    if let Some(v) = b.get("palette") {
        let pp = at("palette");
        if object != Some("character") {
            diag.error(&pp, "`palette` is for `object: character`");
        } else if let Some(o) = as_object(v, &pp, diag) {
            let mut given = Vec::new();
            for (slot, c) in o {
                if let Some(c) = parse_color(c, &ptr(&pp, slot), diag) {
                    given.push((slot.clone(), crate::character::geom::hsl(c.h, c.s, c.l)));
                }
            }
            let r = crate::character::palette::with_recipe(&mode, |r| {
                crate::character::palette::resolve(r, &given)
            });
            match r {
                Some(Ok((opts, warnings))) => {
                    out.extend(opts);
                    for (slot, why) in warnings {
                        diag.warn(&ptr(&pp, &slot), why);
                    }
                }
                Some(Err((slot, why))) => diag.error(&ptr(&pp, &slot), why),
                None => {}
            }
        }
    }
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
            let gains = crate::character::registry::get(&mode)
                .map(|r| r.gains())
                .unwrap_or_default();
            let mut allowed: Vec<&str> = mode_params(&mode).to_vec();
            allowed.extend(gains.iter().map(String::as_str));
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
                if key.starts_with("palette.") {
                    diag.error(&path, format!("`{key}` is set by `palette`; write \"palette\": {{ \"<slot>\": \"#RRGGBB\" }} instead"));
                    continue;
                }
                if key.starts_with("expression") {
                    diag.error(
                        &path,
                        format!("`{key}` is set by `expression`; write \"expression\": \"happy\" instead"),
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
    // 1.13: cosmetics, merged into the character's recipe (design note 21).
    if let Some(c) = root.get("cosmetics").cloned() {
        use_cosmetics(root, &c, &mut diag);
    }
    // 1.12: a character recipe carried in the file (design note 12).
    if let Some(r) = root.get("recipe").cloned() {
        let object = root
            .get("object")
            .and_then(Value::as_str)
            .map(str::to_string);
        use_recipe(root, &r, object.as_deref(), &mut diag);
    }

    // File-wide: object and size.
    let object = root.get("object").and_then(Value::as_str);
    match object {
        None => diag.error(
            "/object",
            "missing `object` (orb, signal, ring, beacon, core or character)",
        ),
        // SinuaEdge was removed in 0.1.0-beta.8 (design note 9): a file that still names it
        // fails loudly, whatever its version, rather than drawing nothing.
        Some("edge") => diag.error(
            "/object",
            "`object: edge` was removed in 0.1.0-beta.8 (FX Spec 1.12); there is no replacement: \
             draw the screen-edge glow in the app",
        ),
        Some(o) if !["orb", "signal", "ring", "beacon", "core", "character"].contains(&o) => {
            diag.error("/object", format!("unknown object `{o}`"))
        }
        // The character family arrived in 1.11 (design-07); an older file can't name it.
        Some("character") if minor < CHARACTER_SINCE => diag.error(
            "/object",
            format!("`object: character` needs \"fxSpec\": \"1.{CHARACTER_SINCE}\" (this file says 1.{minor})"),
        ),
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
        // 1.10: the shared profile lost its particles; a 1.8/1.9 file keeps them.
        if let Some(p) = crate::voice_state::profile_for_minor(&block.state, &key, minor) {
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

    // Transitions (1.9): validated here; views read them per state change
    // through `transition_for`.
    if let Some(t) = root.get("transitions") {
        check_transitions(t, &res.state_keys, strict, &mut diag);
    }
    // Rules (1.9): validated here; views ask `rules::derive` which state the
    // app's inputs pick.
    if let Some(r) = root.get("rules") {
        for p in crate::rules::check(r, &res.state_keys) {
            match p {
                crate::rules::Problem::Error(path, msg) => diag.error(&path, msg),
                crate::rules::Problem::Warning(path, msg) => diag.warn(&path, msg),
                crate::rules::Problem::Unknown(path, key, known) => {
                    diag.unknown(strict, &path, &key, known)
                }
            }
        }
    }
    // Accessibility (1.9): validated here; views read the words through
    // `a11y::read` and speak state changes through `a11y::announce_step`.
    if let Some(a) = root.get("accessibility") {
        for p in crate::a11y::check(a, &res.state_keys) {
            match p {
                crate::a11y::Problem::Error(path, msg) => diag.error(&path, msg),
                crate::a11y::Problem::Warning(path, msg) => diag.warn(&path, msg),
                crate::a11y::Problem::Unknown(path, key, known) => {
                    diag.unknown(strict, &path, &key, known)
                }
            }
        }
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

/// Validates a 1.9 `transitions` block: `default`, `"a->b"`, `"a->*"`,
/// `"*->b"` keys (a state name that isn't in `states` is a warning -- it can
/// never match), each `{ duration: seconds >= 0, curve: CSS keyword }`.
fn check_transitions(t: &Value, state_keys: &[String], strict: bool, diag: &mut Diag) {
    let Some(obj) = t.as_object() else {
        return diag.error("/transitions", "expected an object");
    };
    for (key, entry) in obj {
        let at = ptr("/transitions", key);
        if key != "default" {
            match key.split_once("->") {
                Some((a, b)) => {
                    for side in [a, b] {
                        if side != "*" && !state_keys.iter().any(|s| s == side) {
                            diag.warn(&at, format!("`{side}` is not a key of `states`, so this entry never applies"));
                        }
                    }
                }
                None => {
                    diag.error(&at, "expected `default`, `from->to`, `from->*` or `*->to`");
                    continue;
                }
            }
        }
        let Some(e) = entry.as_object() else {
            diag.error(&at, "expected { duration?, curve? }");
            continue;
        };
        for k in e.keys() {
            if !TRANSITION_KEYS.contains(&k.as_str()) {
                diag.unknown(strict, &ptr(&at, k), k, &TRANSITION_KEYS);
            }
        }
        if let Some(d) = e.get("duration") {
            number(d, &ptr(&at, "duration"), 0.0, 10.0, diag);
        }
        if let Some(c) = e.get("curve") {
            if !c
                .as_str()
                .is_some_and(|c| crate::reactive::CURVES.contains(&c))
            {
                diag.error(
                    &ptr(&at, "curve"),
                    format!("expected one of {}", crate::reactive::CURVES.join(", ")),
                );
            }
        }
    }
}

/// The transition for `from` → `to` (state keys; `""` = the base design):
/// the exact pair, then `from->*`, then `*->to`, then `default`, each field
/// falling back separately to the next match and finally to 0.6 s
/// `easeInOut`. An invalid value is skipped (the resolver reports it).
pub fn transition_for(json: &str, from: &str, to: &str) -> crate::FxTransition {
    let mut out = crate::FxTransition::default();
    let Ok(doc) = serde_json::from_str::<Value>(json) else {
        return out;
    };
    let Some(t) = doc.get("transitions").and_then(Value::as_object) else {
        return out;
    };
    let order = [
        format!("{from}->{to}"),
        format!("{from}->*"),
        format!("*->{to}"),
        "default".to_string(),
    ];
    let find = |field: &str| {
        order
            .iter()
            .filter_map(|k| t.get(k).and_then(|e| e.get(field)))
            .find(|v| match field {
                "duration" => v.as_f64().is_some_and(|d| (0.0..=10.0).contains(&d)),
                _ => v
                    .as_str()
                    .is_some_and(|c| crate::reactive::CURVES.contains(&c)),
            })
            .cloned()
    };
    if let Some(d) = find("duration").and_then(|v| v.as_f64()) {
        out.duration = d;
    }
    if let Some(c) = find("curve").and_then(|v| v.as_str().map(str::to_string)) {
        out.curve = c;
    }
    out
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
            r##"{ "fxSpec": "1.14", "object": "orb", "pattern": "working", "timeline": {}, "layers": [] }"##,
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
            ("playback", include_str!("signal/modes/playback.rs")),
            ("arc", include_str!("ring/modes/arc.rs")),
            ("gauge", include_str!("ring/modes/gauge.rs")),
            ("nested", include_str!("ring/modes/nested.rs")),
            ("segmented", include_str!("ring/modes/segmented.rs")),
            ("spinner", include_str!("ring/modes/spinner.rs")),
            ("speaker", include_str!("ring/modes/speaker.rs")),
            ("ping", include_str!("beacon/modes/ping.rs")),
            ("pulse", include_str!("beacon/modes/pulse.rs")),
            ("halo", include_str!("beacon/modes/halo.rs")),
            ("radar", include_str!("beacon/modes/radar.rs")),
            ("broadcast", include_str!("beacon/modes/broadcast.rs")),
            ("shimmer", include_str!("core_fx/modes/shimmer.rs")),
            ("dots", include_str!("core_fx/modes/dots.rs")),
            // Characters are recipes (design note 11): the reader, the rig, the
            // turn and the part files each one uses; the keys a recipe names
            // itself (`"gain": "flutterGain"`) are checked below.
            ("buzzy", include_str!("character/recipe.rs")),
            ("buzzy", include_str!("character/rig.rs")),
            ("buzzy", include_str!("character/turn.rs")),
            ("buzzy", include_str!("character/parts/common.rs")),
            ("buzzy", include_str!("character/parts/ranger.rs")),
            ("hum", include_str!("character/recipe.rs")),
            ("hum", include_str!("character/rig.rs")),
            ("hum", include_str!("character/turn.rs")),
            ("hum", include_str!("character/parts/common.rs")),
            ("hum", include_str!("character/parts/mic.rs")),
            ("wisp", include_str!("character/recipe.rs")),
            ("wisp", include_str!("character/rig.rs")),
            ("wisp", include_str!("character/turn.rs")),
            ("wisp", include_str!("character/parts/common.rs")),
            ("wisp", include_str!("character/parts/spirit.rs")),
            ("chirp", include_str!("character/recipe.rs")),
            ("chirp", include_str!("character/rig.rs")),
            ("chirp", include_str!("character/turn.rs")),
            ("chirp", include_str!("character/parts/common.rs")),
            ("chirp", include_str!("character/parts/bird.rs")),
            // Cuppa and Bean also use spirit.rs's mouth and sparkles, which read no
            // opts (its one read, `curlGain`, is the spirit part's).
            ("cuppa", include_str!("character/recipe.rs")),
            ("cuppa", include_str!("character/rig.rs")),
            ("cuppa", include_str!("character/turn.rs")),
            ("cuppa", include_str!("character/parts/common.rs")),
            ("cuppa", include_str!("character/parts/mic.rs")),
            ("cuppa", include_str!("character/parts/steam.rs")),
            ("bean", include_str!("character/recipe.rs")),
            ("bean", include_str!("character/rig.rs")),
            ("bean", include_str!("character/turn.rs")),
            ("bean", include_str!("character/parts/common.rs")),
            ("bean", include_str!("character/parts/bird.rs")),
            ("beep", include_str!("character/recipe.rs")),
            ("beep", include_str!("character/rig.rs")),
            ("beep", include_str!("character/turn.rs")),
            ("beep", include_str!("character/parts/common.rs")),
            ("beep", include_str!("character/parts/arms.rs")),
            ("beep", include_str!("character/parts/mic.rs")),
            ("beep", include_str!("character/parts/ranger.rs")),
            ("beep", include_str!("character/parts/bird.rs")),
        ];
        for (mode, src) in sources {
            let code = src.split("#[cfg(test)]").next().unwrap();
            let keys = code
                .split("get(o, \"")
                .skip(1)
                .chain(code.split("ctx.get(\"").skip(1));
            for part in keys {
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
        // The opts a recipe names (a gain a part reads by name).
        for (id, text) in crate::character::recipe::RECIPES {
            fn gains(v: &serde_json::Value, out: &mut Vec<String>) {
                match v {
                    serde_json::Value::Object(o) => {
                        for (k, x) in o {
                            match x.as_str() {
                                Some(s) if k == "gain" => out.push(s.to_string()),
                                _ => gains(x, out),
                            }
                        }
                    }
                    serde_json::Value::Array(a) => a.iter().for_each(|x| gains(x, out)),
                    _ => {}
                }
            }
            let mut keys = Vec::new();
            gains(&serde_json::from_str(text).unwrap(), &mut keys);
            for key in keys {
                assert!(
                    mode_params(id).contains(&key.as_str()),
                    "{id}'s recipe reads `{key}` but the FX Spec params table doesn't list it"
                );
            }
        }
    }

    const EXAMPLES: [(&str, &str); 16] = [
        (
            "coffee-shop",
            include_str!("../../../spec/examples/coffee-shop.fxspec.json"),
        ),
        (
            "custom-character",
            include_str!("../../../spec/examples/custom-character.fxspec.json"),
        ),
        (
            "buzzy-assistant",
            include_str!("../../../spec/examples/buzzy-assistant.fxspec.json"),
        ),
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

    #[test]
    fn from_1_10_the_voice_states_have_no_particles_and_older_files_keep_theirs() {
        let file = |minor: &str| {
            format!(
                r##"{{ "fxSpec": "{minor}", "object": "signal", "pattern": "waveform", "states": {{ "listening": {{}} }} }}"##
            )
        };
        let particles = |minor: &str| {
            let r = resolve_full(&file(minor), Some("listening"), &HashMap::new(), false);
            assert!(r.ok, "{:?}", r.diagnostics);
            r.overrides.get("particleStrength").copied()
        };
        assert_eq!(particles("1.9"), Some(1.0), "1.9 keeps its meaning");
        assert_eq!(particles("1.10"), Some(0.0), "1.10: particles off");
    }

    #[test]
    fn v1_9_examples_resolve_identically() {
        // 1.9 shipped in 0.1.0-beta.6; 1.10 changed the shared voice-state profile
        // (no particles), and older files keep theirs (`voice_state::profile_for_minor`).
        // This holds the 1.9 runtime's lock so that change can't leak into it.
        resolves_like_snapshot(include_str!("../../../spec/fx-spec-1.9-resolved.json"), 13);
    }

    #[test]
    fn v1_10_examples_resolve_identically() {
        // 1.10 shipped in 0.1.0-beta.7; 1.11 adds the `character` object and
        // must not change how any 1.10 file resolves.
        resolves_like_snapshot(include_str!("../../../spec/fx-spec-1.10-resolved.json"), 13);
    }

    #[test]
    fn v1_11_examples_resolve_identically() {
        // 1.11 is the runtime 0.1.0-beta.8's characters were made with; 1.12 adds
        // `recipe` and must not change how any 1.11 file resolves.
        resolves_like_snapshot(include_str!("../../../spec/fx-spec-1.11-resolved.json"), 14);
    }

    /// A file carrying `recipe` (Chirp's own, renamed `id`, plus `extra` keys).
    fn recipe_file(minor: &str, id: &str, extra: &str) -> String {
        let chirp = crate::character::recipe::RECIPES
            .iter()
            .find(|(n, _)| *n == "chirp")
            .unwrap()
            .1
            .replacen("\"id\":\"chirp\"", &format!("\"id\":\"{id}\"{extra}"), 1);
        format!(
            r##"{{ "fxSpec": "{minor}", "object": "character", "pattern": "{id}", "recipe": {chirp},
                  "states": {{ "speaking": {{}}, "listening": {{}} }} }}"##
        )
    }

    #[test]
    fn a_recipe_in_the_file_draws_like_the_built_in_it_copies() {
        let r = resolve(&recipe_file("1.12", "copy", ""));
        assert!(r.ok, "{:?}", r.diagnostics);
        assert!(r.state.starts_with("recipe:copy:"), "{}", r.state);
        let o: HashMap<String, f64> = [
            ("mouthTalk", 1.0),
            ("audioLevel", 0.7),
            ("turnYaw", 0.5),
            ("flutterGain", 0.3),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        for size in [20, 32, 64] {
            for t in [0.0, 0.7, 2.3] {
                let got = crate::frame_with_overrides(r.state.clone(), size, t, o.clone());
                let want = crate::frame_with_overrides("chirp".into(), size, t, o.clone());
                assert!(got.is_some());
                assert_eq!(format!("{got:?}"), format!("{want:?}"), "{size} {t}");
            }
        }
        // The same recipe again: the same key.
        assert_eq!(resolve(&recipe_file("1.12", "copy", "")).state, r.state);
    }

    #[test]
    fn a_recipe_needs_1_12_a_character_and_its_own_id() {
        let old = resolve(&recipe_file("1.11", "copy", ""));
        assert!(old
            .diagnostics
            .iter()
            .any(|d| d.path == "/recipe" && d.message.contains("needs \"fxSpec\": \"1.12\"")));
        let builtin = resolve(&recipe_file("1.12", "chirp", ""));
        assert!(builtin.diagnostics.iter().any(|d| d.path == "/recipe/id"));
        let orb = resolve(
            r##"{ "fxSpec": "1.12", "object": "orb", "pattern": "working", "recipe": {} }"##,
        );
        assert!(orb.diagnostics.iter().any(|d| d.path == "/recipe"));
        let unused = resolve(&recipe_file("1.12", "copy", "").replacen(
            "\"pattern\": \"copy\"",
            "\"pattern\": \"buzzy\"",
            1,
        ));
        assert!(unused.ok);
        assert!(warnings(&unused).iter().any(|d| d.path == "/recipe/id"));
    }

    #[test]
    fn recipe_errors_and_limits_point_into_the_recipe() {
        let cases = [
            (
                "\"segments\":10",
                "\"segments\":1000",
                "/recipe/parts/2/segments",
            ),
            ("\"count\":3", "\"count\":99", "/recipe/parts/7/count"),
            (
                "\"segments\":10",
                "\"segments\":10,\"wobble\":1",
                "/recipe/parts/2/wobble",
            ),
            (
                "\"stagger\":6.0",
                "\"stagger\":6000.0",
                "/recipe/parts/7/stagger",
            ),
            (
                "\"spread\":[-9.0,0.0,9.0]",
                &format!("\"spread\":[{}0]", "1,".repeat(40)),
                "/recipe/parts/2/spread",
            ),
            (
                "\"ellipse\":[100.0,110.0,58.0,56.0,0.0,64]",
                "\"ellipse\":[100.0,110.0,58.0,56.0,0.0,4096]",
                "/recipe/parts/3/shape",
            ),
        ];
        for (from, to, path) in cases {
            let f = recipe_file("1.12", "bad", "").replacen(from, to, 1);
            let r = resolve(&f);
            assert!(!r.ok, "{to}");
            assert!(
                errors(&r).iter().any(|d| d.path.starts_with(path)),
                "{to}: {:?}",
                r.diagnostics
            );
        }
        let huge = recipe_file(
            "1.12",
            "huge",
            &format!(",\"$comment\":\"{}\"", "x".repeat(70_000)),
        );
        assert!(errors(&resolve(&huge))
            .iter()
            .any(|d| d.path == "/recipe" && d.message.contains("at most")));
    }

    #[test]
    fn a_recipe_with_a_path_body_resolves_and_its_path_errors_point_into_it() {
        let ell = "\"ellipse\":[100.0,110.0,58.0,56.0,0.0,64]";
        let path = "\"path\":\"M44 110 C44 40 156 40 156 110 C156 170 44 170 44 110 Z\"";
        let r = resolve(&recipe_file("1.12", "blob", "").replacen(ell, path, 1));
        assert!(r.ok, "{:?}", r.diagnostics);
        assert!(crate::frame_with_overrides(r.state.clone(), 64, 0.5, HashMap::new()).is_some());
        let bad =
            resolve(&recipe_file("1.12", "blob", "").replacen(ell, "\"path\":\"M44 110 Q1 2\"", 1));
        assert!(
            errors(&bad)
                .iter()
                .any(|d| d.path == "/recipe/parts/3/shape/path" && d.message.starts_with("at 12:")),
            "{:?}",
            bad.diagnostics
        );
    }

    #[test]
    fn a_recipe_takes_the_shared_profile_or_a_built_in_one_and_its_own_gains() {
        let shared = resolve_with(
            &recipe_file("1.12", "plain", ""),
            Some("speaking"),
            &HashMap::new(),
        );
        assert!(shared.ok, "{:?}", shared.diagnostics);
        assert_eq!(
            shared.overrides["squashGain"], 0.045,
            "the shared language (buzzy's numbers)"
        );
        assert!(!shared.overrides.contains_key("flutterGain"));
        let chirpy = resolve_with(
            &recipe_file("1.12", "chirpy", ",\"profile\":\"chirp\""),
            Some("speaking"),
            &HashMap::new(),
        );
        assert_eq!(
            chirpy.overrides["flutterGain"], 0.3,
            "chirp's own state language"
        );
        // Its own gain is a param; another character's isn't.
        let f = recipe_file("1.12", "plain", "").replacen(
            "\"pattern\": \"plain\"",
            "\"pattern\": \"plain\", \"params\": { \"flutterGain\": 0.2, \"curlGain\": 1 }",
            1,
        );
        let r = resolve(&f);
        assert!(!r.ok);
        assert!(
            errors(&r).iter().all(|d| d.path == "/params/curlGain"),
            "{:?}",
            r.diagnostics
        );
    }

    #[test]
    fn expression_sets_a_characters_weights_in_the_base_and_each_state() {
        let f = |minor: &str, extra: &str| {
            format!(
                r##"{{ "fxSpec": "{minor}", "object": "character", "pattern": "cuppa", "expression": "happy"{extra},
                    "states": {{ "speaking": {{ "expression": "sad" }}, "idle": {{ "expression": null }} }} }}"##
            )
        };
        let base = resolve(&f("1.12", ""));
        assert!(base.ok, "{:?}", base.diagnostics);
        assert_eq!(base.overrides["expressionHappy"], 1.0);
        assert_eq!(base.overrides["expressionSad"], 0.0);
        let speaking = resolve_with(&f("1.12", ""), Some("speaking"), &HashMap::new());
        assert_eq!(speaking.overrides["expressionSad"], 1.0);
        assert_eq!(speaking.overrides["expressionHappy"], 0.0);
        let idle = resolve_with(&f("1.12", ""), Some("idle"), &HashMap::new());
        assert!(
            !idle.overrides.contains_key("expressionHappy"),
            "null removes it"
        );
        // A 1.11 file can't say it.
        assert!(resolve(&f("1.11", ""))
            .diagnostics
            .iter()
            .any(|d| d.path == "/expression" && d.message.contains("1.12")));
        // An unknown name, a non-character, and the weights in params.
        let typo = resolve(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "bean", "expression": "hapy" }"##,
        );
        assert!(errors(&typo)
            .iter()
            .any(|d| d.path == "/expression" && d.message.contains("did you mean `happy`")));
        let orb = resolve(
            r##"{ "fxSpec": "1.12", "object": "orb", "pattern": "working", "expression": "happy" }"##,
        );
        assert!(errors(&orb).iter().any(|d| d.path == "/expression"));
        let raw = resolve(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "bean", "params": { "expressionSad": 1 } }"##,
        );
        assert!(errors(&raw)
            .iter()
            .any(|d| d.path == "/params/expressionSad" && d.message.contains("expression")));
        assert_eq!(
            crate::expression_overrides("sleepy".into()).unwrap()["expressionSleepy"],
            1.0
        );
        assert!(crate::expression_overrides("grumpy".into()).is_none());
    }

    #[test]
    fn palette_repaints_a_characters_slots_in_the_base_and_each_state() {
        let f = |minor: &str| {
            format!(
                r##"{{ "fxSpec": "{minor}", "object": "character", "pattern": "buzzy",
                    "palette": {{ "shell": "#E63946" }},
                    "states": {{ "listening": {{ "palette": {{ "amber": "#FFFFFF" }} }} }} }}"##
            )
        };
        let base = resolve(&f("1.12"));
        assert!(base.ok, "{:?}", base.diagnostics);
        assert_eq!(base.overrides["palette.shell.w"], 1.0);
        assert!(
            base.overrides.contains_key("palette.shellDark.l"),
            "the tones follow"
        );
        let listening = resolve_with(&f("1.12"), Some("listening"), &HashMap::new());
        assert_eq!(listening.overrides["palette.amber.l"], 1.0);
        assert_eq!(
            listening.overrides["palette.shell.w"], 1.0,
            "merged over the base"
        );
        let unset = resolve_with(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "buzzy", "palette": { "shell": "#E63946" },
                "states": { "listening": { "palette": { "shell": null } } } }"##,
            Some("listening"),
            &HashMap::new(),
        );
        assert!(unset.ok, "{:?}", unset.diagnostics);
        assert!(
            !unset.overrides.contains_key("palette.shell.w"),
            "null removes a slot"
        );
        assert!(resolve(&f("1.11"))
            .diagnostics
            .iter()
            .any(|d| d.path == "/palette" && d.message.contains("1.12")));
        let typo = resolve(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "cuppa", "palette": { "mugg": "#000000" } }"##,
        );
        assert!(errors(&typo)
            .iter()
            .any(|d| d.path == "/palette/mugg" && d.message.contains("did you mean `mug`")));
        let orb = resolve(
            r##"{ "fxSpec": "1.12", "object": "orb", "pattern": "working", "palette": { "shell": "#000000" } }"##,
        );
        assert!(errors(&orb).iter().any(|d| d.path == "/palette"));
        let raw = resolve(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "bean", "params": { "palette.bean.w": 1 } }"##,
        );
        assert!(errors(&raw)
            .iter()
            .any(|d| d.path.starts_with("/params/palette.")));
        // An ink given dark on a dark bean: kept, with a warning.
        let dark = resolve(
            r##"{ "fxSpec": "1.12", "object": "character", "pattern": "bean", "palette": { "bean": "#2B1A12", "ink": "#111111" } }"##,
        );
        assert!(warnings(&dark).iter().any(|d| d.path == "/palette/ink"));
        // The view's entry point says the same.
        let v = crate::palette_overrides("bean".into(), r##"{ "bean": "#2B1A12" }"##.into());
        assert!(v.diagnostics.is_empty() && v.overrides["palette.ink.l"] > 0.8);
    }

    #[test]
    fn a_palette_change_blends_the_colour_without_a_hue_sweep() {
        let side = |o: &[(&str, f64)]| crate::transition::TransitionSide {
            state: "buzzy".into(),
            speed: 1.0,
            overrides: o.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        };
        let from = side(&[]);
        let to = side(&[
            ("palette.shell.h", 350.0),
            ("palette.shell.s", 0.8),
            ("palette.shell.l", 0.5),
            ("palette.shell.w", 1.0),
        ]);
        let m = crate::transition::mix(&from, &to, 64, 0.5, "linear").unwrap();
        assert_eq!(
            m.overrides["palette.shell.h"], 350.0,
            "the colour itself doesn't move"
        );
        assert!(
            (m.overrides["palette.shell.w"] - 0.5).abs() < 1e-9,
            "only the weight blends"
        );
    }

    #[test]
    fn a_removed_edge_file_fails_loudly_at_every_version() {
        for v in ["1.8", "1.10", "1.11"] {
            let r = resolve(&format!(
                r#"{{ "fxSpec": "{v}", "object": "edge", "pattern": "framing" }}"#
            ));
            assert!(!r.ok, "{v}");
            assert!(
                r.diagnostics
                    .iter()
                    .any(|d| d.path == "/object" && d.message.contains("removed in 0.1.0-beta.8")),
                "{v}: {:?}",
                r.diagnostics
            );
        }
    }

    #[test]
    fn a_character_needs_1_11_and_takes_hue_not_colour() {
        let old = resolve(r##"{ "fxSpec": "1.10", "object": "character", "pattern": "buzzy" }"##);
        assert!(!old.ok);
        assert!(old
            .diagnostics
            .iter()
            .any(|d| d.path == "/object" && d.message.contains("1.11")));
        let colour = resolve(
            r##"{ "fxSpec": "1.11", "object": "character", "pattern": "buzzy", "color": "#ff0000" }"##,
        );
        assert!(!colour.ok);
        assert!(colour
            .diagnostics
            .iter()
            .any(|d| d.path == "/color" && d.message.contains("params.hue")));
        let ok = resolve(
            r##"{ "fxSpec": "1.11", "object": "character", "pattern": "buzzy", "params": { "hue": 120, "gazeX": -4 } }"##,
        );
        assert!(ok.ok && ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
        assert_eq!(ok.overrides["hue"], 120.0);
        // `stateAge` is the view's, not the file's.
        let live = resolve(
            r##"{ "fxSpec": "1.11", "object": "character", "pattern": "buzzy", "params": { "stateAge": 1 } }"##,
        );
        assert!(!live.ok);
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
