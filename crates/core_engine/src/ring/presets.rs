//! Ring's own state registry -- separate from `orbs::presets` and
//! `signal::presets`, same reasoning (see `ring/mod.rs`): a sibling
//! family, not an addition to either. State names are unique across all
//! families because `lib.rs`'s `resolve_any` tries them in order
//! (`orbs`, `signal`, then `ring`).

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as the
// other families' `STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &["completing", "loading", "tracking", "stepping", "measuring"];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "completing" => Some("arc"),
        "loading" => Some("spinner"),
        "tracking" => Some("nested"),
        "stepping" => Some("segmented"),
        "measuring" => Some("gauge"),
        _ => None,
    }
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// No base-profile/scaling machinery, like `signal`: every dimension is a
/// fraction of `size` inside the mode functions, so the resolved opts are
/// an empty map and each opt's `get(.., default)` IS the default. Speed is
/// 1.0 -- the spinner's cycle is specified in seconds (Material's
/// 1333ms), and shouldn't run faster when the ring is smaller.
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
