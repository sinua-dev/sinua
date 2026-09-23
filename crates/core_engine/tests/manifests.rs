//! The Rust manifests, checked the way generated files already are.
//!
//! `scripts/codegen/test/codegen.test.mjs` asserts the old brand is gone, but it
//! only inspects generated files and the generator's own sources, so it
//! structurally could not see `Cargo.toml` -- which is where the last `Devin`
//! survived the rename. These tests cover the
//! manifests themselves.
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `Cargo.toml` in the workspace: the root one and one per crate.
fn manifests() -> Vec<(String, String)> {
    let mut out = vec![(
        "Cargo.toml".to_string(),
        fs::read_to_string(root().join("Cargo.toml")).unwrap(),
    )];
    let mut crates: Vec<PathBuf> = fs::read_dir(root().join("crates"))
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            p.join("Cargo.toml").exists().then_some(p)
        })
        .collect();
    crates.sort();
    for dir in crates {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        out.push((format!("crates/{name}/Cargo.toml"), text));
    }
    out
}

/// The workspace `authors` value, as written in the root manifest.
fn workspace_authors(root_manifest: &str) -> String {
    root_manifest
        .lines()
        .find_map(|l| l.trim().strip_prefix("authors = [\"")?.strip_suffix("\"]"))
        .expect("root Cargo.toml has an `authors = [\"...\"]` line")
        .to_string()
}

#[test]
fn no_manifest_still_carries_the_old_brand() {
    for (path, text) in manifests() {
        for (n, line) in text.lines().enumerate() {
            assert!(
                !line.to_lowercase().contains("devin"),
                "{path}:{} still says the old brand: {line}",
                n + 1
            );
        }
    }
}

#[test]
fn every_crate_declares_publish_false() {
    // The engine ships as wasm, SwiftPM and Maven; nothing here goes to
    // crates.io. Publishing one deliberately means editing both its manifest
    // and this test, which is the point -- a new crate cannot become
    // publishable just by existing.
    for (path, text) in manifests() {
        if path == "Cargo.toml" {
            continue; // the virtual workspace root has no package to publish
        }
        assert!(
            text.lines().any(|l| l.trim() == "publish = false"),
            "{path} does not set `publish = false`, so `cargo publish` would \
             release it to crates.io"
        );
    }
}

#[test]
fn the_authors_line_agrees_with_notice() {
    // These drifted once: the manifest said one thing and NOTICE another.
    // Pin them to each other.
    let manifests = manifests();
    let (_, root_manifest) = &manifests[0];
    let authors = workspace_authors(root_manifest);
    let notice = fs::read_to_string(Path::new(&root()).join("NOTICE")).unwrap();
    assert!(
        notice.contains(&authors),
        "Cargo.toml's authors ({authors:?}) does not appear in NOTICE, which reads:\n{}",
        notice.lines().take(2).collect::<Vec<_>>().join("\n")
    );
}
