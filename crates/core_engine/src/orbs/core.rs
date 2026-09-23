//! `orbs`-specific primitives: the Fibonacci *sphere* lattice and the
//! spin/tilt/orthographic sphere projection every `orbs` mode uses to place
//! points on screen, plus `LatticeSample` (the cross-mode transition
//! mechanism for the trio of modes sharing that lattice -- see
//! `orbs::modes::transition`'s header).
//!
//! Ported 1:1 from `thinking-orbs/src/engine/core.ts` (upstream:
//! https://github.com/Jakubantalik/Libraries.dev, MIT, Jakub Antalik — see
//! `third_party/thinking-orbs/LICENSE`). Keep this a faithful transcription,
//! not a "Rustified" rewrite: correctness is proven by comparing output
//! against `spec/orbs-golden.json` within a 1e-4 tolerance, not by
//! re-deriving the math independently.
//!
//! Everything that ISN'T specific to a sphere (`Dot`/`Line`/`OrbFrame`, the
//! noise functions, `finalize_frame`, the mode-agnostic pointer/audio
//! post-processes, ...) moved to `crate::primitives` once a second family
//! (`signal`, see `signal/mod.rs`) needed them too -- re-exported below so
//! none of this crate's `orbs::modes::*` files needed an import change.

pub use crate::primitives::{
    angle_delta, audio_band, finalize_frame, frac, hash_d, lerp, lerp_hue, perlin3, radius_scale,
    vnoise, Dot, Line, OrbFrame,
};

/// A mode's per-point data before final projection -- what
/// `orbs::modes::transition` needs to blend two lattice-sharing modes
/// (`aurora`/`chladni`/`eclipse`, see that module's header) point-by-point
/// instead of cross-dissolving two whole frames. `dir`/`radius_frac`
/// describe where the point sits in 3D (before the shared camera used
/// during a transition projects it); `dot_r`/`white`/`alpha`/`saturation`/
/// `hue` are the same values that mode would put directly on a `Dot`,
/// already fully computed (including that mode's own camera-dependent
/// depth shading -- see `transition.rs`'s header for why each mode still
/// does its own internal projection just to compute these, separately
/// from the transition's shared-camera final position). Specific to the
/// `orbs` family's shared Fibonacci lattice, unlike everything re-exported
/// above -- doesn't belong in `crate::primitives`.
pub struct LatticeSample {
    pub dir: (f64, f64, f64),
    pub radius_frac: f64,
    pub dot_r: f64,
    pub white: f64,
    pub alpha: f64,
    pub saturation: f64,
    pub hue: f64,
}

/// Stable directions on a unit sphere (Fibonacci lattice).
pub fn fib_dir(i: f64, n: f64) -> (f64, f64, f64) {
    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let y = 1.0 - (2.0 * (i + 0.5)) / n;
    let rad = (1.0 - y * y).sqrt();
    let a = i * golden;
    (rad * a.cos(), y, rad * a.sin())
}

/// Shared spin + tilt + orthographic projection.
///
/// A struct instead of upstream's boxed closure — same math, no per-call
/// heap allocation, one instance per frame like the TS `makeProj` call site.
pub struct Proj {
    st: f64,
    ct: f64,
    sy: f64,
    cyw: f64,
    cx: f64,
    cy: f64,
    scale: f64,
}

impl Proj {
    pub fn new(yaw: f64, tilt: f64, cx: f64, cy: f64, scale: f64) -> Self {
        Self {
            st: tilt.sin(),
            ct: tilt.cos(),
            sy: yaw.sin(),
            cyw: yaw.cos(),
            cx,
            cy,
            scale,
        }
    }

    pub fn project(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let x1 = x * self.cyw + z * self.sy;
        let z1 = -x * self.sy + z * self.cyw;
        let y1 = y * self.ct - z1 * self.st;
        let z2 = y * self.st + z1 * self.ct;
        (self.cx + x1 * self.scale, self.cy - y1 * self.scale, z2)
    }
}
