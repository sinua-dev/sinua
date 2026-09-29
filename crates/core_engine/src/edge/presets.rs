//! Edge's own state registry -- a sibling to the other families'
//! registries, tried last by `lib.rs`'s `resolve_any`; state names are
//! unique across all of them.

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as the
// other families' `STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &["framing"];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "framing" => Some("rim"),
        _ => None,
    }
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// No base-profile/scaling machinery (same as `ring` and `beacon`): every
/// dimension is a fraction of the box inside the mode function, and the
/// periods are in seconds.
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
