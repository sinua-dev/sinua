//! `spec/parameters.json` is a checked-in copy of the engine's parameter
//! catalog, words and all (`catalog_json_from_source` over the full
//! `src/catalog_source.json`; docs/parameters.md *Parameter catalog*). The
//! runtime's own `parameter_catalog_json` is the same without the descriptions
//! (design note 10). This test fails when the two drift; regenerate with
//! `PARAMS_CATALOG_WRITE=1 cargo test -p core_engine --test parameter_catalog -- --ignored`.
use std::path::PathBuf;

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/parameters.json")
}

fn full() -> String {
    core_engine::catalog_json_from_source(include_str!("../src/catalog_source.json"))
}

#[test]
fn spec_parameters_json_matches_the_engine() {
    let file = std::fs::read_to_string(path()).expect("spec/parameters.json exists");
    assert!(
        file == full(),
        "spec/parameters.json is stale: regenerate it (see this file's header)"
    );
}

#[test]
#[ignore]
fn write_spec_parameters_json() {
    if std::env::var("PARAMS_CATALOG_WRITE").as_deref() != Ok("1") {
        return;
    }
    std::fs::write(path(), full()).unwrap();
}
