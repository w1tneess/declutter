//! Golden operations owned by core-B (analysis). Port the matching `op_*` functions from tools/gen_golden.py:
//! duplicates, large_old, leftovers, redownloads, trash_duplicates, check_dup_selection, trash_redownloads.
//! Return the same JSON shape as the oracle.

use serde_json::Value;

use super::common::Env;

pub fn run(op: &str, env: &Env, fx: &Value) -> Option<Value> {
    match op {
        // "duplicates" => Some(...), and so on, as core-B lands each one.
        _ => None,
    }
}
