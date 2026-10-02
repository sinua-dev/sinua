//! CUPPA's tests (design note 14): a path body with a hole (the handle), and
//! the `steam` part: soft wisps that rise, fade at the top, rise higher while
//! listening, and are left out at 20 px and with `accessories` off.

use crate::primitives::{Fill, ModeOpts, OrbFrame};

fn frame(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("cuppa", size, t, o).expect("a built-in recipe")
}

fn opts(kv: &[(&str, f64)]) -> ModeOpts {
    kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// The steam's fills: the only blurred fills in the steam's grey.
fn steam(f: &OrbFrame) -> Vec<&Fill> {
    f.fills
        .iter()
        .filter(|x| {
            x.blur > 0.0 && (x.hue - 25.0).abs() < 1e-9 && (x.saturation - 0.12).abs() < 1e-9
        })
        .collect()
}

fn top(fs: &[&Fill]) -> f64 {
    fs.iter()
        .flat_map(|f| f.points.iter())
        .map(|p| p.y)
        .fold(f64::MAX, f64::min)
}

#[test]
fn the_handle_is_a_hole_with_its_own_outline() {
    let f = frame(200.0, 0.0, &opts(&[]));
    assert!(
        f.fills
            .iter()
            .any(|x| x.gradient.is_some() && x.holes.len() == 1),
        "the mug has the hole"
    );
    let line = f
        .fills
        .iter()
        .filter(|x| (x.hue - 10.0).abs() < 1e-9 && x.holes.len() == 1)
        .count();
    assert!(line >= 2, "the mug's edge and the handle's: {line}");
}

#[test]
fn steam_rises_and_fades_and_is_left_out_when_small_or_without_accessories() {
    let idle = opts(&[]);
    let fs: Vec<OrbFrame> = (0..12)
        .map(|i| frame(200.0, i as f64 * 0.15, &idle))
        .collect();
    for f in &fs {
        let s = steam(f);
        assert!(!s.is_empty() && s.len() <= 3, "{}", s.len());
        assert!(s.iter().all(|x| x.a > 0.0 && x.a <= 1.0));
    }
    // A wisp fades in and out over its rise: its alpha changes frame to frame.
    let alphas: Vec<f64> = fs
        .iter()
        .map(|f| steam(f).iter().map(|x| x.a).sum())
        .collect();
    assert!(
        alphas.windows(2).any(|w| (w[0] - w[1]).abs() > 0.01),
        "{alphas:?}"
    );
    assert!(steam(&frame(20.0, 0.4, &idle)).is_empty(), "not at 20 px");
    assert!(steam(&frame(64.0, 0.4, &opts(&[("accessories", 0.0)]))).is_empty());
}

#[test]
fn listening_lifts_the_steam_and_it_stays_in_the_box() {
    let t = 0.4;
    let idle = top(&steam(&frame(200.0, t, &opts(&[]))));
    let listening = top(&steam(&frame(
        200.0,
        t,
        &opts(&[("earGain", 1.0), ("look", 0.0)]),
    )));
    assert!(
        listening < idle - 2.0,
        "higher while listening: {listening} vs {idle}"
    );
    for t in [0.0, 0.5, 1.0, 1.5, 2.0] {
        for o in [
            opts(&[("earGain", 1.0)]),
            opts(&[("mouthTalk", 1.0), ("audioLevel", 1.0)]),
        ] {
            assert!(top(&steam(&frame(200.0, t, &o))) >= 0.0);
        }
    }
}
