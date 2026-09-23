//! Core's own state registry -- tried last by `lib.rs`'s `resolve_any`
//! (`orbs`, `signal`, `ring`, `beacon`, then `core`); state names are
//! unique across all families.

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as the
// other families' `STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &["generating", "typing"];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "generating" => Some("shimmer"),
        "typing" => Some("dots"),
        _ => None,
    }
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// No base-profile/scaling machinery, like every non-`orbs` family: every
/// dimension is a fraction of `size`, so the resolved opts are empty and
/// each `get(.., default)` IS the default. Speed 1.0 -- periods are in
/// seconds (react-loading-skeleton's 1.5s, the typing indicator's cadence).
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
