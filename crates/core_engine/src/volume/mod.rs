//! Light volumes (1.15): shapes made of hundreds of points of light in 3D, seen through a
//! perspective camera, each point lit by its depth and drawn as a soft additive glow. The
//! foundation every light-volume pattern shares:
//!
//! - [`Camera`]: yaw, pitch, zoom and a vertical shift; a point at `(x, y, z)` in the unit
//!   volume (about -1..1) lands on the view with perspective `p = D / (D − z)`.
//! - [`Light`]: collects the points a shape emits (brightness, colour, size) into dots and
//!   one effect run with `blend` 3, "light": every painter draws each dot as a soft radial
//!   glow added onto what's below (`lighter`, `plusLighter`, `BlendMode.Plus`). Under low
//!   power (`blurScale` 0) the run is plain additive (`blend` 1): no soft glow.
//! - [`noise`]: deterministic 3D value noise (motion that doesn't repeat).
//! - [`sphere`], [`disc`]: Fibonacci point sets.
//! - [`budget`]: how many points a view of `size` px may draw.
//!
//! Light adds up, so draw order doesn't matter: no depth sort is needed for the look (the
//! frame is still z-sorted, as every frame is).

use crate::primitives::{finalize_frame, ColorMode, Dot, EffectRun, ModeOpts, OrbFrame};

/// The camera's distance (the preview's `D`).
const DIST: f64 = 3.2;
/// Light per point against the preview's trailing canvas (see [`Light::emit`]).
const GAIN: f64 = 1.8;

/// A view of the unit volume.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
    /// A vertical shift, in units of the volume's half-height on screen.
    pub dy: f64,
}

impl Camera {
    pub fn new(yaw: f64, pitch: f64) -> Camera {
        Camera {
            yaw,
            pitch,
            zoom: 1.0,
            dy: 0.0,
        }
    }
}

/// A colour of light: a hue on the shape's ramp (0 = teal, 1 = blue), white, or amber.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Ramp(f64),
    White,
    /// Amber: a shape that is losing its grip (a connection's quality falling).
    Warm,
}

impl Tone {
    /// `(hue°, saturation, lightness)`: teal (45, 226, 181) to blue (76, 141, 255), white
    /// (200, 215, 255), as HSL.
    fn hsl(self) -> (f64, f64, f64) {
        match self {
            Tone::Ramp(t) => {
                let t = t.clamp(0.0, 1.0);
                (
                    165.0 + (218.0 - 165.0) * t,
                    0.76 + (1.0 - 0.76) * t,
                    0.53 + (0.65 - 0.53) * t,
                )
            }
            Tone::White => (224.0, 1.0, 0.9),
            // (255, 170, 96)
            Tone::Warm => (28.0, 1.0, 0.69),
        }
    }
}

/// Collects a shape's points into a frame (see the module docs).
pub struct Light {
    sin_yaw: f64,
    cos_yaw: f64,
    sin_pitch: f64,
    cos_pitch: f64,
    scale: f64,
    x0: f64,
    y0: f64,
    base: f64,
    dots: Vec<Dot>,
}

impl Light {
    /// A frame `w` × `h` engine units, drawn `px` view px per unit (a view sets `viewPx`;
    /// see [`px_per_unit`]).
    pub fn new(cam: Camera, w: f64, h: f64, px: f64, capacity: usize) -> Light {
        let scale = w.min(h) * 0.42 * cam.zoom;
        Light {
            sin_yaw: cam.yaw.sin(),
            cos_yaw: cam.yaw.cos(),
            sin_pitch: cam.pitch.sin(),
            cos_pitch: cam.pitch.cos(),
            scale,
            x0: w / 2.0,
            y0: h / 2.0 + cam.dy * scale,
            // The glow's radius (its bright core is a fifth of it): the preview's 3 % of the
            // view, with a floor so a small view's points still read as light.
            base: (scale * px * 0.045).max(2.2) / px,
            dots: Vec::with_capacity(capacity),
        }
    }

    /// Where a point lands: screen x, y (engine units), its depth z and perspective factor.
    pub fn project(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64, f64) {
        let x1 = x * self.cos_yaw + z * self.sin_yaw;
        let z1 = -x * self.sin_yaw + z * self.cos_yaw;
        let y2 = y * self.cos_pitch - z1 * self.sin_pitch;
        let z2 = y * self.sin_pitch + z1 * self.cos_pitch;
        let p = DIST / (DIST - z2);
        (
            self.x0 + x1 * p * self.scale,
            self.y0 - y2 * p * self.scale,
            z2,
            p,
        )
    }

    /// One point: brightness `b` (0 = dark, about 1 = bright; above 1 burns brighter),
    /// its tone and size (1 = the shape's usual).
    pub fn emit(&mut self, x: f64, y: f64, z: f64, b: f64, tone: Tone, size: f64) {
        if b <= 0.01 {
            return;
        }
        let (sx, sy, z2, p) = self.project(x, y, z);
        let depth = (0.35 + 0.65 * (z2 + 1.0) / 2.0).clamp(0.2, 1.2);
        let (h, s, l) = tone.hsl();
        self.dots.push(Dot {
            x: sx,
            y: sy,
            z: z2,
            r: self.base * size * p * (0.7 + 0.5 * b.min(1.0)),
            white: l,
            // The preview leaves 45 % of each frame under the next (a trail) and so reads about
            // twice as bright; a frame here stands alone, so its light carries that gain.
            a: (b * depth * GAIN).clamp(0.0, 1.0),
            saturation: s,
            hue: h,
        });
    }

    /// The finished frame: the dots, one light run over them, fixed colours.
    pub fn finish(self, o: &ModeOpts) -> OrbFrame {
        let mut f = finalize_frame(self.dots, Vec::new(), 0.0);
        f.color_mode = ColorMode::Fixed;
        let soft = o.get("blurScale").copied().unwrap_or(1.0) > 0.0;
        if !f.dots.is_empty() {
            f.effects = vec![EffectRun {
                target: 0,
                start: 0,
                count: f.dots.len() as u32,
                blur: 0.0,
                blend: if soft { LIGHT } else { 1 },
            }];
        }
        f
    }
}

/// `EffectRun::blend` for light: soft radial glows added up.
pub const LIGHT: u8 = 3;

/// The light-volume patterns' modes: their dots already glow, so the generic glow halos
/// and the audio swell stay out (`lib.rs` `post`).
pub const MODES: &[&str] = &[
    "veil",
    "crystal",
    "dune",
    "comet",
    "flow",
    "cohesion",
    "scan",
    "constellation",
    "ripple",
    "broadcast",
];

/// View px per engine unit: the view draws the frame `viewPx` px across (it sets the key;
/// engines run at 20 / 32 / 64 and views scale), else 1.
pub fn px_per_unit(o: &ModeOpts, size: f64) -> f64 {
    o.get("viewPx")
        .filter(|v| **v > 0.0)
        .map_or(1.0, |v| v / size)
}

/// The side the view draws in px (`viewPx`), else the engine size.
pub fn view_px(o: &ModeOpts, size: f64) -> f64 {
    size * px_per_unit(o, size)
}

/// The most points a view of `size` px draws: ~150 at 48 px, ~420 at 128, 1,000 from 256.
pub fn budget(size: f64) -> usize {
    if size <= 48.0 {
        150
    } else if size <= 128.0 {
        (150.0 + (size - 48.0) / 80.0 * 270.0) as usize
    } else if size < 256.0 {
        (420.0 + (size - 128.0) / 128.0 * 580.0) as usize
    } else {
        1000
    }
}

/// How many of `n` along an axis fit when the whole grid must stay within `budget`
/// (`total` points at full resolution): the axis count scaled by the grid's share.
pub fn fit(n: usize, total: usize, budget: usize, dims: u32) -> usize {
    if total <= budget {
        return n;
    }
    let k = (budget as f64 / total as f64).powf(1.0 / f64::from(dims));
    ((n as f64 * k).round() as usize).max(2)
}

fn h3(x: i64, y: i64, z: i64) -> f64 {
    let mut n = (x.wrapping_mul(374_761_393) as i32)
        .wrapping_add(y.wrapping_mul(668_265_263) as i32)
        .wrapping_add(z.wrapping_mul(2_147_483_647) as i32);
    n = (n ^ ((n as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    f64::from((n ^ ((n as u32) >> 16) as i32) as u32) / 4_294_967_296.0
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// 3D value noise in -1..1, smooth, the same on every platform.
pub fn noise(x: f64, y: f64, z: f64) -> f64 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (smooth(x - xi), smooth(y - yi), smooth(z - zi));
    let (xi, yi, zi) = (xi as i64, yi as i64, zi as i64);
    let c = |i: i64, j: i64, k: i64| h3(xi + i, yi + j, zi + k);
    let l = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let near = l(
        l(c(0, 0, 0), c(1, 0, 0), xf),
        l(c(0, 1, 0), c(1, 1, 0), xf),
        yf,
    );
    let far = l(
        l(c(0, 0, 1), c(1, 0, 1), xf),
        l(c(0, 1, 1), c(1, 1, 1), xf),
        yf,
    );
    l(near, far, zf) * 2.0 - 1.0
}

/// A repeatable 0..1 from three integers: where a scattered point sits, so a shape needs no
/// random state.
pub fn hash3(x: i64, y: i64, z: i64) -> f64 {
    h3(x, y, z)
}

const GOLDEN: f64 = 2.399_963_229_728_653; // π (3 − √5)

/// `n` points evenly over the unit sphere.
pub fn sphere(n: usize) -> Vec<(f64, f64, f64)> {
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = i as f64 * GOLDEN;
            (a.cos() * r, y, a.sin() * r)
        })
        .collect()
}

/// `n` points evenly over the unit disc.
pub fn disc(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let r = ((i as f64 + 0.5) / n as f64).sqrt();
            let a = i as f64 * GOLDEN;
            (a.cos() * r, a.sin() * r)
        })
        .collect()
}

/// True when a bound voice's audio (`audioLevel`, the bands) is an input now: while it listens
/// or speaks (`voiceStateCode` 2 or 4), or always when no voice state is set (an app feeding the
/// keys itself). A voice sends zeros while idle or thinking; read as a value, they would bury
/// the shape's own life.
pub fn voice_audio(o: &ModeOpts) -> bool {
    match o.get("voiceStateCode") {
        None => true,
        Some(c) => matches!(c.round() as i64, 2 | 4),
    }
}

/// A shape's main value (0..1): `key` when the app sets it (a progress, a quota, a bound
/// input), else the voice's level (`audioLevel`) while a voice listens or speaks, else `None`
/// (the shape lives on its own).
pub fn value(o: &ModeOpts, key: &str) -> Option<f64> {
    o.get(key)
        .copied()
        .or_else(|| {
            voice_audio(o)
                .then(|| o.get("audioLevel").copied())
                .flatten()
        })
        .map(|v| v.clamp(0.0, 1.0))
}

/// The voice's bands (low first, 0..1) while it listens or speaks, else none.
pub fn bands(o: &ModeOpts) -> Vec<f64> {
    if !voice_audio(o) {
        return Vec::new();
    }
    let n = o
        .get("audioBandCount")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 16.0) as usize;
    (0..n)
        .filter_map(|i| crate::primitives::audio_band(o, i))
        .map(|v| v.clamp(0.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_is_smooth_bounded_and_repeatable() {
        let mut prev = noise(0.0, 0.3, 0.7);
        for i in 1..400 {
            let v = noise(f64::from(i) * 0.01, 0.3, 0.7);
            assert!((-1.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.05, "{i}");
            prev = v;
        }
        assert_eq!(noise(1.234, -5.6, 7.89), noise(1.234, -5.6, 7.89));
    }

    #[test]
    fn point_sets_are_on_their_shapes() {
        for (x, y, z) in sphere(300) {
            assert!((x * x + y * y + z * z - 1.0).abs() < 1e-9);
        }
        assert!(disc(200).iter().all(|(x, y)| x * x + y * y <= 1.0 + 1e-9));
    }

    #[test]
    fn the_camera_projects_with_perspective_and_depth_light() {
        let o = ModeOpts::new();
        let mut l = Light::new(Camera::new(0.0, 0.0), 200.0, 200.0, 1.0, 4);
        l.emit(0.0, 0.0, 0.5, 1.0, Tone::Ramp(0.0), 1.0); // near
        l.emit(0.0, 0.0, -0.5, 1.0, Tone::Ramp(0.0), 1.0); // far
        l.emit(1.0, 0.0, 0.0, 1.0, Tone::Ramp(1.0), 1.0);
        let f = l.finish(&o);
        let near = f.dots.iter().find(|d| d.z > 0.4).unwrap();
        let far = f.dots.iter().find(|d| d.z < -0.4).unwrap();
        assert!(
            near.r > far.r && near.a > far.a,
            "near is bigger and brighter"
        );
        assert!((near.x - 100.0).abs() < 1e-9 && (near.y - 100.0).abs() < 1e-9);
        // x = 1 at z = 0 lands 0.42 of the view out.
        let side = f.dots.iter().find(|d| d.x > 150.0).unwrap();
        assert!((side.x - 100.0 - 84.0).abs() < 1e-9);
        assert_eq!(f.effects.len(), 1);
        assert_eq!((f.effects[0].blend, f.effects[0].count), (LIGHT, 3));
        // Low power: plain additive, no soft glow.
        let mut l = Light::new(Camera::new(0.0, 0.0), 200.0, 200.0, 1.0, 1);
        l.emit(0.0, 0.0, 0.0, 1.0, Tone::White, 1.0);
        let lp: ModeOpts = [("blurScale".to_string(), 0.0)].into();
        assert_eq!(l.finish(&lp).effects[0].blend, 1);
    }

    #[test]
    fn a_view_sizes_points_by_what_it_draws_not_the_engine_size() {
        // Views run the engine at 64 and scale; `viewPx` says how big it is drawn.
        let at = |px: f64| {
            let o: ModeOpts = [("viewPx".to_string(), px), ("level".to_string(), 0.5)].into();
            crate::frame_with_overrides("veil".into(), 64, 1.0, o).unwrap()
        };
        assert!(at(512.0).dots.len() > 3 * at(48.0).dots.len());
        // The glow's radius in view px is the same as an engine run at that size.
        let r_view = at(512.0).dots.iter().map(|d| d.r).fold(0.0, f64::max) * 512.0 / 64.0;
        let o: ModeOpts = [("level".to_string(), 0.5)].into();
        let direct = crate::frame_with_overrides("veil".into(), 512, 1.0, o).unwrap();
        let r_direct = direct.dots.iter().map(|d| d.r).fold(0.0, f64::max);
        assert!((r_view - r_direct).abs() < 1e-6, "{r_view} {r_direct}");
    }

    /// A bound voice as views send it (`VoiceOverrides`: level, 8 bands and the state code in
    /// every state, zeros while idle or thinking) under the pattern's profile.
    fn with_voice(pattern: &str, state: &str, t: f64) -> ModeOpts {
        let (code, level) = match state {
            "idle" => (0.0, 0.0),
            "listening" => (2.0, 0.5 + 0.3 * (t * 9.0).sin()),
            "thinking" => (3.0, 0.0),
            _ => (4.0, 0.55 + 0.3 * (t * 11.0).sin()),
        };
        let mut o: ModeOpts = crate::voice_state::profile(pattern, state)
            .unwrap()
            .overrides;
        o.insert("voiceStateCode".into(), code);
        o.insert("audioLevel".into(), level);
        o.insert("audioBandCount".into(), 8.0);
        for i in 0..8 {
            o.insert(
                format!("audioBand{i}"),
                (level * (1.0 - i as f64 / 10.0)).max(0.0),
            );
        }
        o.insert("viewPx".into(), 160.0);
        o
    }

    fn brightness(f: &OrbFrame) -> f64 {
        f.dots.iter().map(|d| d.a * d.r * d.r).sum()
    }

    /// The four voice states read apart on every light volume with a voice bound (a check the
    /// coordinator ran: thinking drew like idle, because the voice's idle zeros were read as
    /// the app's value).
    #[test]
    fn with_a_voice_the_four_states_read_apart() {
        for pattern in MODES {
            let at = |st: &str| {
                let f = crate::frame_with_overrides(
                    (*pattern).into(),
                    64,
                    1.3,
                    with_voice(pattern, st, 1.3),
                )
                .unwrap();
                brightness(&f)
            };
            let [idle, listening, thinking, speaking] =
                ["idle", "listening", "thinking", "speaking"].map(at);
            for (a, b, what) in [
                (idle, thinking, "idle/thinking"),
                (idle, listening, "idle/listening"),
                (idle, speaking, "idle/speaking"),
                (thinking, speaking, "thinking/speaking"),
            ] {
                assert!(
                    (a - b).abs() > 0.08 * a.max(b),
                    "{pattern} {what}: {a:.1} vs {b:.1}"
                );
            }
        }
    }

    #[test]
    fn at_rest_with_a_voice_bound_they_still_live() {
        for pattern in MODES {
            let f = |t: f64| {
                crate::frame_with_overrides(
                    (*pattern).into(),
                    64,
                    t,
                    with_voice(pattern, "idle", t),
                )
                .unwrap()
            };
            let (a, b) = (f(1.0), f(1.6));
            assert_ne!(a, b, "{pattern} moves at rest");
            assert!(
                a.dots.iter().filter(|d| d.a > 0.25).count() > 20,
                "{pattern} reads at rest"
            );
        }
    }

    #[test]
    fn the_shapes_stay_inside_the_view() {
        for (pattern, aspect) in [
            ("veil", 1.0),
            ("crystal", 1.0),
            ("dune", 1.0),
            ("dune", 2.5),
            ("dune", 4.0),
            ("dune", 5.5),
            ("dune", 8.0),
        ] {
            for t in [0.0, 3.0, 9.0, 21.0] {
                let mut o = with_voice(pattern, "speaking", t);
                o.insert("viewPx".into(), 400.0);
                o.insert("aspect".into(), aspect);
                let f = crate::frame_with_overrides(pattern.into(), 64, t, o).unwrap();
                let w = 64.0 * aspect;
                for d in f.dots.iter().filter(|d| d.a > 0.1) {
                    assert!(
                        d.x > 0.0 && d.x < w && d.y > 0.0 && d.y < 64.0,
                        "{pattern} {aspect} t={t}: ({:.1}, {:.1})",
                        d.x,
                        d.y
                    );
                }
            }
        }
    }

    #[test]
    fn the_budget_grows_with_the_view() {
        assert_eq!(budget(20.0), 150);
        assert_eq!(budget(48.0), 150);
        assert!(budget(64.0) > 150 && budget(64.0) < 420);
        assert_eq!(budget(128.0), 420);
        assert_eq!(budget(512.0), 1000);
        assert_eq!(fit(70, 630, 1000, 2), 70);
        assert!(fit(70, 630, 150, 2) * fit(9, 630, 150, 2) <= 200);
    }
}

/// Writes light-volume frames for the visual checks (`SINUA_VOLUME_DUMP=<file>`): every
/// shape in the four voice states and its silent uses, at 48, 128 and 512 px, and a 4 s
/// loop at 30 fps per state (a fake voice; real inputs are the app's).
#[cfg(test)]
mod dump {
    use serde_json::{json, Value};

    fn frame_json(f: &crate::primitives::OrbFrame) -> Value {
        json!({
            "dots": f.dots.iter().map(|d| json!({"x": (d.x * 100.0).round() / 100.0, "y": (d.y * 100.0).round() / 100.0, "z": 0, "r": (d.r * 100.0).round() / 100.0, "white": d.white, "a": (d.a * 1000.0).round() / 1000.0, "saturation": d.saturation, "hue": (d.hue * 10.0).round() / 10.0})).collect::<Vec<_>>(),
            "lines": [], "polylines": [], "fills": [],
            "effects": f.effects.iter().map(|e| json!({"target": e.target, "start": e.start, "count": e.count, "blur": e.blur, "blend": e.blend})).collect::<Vec<_>>(),
            "colorMode": "fixed",
        })
    }

    /// A fake voice at `t`: syllables at ~4 Hz, 16 bands tilted toward the low end.
    fn voice(t: f64, o: &mut crate::primitives::ModeOpts) {
        let level = (0.5 + 0.5 * (t * 25.0).sin()).powi(2) * (0.6 + 0.4 * (t * 1.3).sin().abs());
        o.insert("audioLevel".into(), level);
        o.insert("audioBandCount".into(), 16.0);
        for i in 0..16 {
            let n = 0.55 + 0.45 * super::noise(i as f64 * 0.9, t * 4.0, 1.7);
            o.insert(
                format!("audioBand{i}"),
                (level * (1.0 - i as f64 / 22.0) * n * 1.3).min(1.0),
            );
        }
    }

    /// A state as a view draws it: the pattern's profile under a bound voice's keys (level, 16
    /// bands and the state code in every state; zeros while idle or thinking).
    fn state_opts_for(pattern: &str, state: &str, t: f64) -> crate::primitives::ModeOpts {
        let mut o = crate::voice_state::profile(pattern, state)
            .map(|p| p.overrides)
            .unwrap_or_default();
        let code = match state {
            "idle" => 0.0,
            "listening" => 2.0,
            "thinking" => 3.0,
            _ => 4.0,
        };
        o.insert("voiceStateCode".into(), code);
        match state {
            "listening" => {
                voice(t + 3.3, &mut o);
                let l = o["audioLevel"] * 0.8;
                o.insert("audioLevel".into(), l);
            }
            "speaking" => voice(t, &mut o),
            _ => {
                o.insert("audioLevel".into(), 0.0);
                o.insert("audioBandCount".into(), 16.0);
                for i in 0..16 {
                    o.insert(format!("audioBand{i}"), 0.0);
                }
            }
        }
        o
    }

    #[test]
    #[ignore]
    fn dump_frames() {
        let Ok(path) = std::env::var("SINUA_VOLUME_DUMP") else {
            return;
        };
        let mut out = serde_json::Map::new();
        let states = ["idle", "listening", "thinking", "speaking"];
        // (pattern, the use it shows, its value inputs)
        #[allow(clippy::type_complexity)]
        let silent: [(&str, &str, Vec<(&str, f64)>); 14] = [
            ("veil", "processing", vec![("churn", 0.6)]),
            ("veil", "value-0.8", vec![("level", 0.8)]),
            ("crystal", "battery-35", vec![("level", 0.35)]),
            ("crystal", "quota-80", vec![("level", 0.8)]),
            ("dune", "task-30", vec![("progress", 0.3)]),
            ("dune", "task-75", vec![("progress", 0.75)]),
            ("comet", "value-0.8", vec![("level", 0.8)]),
            ("flow", "value-0.8", vec![("level", 0.8)]),
            ("cohesion", "quality-0.6", vec![("quality", 0.6)]),
            ("cohesion", "quality-0.25", vec![("quality", 0.25)]),
            ("scan", "blips-12", vec![("blips", 12.0)]),
            ("constellation", "progress-0.4", vec![("progress", 0.4)]),
            ("constellation", "progress-1", vec![("progress", 1.0)]),
            ("broadcast", "level-0.9", vec![("level", 0.9)]),
        ];
        for mode in super::MODES {
            for size in [48u32, 128, 512] {
                for st in states {
                    let mut o = state_opts_for(mode, st, 1.7);
                    o.insert("viewPx".into(), f64::from(size));
                    let f = crate::frame_with_overrides((*mode).into(), 64, 1.7, o).unwrap();
                    out.insert(format!("{mode}-{st}-{size}"), frame_json(&f));
                }
            }
            for (m, name, kv) in &silent {
                if m == mode {
                    for size in [128u32, 512] {
                        let mut o: crate::primitives::ModeOpts =
                            kv.iter().map(|(k, v)| (k.to_string(), *v)).collect();
                        o.insert("viewPx".into(), f64::from(size));
                        let f = crate::frame_with_overrides((*mode).into(), 64, 1.7, o).unwrap();
                        out.insert(format!("{mode}-{name}-{size}"), frame_json(&f));
                    }
                }
            }
            for st in states {
                for i in 0..120 {
                    let t = f64::from(i) / 30.0;
                    let mut o = state_opts_for(mode, st, t);
                    o.insert("viewPx".into(), 240.0);
                    let f = crate::frame_with_overrides((*mode).into(), 64, t, o).unwrap();
                    out.insert(format!("{mode}-{st}-loop{i}"), frame_json(&f));
                }
            }
        }
        std::fs::write(path, Value::Object(out).to_string()).unwrap();
    }

    /// Engine time per frame (`SINUA_VOLUME_BENCH=1`, run with `--release`): each light volume and
    /// four references, drawn at 48 / 128 / 512 px, speaking on the fake voice, 600 frames; mean and p95 µs.
    #[test]
    #[ignore]
    fn bench_frames() {
        if std::env::var("SINUA_VOLUME_BENCH").is_err() {
            return;
        }
        // Views run the engine at 64 and pass the drawn side as `viewPx`.
        let cases = ["veil", "crystal", "dune", "glowing", "breathing"];
        for case in cases {
            let mut line = format!("{case:>9}");
            for size in [48u32, 128, 512] {
                let mut us: Vec<f64> = (0..600)
                    .map(|i| {
                        let t = f64::from(i) / 60.0;
                        let mut o = state_opts_for(case, "speaking", t);
                        o.insert("viewPx".into(), f64::from(size));
                        let start = std::time::Instant::now();
                        let f = crate::frame_with_overrides(case.into(), 64, t, o).unwrap();
                        let e = start.elapsed().as_secs_f64() * 1e6;
                        std::hint::black_box(f);
                        e
                    })
                    .collect();
                us.sort_by(f64::total_cmp);
                let mean = us.iter().sum::<f64>() / us.len() as f64;
                let p95 = us[(us.len() * 95 / 100).min(us.len() - 1)];
                line += &format!("  {size:>3}px mean {mean:>6.1} p95 {p95:>6.1}");
            }
            eprintln!("{line}");
        }
    }
}
