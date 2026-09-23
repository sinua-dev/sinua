//! Core: inline "generating..." indicators -- the product plan's "AI / system /
//! processing / intelligence" primitive, the fifth and last sibling family
//! (after `orbs`, `signal`, `ring`, `beacon`). Direction confirmed with the
//! user: a small, **inline** shimmer / gradient-sweep meant to sit next to
//! or inside other UI (a button, a line of text, a toolbar) -- explicitly
//! *not* a big floating "nucleus" presence, which was considered and
//! rejected for sitting too close to Orb's own territory.
//!
//! The Rust module is `core_fx`, not `core`: a user module named `core`
//! shadows the `core` crate that std and derive macros expand to (`core::
//! fmt`, `core::option`), and UniFFI's generated scaffolding uses those
//! paths. The family is still called `core` everywhere else (state names,
//! docs, Studio).
//!
//! Two states (see `presets.rs`): `generating` -> `shimmer` (a pill track
//! with a bright highlight sweeping left to right) and `typing` -> `dots`
//! (three dots bouncing in sequence, the chat "typing" indicator).
//!
//! Prior art, read from shipped source (fetched -- see
//! fetched 2026-09-18): react-loading-skeleton's
//! `skeleton.css` (`linear-gradient(90deg, base 0%, highlight 50%, base
//! 100%)` swept `translateX(-100%) -> 100%` over `1.5s ease-in-out`), VS
//! Code's `progressbar.css` (the infinite bar Copilot's "generating" state
//! shows: a 2%-wide bit swept across in 4s, `scaleX(3)` mid-way), Material
//! 3's linear indeterminate constants (1750ms, two lines with staggered
//! head/tail delays), shadcn/ui's `Skeleton` (`animate-pulse rounded-md`),
//! and the iMessage/Messenger typing indicator (three dots, identical
//! bounce with 0 / 0.2 / 0.4s delays -- "identical animation with offset
//! start times"). Perplexity's step spinner had no fetchable source and
//! was left out.
//!
//! The engine has no gradient primitive, so the shimmer's soft highlight is
//! built from a few concentric `Polyline` layers of stepped alpha (see
//! `modes/shimmer.rs`). `Dot` + `Polyline` only; no base-profile scaling.

pub mod modes;
pub mod presets;
