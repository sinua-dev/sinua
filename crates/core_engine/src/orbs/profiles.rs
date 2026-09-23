//! Density profiles + the multiplier machinery that scales them.
//!
//! Ported from `thinking-orbs/src/engine/profiles.ts`. `ModeOpts` stays a
//! generic string-keyed map (not a per-mode struct) so `scale_counts` /
//! `scale_radii` can be transcribed verbatim instead of redesigned per mode.

use std::collections::{HashMap, HashSet};

pub type ModeOpts = HashMap<String, f64>;

pub fn opts(pairs: &[(&str, f64)]) -> ModeOpts {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

// 2-D lattices (rings x dots-per-ring) come in pairs -- each side takes
// sqrt(scale) so the TOTAL dot count scales by `scale`; flat lists scale
// linearly.
const COUNT_PAIRS: &[(&str, &str)] = &[
    ("latRings", "lonDensity"),
    ("rings", "lonDensity"),
    ("lanes", "segs"),
];
// `nodeCount` is the additive modes' (aurora/chladni/eclipse/hush) name for
// the same thing `nodeN` is in the ported profiles -- see docs/parameters.md.
const COUNT_KEYS: &[&str] = &[
    "orbitN",
    "ghostN",
    "nodeN",
    "nodeCount",
    "strandN",
    "signals",
];
const ICON_DENSITY_KEYS: &[&str] = &["iconD"];

// Every key that sets a dot's rendered radius -- scaling all of them keeps a
// dot's near/far falloff intact while shrinking or growing the mark.
const RADIUS_KEYS: &[&str] = &[
    "rBase",
    "rDepth",
    "rActive",
    "rDot",
    "ghostR",
    "partR",
    "partRDepth",
    "nodeR",
    "nodeRDepth",
    // additive modes' name for `nodeR` (see `COUNT_KEYS`)
    "nodeSize",
];

// JS `Math.round` rounds half-up; Rust's `f64::round` rounds half-away-from-
// zero. Every value scaled here (counts, radii) is non-negative, so the two
// rules agree in practice -- this crate never rounds a negative value.
pub fn scale_counts(src: &ModeOpts, scale: f64) -> ModeOpts {
    let mut out = src.clone();
    let mut done: HashSet<&str> = HashSet::new();
    let rt = scale.sqrt();

    for (a, b) in COUNT_PAIRS {
        if done.contains(a) || done.contains(b) {
            continue;
        }
        if let (Some(&va), Some(&vb)) = (src.get(*a), src.get(*b)) {
            out.insert((*a).to_string(), (va * rt).round().max(2.0));
            out.insert((*b).to_string(), (vb * rt).round().max(2.0));
            done.insert(a);
            done.insert(b);
        }
    }
    for k in COUNT_KEYS {
        if done.contains(k) {
            continue;
        }
        // an explicit 0 means the mode opted out of that layer entirely
        // (e.g. `ring` has no ghost sphere) -- scaling must not resurrect it.
        if let Some(&v) = src.get(*k) {
            if v != 0.0 {
                out.insert((*k).to_string(), (v * scale).round().max(1.0));
            }
        }
    }
    for k in ICON_DENSITY_KEYS {
        if let Some(&v) = src.get(*k) {
            out.insert((*k).to_string(), (v * scale).max(0.02));
        }
    }
    out
}

pub fn scale_radii(src: &ModeOpts, scale: f64) -> ModeOpts {
    let mut out = src.clone();
    for k in RADIUS_KEYS {
        if let Some(&v) = src.get(*k) {
            out.insert((*k).to_string(), v * scale);
        }
    }
    // remember the multiplier itself -- spacing-derived radii (the morph
    // outline) use it, since they aren't based on any single radius key.
    let prev = *out.get("rSizeMul").unwrap_or(&1.0);
    out.insert("rSizeMul".to_string(), prev * scale);
    out
}

/// Base ("fine") profiles per mode, before preset multipliers.
pub fn base_profiles() -> HashMap<&'static str, ModeOpts> {
    HashMap::from([
        (
            "globe",
            opts(&[
                ("latRings", 17.0),
                ("lonDensity", 44.0),
                ("rBase", 0.6),
                ("rDepth", 1.7),
                ("rBoost", 1.0),
                ("inkFar", 0.62),
                ("inkSpan", 0.54),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "orbits",
            opts(&[
                ("orbitN", 12.0),
                ("ghostN", 40.0),
                ("ghostR", 0.9),
                ("ghostA", 0.5),
                ("particles", 3.0),
                ("partR", 1.2),
                ("partRDepth", 1.6),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "rubik",
            opts(&[
                ("latRings", 15.0),
                ("lonDensity", 40.0),
                ("moveCount", 14.0),
                ("rBase", 0.6),
                ("rDepth", 1.7),
                ("rActive", 0.3),
                ("inkFar", 0.62),
                ("inkSpan", 0.54),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "wave",
            opts(&[
                ("rings", 15.0),
                ("lonDensity", 40.0),
                ("rBase", 0.6),
                ("rDepth", 1.7),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "web",
            opts(&[
                ("nodeN", 30.0),
                ("thr", 0.72),
                ("signals", 5.0),
                ("nodeR", 1.4),
                ("nodeRDepth", 1.8),
                ("lineW", 0.8),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/spectrum.rs`'s header.
            "spectrum",
            opts(&[
                ("barCount", 24.0),
                ("barDotCount", 6.0),
                ("jumpSpeed", 4.0),
                ("dotSize", 1.0),
                ("hue", 200.0),
                ("saturation", 0.6),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/sonar.rs`'s header.
            "sonar",
            opts(&[
                ("period", 1.6),
                ("ringCount", 48.0),
                ("echoCount", 2.0),
                ("echoSpacing", 0.18),
                ("coreSize", 1.3),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/warp.rs`'s header.
            "warp",
            opts(&[
                ("starCount", 120.0),
                ("period", 2.2),
                ("warpSpeed", 1.0),
                ("decay", 0.06),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/chladni.rs`'s header. `nodeCount`/
            // `nodeSize` reused from existing count/radius keys, same as
            // `aurora`.
            "chladni",
            opts(&[
                ("nodeCount", 260.0),
                ("nodeSize", 0.9),
                ("holdDuration", 2.5),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/eclipse.rs`'s header. `progress`
            // is meant to be overridden per-call via `frame_with_overrides`,
            // not tuned as a static preset value -- 0.5 here is only the
            // "no override given" fallback.
            "eclipse",
            opts(&[
                ("nodeCount", 260.0),
                ("nodeSize", 0.9),
                ("progress", 0.5),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/crystallize.rs`'s header.
            "crystallize",
            opts(&[
                ("period", 6.0),
                ("driftAmplitude", 0.35),
                ("dotSize", 1.1),
                ("lineWidth", 0.7),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/webflow.rs`'s header. Identical
            // to "web"'s own profile on purpose -- only the noise function
            // differs between `connecting` and `drifting`.
            "webflow",
            opts(&[
                ("nodeN", 30.0),
                ("thr", 0.72),
                ("signals", 5.0),
                ("nodeR", 1.4),
                ("nodeRDepth", 1.8),
                ("lineW", 0.8),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "braid",
            opts(&[
                ("strandN", 52.0),
                ("turns", 3.0),
                ("ghostN", 150.0),
                ("rBase", 1.2),
                ("rDepth", 1.8),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "ribbon",
            opts(&[
                ("lanes", 5.0),
                ("segs", 88.0),
                ("ghostN", 150.0),
                ("rBase", 1.1),
                ("rDepth", 1.7),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/aurora.rs`'s header. `surfaceScale`/
            // `surfaceSpeed`/`hueSpread`/`saturation` aren't in `COUNT_KEYS` or
            // `RADIUS_KEYS` above, so `scale_counts`/`scale_radii` pass them
            // through unchanged; only `nodeCount`/`nodeSize` (already-existing keys,
            // reused from `web`'s profile) scale with size.
            "aurora",
            opts(&[
                ("nodeCount", 260.0),
                ("nodeSize", 1.1),
                ("rsPow", 0.6),
                ("rMin", 0.3),
                ("surfaceScale", 1.4),
                ("surfaceSpeed", 0.25),
                ("hueSpread", 140.0),
                ("hueOffset", 0.0),
                ("hueSpeed", 12.0),
                ("saturation", 0.55),
            ]),
        ),
        (
            // Not a port -- see `orbs/modes/hush.rs`'s header. `nodeCount`/`nodeSize`
            // are count/radius keys, so the shared scaling applies; the rest
            // (`pulseAmplitude`/`period`/`dim`/`yaw`/`saturation`/`hue`) pass
            // through unscaled. Grey by default -- red (8) or amber (~35) is
            // a `saturation` > 0 away.
            "hush",
            opts(&[
                ("nodeCount", 220.0),
                ("nodeSize", 1.1),
                ("rsPow", 0.6),
                ("rMin", 0.3),
                ("pulseAmplitude", 0.03),
                ("period", 3.2),
                ("dim", 0.6),
                ("yaw", 0.6),
                ("saturation", 0.0),
                ("hue", 8.0),
            ]),
        ),
        (
            // ring shares ribbon's painter; faceOn cancels the camera tilt and
            // moves the undulation onto the radius; no ghost sphere behind it.
            "ring",
            opts(&[
                ("lanes", 5.0),
                ("segs", 88.0),
                ("ghostN", 0.0),
                ("faceOn", 1.0),
                ("rBase", 1.1),
                ("rDepth", 1.7),
                ("rsPow", 0.6),
                ("rMin", 0.3),
            ]),
        ),
        (
            "morph",
            opts(&[("rDot", 0.021), ("iconD", 1.0), ("rMin", 0.25)]),
        ),
    ])
}
