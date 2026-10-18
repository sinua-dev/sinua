//! 1.15 characters, each built on its own: a drawing split into named parts (from the
//! artist's SVG by an internal authoring step) and the character's own rig. What they
//! share is only this: reading the part data, and drawing the parts through the
//! transforms a rig gives each one. How a character moves is its own file.
//!
//! The data (`spec/characters/bots/<id>.json`): `parts` (an id, an optional parent
//! that comes earlier, a pivot) and `fills` in paint order, each with its part `p`, an
//! SVG path `d` (M L Q C Z) and either a solid `c` `[h, s, l]` or a gradient `g`
//! `[kind, x0, y0, x1, y1, r]` (0 linear, 2 elliptical; see [`FillGradient`]) with
//! stops `s` `[[offset, h, s, l, a], …]`; `a` is the fill's opacity (default 1).
//! Everything is in the 200-unit design box.

pub mod hover;
pub mod walker;

use std::sync::OnceLock;

use serde_json::Value;

use crate::character::geom::{self, Xf};
use crate::character::path;
use crate::character::region::Shape;
use crate::primitives::{Fill, FillGradient, GradientStop};

/// One part: its parent's index (earlier in the list) and its pivot.
pub struct Part {
    pub id: String,
    pub parent: Option<usize>,
    pub pivot: (f64, f64),
}

/// One fill of the drawing: its part, its outline at two resolutions, its paint.
struct Piece {
    part: usize,
    coarse: Shape,
    fine: Shape,
    paint: Fill,
}

pub struct Bot {
    pub parts: Vec<Part>,
    pieces: Vec<Piece>,
}

/// Curve steps (design units): fine above [`FINE_FROM`] px, about 1.5 px a piece at 512.
const COARSE: f64 = 4.0;
const FINE: f64 = 0.6;
const FINE_FROM: f64 = 128.0;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

impl Bot {
    /// Reads a character's part data. The data ships with the engine, so a broken file
    /// is a build-time mistake: the tests read every one.
    pub fn parse(text: &str) -> Result<Bot, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut parts: Vec<Part> = Vec::new();
        for (i, p) in v["parts"].as_array().ok_or("parts")?.iter().enumerate() {
            let id = p["id"].as_str().ok_or("part id")?.to_string();
            let parent = match p["parent"].as_str() {
                None => None,
                Some(name) => Some(
                    parts
                        .iter()
                        .position(|q| q.id == name)
                        .ok_or_else(|| format!("part {i}: parent {name} must come first"))?,
                ),
            };
            let pv = nums(&p["pivot"]);
            parts.push(Part {
                id,
                parent,
                pivot: (pv[0], pv[1]),
            });
        }
        let mut pieces = Vec::new();
        for (i, f) in v["fills"].as_array().ok_or("fills")?.iter().enumerate() {
            let at = format!("fills/{i}");
            let part = f["p"].as_u64().ok_or(format!("{at}: p"))? as usize;
            if part >= parts.len() {
                return Err(format!("{at}: no part {part}"));
            }
            let d = f["d"].as_str().ok_or(format!("{at}: d"))?;
            let a = f["a"].as_f64().unwrap_or(1.0);
            let paint = match f.get("g") {
                None => {
                    let c = nums(&f["c"]);
                    geom::solid(Vec::new(), geom::hsl(c[0], c[1], c[2]), a)
                }
                Some(g) => {
                    let g = nums(g);
                    let stops: Vec<GradientStop> = f["s"]
                        .as_array()
                        .ok_or(format!("{at}: s"))?
                        .iter()
                        .map(|s| {
                            let s = nums(s);
                            GradientStop {
                                offset: s[0],
                                hue: s[1],
                                saturation: s[2],
                                white: s[3],
                                a: s[4],
                            }
                        })
                        .collect();
                    let mid = &stops[stops.len() / 2];
                    Fill {
                        white: mid.white,
                        a,
                        saturation: mid.saturation,
                        hue: mid.hue,
                        gradient: Some(FillGradient {
                            kind: g[0] as u8,
                            x0: g[1],
                            y0: g[2],
                            x1: g[3],
                            y1: g[4],
                            r: g[5],
                            stops,
                        }),
                        ..Default::default()
                    }
                }
            };
            pieces.push(Piece {
                part,
                coarse: path::parse_step(d, &at, COARSE)?,
                fine: path::parse_step(d, &at, FINE)?,
                paint,
            });
        }
        Ok(Bot { parts, pieces })
    }

    /// A part's index by id (the rigs look theirs up once).
    pub fn part(&self, id: &str) -> usize {
        self.parts
            .iter()
            .position(|p| p.id == id)
            .unwrap_or_else(|| panic!("no part {id}"))
    }

    /// Every part's transform to the view: `local[i]` (about the design box, already
    /// placed around its pivot by the rig, see [`about`]) after its parent's, then
    /// `view`.
    pub fn world(&self, local: &[Xf], view: Xf) -> Vec<Xf> {
        let mut w: Vec<Xf> = Vec::with_capacity(self.parts.len());
        for (i, p) in self.parts.iter().enumerate() {
            let parent = p.parent.map_or(view, |j| w[j]);
            w.push(local[i].then(parent));
        }
        w
    }

    /// The drawing's fills through each part's transform, in paint order. `alpha[i]`
    /// scales part `i` (1 = as drawn; 0 skips it).
    pub fn draw(&self, world: &[Xf], alpha: &[f64], size: f64) -> Vec<Fill> {
        let fine = size >= FINE_FROM;
        let mut out = Vec::with_capacity(self.pieces.len());
        for pc in &self.pieces {
            let k = alpha[pc.part];
            if k <= 0.0 {
                continue;
            }
            let s = if fine { &pc.fine } else { &pc.coarse };
            let mut f = Fill {
                points: s.outer.clone(),
                holes: s.holes.clone(),
                ..pc.paint.clone()
            };
            f.a *= k;
            out.push(geom::transform(f, &world[pc.part]));
        }
        out
    }
}

/// A rotation `r` and scale `(sx, sy)` about `pivot`, then a move `(dx, dy)`: the usual
/// local transform of a part.
pub fn about(pivot: (f64, f64), r: f64, (sx, sy): (f64, f64), (dx, dy): (f64, f64)) -> Xf {
    Xf::translate(-pivot.0, -pivot.1)
        .then(Xf::scale(sx, sy))
        .then(Xf::rotate(r))
        .then(Xf::translate(pivot.0 + dx, pivot.1 + dy))
}

/// A damped spring's displacement at `t` seconds after a unit kick at 0: overlap and
/// follow-through for antennae and floating hands. Closed form, so the same `t` gives the
/// same pose on every platform and at every frame rate (no state between frames).
pub fn spring(t: f64, freq: f64, damping: f64) -> f64 {
    // Before the kick, or none at all (an infinite age).
    if !(0.0..1e6).contains(&t) {
        return 0.0;
    }
    let w = std::f64::consts::TAU * freq;
    (-damping * w * t).exp() * (w * t).sin()
}

/// The lagging response of a spring to a periodic drive `sin(TAU * f * t)`: the same
/// motion delayed by the spring's phase lag (a cheap follow-through for a bob), shifted so
/// it is 0 at `t = 0` (the drawing's own pose).
pub fn follow(t: f64, f: f64, freq: f64, damping: f64) -> f64 {
    let r = f / freq;
    let lag = (2.0 * damping * r).atan2(1.0 - r * r);
    let gain = 1.0 / ((1.0 - r * r).powi(2) + (2.0 * damping * r).powi(2)).sqrt();
    gain * ((std::f64::consts::TAU * f * t - lag).sin() + lag.sin())
}

/// The 1.15 characters (their modes).
pub const BOTS: &[&str] = &["whirr", "clank"];

/// An option's value, or `d`.
pub fn get(o: &crate::primitives::ModeOpts, k: &str, d: f64) -> f64 {
    o.get(k).copied().unwrap_or(d)
}

/// A character's data, read once.
pub fn load(cell: &'static OnceLock<Bot>, text: &str) -> &'static Bot {
    cell.get_or_init(|| Bot::parse(text).unwrap_or_else(|e| panic!("bot data: {e}")))
}

/// A frame as the web painter's JSON (`fills` only): for the visual checks.
#[cfg(test)]
pub fn frame_json(f: &crate::primitives::OrbFrame) -> Value {
    use serde_json::json;
    let pts = |p: &[crate::primitives::Point]| -> Vec<Value> {
        p.iter().map(|q| json!({"x": q.x, "y": q.y})).collect()
    };
    let fills: Vec<Value> = f
        .fills
        .iter()
        .map(|x| {
            json!({
                "points": pts(&x.points),
                "holes": x.holes.iter().map(|h| pts(h)).collect::<Vec<_>>(),
                "white": x.white, "a": x.a, "saturation": x.saturation, "hue": x.hue,
                "blur": x.blur, "blend": x.blend,
                "gradient": x.gradient.as_ref().map(|g| json!({
                    "kind": g.kind, "x0": g.x0, "y0": g.y0, "x1": g.x1, "y1": g.y1, "r": g.r,
                    "stops": g.stops.iter().map(|s| json!({"offset": s.offset, "white": s.white, "a": s.a, "saturation": s.saturation, "hue": s.hue})).collect::<Vec<_>>(),
                })),
            })
        })
        .collect();
    json!({"dots": [], "lines": [], "polylines": [], "fills": fills, "effects": [], "colorMode": "fixed"})
}

/// Writes a character's frames for the visual checks: the rest pose at 64, 200 and 512, and
/// 12-frame strips over 2.4 s of the four states (speaking on a fake level).
#[cfg(test)]
pub fn dump(
    path: &str,
    frame: fn(f64, f64, &crate::primitives::ModeOpts) -> crate::primitives::OrbFrame,
) {
    use crate::primitives::ModeOpts;
    let opts =
        |kv: &[(&str, f64)]| -> ModeOpts { kv.iter().map(|(k, v)| (k.to_string(), *v)).collect() };
    // `SINUA_BOT_FPS` > 0: a smooth 4 s loop per state at that rate (for watching), and a
    // whole session through the transition clock.
    let fps: u32 = std::env::var("SINUA_BOT_FPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut out = serde_json::Map::new();
    if fps > 0 {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .split('-')
            .next()
            .unwrap()
            .to_string();
        for (i, (_, f)) in session(&name, 160, f64::from(fps), 1.5, 0.6)
            .iter()
            .enumerate()
        {
            out.insert(
                format!("session-{i}"),
                serde_json::json!({"size": 160, "frame": frame_json(f)}),
            );
        }
    }
    let mut put = |k: String, size: f64, t: f64, o: ModeOpts| {
        out.insert(
            k,
            serde_json::json!({"size": size, "frame": frame_json(&frame(size, t, &o))}),
        );
    };
    for s in [64.0, 200.0, 512.0] {
        put(format!("rest-{s}"), s, 0.0, ModeOpts::new());
    }
    let states: [(&str, Vec<(&str, f64)>); 4] = [
        ("idle", vec![("breath", 1.0), ("look", 1.0)]),
        ("listening", vec![("earGain", 1.0), ("gazeY", 2.0)]),
        ("thinking", vec![("mouthDots", 1.0), ("lid", 0.28)]),
        (
            "speaking",
            vec![("mouthTalk", 1.0), ("mouthGain", 1.0), ("eyeSmile", 0.45)],
        ),
    ];
    let (n, dt, size) = if fps > 0 {
        (4 * fps, 1.0 / f64::from(fps), 160.0)
    } else {
        (12, 0.2, 200.0)
    };
    for (name, kv) in &states {
        for i in 0..n {
            let t = dt * f64::from(i);
            let mut o = opts(kv);
            if *name == "speaking" {
                // A fake voice: syllables at ~4 Hz.
                o.insert("audioLevel".into(), (0.5 + 0.5 * (t * 25.0).sin()).powi(2));
            }
            put(format!("{name}-{i}"), size, t, o);
        }
    }
    std::fs::write(path, Value::Object(out).to_string()).unwrap();
}

/// A voice session through the views' transition clock (each state's weight through three
/// first-order lags, `ω = 6.3 / duration`; `voiceBlend` mixes the profile sides): idle →
/// listening → thinking → speaking → idle, `hold` seconds each, at `fps`. Returns each
/// frame and its time. Speaking runs on a fake level.
#[cfg(test)]
pub fn session(
    pattern: &str,
    size: u32,
    fps: f64,
    hold: f64,
    duration: f64,
) -> Vec<(f64, crate::primitives::OrbFrame)> {
    use crate::transition::TransitionSide;
    let states = ["idle", "listening", "thinking", "speaking"];
    let sides: Vec<TransitionSide> = states
        .iter()
        .map(|s| {
            let p = crate::voice_state::profile(pattern, s).unwrap();
            TransitionSide {
                state: pattern.into(),
                speed: p.speed,
                overrides: p.overrides,
            }
        })
        .collect();
    let order = [0usize, 1, 2, 3, 0];
    let omega = 6.3 / duration;
    let mut lags = [[1.0, 0.0, 0.0, 0.0]; 3];
    let mut changed = 0.0;
    let mut last = 0usize;
    let dt = 1.0 / fps;
    let n = (hold * order.len() as f64 * fps) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 * dt;
        let target = order[((t / hold) as usize).min(order.len() - 1)];
        if target != last {
            (last, changed) = (target, t);
        }
        let k = 1.0 - (-omega * dt).exp();
        for j in 0..4 {
            let goal = if j == target { 1.0 } else { 0.0 };
            lags[0][j] += (goal - lags[0][j]) * k;
            lags[1][j] += (lags[0][j] - lags[1][j]) * k;
            lags[2][j] += (lags[1][j] - lags[2][j]) * k;
        }
        let mix = crate::transition::blend(&sides, &lags[2], target, size).unwrap();
        let mut o = mix.overrides;
        // The speaking level, faded by the speaking weight like a view's input.
        o.insert(
            "audioLevel".into(),
            lags[2][3] * (0.5 + 0.5 * (t * 25.0).sin()).powi(2),
        );
        if t > 0.0 && changed > 0.0 {
            o.insert("stateAge".into(), t - changed);
        }
        out.push((
            t,
            crate::frame_with_overrides(pattern.into(), size, t, o).unwrap(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Through a whole session, no frame jumps: the largest step between two frames at a
    /// state change is no bigger than the motion's own steps inside a state.
    #[test]
    fn state_changes_flow() {
        for pattern in BOTS {
            let frames = session(pattern, 200, 30.0, 1.5, 0.6);
            let step = |a: &crate::primitives::OrbFrame, b: &crate::primitives::OrbFrame| -> f64 {
                a.fills
                    .iter()
                    .zip(&b.fills)
                    .flat_map(|(x, y)| x.points.iter().zip(&y.points))
                    .map(|(p, q)| (p.x - q.x).hypot(p.y - q.y))
                    .fold(0.0, f64::max)
            };
            let (mut at_change, mut inside) = (0.0f64, 0.0f64);
            for w in frames.windows(2) {
                let d = step(&w[0].1, &w[1].1);
                let near = (w[1].0 % 1.5) < 0.6 && w[1].0 > 1.0;
                if near {
                    at_change = at_change.max(d);
                } else {
                    inside = inside.max(d);
                }
            }
            assert!(
                at_change <= inside * 1.5 + 0.5,
                "{pattern}: {at_change} at a change vs {inside} inside"
            );
        }
    }

    #[test]
    fn an_fx_spec_draws_them_and_a_palette_is_an_error() {
        for id in BOTS {
            let doc = format!(
                r#"{{"fxSpec":"1.13","object":"character","pattern":"{id}","expression":"happy"}}"#
            );
            assert!(crate::fx_spec::resolve(&doc).diagnostics.is_empty());
            assert!(crate::frame_from_fx_spec(doc, 1.0).is_some_and(|f| !f.fills.is_empty()));
            let doc = format!(
                r##"{{"fxSpec":"1.13","object":"character","pattern":"{id}","palette":{{"primary":"#ff0000"}}}}"##
            );
            let d = crate::fx_spec::resolve(&doc).diagnostics;
            assert!(
                d.iter()
                    .any(|x| x.path == "/palette" && x.message.contains("no palette")),
                "{d:?}"
            );
        }
    }

    #[test]
    fn a_spring_settles() {
        assert_eq!(spring(-1.0, 2.0, 0.2), 0.0);
        assert!(spring(3.0, 2.0, 0.2).abs() < 0.01);
        // Same t, same value: no state between frames.
        assert_eq!(spring(0.37, 2.0, 0.2), spring(0.37, 2.0, 0.2));
    }

    #[test]
    fn a_follower_lags_its_drive() {
        // 0 at rest; its peak comes a little after the drive's (a quarter period).
        assert!(follow(0.0, 0.5, 3.0, 0.3).abs() < 1e-12);
        let q = 0.5;
        assert!(follow(q + 0.05, 0.5, 3.0, 0.3) > follow(q - 0.05, 0.5, 3.0, 0.3));
    }
}
