//! The `thinking-orbs` family: AI-state "thinking orb" spinners (dotted,
//! z-sorted halftone-sphere geometry). Ported from
//! https://github.com/Jakubantalik/Libraries.dev (MIT, Jakub Antalik).
//!
//! `Dot`/`Line`/`OrbFrame`, the ink/z-sort painter contract, and other
//! family-agnostic pieces (noise, lerp, the pointer/audio-reactive
//! post-processes) live in the sibling `crate::primitives` module, not
//! here -- `orbs::core` re-exports them for backward compatibility with
//! this family's own mode files. What IS specific to this family: the
//! spherical lattice (`fib_dir`), the spin/tilt projection (`Proj`), the
//! count/radius scaling rules (`orbs::profiles`), and the state-to-state
//! lattice morph (`orbs::modes::transition`). The `signal` family
//! (`crate::signal`) is the proof this split works: a genuinely different
//! shape (a linear, dockable strip, not a sphere) that still reuses
//! `Dot`/`Line`/`OrbFrame` from `primitives` directly, because a bar/curve
//! layout is still just points and lines, arranged differently -- only a
//! family with a truly different rendering technology (the deferred
//! Glass/Liquid material work) would need its own primitives.

pub mod core;
pub mod modes;
pub mod presets;
pub mod profiles;
