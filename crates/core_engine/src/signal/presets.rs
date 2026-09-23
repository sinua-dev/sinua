//! Signal's own state registry -- deliberately separate from
//! `orbs::presets`, not an addition to it. Signal is a genuinely different
//! family (see `signal/mod.rs`'s header); its states never appear in
//! `orbs::presets::STATES`, and vice versa.

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as
// `orbs::presets::STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &["signaling", "waveform", "scrolling", "metering"];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "signaling" => Some("bar"),
        "waveform" => Some("waveform"),
        "scrolling" => Some("scroll"),
        "metering" => Some("matrix"),
        _ => None,
    }
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// Unlike `orbs::presets::resolve_preset`, there's no separate base-profile/
/// scaling machinery here -- `signal::modes::bar::frame_bar` already
/// computes every dimension proportionally from `size` directly, so "the
/// resolved opts" is just an empty map: every opt's own `get(.., default)`
/// fallback in the mode function IS the default. No separate registry
/// needed for a family this simple yet -- revisit if/when Signal grows
/// enough states or styles to need per-size tuning beyond what's already
/// proportional.
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
