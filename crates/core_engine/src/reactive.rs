//! The Reactive-Inputs binding contract in Rust: the curated targets
//! (`docs/parameters.md`, *Reactive Inputs*) and the value -> target
//! mapping FX Spec v1.1's `bindings` use. A line-for-line port of
//! `packages/core/src/reactive.ts`'s `REACTIVE_TARGETS` + `compile`
//! (Motion's `interpolate()` semantics: multi-stop, either direction, one
//! CSS keyword curve per segment, always clamped). `reactive.ts` keeps its
//! own copy because it also takes JS function curves, which can't cross
//! wasm; `spec/reactive-vectors.json` holds the two together
//! (`tests/reactive_vectors.rs` + `packages/core/test/reactive-parity.test.mjs`).

use crate::primitives::cubic_bezier;

/// A curated bind target: the engine's legal range, and the companion keys
/// (with tuned defaults) it needs alongside it to have any effect.
pub struct ReactiveTarget {
    pub name: &'static str,
    /// The engine's legal range: every output is clamped to it.
    pub range: (f64, f64),
    /// What an omitted `outputRange` maps onto. Equal to `range` except for
    /// `progress0..3`, whose range allows extra laps (up to 3) but whose
    /// natural default is one lap = the goal -- extra laps are opted into
    /// with an explicit multi-stop output.
    pub default_output: (f64, f64),
    pub companions: &'static [(&'static str, f64)],
}

/// `DEFAULT_AUDIO_STRENGTH` in voice.ts / `AUDIO_STRENGTH_DEFAULT` in
/// reactive.ts -- keep the three in sync (the vectors test checks TS).
const AUDIO_STRENGTH_DEFAULT: f64 = 0.18;

pub const REACTIVE_TARGETS: [ReactiveTarget; 14] = [
    ReactiveTarget {
        name: "progress",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "progress0",
        range: (0.0, 3.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "progress1",
        range: (0.0, 3.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "progress2",
        range: (0.0, 3.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "progress3",
        range: (0.0, 3.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "quality",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "accuracy",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "audioLevel",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[("audioStrength", AUDIO_STRENGTH_DEFAULT)],
    },
    ReactiveTarget {
        name: "glowStrength",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "noiseStrength",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "gradientStrength",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "pulseStrength",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "colorMix",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
    ReactiveTarget {
        name: "muted",
        range: (0.0, 1.0),
        default_output: (0.0, 1.0),
        companions: &[],
    },
];

pub fn target(name: &str) -> Option<&'static ReactiveTarget> {
    REACTIVE_TARGETS.iter().find(|t| t.name == name)
}

pub const CURVES: [&str; 5] = ["linear", "ease", "easeIn", "easeOut", "easeInOut"];

/// CSS keyword curves, exact values from MDN `animation-timing-function`
/// (the same table as reactive.ts's `resolveCurve`).
fn curve_points(name: &str) -> Option<Option<(f64, f64, f64, f64)>> {
    match name {
        "linear" => Some(None),
        "ease" => Some(Some((0.25, 0.1, 0.25, 1.0))),
        "easeIn" => Some(Some((0.42, 0.0, 1.0, 1.0))),
        "easeOut" => Some(Some((0.0, 0.0, 0.58, 1.0))),
        "easeInOut" => Some(Some((0.42, 0.0, 0.58, 1.0))),
        _ => None,
    }
}

/// A validated binding: `map(value)` is the clamped target value.
#[derive(Clone, Debug)]
pub struct Mapper {
    input: Vec<f64>,
    output: Vec<f64>,
    curves: Vec<Option<(f64, f64, f64, f64)>>,
    dir: f64,
    lo: f64,
    hi: f64,
}

/// `reactive.ts`'s `compile`: the same checks, in the same order, with the
/// same messages (minus the `bindReactiveInput:` prefix).
pub fn compile(
    target_name: &str,
    input: Option<&[f64]>,
    output: Option<&[f64]>,
    curves: &[String],
) -> Result<Mapper, String> {
    let t = target(target_name).ok_or_else(|| format!("unknown target \"{target_name}\""))?;
    let (lo, hi) = t.range;
    let input: Vec<f64> = input.map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0, 1.0]);
    let (d0, d1) = t.default_output;
    let output: Vec<f64> = output.map(<[f64]>::to_vec).unwrap_or_else(|| vec![d0, d1]);
    if input.len() < 2 {
        return Err("`inputRange` needs at least two stops".into());
    }
    if input.len() != output.len() {
        return Err("`inputRange` and `outputRange` must be the same length".into());
    }
    if !input.iter().chain(output.iter()).all(|v| v.is_finite()) {
        return Err("range stops must be finite numbers".into());
    }
    // JS `Math.sign` (Rust's `signum(0.0)` is 1, which would let a flat
    // range through).
    let sign = |d: f64| if d == 0.0 { 0.0 } else { d.signum() };
    let dir = sign(input[input.len() - 1] - input[0]);
    for w in input.windows(2) {
        if dir == 0.0 || sign(w[1] - w[0]) != dir {
            return Err("`inputRange` must be strictly ascending or strictly descending".into());
        }
    }
    let segments = input.len() - 1;
    let names: Vec<String> = match curves.len() {
        0 => vec!["linear".to_string(); segments],
        1 => vec![curves[0].clone(); segments],
        _ => curves.to_vec(),
    };
    if names.len() != segments {
        return Err(format!(
            "{segments} segment(s) need {segments} curve(s), got {}",
            names.len()
        ));
    }
    let curves = names
        .iter()
        .map(|n| curve_points(n).ok_or_else(|| format!("unknown curve \"{n}\"")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Mapper {
        input,
        output,
        curves,
        dir,
        lo,
        hi,
    })
}

impl Mapper {
    pub fn map(&self, value: f64) -> f64 {
        if value.is_nan() {
            return self.lo;
        }
        let seg = self.input.len() - 1;
        let (first, last) = (self.input[0], self.input[seg]);
        // Same order of operations as reactive.ts, for bit-level parity.
        let v = if self.dir > 0.0 {
            value.max(first).min(last)
        } else {
            value.max(last).min(first)
        };
        let mut i = 0;
        while i < seg - 1
            && (if self.dir > 0.0 {
                v > self.input[i + 1]
            } else {
                v < self.input[i + 1]
            })
        {
            i += 1;
        }
        let u = (v - self.input[i]) / (self.input[i + 1] - self.input[i]);
        let u = u.clamp(0.0, 1.0);
        let e = match self.curves[i] {
            None => u,
            Some((x1, y1, x2, y2)) => cubic_bezier(x1, y1, x2, y2, u),
        };
        let out = self.output[i] + (self.output[i + 1] - self.output[i]) * e;
        out.max(self.lo).min(self.hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(t: &str, i: &[f64], o: Option<&[f64]>, c: &[&str]) -> Mapper {
        let c: Vec<String> = c.iter().map(|s| s.to_string()).collect();
        compile(t, Some(i), o, &c).unwrap()
    }

    #[test]
    fn linear_clamped_and_descending() {
        let a = m("progress", &[0.0, 10_000.0], None, &[]);
        assert_eq!(a.map(5_000.0), 0.5);
        assert_eq!(a.map(-1.0), 0.0);
        assert_eq!(a.map(20_000.0), 1.0);
        assert_eq!(a.map(f64::NAN), 0.0);
        let d = m("glowStrength", &[100.0, 0.0], None, &[]);
        assert_eq!(d.map(25.0), 0.75);
        let o = m("progress0", &[0.0, 1.0], Some(&[0.0, 5.0]), &[]);
        assert_eq!(o.map(1.0), 3.0, "clamped to the target's range");
        // Per-ring progress defaults to one lap = the goal; laps are explicit.
        let goal = m("progress0", &[0.0, 10_000.0], None, &[]);
        assert_eq!(goal.map(10_000.0), 1.0);
        assert_eq!(goal.map(30_000.0), 1.0);
        let laps = m(
            "progress0",
            &[0.0, 10_000.0, 30_000.0],
            Some(&[0.0, 1.0, 3.0]),
            &[],
        );
        assert_eq!(laps.map(20_000.0), 2.0);
    }

    #[test]
    fn curves_and_errors() {
        let e = m("progress", &[0.0, 1.0], None, &["easeIn"]);
        assert!(e.map(0.5) < 0.5);
        let e = m("progress", &[0.0, 1.0], None, &["easeOut"]);
        assert!(e.map(0.5) > 0.5);
        assert!(compile("dotSize", None, None, &[]).is_err());
        assert!(compile("progress", Some(&[0.0]), None, &[]).is_err());
        assert!(compile(
            "progress",
            Some(&[0.0, 0.5, 0.2]),
            Some(&[0.0, 1.0, 0.5]),
            &[]
        )
        .is_err());
        assert!(compile("progress", Some(&[0.0, 1.0]), Some(&[0.0, 1.0, 1.0]), &[]).is_err());
        assert!(compile("progress", None, None, &["bounce".into()]).is_err());
        let two: Vec<String> = vec!["linear".into(), "easeIn".into()];
        assert!(compile("progress", None, None, &two).is_err());
    }
}
