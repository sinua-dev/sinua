//! Character: voice-assistant characters -- the seventh sibling family,
//! after `orbs`, `signal`, `ring`, `beacon`, `core` and `edge`. Where the
//! others are abstract shapes, a character has a face: shape eyes (no
//! pupils, no brows), a gaze that knows whose turn it is, and a mouth that
//! only follows the voice's amplitude. Each character is its own pattern,
//! named after a sound (`buzzy` first; `hum`, `wisp`, `chirp` to follow).
//!
//! Design note: sinua-studio/docs/agents/families/design-07-characters.md;
//! research: research-characters.md next to it.
//!
//! - `geom`: shapes, strokes and convex clipping, all as fills.
//! - `face`: the shared face (eyes and mouth), independent of any body.
//! - `rig`: the pose from the opts and the clock (blinks, glances, the turn
//!   blink, the startle, effect expressions).
//! - `turn`: the head turn (yaw / pitch), a 1.11 candidate prototyped on Buzzy.
//! - `modes`: one file per character.

pub mod cosmetic;
pub mod face;
pub mod geom;
pub mod kit;
pub mod palette;
pub mod parts;
pub mod path;
pub mod presets;
pub mod recipe;
#[cfg(test)]
mod recipe_schema;
#[cfg(test)]
mod recipe_tests;
pub mod region;
pub mod registry;
pub mod rig;
pub mod turn;
