//! BEAN's tests (design note 14): a path body with path patches (the groove and
//! its lit edge) clipped inside it, feet, aroma sparkles.

use crate::primitives::{ModeOpts, OrbFrame};

fn frame(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("bean", size, t, o).expect("a built-in recipe")
}

#[test]
fn the_groove_is_drawn_inside_the_bean() {
    let r = &crate::character::recipe::recipes()["bean"];
    let body = r
        .parts
        .iter()
        .find(|p| p.kind == crate::character::parts::Kind::Body)
        .unwrap();
    assert!(body.params.shapes.iter().any(|s| s.path), "a path body");
    assert!(
        body.inner
            .iter()
            .filter(|l| !l.params.shapes.is_empty())
            .count()
            >= 2,
        "two path patches"
    );
    let f = frame(200.0, 0.0, &ModeOpts::new());
    // The groove's dark: a solid fill, every point inside the bean's box.
    let groove: Vec<_> = f
        .fills
        .iter()
        .filter(|x| (x.hue - 18.0).abs() < 1e-9 && x.gradient.is_none() && x.holes.is_empty())
        .collect();
    assert!(!groove.is_empty());
    for p in groove.iter().flat_map(|x| x.points.iter()) {
        assert!(
            p.x > 40.0 && p.x < 160.0 && p.y > 28.0 && p.y < 180.0,
            "{p:?}"
        );
    }
}
