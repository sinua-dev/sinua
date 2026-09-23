//! Material: **particles** (materials phase 3) -- a layer of small ink dots
//! emitted from the state's own geometry (dots, polyline vertices, fill
//! points), drifting out, flowing in, orbiting or rising, then fading. Plain
//! `Dot`s with the emitter's ink, so every renderer, exporter and later
//! material (glow, blur, colour, liquid) handles them with no change.
//!
//! **Stateless**, the way GPU particle systems do it: Lutz Latta, *Building
//! a Million Particle System* (GDC 2004) -- a particle is "computed from its
//! birth to its death by a closed form function which is defined by a set of
//! start values and the current time", needing "no extra storage for
//! intermediate particle state"; the lifecycle is `life = floor(t / L)`,
//! `age = frac(t / L)` (GameDev.net, *Stateless particles: time alive*), and
//! each life's start values are hashed from the particle id and the life
//! number, so a particle is re-born somewhere new, deterministically.
//!
//! Not copied: tsParticles / particles.js integrate velocities frame by frame
//! (plus collisions, links, bounce "outModes") -- stateful simulation this
//! engine doesn't carry. Only their vocabulary (count, direction, attract,
//! life) informed the knobs, with the plan doc's section 12 set ("count,
//! size, spread, attraction").

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use crate::primitives::{hash_d, Dot, OrbFrame};

fn get(o: &HashMap<String, f64>, key: &str, default: f64) -> f64 {
    o.get(key).copied().unwrap_or(default)
}

/// One emitter: position, size, and the ink it lends its particles.
#[derive(Clone, Copy)]
struct Emitter {
    x: f64,
    y: f64,
    /// Particles leave from the mark's edge, not its centre (a ping's big
    /// dot would otherwise swallow its own burst).
    r: f64,
    z: f64,
    white: f64,
    a: f64,
    saturation: f64,
    hue: f64,
}

fn emitters(f: &OrbFrame) -> Vec<Emitter> {
    let mut out: Vec<Emitter> = f
        .dots
        .iter()
        .filter(|d| d.a > 0.02)
        .map(|d| Emitter {
            x: d.x,
            y: d.y,
            r: d.r,
            z: d.z,
            white: d.white,
            a: d.a,
            saturation: d.saturation,
            hue: d.hue,
        })
        .collect();
    // Dots when there are any, else strokes and fills -- never a mix. A
    // mixed set let a ping's fading ring or a broadcast's cycling arcs pull
    // the nearest emitter away mid-life and the particles jumped (motion
    // strips, 2026-09-19); the dot set of a state never switches like that.
    if !out.is_empty() {
        return out;
    }
    for p in &f.polylines {
        for q in &p.points {
            out.push(Emitter {
                x: q.x,
                y: q.y,
                r: p.w * 0.5,
                z: 0.0,
                white: p.white,
                a: p.a,
                saturation: p.saturation,
                hue: p.hue,
            });
        }
    }
    for x in &f.fills {
        for q in &x.points {
            out.push(Emitter {
                x: q.x,
                y: q.y,
                r: 0.0,
                z: 0.0,
                white: x.white,
                a: x.a,
                saturation: x.saturation,
                hue: x.hue,
            });
        }
    }
    out
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Per-mode particle defaults (2026-09-19, user-approved): what particles
/// *mean* on each object. Applied only while particles are on, and only to
/// keys the caller left unset -- explicit values (Studio knobs, FX Spec
/// `materials.particles`) always win; the same mechanism as liquid's.
/// Styles: 0 drift, 1 attract, 2 orbit, 3 rise.
pub fn mode_defaults(mode: &str) -> &'static [(&'static str, f64)] {
    match mode {
        // Ambient orbs (breathing, glowing): a slow, sparse orbit.
        "ring" | "aurora" => &[
            ("particleStyle", 2.0),
            ("particleCount", 20.0),
            ("particleLife", 6.0),
            ("particleSpread", 0.12),
        ],
        // Drifting (2026-09-19, after the user watched it live): its nodes
        // move fast (3.3x), so an orbit read as a band on the rim and a
        // node-following drift rode the nodes. Anchored drift instead: born
        // at a fixed point by the silhouette, out and away, dissolving.
        "webflow" => &[
            ("particleStyle", 0.0),
            ("particleAnchor", 1.0),
            ("particleCount", 22.0),
            ("particleLife", 6.0),
            ("particleSpread", 0.22),
        ],
        // Speaking and the signal family: drift that follows the voice.
        "spectrum" | "bar" | "waveform" | "scroll" | "matrix" => &[
            ("particleAudio", 1.0),
            ("particleCount", 32.0),
            ("particleLife", 3.0),
        ],
        // Ring family (progress, done): a few particles lifting off.
        "arc" | "spinner" | "nested" | "segmented" | "gauge" => &[
            ("particleStyle", 3.0),
            ("particleCount", 12.0),
            ("particleLife", 5.0),
            ("particleSpread", 0.2),
        ],
        // Beacon: scanning gathers a signal in; notifying bursts outward
        // with each ping; broadcasting radiates out along its waves.
        "radar" => &[
            ("particleStyle", 1.0),
            ("particleCount", 24.0),
            ("particleSpread", 0.3),
        ],
        "ping" => &[
            ("particleSync", 1.0),
            ("particleCount", 16.0),
            ("particleLife", 2.4),
            ("particleSpread", 0.28),
        ],
        "broadcast" => &[("particleCount", 18.0), ("particleSpread", 0.24)],
        "pulse" | "halo" => &[("particleCount", 14.0)],
        // Core (generating, typing): minimal.
        "shimmer" | "dots" => &[
            ("particleCount", 8.0),
            ("particleSpread", 0.1),
            ("particleLife", 5.0),
        ],
        _ => &[],
    }
}

/// The engine-wide particle defaults (what `apply_particles` uses for an
/// unset key on a mode without its own).
pub const BASE_DEFAULTS: [(&str, f64); 7] = [
    ("particleCount", 28.0),
    ("particleSize", 0.8),
    ("particleSpread", 0.18),
    ("particleLife", 4.5),
    ("particleStyle", 0.0),
    ("particleSync", 0.0),
    ("particleAudio", 0.0),
];

fn base(key: &str) -> f64 {
    BASE_DEFAULTS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(0.0, |(_, v)| *v)
}

/// Every particle knob's default for `mode`: the base table with the mode's
/// own values over it -- what the Studio shows as the knob defaults.
pub fn defaults_for(mode: &str) -> HashMap<String, f64> {
    let mut d: HashMap<String, f64> = BASE_DEFAULTS
        .iter()
        .map(|(k, v)| ((*k).to_string(), *v))
        .collect();
    for (k, v) in mode_defaults(mode) {
        d.insert((*k).to_string(), *v);
    }
    d
}

/// `opts` with the mode's particle defaults filled in where unset -- only
/// when particles are on, so every other frame is untouched.
pub fn with_mode_defaults<'a>(
    mode: &str,
    opts: &'a HashMap<String, f64>,
) -> std::borrow::Cow<'a, HashMap<String, f64>> {
    let on = get(opts, "particleStrength", 0.0) > 0.0;
    let missing = mode_defaults(mode)
        .iter()
        .any(|(k, _)| !opts.contains_key(*k));
    if !on || !missing {
        return std::borrow::Cow::Borrowed(opts);
    }
    let mut o = opts.clone();
    for (k, v) in mode_defaults(mode) {
        o.entry((*k).to_string()).or_insert(*v);
    }
    std::borrow::Cow::Owned(o)
}

/// `particleStrength` > 0: `particleCount` particles emitted from the
/// frame's geometry; `particleStyle` 0 drift (out) / 1 attract (in) /
/// 2 orbit / 3 rise. See docs/materials.md.
///
/// `t` is **wall-clock** seconds (`render` divides the preset speed back
/// out), so `particleLife` is real seconds on every state. Particles stay
/// inside the canvas: they fade out over the last `0.08 * size` before the
/// edge instead of being clipped.
pub fn apply_particles(
    mut frame: OrbFrame,
    size: f64,
    t: f64,
    o: &HashMap<String, f64>,
) -> OrbFrame {
    let strength = get(o, "particleStrength", 0.0).clamp(0.0, 1.0);
    if strength <= 0.0 {
        return frame;
    }
    // Calm defaults (2026-09-19, after the user watched it live): a 4.5 s
    // life and a short travel -- a slow shed, not a spray.
    let count = get(o, "particleCount", base("particleCount"))
        .round()
        .clamp(0.0, 200.0) as usize;
    let size_k = get(o, "particleSize", base("particleSize")).clamp(0.05, 4.0);
    let spread = get(o, "particleSpread", base("particleSpread")).clamp(0.0, 1.0) * size;
    let life = get(o, "particleLife", base("particleLife")).clamp(0.1, 30.0);
    let style = get(o, "particleStyle", base("particleStyle"))
        .round()
        .clamp(0.0, 3.0) as u8;
    let seed = get(o, "particleSeed", 0.0);
    // `particleAnchor` (internal, drift only; `webflow`'s default): each
    // life is born at a fixed *world* point instead of riding its emitter --
    // Unity's "World" simulation space ("once emitted, particles won't
    // follow the moving GameObject") vs "Local" ("moving with the parent").
    // Stateless: the birth point is the frame centre plus the life's hashed
    // direction at 0.85 x the rim, so it can't depend on which node is
    // nearest now. The emitter lends only its ink.
    let anchor = style == 0 && get(o, "particleAnchor", 0.0) >= 0.5;
    // 1.6: `particleSync` 0..1 pulls every particle's birth together (1 = a
    // burst each life); `particleAudio` 0..1 couples brightness to the
    // caller's `audioLevel` when one is present -- stateless, so it scales
    // alpha by the *current* level, `lerp(1, 0.25 + 0.75 * level, audio)`;
    // positions never depend on it, so a jumpy level can't make them jump.
    let sync = get(o, "particleSync", base("particleSync")).clamp(0.0, 1.0);
    let audio = match o.get("audioLevel") {
        Some(level) => {
            let k = get(o, "particleAudio", base("particleAudio")).clamp(0.0, 1.0);
            1.0 + k * (0.25 + 0.75 * level.clamp(0.0, 1.0) - 1.0)
        }
        None => 1.0,
    };
    let src = emitters(&frame);
    if count == 0 || src.is_empty() {
        return frame;
    }
    // Centroid of the emitters (drift/attract/orbit reference).
    let n = src.len() as f64;
    let (cx, cy) = src
        .iter()
        .fold((0.0, 0.0), |(x, y), e| (x + e.x / n, y + e.y / n));
    // The rim: the farthest emitter from the centroid (stable as shapes spin).
    let rim = src
        .iter()
        .map(|e| ((e.x - cx).powi(2) + (e.y - cy).powi(2)).sqrt())
        .fold(0.0, f64::max);
    // Anchored drift's reference: the frame centre (the emitter centroid
    // wobbles with the nodes), the rim measured from it, and the emitters'
    // mean alpha (a nearest-node swap must not make a particle blink).
    let (ax, ay) = (0.5 * size, 0.5 * size);
    let anchor_rim = src
        .iter()
        .map(|e| ((e.x - ax).powi(2) + (e.y - ay).powi(2)).sqrt())
        .fold(0.0, f64::max);
    let mean_a = src.iter().map(|e| e.a).sum::<f64>() / n;
    let r_med = median(frame.dots.iter().map(|d| d.r).collect());
    // Follows the state's dot size, but capped both ways: a ping's single
    // big dot must not make boulders, a hairline state not specks (contact
    // sheet, 2026-09-19).
    let base = if r_med > 0.0 {
        r_med
    } else {
        0.012 * size / 0.6
    };
    // Capped at 1.8% of the frame (was 2.5%): at 2.5% x the 0.8 default,
    // a big-dot state's particles read as dark blobs in motion.
    let radius = (base * 0.6).clamp(0.008 * size, 0.018 * size) * (size_k / 0.6);

    for i in 0..count {
        let fi = i as f64 + 1.0;
        let li = life * (1.0 + (1.0 - sync) * 0.6 * (hash_d(fi, seed + 11.0) - 0.5));
        let phase = li * hash_d(fi, seed + 23.0) * (1.0 - sync);
        let k = ((t + phase) / li).floor();
        let u = (t + phase) / li - k;
        // This life's start values, from (id, life number).
        let h = |salt: f64| hash_d(fi * 7.31 + k * 0.917, seed + salt);
        // The emitter is found by *position*, never by list index: orbs
        // z-sort their dots every frame, so an index picks a different dot
        // each frame and the particle jumps (2026-09-19, seen live). Each
        // life hashes a fixed target -- on the rim circle for drift / orbit /
        // rise (born at the silhouette, not inside a dense cloud where they
        // vanish), anywhere in the disc for attract -- and takes the nearest
        // emitter to it, which only changes by a neighbour's spacing as the
        // geometry moves. The direction is the target's, so it never jumps.
        let theta = TAU * h(5.0);
        let reach = if style == 1 { rim * h(3.0).sqrt() } else { rim };
        let (tx, ty) = (cx + reach * theta.cos(), cy + reach * theta.sin());
        let e = *src
            .iter()
            .min_by(|a, b| {
                let da = (a.x - tx).powi(2) + (a.y - ty).powi(2);
                let db = (b.x - tx).powi(2) + (b.y - ty).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("src is not empty");
        let out_angle = theta;
        let jitter = (h(7.0) - 0.5) * 0.9;
        let travel = spread * (0.6 + 0.4 * h(9.0));
        // Gentle ease-out travel: a little faster off the emitter (1.6x the
        // mean speed, was 2x), settling as it fades.
        let s = u * (1.6 - 0.6 * u);
        let (px, py) = if anchor {
            // Anchored drift: from the fixed birth point, straight out.
            let a = out_angle + jitter;
            let (bx, by) = (
                ax + 0.85 * anchor_rim * out_angle.cos(),
                ay + 0.85 * anchor_rim * out_angle.sin(),
            );
            let d = travel * s;
            (bx + d * a.cos(), by + d * a.sin())
        } else {
            match style {
                1 => {
                    // Attract: start out at the travel distance, flow in.
                    let a = out_angle + jitter;
                    let d = e.r + travel * (1.0 - s);
                    (e.x + d * a.cos(), e.y + d * a.sin())
                }
                2 => {
                    // Orbit: turn around the centroid just outside the emitter
                    // (so they read against the shape), and slowly: a quarter
                    // to three quarters of a half-turn per life.
                    // The radius is the life's rim target's, not the emitter's:
                    // a fast state's nearest node changes while it orbits.
                    // Capped inside the edge-fade band so a frame-filling cloud's
                    // orbit doesn't blink out at the canvas edge.
                    let rr = (reach + e.r + travel * (0.3 + 0.5 * h(13.0)))
                        .min(0.5 * size - 0.09 * size - radius);
                    let a = out_angle
                        + (0.25 + 0.5 * h(15.0)) * PI * s * if h(17.0) < 0.5 { 1.0 } else { -1.0 };
                    (cx + rr * a.cos(), cy + rr * a.sin())
                }
                3 => {
                    // Rise: up (screen -y), drifting sideways a little.
                    (e.x + travel * jitter * 0.5 * s, e.y - e.r - travel * s)
                }
                _ => {
                    // Drift: out from the centroid, with an angle jitter.
                    let a = out_angle + jitter;
                    let d = e.r + travel * s;
                    (e.x + d * a.cos(), e.y + d * a.sin())
                }
            }
        };
        // Sideways wobble, hashed per life.
        let w = (TAU * (u * 1.5 + h(19.0))).sin() * radius * 0.8;
        let (nx, ny) = (-(out_angle).sin(), out_angle.cos());
        // A floor on the emitter's alpha: faint scope/depth dots would
        // otherwise hide their particles entirely.
        let (qx, qy) = (px + nx * w, py + ny * w);
        let mut r = radius * (0.7 + 0.6 * h(21.0));
        if anchor {
            // Dissolve: shrink to 60% over the life.
            r *= 1.0 - 0.4 * u;
        }
        // Inside the canvas, never clipped: fade to 0 as the dot's edge
        // reaches the frame's (smoothstep over the last 8% of the size).
        let room = qx.min(qy).min(size - qx).min(size - qy) - r;
        let e_fade = {
            let x = (room / (0.08 * size)).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let a = if anchor {
            // A quick fade-in, then a slow dissolve.
            let env = (u / 0.12).min(1.0) * (1.0 - u).powf(1.5);
            strength * audio * env * mean_a.max(0.7) * e_fade
        } else {
            strength * audio * (PI * u).sin() * e.a.max(0.7) * e_fade
        };
        if a < 0.02 {
            continue;
        }
        frame.dots.push(Dot {
            x: qx,
            y: qy,
            z: e.z,
            r,
            white: e.white,
            a: a.min(1.0),
            saturation: e.saturation,
            hue: e.hue,
        });
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{Point, Polyline};

    fn o(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn ring(n: usize) -> OrbFrame {
        OrbFrame {
            dots: (0..n)
                .map(|k| {
                    let a = k as f64 / n as f64 * TAU;
                    Dot {
                        x: 32.0 + 12.0 * a.cos(),
                        y: 32.0 + 12.0 * a.sin(),
                        r: 1.5,
                        white: 0.15,
                        a: 0.9,
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn off_and_empty_are_no_ops_and_output_is_deterministic() {
        let f = ring(12);
        assert_eq!(apply_particles(f.clone(), 64.0, 1.0, &o(&[])), f);
        assert_eq!(
            apply_particles(
                OrbFrame::default(),
                64.0,
                1.0,
                &o(&[("particleStrength", 1.0)])
            ),
            OrbFrame::default()
        );
        let on = o(&[("particleStrength", 1.0)]);
        let a = apply_particles(f.clone(), 64.0, 1.7, &on);
        assert_eq!(
            a,
            apply_particles(f.clone(), 64.0, 1.7, &on),
            "a pure function of t"
        );
        assert!(a.dots.len() > 12 && a.dots.len() <= 12 + 28);
        assert_eq!(
            &a.dots[..12],
            &f.dots[..],
            "the state's own dots come first, untouched"
        );
        let other = apply_particles(
            f,
            64.0,
            1.7,
            &o(&[("particleStrength", 1.0), ("particleSeed", 5.0)]),
        );
        assert_ne!(a, other, "the seed changes the field");
    }

    #[test]
    fn styles_move_the_right_way() {
        let f = ring(12);
        let dist = |d: &Dot| ((d.x - 32.0).powi(2) + (d.y - 32.0).powi(2)).sqrt();
        let mean = |style: f64, t: f64| {
            let g = apply_particles(
                f.clone(),
                64.0,
                t,
                &o(&[
                    ("particleStrength", 1.0),
                    ("particleStyle", style),
                    ("particleCount", 60.0),
                ]),
            );
            let p = &g.dots[12..];
            (
                p.iter().map(dist).sum::<f64>() / p.len() as f64,
                p.iter().map(|d| d.y).sum::<f64>() / p.len() as f64,
            )
        };
        let (drift, _) = mean(0.0, 1.3);
        let (attract, _) = mean(1.0, 1.3);
        let (orbit, _) = mean(2.0, 1.3);
        let (_, rise_y) = mean(3.0, 1.3);
        assert!(drift > 12.5, "drift goes out past the ring: {drift}");
        assert!(
            attract > 12.0 && attract < drift,
            "attract comes back in: {attract}"
        );
        assert!(
            orbit > 12.0 && orbit < 12.0 + 0.18 * 64.0,
            "orbit circles just outside the ring: {orbit}"
        );
        assert!(rise_y < 32.0, "rise goes up: {rise_y}");
    }

    #[test]
    fn lifecycles_fade_in_and_out() {
        // One particle over one life: alpha rises then falls with sin(pi u).
        let f = ring(1);
        let on = o(&[
            ("particleStrength", 1.0),
            ("particleCount", 1.0),
            ("particleLife", 1.0),
        ]);
        let alphas: Vec<f64> = (0..800)
            .map(|k| {
                let g = apply_particles(f.clone(), 64.0, k as f64 * 0.002, &on);
                g.dots.get(1).map_or(0.0, |d| d.a)
            })
            .collect();
        assert!(alphas.iter().any(|a| *a > 0.8));
        assert!(
            alphas.contains(&0.0),
            "dies (culled) near the ends of a life"
        );
    }

    #[test]
    fn emitters_are_found_by_position_not_list_order() {
        // Orbs re-sort their dots every frame; the particle field must not care.
        let f = ring(16);
        let mut g = f.clone();
        g.dots.reverse();
        g.dots.rotate_left(5);
        let on = o(&[("particleStrength", 1.0), ("particleCount", 40.0)]);
        let a = apply_particles(f, 64.0, 2.3, &on);
        let b = apply_particles(g, 64.0, 2.3, &on);
        assert_eq!(&a.dots[16..], &b.dots[16..]);
    }

    #[test]
    fn sync_bursts_and_audio_scales_brightness() {
        let f = ring(12);
        let alive = |extra: &[(&str, f64)], t: f64| {
            let mut on = o(&[("particleStrength", 1.0), ("particleCount", 20.0)]);
            on.extend(extra.iter().map(|(k, v)| (k.to_string(), *v)));
            apply_particles(f.clone(), 64.0, t, &on).dots[12..].to_vec()
        };
        // Sync 1: every particle is born together -- none at a life's ends, all mid-life.
        let s = [("particleSync", 1.0), ("particleLife", 2.0)];
        assert!(alive(&s, 0.001).is_empty() && alive(&s, 1.999).is_empty());
        assert_eq!(alive(&s, 1.0).len(), 20);
        // Audio: no audioLevel = unchanged; silence dims to a quarter; loud = full.
        let a0 = alive(&[("particleAudio", 1.0)], 1.3);
        assert_eq!(a0, alive(&[], 1.3));
        let quiet = alive(&[("particleAudio", 1.0), ("audioLevel", 0.0)], 1.3);
        let loud = alive(&[("particleAudio", 1.0), ("audioLevel", 1.0)], 1.3);
        assert_eq!(loud, a0);
        let sum = |v: &[Dot]| v.iter().map(|d| d.a).sum::<f64>();
        assert!(sum(&quiet) < 0.3 * sum(&loud));
        assert!(
            quiet
                .iter()
                .zip(&loud)
                .all(|(q, l)| q.x == l.x && q.y == l.y)
                || quiet.len() < loud.len()
        );
    }

    #[test]
    fn particles_stay_inside_the_canvas() {
        // A ring near the frame's edge, a long spread: every particle drawn
        // is fully inside, and the ones near the edge are faded.
        let f = OrbFrame {
            dots: (0..24)
                .map(|k| {
                    let a = k as f64 / 24.0 * TAU;
                    Dot {
                        x: 32.0 + 28.0 * a.cos(),
                        y: 32.0 + 28.0 * a.sin(),
                        r: 1.0,
                        white: 0.2,
                        a: 1.0,
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        };
        let on = o(&[
            ("particleStrength", 1.0),
            ("particleCount", 200.0),
            ("particleSpread", 0.6),
        ]);
        let mut seen = 0;
        for k in 0..40 {
            let g = apply_particles(f.clone(), 64.0, k as f64 * 0.37, &on);
            for d in &g.dots[24..] {
                seen += 1;
                assert!(
                    d.x - d.r >= 0.0 && d.y - d.r >= 0.0 && d.x + d.r <= 64.0 && d.y + d.r <= 64.0,
                    "clipped: {d:?}"
                );
            }
        }
        assert!(seen > 100, "still emits: {seen}");
    }

    #[test]
    fn strokes_and_fills_emit_too() {
        let f = OrbFrame {
            polylines: vec![Polyline {
                points: vec![Point { x: 10.0, y: 32.0 }, Point { x: 54.0, y: 32.0 }],
                white: 0.2,
                a: 0.8,
                w: 2.0,
                saturation: 0.5,
                hue: 120.0,
                hues: Vec::new(),
            }],
            ..Default::default()
        };
        let g = apply_particles(f, 64.0, 1.0, &o(&[("particleStrength", 1.0)]));
        assert!(!g.dots.is_empty());
        assert!(
            g.dots.iter().all(|d| d.hue == 120.0 && d.saturation == 0.5),
            "the emitter's ink"
        );
    }

    #[test]
    fn anchored_drift_is_born_in_world_space_and_moves_out() {
        let on = o(&[
            ("particleStrength", 1.0),
            ("particleAnchor", 1.0),
            ("particleCount", 24.0),
            ("particleLife", 6.0),
            ("particleSpread", 0.22),
        ]);
        // Rotating the emitter ring (same rim, nodes moved along it) must
        // not move an anchored particle: it no longer rides a node.
        let mut turned = ring(12);
        for d in turned.dots.iter_mut() {
            let (x, y) = (d.x - 32.0, d.y - 32.0);
            let a: f64 = 0.2;
            (d.x, d.y) = (
                32.0 + x * a.cos() - y * a.sin(),
                32.0 + x * a.sin() + y * a.cos(),
            );
        }
        let p = |f: OrbFrame| -> Vec<(f64, f64)> {
            apply_particles(f, 64.0, 2.0, &on).dots[12..]
                .iter()
                .map(|d| (d.x, d.y))
                .collect()
        };
        let (a, b) = (p(ring(12)), p(turned));
        assert_eq!(a.len(), b.len());
        for (u, v) in a.iter().zip(&b) {
            assert!((u.0 - v.0).abs() < 1e-9 && (u.1 - v.1).abs() < 1e-9);
        }
        // Over a life, distance from the centre only grows (wobble aside:
        // compare the start and end of each particle's first life).
        let one = o(&[
            ("particleStrength", 1.0),
            ("particleAnchor", 1.0),
            ("particleCount", 1.0),
            ("particleLife", 6.0),
            ("particleSync", 1.0),
            ("particleSpread", 0.22),
        ]);
        let dist = |t: f64| {
            let f = apply_particles(ring(12), 64.0, t, &one);
            f.dots
                .get(12)
                .map(|d| ((d.x - 32.0).powi(2) + (d.y - 32.0).powi(2)).sqrt())
        };
        let (d0, d1) = (dist(0.9).unwrap(), dist(4.5).unwrap());
        assert!(d1 > d0 + 5.0, "moves out: {d0} -> {d1}");
        assert!(d0 > 0.8 * 12.0, "born by the rim: {d0}");
        // Anchor is drift-only: other styles ignore it.
        for style in [1.0, 2.0, 3.0] {
            let mut with = on.clone();
            with.insert("particleStyle".into(), style);
            let mut without = with.clone();
            without.remove("particleAnchor");
            assert_eq!(
                apply_particles(ring(12), 64.0, 2.0, &with),
                apply_particles(ring(12), 64.0, 2.0, &without)
            );
        }
    }

    #[test]
    fn drifting_gets_anchored_drift_and_the_other_ambient_orbs_keep_orbit() {
        let w = defaults_for("webflow");
        assert_eq!(w["particleStyle"], 0.0);
        assert_eq!(w["particleAnchor"], 1.0);
        assert_eq!(w["particleLife"], 6.0);
        for m in ["ring", "aurora"] {
            let d = defaults_for(m);
            assert_eq!(d["particleStyle"], 2.0, "{m}");
            assert!(!d.contains_key("particleAnchor"), "{m}");
        }
    }
}
