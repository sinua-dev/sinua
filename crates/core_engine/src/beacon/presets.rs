//! Beacon's own state registry -- a sibling to the other families'
//! registries, tried last by `lib.rs`'s `resolve_any` (`orbs`, `signal`,
//! `ring`, then `beacon`); state names are unique across all of them.

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as the
// other families' `STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &[
    "notifying",
    "reconnecting",
    "locating",
    "scanning",
    "broadcasting",
];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "notifying" => Some("ping"),
        "reconnecting" => Some("pulse"),
        "locating" => Some("halo"),
        "scanning" => Some("radar"),
        "broadcasting" => Some("broadcast"),
        _ => None,
    }
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// No base-profile/scaling machinery (same as `signal` and `ring`): every
/// dimension is a fraction of `size` inside the mode function, so the
/// resolved opts are empty and each `get(.., default)` IS the default.
/// Speed 1.0: periods are specified in seconds (Tailwind's 1s ping, 2s
/// pulse) and shouldn't change with the beacon's size.
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
