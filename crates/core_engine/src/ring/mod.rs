//! Ring: a flat, precise, dashboard-feeling circular indicator family --
//! the product plan's "Progress / activity / energy / status" primitive, and
//! the third sibling family after `orbs` and `signal` (see
//! `docs/architecture.md`'s family pattern). Deliberately the opposite
//! texture from Orb's organic ambient sphere, and deliberately *not*
//! voice-specific: its inputs are a generic `progress` (determinate) or
//! nothing at all (indeterminate), so any app with a task in flight can
//! use it -- "FX doesn't know business logic, just generic inputs".
//!
//! Two states today (see `presets.rs`): `completing` -> `arc`, a
//! determinate progress ring driven by a `progress: 0..1` opt (the literal
//! ring geometry that `orbs`' `eclipse`/`progressing` expresses as a
//! day/night terminator), and `loading` -> `spinner`, the classic
//! indeterminate chasing-arc activity indicator.
//!
//! Prior art, read from shipped source (fetched -- see
//! fetched 2026-09-18): androidx Material 3
//! `ProgressIndicator.kt` (start angle 270 = 12 o'clock, determinate sweep
//! `progress * 360` clockwise, `StrokeCap.Round`, a `TrackActiveSpace` gap
//! between indicator and track), Material Components Web's
//! `mdc-circular-progress` (`$arc-time 1333ms`, `$arc-size 270deg`,
//! `$arc-start-rotation-interval 216deg`, `cubic-bezier(0.4, 0, 0.2, 1)`,
//! 48px / 4px stroke), Vercel Geist's Spinner usage guidance, and Apple's
//! HIG entry on Activity rings -- which says those rings are for
//! Move/Exercise/Stand only and must never be restyled, so this family
//! builds a *generic* single ring and spinner and does not imitate them.
//!
//! Everything renders as `Polyline` (round caps from the paint contract
//! are what make an arc's ends look right); no `Dot`/`Line`, no new
//! primitive. Like `signal`, there's no base-profile scaling: every
//! dimension is a fraction of `size`.

pub mod modes;
pub mod presets;
