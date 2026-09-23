//! Signal: a linear, non-spherical, dockable audio-reactive visual family
//! -- a sibling to `orbs`, not a mode inside it. See the product plan's
//! component taxonomy (Orb/Core/Ring/Signal/Beacon as peer primitives, not
//! sub-features of one another) and `docs/effects-research.md`'s survey of
//! real linear/dockable voice-UI designs (Siri's waveform, WhatsApp voice
//! notes, LiveKit's/ElevenLabs' `BarVisualizer`) this family's styles are
//! built from.
//!
//! Reuses `crate::primitives`' `Dot`/`Line`/`OrbFrame`/`finalize_frame`
//! (and the mode-agnostic `apply_pointer`/`apply_audio_reactive` post-
//! processes `render()` in `lib.rs` applies to every family) -- a linear
//! bar/wave layout needs no new primitive type, unlike a genuinely
//! different rendering technology (e.g. the deferred Glass/Liquid material
//! work would need). `primitives` is a family-agnostic module, not part of
//! `orbs` (see its own header for why it was extracted there): this
//! family's own state list, mode dispatch, and preset resolution
//! (`signal::presets`) are entirely separate from `orbs::presets` --
//! `signal`'s states never appear in `orbs::presets::STATES`, and vice
//! versa, and `signal::modes` never imports from `orbs`.
//!
//! Three styles today: `bar` (discrete EQ pills, live), `waveform`
//! (layered continuous oscilloscope-style traces, live), and `scroll` (a
//! scrolling history of amplitude samples, WhatsApp/SoundCloud-style) --
//! see each module's own header for its specific technique.

pub mod modes;
pub mod presets;
