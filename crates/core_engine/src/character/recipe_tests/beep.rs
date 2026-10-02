//! BEEP's tests (design note 17): the arms follow the voice state (a hand to
//! the ear, to the chin), turn behind the body, switch off, and the robot stays
//! within its cost budget (at most 80 % of the medium line at 64 px).

use crate::primitives::{Fill, ModeOpts, OrbFrame};

fn frame(size: f64, t: f64, o: &ModeOpts) -> OrbFrame {
    crate::character::recipe::frame("beep", size, t, o).expect("a built-in recipe")
}

fn opts(kv: &[(&str, f64)]) -> ModeOpts {
    kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// The hands: the accent-coloured round fills (20 points), as (index, centre).
fn hands(f: &OrbFrame) -> Vec<(usize, f64, f64)> {
    f.fills
        .iter()
        .enumerate()
        .filter(|(_, x)| {
            (x.hue - 24.0).abs() < 1e-9 && x.points.len() == 20 && x.gradient.is_none()
        })
        .map(|(i, x): (usize, &Fill)| {
            let n = x.points.len() as f64;
            (
                i,
                x.points.iter().map(|p| p.x).sum::<f64>() / n,
                x.points.iter().map(|p| p.y).sum::<f64>() / n,
            )
        })
        .collect()
}

#[test]
fn a_hand_goes_to_the_ear_to_listen_and_to_the_chin_to_think() {
    let listening = frame(200.0, 1.0, &opts(&[("earGain", 1.0), ("look", 0.0)]));
    let h = hands(&listening);
    assert_eq!(h.len(), 2);
    let right = h.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    assert!(
        right.1 > 150.0 && right.2 < 90.0,
        "the right hand up by the head: {right:?}"
    );
    let thinking = frame(200.0, 1.0, &opts(&[("mouthDots", 1.0), ("look", 0.0)]));
    let near_chin = hands(&thinking)
        .iter()
        .any(|(_, x, y)| (x - 118.0).abs() < 12.0 && (y - 112.0).abs() < 12.0);
    assert!(near_chin, "{:?}", hands(&thinking));
}

#[test]
fn arms_off_and_at_20_px_draw_no_arms() {
    assert!(hands(&frame(200.0, 1.0, &opts(&[("arms", 0.0)]))).is_empty());
    assert!(frame(20.0, 1.0, &opts(&[]))
        .fills
        .iter()
        .all(|x| !((x.hue - 24.0).abs() < 1e-9 && x.points.len() == 20)));
}

#[test]
fn turned_the_far_arm_goes_behind_the_body() {
    let f = frame(200.0, 1.0, &opts(&[("turnYaw", -0.7), ("look", 0.0)]));
    let body = f.fills.iter().position(|x| x.gradient.is_some()).unwrap();
    let h = hands(&f);
    assert!(
        h.iter().any(|(i, _, _)| *i < body),
        "one hand drawn before the body: {h:?}"
    );
    assert!(h.iter().any(|(i, _, _)| *i > body), "the near one after it");
}

#[test]
fn it_stays_within_its_cost_budget() {
    for kv in [
        vec![],
        vec![("earGain", 1.0), ("audioLevel", 0.8)],
        vec![("mouthDots", 1.0)],
        vec![("mouthTalk", 1.0), ("mouthGain", 1.0), ("audioLevel", 0.9)],
        vec![("effectCode", 3.0), ("effectAge", 0.5)],
    ] {
        let c = crate::cost::frame_cost(&frame(64.0, 1.3, &opts(&kv)), 64);
        assert!(
            c.coverage <= 0.8 && (c.elements as f64) <= 0.8 * 700.0,
            "{kv:?}: {c:?}"
        );
    }
}
