//! The published iOS / Android artefacts are built without the cargo feature `dev`
//! (design note 34; the release workflow sets SINUA_NATIVE_RELEASE=1), so their
//! bindings lack the Studio / dev exports. Shipped Swift and Kotlin code must not call
//! them, or the release variant wouldn't compile. Tests and the Studios build with `dev`.
use std::fs;
use std::path::{Path, PathBuf};

const DEV_ONLY: [&str; 5] = [
    "estimateCost",
    "fxSpecCost",
    "liquidSuitability",
    "parameterCatalogJson",
    "checkOverrides",
];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            // Generated bindings carry the functions in a dev build; they're regenerated.
            if !p.ends_with("uniffi") && !p.ends_with("CoreEngine") {
                sources(&p, out);
            }
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("swift" | "kt")) {
            out.push(p);
        }
    }
}

#[test]
fn shipped_native_code_doesnt_call_the_dev_only_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    let mut files = Vec::new();
    for dir in [
        "ios/Sources",
        "ios-openai/Sources",
        "ios-livekit/Sources",
        "android/src/main",
        "android/view/src/main",
        "android/openai/src/main",
        "android/gemini/src/main",
        "android/elevenlabs/src/main",
        "android/livekit/src/main",
        "android/websocket/src/main",
        "react-native/ios",
        "react-native/android/src/main",
    ] {
        sources(&root.join(dir), &mut files);
    }
    assert!(
        files.len() > 40,
        "only {} shipped sources found",
        files.len()
    );
    let mut hits = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).unwrap_or_default();
        for name in DEV_ONLY {
            if text.contains(&format!("{name}(")) {
                hits.push(format!("{}: {name}", f.display()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "shipped code calls dev-only exports:\n{}",
        hits.join("\n")
    );
}
