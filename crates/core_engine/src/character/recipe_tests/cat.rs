//! A path-bodied test recipe (design note 13): a cat head whose ears make the
//! body concave, a forehead band that must clip into one piece per ear, a path
//! muzzle; and the same head with a hole (the mug's handle case).

use crate::character::recipe::{frame_recipe, Recipe};
use crate::primitives::{Fill, ModeOpts};

pub const CAT: &str = include_str!("cat.json");

fn cat() -> Recipe {
    Recipe::parse(CAT).unwrap()
}

fn opts(kv: &[(&str, f64)]) -> ModeOpts {
    kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn of_hue(fills: &[Fill], hue: f64) -> Vec<&Fill> {
    fills
        .iter()
        .filter(|f| (f.hue - hue).abs() < 1e-9 && f.gradient.is_none())
        .collect()
}

#[test]
fn the_band_shows_on_both_ears() {
    let f = frame_recipe(&cat(), 200.0, 0.0, &opts(&[]));
    let band = of_hue(&f.fills, 280.0);
    assert_eq!(band.len(), 2, "one piece per ear");
    let mid_x = |f: &Fill| f.points.iter().map(|p| p.x).sum::<f64>() / f.points.len() as f64;
    let (a, b) = (mid_x(band[0]), mid_x(band[1]));
    assert!(a.min(b) < 90.0 && a.max(b) > 110.0, "{a} {b}");
    // Nothing of the band between the ears (the dip at x 100 is above y 54).
    for f in &band {
        assert!(f
            .points
            .iter()
            .all(|p| !(p.x > 92.0 && p.x < 108.0 && p.y < 50.0)));
    }
}

#[test]
fn a_hole_is_cut_and_outlined() {
    let holed = CAT.replacen(
        "C40 152 34 100 50 74 Z",
        "C40 152 34 100 50 74 Z M92 150 L108 150 L100 160 Z",
        1,
    );
    let r = Recipe::parse(&holed).unwrap();
    let f = frame_recipe(&r, 200.0, 0.0, &opts(&[]));
    assert!(
        f.fills
            .iter()
            .any(|x| x.gradient.is_some() && x.holes.len() == 1),
        "the body has the hole"
    );
    let line = of_hue(&f.fills, 200.0)
        .into_iter()
        .filter(|x| x.holes.len() == 1)
        .count();
    assert!(line >= 2, "outer edge and hole edge: {line}");
}

#[test]
fn every_pose_is_deterministic_fills_only_and_in_its_box() {
    let r = cat();
    let poses = [
        opts(&[]),
        opts(&[("earGain", 1.0), ("audioLevel", 0.7), ("lean", 3.0)]),
        opts(&[("mouthDots", 1.0), ("turnYaw", -0.7), ("turnPitch", 0.8)]),
        opts(&[
            ("mouthTalk", 1.0),
            ("mouthGain", 1.0),
            ("audioLevel", 0.8),
            ("turn", 1.0),
            ("turnYaw", 0.6),
        ]),
        opts(&[("effectCode", 3.0), ("effectAge", 0.5)]),
    ];
    for size in [20.0, 32.0, 64.0] {
        for o in &poses {
            for t in [0.0, 0.9, 2.3] {
                let a = frame_recipe(&r, size, t, o);
                assert_eq!(
                    format!("{a:?}"),
                    format!("{:?}", frame_recipe(&r, size, t, o))
                );
                assert!(a.dots.is_empty() && a.lines.is_empty() && a.polylines.is_empty());
                for p in a
                    .fills
                    .iter()
                    .flat_map(|f| f.points.iter().chain(f.holes.iter().flatten()))
                {
                    assert!(p.x.is_finite() && p.y.is_finite());
                    assert!(
                        p.x > -0.3 * size
                            && p.x < 1.3 * size
                            && p.y > -0.3 * size
                            && p.y < 1.3 * size
                    );
                }
            }
        }
    }
}

#[test]
fn a_bad_path_says_where() {
    let e = Recipe::parse(&CAT.replacen("L56 22", "A5 5 0 0 1 56 22", 1)).unwrap_err();
    assert!(e.starts_with("/parts/1/shape/path: at 7: arcs"), "{e}");
}
