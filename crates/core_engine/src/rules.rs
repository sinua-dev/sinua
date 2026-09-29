//! Derived-state rules (FX Spec 1.9 `rules`): the app passes numbers, the
//! file decides which `states` entry renders.
//!
//! ```json
//! "rules": [
//!   { "when": { "input": "steps", "gte": 10000 }, "state": "goalReached" },
//!   { "when": { "input": "heartRate", "gt": 150 }, "state": "intense", "hysteresis": 5 }
//! ]
//! ```
//!
//! Rules are tried in order and the first one that holds wins; when none
//! holds, `derive` returns `None` and the caller keeps its own `state` (or
//! the base design). A rule whose input wasn't passed doesn't hold.
//!
//! `hysteresis` keeps a derived state from flickering around its threshold:
//! while the caller is already in the rule's state (`previous`), the
//! condition is relaxed by that amount -- `gt 150, hysteresis 5` is entered
//! above 150 and left only below 145. That needs the previous state, and the
//! engine keeps no memory between calls, so the caller passes it back in
//! (the same split as easing and cross-fades: docs/fx-spec.md, *Caller
//! loop*). `derive` is a pure function of (file, inputs, previous).

use std::collections::HashMap;

use serde_json::{Map, Value};

/// A rule body's keys.
pub(crate) const RULE_KEYS: [&str; 3] = ["when", "state", "hysteresis"];
/// A `when`'s keys: the input and one comparison.
pub(crate) const WHEN_KEYS: [&str; 6] = ["input", "gt", "gte", "lt", "lte", "between"];
const OPS: [&str; 5] = ["gt", "gte", "lt", "lte", "between"];
/// Largest `rules` list a file may hold; more is almost certainly a mistake
/// and each rule costs a lookup per input update.
const MAX_RULES: usize = 32;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Gt(f64),
    Gte(f64),
    Lt(f64),
    Lte(f64),
    Between(f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
struct Rule {
    input: String,
    op: Op,
    state: String,
    hysteresis: f64,
}

impl Rule {
    fn holds(&self, v: f64, relaxed: bool) -> bool {
        let h = if relaxed { self.hysteresis } else { 0.0 };
        match self.op {
            Op::Gt(x) => v > x - h,
            Op::Gte(x) => v >= x - h,
            Op::Lt(x) => v < x + h,
            Op::Lte(x) => v <= x + h,
            Op::Between(a, b) => v >= a - h && v <= b + h,
        }
    }
}

/// What `check` found, for `fx_spec`'s diagnostics.
#[derive(Debug, PartialEq)]
pub(crate) enum Problem {
    Error(String, String),
    Warning(String, String),
    /// An unknown key at a path, with the keys that would have been valid.
    Unknown(String, String, &'static [&'static str]),
}

fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite())
}

/// Parses one rule; `None` (with problems pushed) when it can't be used.
fn parse_rule(v: &Value, at: &str, state_keys: &[String], out: &mut Vec<Problem>) -> Option<Rule> {
    let Some(r) = v.as_object() else {
        out.push(Problem::Error(
            at.into(),
            "expected { when, state, hysteresis? }".into(),
        ));
        return None;
    };
    for k in r.keys() {
        if !RULE_KEYS.contains(&k.as_str()) {
            out.push(Problem::Unknown(format!("{at}/{k}"), k.clone(), &RULE_KEYS));
        }
    }
    let state = match r.get("state").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            out.push(Problem::Error(
                format!("{at}/state"),
                "expected the name of a `states` entry".into(),
            ));
            return None;
        }
    };
    if !state_keys.contains(&state) {
        out.push(Problem::Warning(
            format!("{at}/state"),
            format!("`{state}` is not a key of `states`, so this rule renders the base design"),
        ));
    }
    let hysteresis = match r.get("hysteresis") {
        None => 0.0,
        Some(h) => match finite(h) {
            Some(h) if h >= 0.0 => h,
            _ => {
                out.push(Problem::Error(
                    format!("{at}/hysteresis"),
                    "expected a number >= 0".into(),
                ));
                return None;
            }
        },
    };
    let wp = format!("{at}/when");
    let Some(w) = r.get("when").and_then(Value::as_object) else {
        out.push(Problem::Error(
            wp,
            "expected { input, gt | gte | lt | lte | between }".into(),
        ));
        return None;
    };
    parse_when(w, &wp, out).map(|(input, op)| Rule {
        input,
        op,
        state,
        hysteresis,
    })
}

fn parse_when(w: &Map<String, Value>, wp: &str, out: &mut Vec<Problem>) -> Option<(String, Op)> {
    for k in w.keys() {
        if !WHEN_KEYS.contains(&k.as_str()) {
            out.push(Problem::Unknown(format!("{wp}/{k}"), k.clone(), &WHEN_KEYS));
        }
    }
    let input = match w.get("input").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            out.push(Problem::Error(
                format!("{wp}/input"),
                "expected the name of an input".into(),
            ));
            return None;
        }
    };
    let ops: Vec<&str> = OPS.iter().copied().filter(|o| w.contains_key(*o)).collect();
    let [op] = ops.as_slice() else {
        out.push(Problem::Error(
            wp.into(),
            format!(
                "expected exactly one of {} (found {})",
                OPS.join(", "),
                ops.len()
            ),
        ));
        return None;
    };
    let at = format!("{wp}/{op}");
    let value = &w[*op];
    let parsed = if *op == "between" {
        match value
            .as_array()
            .map(|a| a.iter().map(finite).collect::<Vec<_>>())
        {
            Some(a) if a.len() == 2 && a.iter().all(Option::is_some) => {
                let (lo, hi) = (a[0].unwrap(), a[1].unwrap());
                if lo > hi {
                    out.push(Problem::Error(
                        at,
                        format!("expected [low, high], got [{lo}, {hi}]"),
                    ));
                    return None;
                }
                Op::Between(lo, hi)
            }
            _ => {
                out.push(Problem::Error(at, "expected [low, high]".into()));
                return None;
            }
        }
    } else {
        let Some(x) = finite(value) else {
            out.push(Problem::Error(at, "expected a number".into()));
            return None;
        };
        match *op {
            "gt" => Op::Gt(x),
            "gte" => Op::Gte(x),
            "lt" => Op::Lt(x),
            _ => Op::Lte(x),
        }
    };
    Some((input, parsed))
}

/// Parses a `rules` value, reporting every problem. Rules that can't be used
/// are left out; the rest still apply.
fn parse(v: &Value, state_keys: &[String], out: &mut Vec<Problem>) -> Vec<Rule> {
    let Some(list) = v.as_array() else {
        out.push(Problem::Error(
            "/rules".into(),
            "expected a list of rules".into(),
        ));
        return Vec::new();
    };
    if list.len() > MAX_RULES {
        out.push(Problem::Error(
            "/rules".into(),
            format!(
                "at most {MAX_RULES} rules (found {}); only the first {MAX_RULES} apply",
                list.len()
            ),
        ));
    }
    list.iter()
        .take(MAX_RULES)
        .enumerate()
        .filter_map(|(i, r)| parse_rule(r, &format!("/rules/{i}"), state_keys, out))
        .collect()
}

/// Validates a file's `rules` for the resolver's diagnostics.
pub(crate) fn check(v: &Value, state_keys: &[String]) -> Vec<Problem> {
    let mut out = Vec::new();
    parse(v, state_keys, &mut out);
    out
}

/// The state the file's `rules` pick for these `inputs`, given the state the
/// caller rendered last (`previous`, for hysteresis): the first rule that
/// holds, else `None` (keep the caller's own state). A file without rules,
/// or one that doesn't parse, derives nothing.
pub fn derive(json: &str, inputs: &HashMap<String, f64>, previous: Option<&str>) -> Option<String> {
    let doc: Value = serde_json::from_str(json).ok()?;
    let rules_v = doc.get("rules")?;
    let state_keys: Vec<String> = doc
        .get("states")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let rules = parse(rules_v, &state_keys, &mut Vec::new());
    rules
        .iter()
        .find(|r| {
            inputs
                .get(&r.input)
                .is_some_and(|v| v.is_finite() && r.holds(*v, previous == Some(r.state.as_str())))
        })
        .map(|r| r.state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"{ "fxSpec": "1.9", "object": "ring", "pattern": "completing",
      "states": { "goalReached": {}, "intense": {}, "calm": {} },
      "rules": [
        { "when": { "input": "steps", "gte": 10000 }, "state": "goalReached" },
        { "when": { "input": "heartRate", "gt": 150 }, "state": "intense", "hysteresis": 5 },
        { "when": { "input": "heartRate", "between": [50, 70] }, "state": "calm" }
      ] }"#;

    fn inputs(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn first_rule_that_holds_wins() {
        let d = |p: &[(&str, f64)]| derive(SPEC, &inputs(p), None);
        assert_eq!(
            d(&[("steps", 12000.0), ("heartRate", 160.0)]).as_deref(),
            Some("goalReached")
        );
        assert_eq!(
            d(&[("steps", 500.0), ("heartRate", 160.0)]).as_deref(),
            Some("intense")
        );
        assert_eq!(d(&[("heartRate", 60.0)]).as_deref(), Some("calm"));
        assert_eq!(
            d(&[("steps", 10000.0)]).as_deref(),
            Some("goalReached"),
            "gte includes the edge"
        );
    }

    #[test]
    fn nothing_holds_or_input_missing_derives_nothing() {
        assert_eq!(
            derive(
                SPEC,
                &inputs(&[("steps", 10.0), ("heartRate", 100.0)]),
                None
            ),
            None
        );
        assert_eq!(derive(SPEC, &inputs(&[]), None), None);
        assert_eq!(
            derive(SPEC, &inputs(&[("heartRate", f64::NAN)]), None),
            None
        );
        assert_eq!(
            derive(r#"{ "fxSpec": "1.9" }"#, &inputs(&[("steps", 1e6)]), None),
            None
        );
        assert_eq!(derive("not json", &inputs(&[("steps", 1e6)]), None), None);
    }

    #[test]
    fn hysteresis_holds_a_state_until_the_value_clears_the_band() {
        let hr = |v: f64, prev: Option<&str>| derive(SPEC, &inputs(&[("heartRate", v)]), prev);
        // Entering needs the plain threshold.
        assert_eq!(hr(148.0, None), None);
        assert_eq!(hr(151.0, None).as_deref(), Some("intense"));
        // Once in, it holds down to 145 (exclusive, like `gt`).
        assert_eq!(hr(148.0, Some("intense")).as_deref(), Some("intense"));
        assert_eq!(hr(145.5, Some("intense")).as_deref(), Some("intense"));
        assert_eq!(hr(145.0, Some("intense")), None);
        // Relaxing only applies to the state the caller is in.
        assert_eq!(hr(148.0, Some("calm")), None);
    }

    #[test]
    fn every_operator_and_its_relaxed_edge() {
        let r = |op: Op, h: f64| Rule {
            input: "x".into(),
            op,
            state: "s".into(),
            hysteresis: h,
        };
        assert!(r(Op::Gt(1.0), 0.0).holds(1.1, false) && !r(Op::Gt(1.0), 0.0).holds(1.0, false));
        assert!(r(Op::Gte(1.0), 0.0).holds(1.0, false));
        assert!(r(Op::Lt(1.0), 0.0).holds(0.9, false) && !r(Op::Lt(1.0), 0.0).holds(1.0, false));
        assert!(r(Op::Lte(1.0), 0.0).holds(1.0, false));
        assert!(r(Op::Lt(1.0), 0.5).holds(1.4, true) && !r(Op::Lt(1.0), 0.5).holds(1.4, false));
        let b = r(Op::Between(2.0, 4.0), 1.0);
        assert!(b.holds(2.0, false) && b.holds(4.0, false) && !b.holds(4.5, false));
        assert!(b.holds(4.5, true) && b.holds(1.0, true) && !b.holds(5.1, true));
    }

    #[test]
    fn check_reports_bad_rules_and_keeps_the_good_ones() {
        let v: Value = serde_json::from_str(
            r#"[
              { "when": { "input": "a", "gt": 1 }, "state": "ok" },
              { "when": { "input": "a", "gt": 1, "lt": 5 }, "state": "ok" },
              { "when": { "input": "a", "between": [5, 1] }, "state": "ok" },
              { "when": { "input": "a", "gt": "x" }, "state": "ok" },
              { "when": { "gt": 1 }, "state": "ok" },
              { "when": { "input": "a", "gt": 1 }, "state": "missing" },
              { "when": { "input": "a", "gt": 1 }, "state": "ok", "hysteresis": -1 },
              { "when": { "input": "a", "gt": 1 }, "state": "ok", "extra": 1 },
              "nope"
            ]"#,
        )
        .unwrap();
        let keys = vec!["ok".to_string()];
        let problems = check(&v, &keys);
        let errors: Vec<&str> = problems
            .iter()
            .filter_map(|p| match p {
                Problem::Error(path, _) => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            errors,
            [
                "/rules/1/when",
                "/rules/2/when/between",
                "/rules/3/when/gt",
                "/rules/4/when/input",
                "/rules/6/hysteresis",
                "/rules/8"
            ]
        );
        assert!(problems.contains(&Problem::Warning(
            "/rules/5/state".into(),
            "`missing` is not a key of `states`, so this rule renders the base design".into()
        )));
        assert!(problems
            .iter()
            .any(|p| matches!(p, Problem::Unknown(path, key, _) if path == "/rules/7/extra" && key == "extra")));
        // Rules 0, 5 and 7 are usable.
        assert_eq!(parse(&v, &keys, &mut Vec::new()).len(), 3);
        assert_eq!(
            check(&Value::from(3), &keys),
            [Problem::Error(
                "/rules".into(),
                "expected a list of rules".into()
            )]
        );
    }

    #[test]
    fn too_many_rules_is_an_error_and_the_rest_are_dropped() {
        let one = r#"{ "when": { "input": "a", "gt": 1 }, "state": "s" }"#;
        let many = format!("[{}]", vec![one; MAX_RULES + 3].join(","));
        let v: Value = serde_json::from_str(&many).unwrap();
        let mut out = Vec::new();
        assert_eq!(parse(&v, &["s".to_string()], &mut out).len(), MAX_RULES);
        assert!(matches!(&out[0], Problem::Error(p, _) if p == "/rules"));
    }
}
