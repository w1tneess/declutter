//! Runs every fixture in `tests/golden/*.json` (generated from the Python oracle by `tools/gen_golden.py`)
//! against the Rust core. Operations are registered in `ops_a` (foundation) and `ops_b` (analysis); an operation
//! nobody registered is a failure. Set `GOLDEN_ONLY=<substring>` to run just the fixtures whose name contains it.

mod common;
mod ops_a;
mod ops_b;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use serde_json::Value;

fn run_fixture(fx: &Value) -> Result<(), String> {
    let op = fx["op"].as_str().unwrap();
    let name = fx["name"].as_str().unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let env = common::Env::new(fx);
        ops_a::run(op, &env, fx).or_else(|| ops_b::run(op, &env, fx))
    }));
    match result {
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>().cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("{name}: panicked: {msg}"))
        }
        Ok(None) => Err(format!("{name}: op `{op}` is not registered")),
        Ok(Some(actual)) => common::compare(name, &fx["expected"], &actual),
    }
}

#[test]
fn golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let index: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).unwrap()).unwrap();
    let only = std::env::var("GOLDEN_ONLY").ok();
    let (mut pass, mut fail) = (0, vec![]);
    for name in index["fixtures"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if only.as_deref().is_some_and(|o| !name.contains(o)) { continue; }
        let fx: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        match run_fixture(&fx) {
            Ok(()) => pass += 1,
            Err(why) => fail.push(why),
        }
    }
    println!("golden: {pass} passed, {} failed", fail.len());
    assert!(fail.is_empty(), "{} golden fixtures failed:\n{}", fail.len(), fail.join("\n\n"));
}
