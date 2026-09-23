//! Rubik: bands twist in quarter turns, scramble -> solve -- the "solving"
//! state.
//!
//! Ported 1:1 from `thinking-orbs/src/engine/lattice.ts` (`frameRubik` and
//! its `solveCycle`/`applyMoves`/`makeMoves` helpers).

use crate::orbs::core::{finalize_frame, hash_d, radius_scale, Dot, OrbFrame, Proj};
use crate::orbs::profiles::ModeOpts;
use std::f64::consts::PI;

fn get(o: &ModeOpts, key: &str, default: f64) -> f64 {
    *o.get(key).unwrap_or(&default)
}

#[derive(Clone, Copy)]
struct Move {
    axis: u8, // 0, 1, or 2
    lo: f64,
    hi: f64,
    ang: f64,
}

struct SolveCycle {
    amount: Vec<f64>,
    active: i64,
}

// Rapid eased moves scramble, then replay in reverse (palindrome) so
// everything clicks back to solved, rests, repeats.
fn solve_cycle(time: f64, count: usize, slot_dur: f64, rest: f64) -> SolveCycle {
    let cyc = 2.0 * count as f64 * slot_dur + rest;
    let tc = time.rem_euclid(cyc);
    let mut amount = vec![0.0; count];
    let mut active: i64 = -1;
    if tc < 2.0 * count as f64 * slot_dur {
        let slot = (tc / slot_dur).floor() as i64;
        let p = (tc - slot as f64 * slot_dur) / slot_dur;
        let cl = (p / 0.7).min(1.0);
        let ep = 1.0 - (1.0 - cl).powi(3); // machine ease-out
        if (slot as usize) < count {
            for a in amount.iter_mut().take(slot as usize) {
                *a = 1.0;
            }
            amount[slot as usize] = ep;
            active = slot;
        } else {
            let u = 2 * count as i64 - 1 - slot;
            for a in amount.iter_mut().take(u as usize) {
                *a = 1.0;
            }
            amount[u as usize] = 1.0 - ep;
            active = u;
        }
    }
    SolveCycle { amount, active }
}

fn apply_moves(pt3: (f64, f64, f64), moves: &[Move], sc: &SolveCycle) -> (f64, f64, f64, bool) {
    let (mut x, mut y, mut z) = pt3;
    let mut in_active = false;
    for (i, mv) in moves.iter().enumerate() {
        if sc.amount[i] <= 0.0 {
            continue;
        }
        let coord = match mv.axis {
            0 => x,
            1 => y,
            _ => z,
        };
        if coord < mv.lo || coord >= mv.hi {
            continue;
        }
        if i as i64 == sc.active {
            in_active = true;
        }
        let a = mv.ang * sc.amount[i];
        let ca = a.cos();
        let sa = a.sin();
        match mv.axis {
            0 => {
                let y2 = y * ca - z * sa;
                z = y * sa + z * ca;
                y = y2;
            }
            1 => {
                let x2 = x * ca + z * sa;
                z = -x * sa + z * ca;
                x = x2;
            }
            _ => {
                let x2 = x * ca - y * sa;
                y = x * sa + y * ca;
                x = x2;
            }
        }
    }
    (x, y, z, in_active)
}

fn make_moves(count: usize) -> Vec<Move> {
    let mut moves = Vec::with_capacity(count);
    for i in 0..count {
        let i_f = i as f64;
        let axis = ((hash_d(i_f, 2.3) * 3.0).floor() as u8).min(2);
        let lo = -1.0 + 0.5 * (hash_d(i_f, 5.9) * 4.0).floor().min(3.0);
        let dir = if hash_d(i_f, 7.7) < 0.5 { 1.0 } else { -1.0 };
        moves.push(Move {
            axis,
            lo,
            hi: lo + 0.5,
            ang: dir * PI / 2.0,
        });
    }
    moves
}

pub fn frame_rubik(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    let cx = size / 2.0;
    let cy = size / 2.0;
    let r = (size / 2.0) * 0.82;
    let pt = Proj::new(t * 0.55, 0.35 + 0.1 * (t * 0.9).sin(), cx, cy, r);
    let rs = radius_scale(size, get(o, "rsPow", 0.6));
    let move_count = get(o, "moveCount", 14.0) as usize;
    let moves = make_moves(move_count);
    let sc = solve_cycle(t, move_count, 0.42, 1.2);

    let lat_rings = get(o, "latRings", 15.0) as i64;
    let lon_density = get(o, "lonDensity", 40.0);
    let r_base = get(o, "rBase", 0.6);
    let r_depth = get(o, "rDepth", 1.7);
    let r_active = get(o, "rActive", 0.3);
    let ink_far = get(o, "inkFar", 0.62);
    let ink_span = get(o, "inkSpan", 0.54);

    let mut dots: Vec<Dot> = Vec::new();
    for li in 0..=lat_rings {
        let lat = -PI / 2.0 + (li as f64 / lat_rings as f64) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let lon_count = ((cos_lat.abs() * lon_density).round() as i64).max(1);
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let (x, y, z, in_active) = apply_moves(
                (cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin()),
                &moves,
                &sc,
            );
            let (px, py, zr) = pt.project(x, y, z);
            let depth = (zr + 1.0) / 2.0;
            // the band being turned inks a touch darker -- the "hand"
            dots.push(Dot {
                x: px,
                y: py,
                z: zr,
                r: (r_base + r_depth * depth + if in_active { r_active } else { 0.0 }) * rs,
                white: ink_far - ink_span * depth - if in_active { 0.14 } else { 0.0 },
                a: 1.0,
                ..Default::default()
            });
        }
    }
    finalize_frame(dots, vec![], get(o, "rMin", 0.3))
}
