//! `spec/parameters.json` is a checked-in copy of the engine's parameter
//! catalog (`parameter_catalog_json`, docs/parameters.md *Parameter
//! catalog*). This test fails when the two drift; regenerate with
//! `PARAMS_CATALOG_WRITE=1 cargo test -p core_engine --test parameter_catalog -- --ignored`.
use std::path::PathBuf;

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/parameters.json")
}

#[test]
fn spec_parameters_json_matches_the_engine() {
    let file = std::fs::read_to_string(path()).expect("spec/parameters.json exists");
    assert!(
        file == core_engine::parameter_catalog_json(),
        "spec/parameters.json is stale: regenerate it (see this file's header)"
    );
}

#[test]
#[ignore]
fn write_spec_parameters_json() {
    if std::env::var("PARAMS_CATALOG_WRITE").as_deref() != Ok("1") {
        return;
    }
    std::fs::write(path(), core_engine::parameter_catalog_json()).unwrap();
}
