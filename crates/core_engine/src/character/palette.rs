//! A character's palette, overridden in part (FX Spec 1.12, design note 19):
//! `palette: { "shell": "#E63946" }` repaints those slots (the recipe's palette
//! names) and leaves the rest as drawn. A slot's `Light` / `Dark` tones follow
//! it, keeping the recipe's own offsets, unless they are given too. Overrides
//! land after `hue`, so a given colour is never turned. Where the recipe pairs
//! an ink with what it sits on (`contrast`), an ink left as drawn is pulled to
//! the far side of the lightness when the new ground is too close to it; an
//! ink given outright is kept and warned about.
//!
//! The result is engine opts, `palette.<slot>.h/.s/.l/.w`, so a state change
//! blends them like any number (`transition.rs` keeps the colour, moves `w`).

use crate::character::geom::Hsl;
use crate::character::recipe::Recipe;

/// Below this lightness gap an ink isn't readable on its ground.
const MIN_GAP: f64 = 0.35;

/// The opts for `given` (slot, colour) on `r`, and the warnings (slot, why).
/// An unknown slot is an error (slot, message with a suggestion).
#[allow(clippy::type_complexity)]
pub fn resolve(
    r: &Recipe,
    given: &[(String, Hsl)],
) -> Result<(Vec<(String, f64)>, Vec<(String, String)>), (String, String)> {
    let names: Vec<&str> = r
        .palette
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| *n != "celebrate")
        .collect();
    let index = |n: &str| r.palette.iter().position(|(m, _)| m == n);
    let mut out: Vec<(usize, Hsl)> = Vec::new();
    for (slot, c) in given {
        match index(slot).filter(|_| slot != "celebrate") {
            Some(i) => out.push((i, *c)),
            None => {
                let hint = crate::fx_spec::suggest(slot, &names)
                    .map(|s| format!(" -- did you mean `{s}`?"))
                    .unwrap_or_default();
                return Err((
                    slot.clone(),
                    format!("`{}` has no colour `{slot}`{hint}", r.id),
                ));
            }
        }
    }
    let set = |out: &[(usize, Hsl)], i: usize| out.iter().any(|(j, _)| *j == i);
    // The tones follow their slot, keeping the recipe's offsets.
    for (slot, c) in given {
        let o = r.palette[index(slot).unwrap()].1;
        for suffix in ["Light", "Dark"] {
            if let Some(t) = index(&format!("{slot}{suffix}")) {
                if !set(&out, t) {
                    let d = r.palette[t].1;
                    out.push((
                        t,
                        Hsl {
                            h: (c.h + d.h - o.h).rem_euclid(360.0),
                            s: (c.s + d.s - o.s).clamp(0.0, 1.0),
                            l: (c.l + d.l - o.l).clamp(0.0, 1.0),
                        },
                    ));
                }
            }
        }
    }
    let colour = |out: &[(usize, Hsl)], i: usize| {
        out.iter()
            .find(|(j, _)| *j == i)
            .map(|(_, c)| *c)
            .unwrap_or(r.palette[i].1)
    };
    let mut warnings = Vec::new();
    for &(ink, ground) in &r.contrast {
        let (a, b) = (colour(&out, ink), colour(&out, ground));
        if (a.l - b.l).abs() >= MIN_GAP {
            continue;
        }
        let ink_given = given.iter().any(|(s, _)| index(s) == Some(ink));
        if ink_given {
            warnings.push((
                r.palette[ink].0.clone(),
                format!(
                    "`{}` is hard to read on `{}` (lightness {:.2} vs {:.2})",
                    r.palette[ink].0, r.palette[ground].0, a.l, b.l
                ),
            ));
        } else if set(&out, ground) {
            let l = if b.l < 0.5 { 0.9 } else { 0.12 };
            out.push((
                ink,
                Hsl {
                    s: a.s * 0.4,
                    l,
                    ..a
                },
            ));
        }
    }
    let mut opts = Vec::new();
    for (i, c) in out {
        let n = &r.palette[i].0;
        opts.extend([
            (format!("palette.{n}.h"), c.h),
            (format!("palette.{n}.s"), c.s),
            (format!("palette.{n}.l"), c.l),
            (format!("palette.{n}.w"), 1.0),
        ]);
    }
    Ok((opts, warnings))
}

/// `f` on the recipe a pattern draws (a built-in, or a registered one).
/// By reference: cloning a whole recipe would cost the wasm its `Clone` glue.
pub fn with_recipe<T>(pattern: &str, f: impl FnOnce(&Recipe) -> T) -> Option<T> {
    if let Some(r) = crate::character::recipe::recipes().get(pattern) {
        return Some(f(r));
    }
    crate::character::registry::get(pattern).map(|r| f(&r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::geom::hsl;

    fn buzzy() -> Recipe {
        crate::character::recipe::recipes()["buzzy"].clone()
    }

    fn get(o: &[(String, f64)], k: &str) -> Option<f64> {
        o.iter().find(|(n, _)| n == k).map(|(_, v)| *v)
    }

    #[test]
    fn tones_follow_their_slot_and_a_given_tone_wins() {
        let r = buzzy();
        let shell = r.colour_named("shell").unwrap();
        let light = r.colour_named("shellLight").unwrap();
        let (o, _) = resolve(&r, &[("shell".into(), hsl(0.0, 0.8, 0.5))]).unwrap();
        assert_eq!(get(&o, "palette.shell.w"), Some(1.0));
        let dl = light.l - shell.l;
        assert!((get(&o, "palette.shellLight.l").unwrap() - (0.5 + dl)).abs() < 1e-12);
        let (o, _) = resolve(
            &r,
            &[
                ("shell".into(), hsl(0.0, 0.8, 0.5)),
                ("shellLight".into(), hsl(60.0, 1.0, 0.9)),
            ],
        )
        .unwrap();
        assert_eq!(get(&o, "palette.shellLight.h"), Some(60.0));
        assert!(
            get(&o, "palette.amber.w").is_none(),
            "the rest is the recipe's"
        );
    }

    #[test]
    fn a_built_in_recipe_is_its_own_json() {
        let r: serde_json::Value =
            serde_json::from_str(crate::character_recipe("cuppa").unwrap()).unwrap();
        assert_eq!(r["id"], "cuppa");
        assert!(r.get("$comment").is_none());
        assert!(crate::character_recipe("working").is_none());
    }

    #[test]
    fn an_unknown_slot_says_what_it_meant() {
        let e = resolve(&buzzy(), &[("shel".into(), hsl(0.0, 1.0, 0.5))]).unwrap_err();
        assert_eq!(e.0, "shel");
        assert!(e.1.contains("did you mean `shell`"), "{}", e.1);
    }

    #[test]
    fn a_dark_ground_lifts_the_ink_unless_the_ink_is_given() {
        let bean = crate::character::recipe::recipes()["bean"].clone();
        let dark = hsl(20.0, 0.4, 0.12);
        let (o, w) = resolve(&bean, &[("bean".into(), dark)]).unwrap();
        assert_eq!(get(&o, "palette.ink.l"), Some(0.9));
        assert!(w.is_empty());
        let (o, w) = resolve(
            &bean,
            &[("bean".into(), dark), ("ink".into(), hsl(0.0, 0.0, 0.1))],
        )
        .unwrap();
        assert_eq!(get(&o, "palette.ink.l"), Some(0.1));
        assert_eq!(w.len(), 1);
        assert!(w[0].1.contains("hard to read"));
    }
}
