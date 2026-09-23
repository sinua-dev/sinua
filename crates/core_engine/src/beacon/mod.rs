//! Beacon: discrete-event attention indicators -- the product plan's "Event /
//! notification / location / network / attention" primitive, the fourth
//! sibling family after `orbs`, `signal` and `ring`. Unlike those (all
//! continuous, ongoing indicators) a beacon draws attention to a **single
//! moment** -- often one-shot, sometimes sparsely repeating -- then fades.
//! Directly relevant to this project's own audience: a "new voice
//! message" / "incoming call" badge, a connection-quality / reconnecting
//! indicator (a real pain point in WebRTC voice apps), a location mark.
//!
//! Three states (see `presets.rs`): `notifying` -> `ping` (a badge dot
//! with expanding, fading rings), `reconnecting` -> `pulse` (a pulsing
//! dot with a four-segment connection-quality ring) and `locating` ->
//! `halo` (a dot inside a translucent accuracy halo with a slow pulse).
//!
//! Prior art, read from shipped source/docs (fetched -- see
//! fetched 2026-09-18): Tailwind CSS's `animate-ping`
//! (`75%, 100% { transform: scale(2); opacity: 0 }`, 1s
//! `cubic-bezier(0, 0, 0.2, 1)`, "useful for things like notification
//! badges") and `animate-pulse` (`50% { opacity: .5 }`, 2s
//! `cubic-bezier(0.4, 0, 0.6, 1)`); androidx Material 3 `Badge.kt` (a
//! content-less badge is a small circle); LiveKit's
//! `ConnectionQualityIndicator` (`excellent/good/poor/lost/unknown`);
//! the common "Reconnecting..." banner pattern (a pulsing amber dot plus
//! text -- "color carries none of the meaning on its own", so the color
//! here is an opt and the text is the caller's job); and Google Maps'
//! blue dot (a pulse that grows into a translucent circle and shrinks
//! back; the translucent halo's size is the position's uncertainty).
//!
//! Stateless like everything else: a one-shot event is expressed by the
//! caller restarting `t` at the moment of the event and passing
//! `once: 1` -- the engine equivalent of Geist's "mount the spinner only
//! after the action starts". The interrupt/barge-in flash
//! (`primitives::apply_interrupt`) is conceptually a beacon behavior too,
//! but it has to layer *over another family's frame*, which a family
//! can't do -- it stays a post-process; see `docs/beacon.md`.
//!
//! Renders with `Dot` (badge, halo) and `Polyline` (rings, quality
//! segments) via the shared `primitives::{arc_polyline, cubic_bezier}`;
//! no new primitive. Like `signal`/`ring`, no base-profile scaling:
//! every dimension is a fraction of `size`.

pub mod modes;
pub mod presets;
