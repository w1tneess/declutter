//! Golden operations owned by core-A (foundation). Port the matching `op_*` functions from tools/gen_golden.py:
//! scan, plan_sort, empty_dirs, check_root, undo, recycle, trash_empty. Return the same JSON shape as the oracle.

use serde_json::Value;

use super::common::Env;

pub fn run(op: &str, env: &Env, fx: &Value) -> Option<Value> {
    match op {
        // "scan" => Some(...), and so on, as core-A lands each one.
        _ => None,
    }
}
