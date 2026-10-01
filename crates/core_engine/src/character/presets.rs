//! Character's own state registry -- a sibling to the other families'
//! registries, tried after them by `lib.rs`'s `resolve_any`; state names are
//! unique across all of them.

use crate::primitives::ModeOpts;

// Reserved for a future uniffi-bindgen/enum-export pass, same as the
// other families' `STATES`.
#[allow(dead_code)]
pub const STATES: &[&str] = &["buzzy", "hum", "wisp", "chirp"];

fn state_to_mode(state: &str) -> Option<&'static str> {
    match state {
        "buzzy" => Some("buzzy"),
        "hum" => Some("hum"),
        "wisp" => Some("wisp"),
        "chirp" => Some("chirp"),
        _ => None,
    }
}

/// True for a character's mode (each character's mode is named like its pattern).
/// The engine skips the generic post-processes a character does itself
/// (`effects::draws_own`), and the other per-mode tables key off this.
pub fn is_character_mode(mode: &str) -> bool {
    STATES.contains(&mode)
}

pub struct Resolved {
    pub mode: &'static str,
    pub speed: f64,
    pub opts: ModeOpts,
}

/// No base-profile scaling (same as `ring`, `beacon`, `edge`): a character
/// draws in a 200-unit design box scaled to `size`, and its living motion
/// is in real seconds.
pub fn resolve_preset(state: &str, _size: u32) -> Option<Resolved> {
    let mode = state_to_mode(state)?;
    Some(Resolved {
        mode,
        speed: 1.0,
        opts: ModeOpts::new(),
    })
}
