//! Edge: an in-app screen-edge glow -- the sixth sibling family, after
//! `orbs`, `signal`, `ring`, `beacon` and `core`. Where the others are a
//! shape *on* the screen, an edge is the frame *of* it: a glow that runs
//! round the inside of the app's screen or container while the assistant
//! listens and speaks. The visual language Siri (iOS 18) and Gemini Live
//! made familiar for "the whole screen is listening now". In-app only: the
//! host puts `SinuaEdge` over its content; nothing here reaches the system
//! UI.
//!
//! The first **box-layout** family (catalog `layout: "box"`): engine space
//! follows the view's box ratio through the `aspect` runtime input (width
//! = `size * aspect`, height = `size`), so the frame hugs a portrait phone
//! screen as well as a landscape panel. A view that doesn't pass `aspect`
//! gets a square frame.

pub mod modes;
pub mod presets;
