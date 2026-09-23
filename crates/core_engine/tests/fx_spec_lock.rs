//! The FX Spec identity locks (`spec/fx-spec-1.<minor>-resolved.json`): how
//! one runtime resolved every `spec/examples` file, for every state key and
//! both power states, with the parity tests' fixed inputs. `fx_spec.rs`'s
//! `v1_<N>_examples_resolve_identically` holds every later runtime to the
//! lock for minor N.
//!
//! **Every minor the runtime supports has a lock, including the current one.**
//! Capture a new minor's lock once that minor is stable and before anything
//! that uses it is published -- not, as this file used to say, on the last
//! commit before the *next* bump, which left the shipping runtime unlocked.
//!
//! Capturing one is deliberate and rare:
//! `FX_SPEC_LOCK_WRITE=1 cargo test -p core_engine --test fx_spec_lock -- --ignored`
//! writes the lock for the current `RUNTIME_MINOR`. Never run it to paper
//! over a diff: a lock that changes means the older wording's meaning
//! changed, which is the one thing these files exist to prevent
//! (docs/fx-spec.md, *Identity*).
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The inputs every platform's parity test passes (`fx-spec.test.mjs`'s
/// `TEST_INPUTS`, mirrored in each lock's `inputs`).
fn inputs() -> BTreeMap<String, f64> {
    [
        ("micMuted", 0.0),
        ("micLevel", 0.6),
        ("agentVolume", 0.5),
        ("steps", 6200.0),
        ("waterMl", 1800.0),
        ("activeMinutes", 12.0),
        ("heartRate", 128.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples")
}

fn lock_path(minor: u64) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../spec/fx-spec-1.{minor}-resolved.json"))
}

/// One row per state key x power state, in the locks' shape.
fn rows(json: &str) -> Map<String, Value> {
    let ins: std::collections::HashMap<String, f64> = inputs().into_iter().collect();
    let base = core_engine::resolve_fx_spec_with(json.to_string(), None, ins.clone(), false);
    let mut keys: Vec<Option<String>> = vec![None];
    keys.extend(base.state_keys.iter().cloned().map(Some));
    let mut out = Map::new();
    for key in keys {
        for low in [false, true] {
            let r =
                core_engine::resolve_fx_spec_with(json.to_string(), key.clone(), ins.clone(), low);
            let name = format!(
                "{}{}",
                key.clone().unwrap_or_default(),
                if low { "|lowPower" } else { "" }
            );
            let overrides: BTreeMap<String, f64> = r.overrides.clone().into_iter().collect();
            let diagnostics: Vec<Value> = r
                .diagnostics
                .iter()
                .map(|d| json!({ "severity": d.severity, "path": d.path, "message": d.message }))
                .collect();
            out.insert(
                name,
                json!({
                    "ok": r.ok,
                    "state": r.state,
                    "size": r.size,
                    "speed": r.speed,
                    "overrides": overrides,
                    "diagnostics": diagnostics,
                    "stateKey": r.state_key,
                    "stateKeys": r.state_keys,
                    "inactiveBindings": r.inactive_bindings,
                    "maxFps": r.max_fps,
                    "disabledMaterials": r.disabled_materials,
                }),
            );
        }
    }
    out
}

fn capture(minor: u64) -> String {
    let mut examples = Map::new();
    let mut files: Vec<_> = std::fs::read_dir(examples_dir())
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().into_owned();
            name.ends_with(".fxspec.json").then_some(name)
        })
        .collect();
    files.sort();
    for file in files {
        let json = std::fs::read_to_string(examples_dir().join(&file)).unwrap();
        // A lock covers the files that runtime could have been given. An
        // example that declares a newer minor (because it needs a key this
        // one doesn't have) belongs to that minor's lock instead.
        let declared: Option<u64> = serde_json::from_str::<Value>(&json)
            .ok()
            .and_then(|v| v["fxSpec"].as_str().map(str::to_string))
            .and_then(|v| v.split('.').nth(1)?.parse().ok());
        if declared.is_some_and(|m| m > minor) {
            continue;
        }
        examples.insert(file, Value::Object(rows(&json)));
    }
    let doc = json!({
        "note": format!(
            "Identity lock: how the FX Spec 1.{minor} runtime resolved all spec/examples (every \
             state key, base = \"\"; \"|lowPower\" = ctx.lowPower) with the parity tests' fixed \
             inputs, captured from the 1.{minor} build. \
             crates/core_engine/src/fx_spec.rs (v1_{minor}_examples_resolve_identically) requires \
             every later runtime to reproduce it exactly. Regenerate only when deliberately \
             capturing a new minor: crates/core_engine/tests/fx_spec_lock.rs."
        ),
        "inputs": inputs(),
        "examples": examples,
    });
    serde_json::to_string_pretty(&doc).unwrap() + "\n"
}

/// Set to `Some(n)` **only** while bumping `RUNTIME_MINOR` to `n`, in the same
/// change that bumps it, and back to `None` in the change that captures the new
/// lock. It exists so that a missing lock is a deliberate edit a reviewer can
/// see in the diff, rather than something this test infers from a file not
/// being there -- which is what let 1.8 ship unlocked while the test stayed
/// green.
const BUMP_IN_PROGRESS_TO: Option<u64> = None;

/// The lock for the current runtime exists and still matches it.
#[test]
fn the_current_runtimes_lock_is_present_and_still_matches() {
    let minor = core_engine::fx_spec_runtime_minor();
    let path = lock_path(minor);
    if BUMP_IN_PROGRESS_TO == Some(minor) && !path.exists() {
        // Declared mid-bump: the outgoing lock must exist instead.
        assert!(
            lock_path(minor - 1).exists(),
            "BUMP_IN_PROGRESS_TO says 1.{minor} is mid-bump, but the outgoing 1.{} lock is \
             missing too",
            minor - 1
        );
        return;
    }
    assert!(
        path.exists(),
        "spec/fx-spec-1.{minor}-resolved.json is missing. Every minor the runtime supports has a \
         lock, including the current one: capture it with FX_SPEC_LOCK_WRITE=1 cargo test -p \
         core_engine --test fx_spec_lock -- --ignored. If 1.{minor} is genuinely mid-bump, say so \
         by setting BUMP_IN_PROGRESS_TO in this file."
    );
    let file = std::fs::read_to_string(&path).unwrap();
    let want: Value = serde_json::from_str(&file).unwrap();
    let got: Value = serde_json::from_str(&capture(minor)).unwrap();
    assert_eq!(
        got["examples"], want["examples"],
        "spec/fx-spec-1.{minor}-resolved.json no longer matches this runtime"
    );
}

#[test]
#[ignore = "writes spec/fx-spec-1.<minor>-resolved.json; run deliberately with FX_SPEC_LOCK_WRITE=1"]
fn write_the_current_runtimes_lock() {
    if std::env::var("FX_SPEC_LOCK_WRITE").as_deref() != Ok("1") {
        return;
    }
    let minor = core_engine::fx_spec_runtime_minor();
    std::fs::write(lock_path(minor), capture(minor)).unwrap();
}
