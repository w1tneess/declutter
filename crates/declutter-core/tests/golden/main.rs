//! Runs every fixture in `tests/golden/*.json` (generated from the Python oracle by `tools/gen_golden.py`)
//! against the Rust core. Operations are registered in `ops_a` (foundation) and `ops_b` (analysis); an operation
//! nobody has implemented yet is reported as pending, not failed. Set `GOLDEN_STRICT=1` to fail on pending too.
//! Set `GOLDEN_ONLY=<substring>` to run just the fixtures whose name contains it.

// The helpers and op stubs go unused until core-A and core-B land their operations.
#![allow(dead_code, unused_variables)]

mod common;
mod ops_a;
mod ops_b;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use serde_json::Value;

enum Outcome { Pass, Pending(String), Fail(String) }

fn run_fixture(fx: &Value) -> Outcome {
    let op = fx["op"].as_str().unwrap();
    let name = fx["name"].as_str().unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let env = common::Env::new(fx);
        let actual = ops_a::run(op, &env, fx).or_else(|| ops_b::run(op, &env, fx));
        actual.map(|a| (env, a))
    }));
    match result {
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>().cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            if msg.starts_with("not yet implemented") { Outcome::Pending(msg) } else { Outcome::Fail(format!("{name}: panicked: {msg}")) }
        }
        Ok(None) => Outcome::Pending(format!("op `{op}` is not registered")),
        Ok(Some((_env, actual))) => match common::compare(name, &fx["expected"], &actual) {
            Ok(()) => Outcome::Pass,
            Err(e) => Outcome::Fail(e),
        },
    }
}

#[test]
fn golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let index: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).unwrap()).unwrap();
    let only = std::env::var("GOLDEN_ONLY").ok();
    let (mut pass, mut pending, mut fail) = (0, vec![], vec![]);
    for name in index["fixtures"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if only.as_deref().is_some_and(|o| !name.contains(o)) { continue; }
        let fx: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        match run_fixture(&fx) {
            Outcome::Pass => pass += 1,
            Outcome::Pending(why) => pending.push(format!("{name}: {why}")),
            Outcome::Fail(why) => fail.push(why),
        }
    }
    println!("golden: {pass} passed, {} pending, {} failed", pending.len(), fail.len());
    for p in &pending { println!("  pending {p}"); }
    assert!(fail.is_empty(), "{} golden fixtures failed:\n{}", fail.len(), fail.join("\n\n"));
    if std::env::var_os("GOLDEN_STRICT").is_some() {
        assert!(pending.is_empty(), "{} golden fixtures still pending", pending.len());
    }
}
