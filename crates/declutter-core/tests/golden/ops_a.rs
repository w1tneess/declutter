//! Golden operations owned by core-A (foundation). Port the matching `op_*` functions from tools/gen_golden.py:
//! scan, plan_sort, empty_dirs, check_root, undo, recycle, trash_empty. Return the same JSON shape as the oracle.

use std::fs::{self, FileTimes, OpenOptions};
use std::os::windows::fs::FileTimesExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use declutter_core::model::{ActionResult, SortPlan};
use declutter_core::{empty, paths, sort, trash, undo, walk};
use serde_json::{json, Map, Value};

use super::common::{compare_unordered_desc, Env};

pub fn run(op: &str, env: &Env, fx: &Value) -> Option<Value> {
    let p = &fx["params"];
    Some(match op {
        "scan" => scan(env, p),
        "plan_sort" => {
            let plan = plan(env, p["recent_hours"].as_u64().unwrap_or(0) as u32);
            let moves: Vec<Value> = plan
                .moves
                .iter()
                .map(|m| {
                    json!({"src": rel(env, &m.src), "dst": rel(env, &m.dst), "size": m.size, "category": m.category})
                })
                .collect();
            json!({"moves": moves, "recent_left": plan.recent_left})
        }
        "empty_dirs" => empty_dirs(env, p, &fx["expected"]),
        "check_root" => check_root(env, p),
        "undo" => undo_op(env, p),
        "recycle" => {
            let paths: Vec<PathBuf> = strs(&p["paths"]).map(|x| env.path(x)).collect();
            let with_root = p["with_root"].as_bool().unwrap_or(true);
            let res =
                trash::to_recycle_bin(&env.ctx, &paths, with_root.then_some(env.root.as_path()));
            json!({"result": result(env, &res), "tree": env.snapshot()})
        }
        "trash_empty" => {
            let dirs = empty::find_empty_dirs(&env.ctx, &env.root, &[]).unwrap();
            for a in p["after_scan"].as_array().into_iter().flatten() {
                env.act(a);
            }
            let res = trash::trash_empty_dirs(&env.ctx, &dirs, &env.root);
            let scanned: Vec<String> = dirs.iter().map(|d| env.rel(d)).collect();
            json!({"scanned": scanned, "result": result(env, &res), "tree": env.snapshot()})
        }
        _ => return None,
    })
}

fn strs(v: &Value) -> impl Iterator<Item = &str> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str)
}

fn rel(env: &Env, p: &str) -> String {
    env.rel(Path::new(p))
}

fn result(env: &Env, r: &ActionResult) -> Value {
    let errors: Vec<String> = r.errors.iter().map(|e| env.norm(e)).collect();
    json!({"count": r.count, "errors": errors})
}

fn plan(env: &Env, recent_hours: u32) -> SortPlan {
    let scan = walk::scan_entries(&env.ctx, &env.root, &[]).unwrap();
    sort::plan_sort(&env.ctx, &env.root, &scan.entries, recent_hours).unwrap()
}

fn scan(env: &Env, p: &Value) -> Value {
    let ignore: Vec<PathBuf> = strs(&p["ignore"]).map(|x| env.path(x)).collect();
    let scan = walk::scan_entries(&env.ctx, &env.root, &ignore).unwrap();
    let mut files: Vec<String> = scan.entries.iter().map(|e| env.rel(&e.path)).collect();
    let mut skipped: Vec<String> = scan.skipped.iter().map(|s| env.rel(s)).collect();
    files.sort();
    skipped.sort();
    json!({"files": files, "skipped_dirs": skipped})
}

/// Equal-length ties are unordered in the oracle: when the members match and the order is longest first, report
/// the expected value so the exact comparison passes; otherwise report what we got.
fn empty_dirs(env: &Env, p: &Value, expected: &Value) -> Value {
    let ignore: Vec<PathBuf> = strs(&p["ignore"]).map(|x| env.path(x)).collect();
    let found = empty::find_empty_dirs(&env.ctx, &env.root, &ignore).unwrap();
    let dirs: Vec<String> = found.iter().map(|d| env.rel(d)).collect();
    let want: Vec<String> = strs(&expected["dirs"]).map(String::from).collect();
    match compare_unordered_desc("empty_dirs", &want, &dirs, |s| s.chars().count()) {
        Ok(()) => expected.clone(),
        Err(_) => json!({ "dirs": dirs }),
    }
}

fn check_root(env: &Env, p: &Value) -> Value {
    let home = env.platform.home.clone();
    let results: Vec<Value> = strs(&p["cases"])
        .map(|case| {
            let path = match case {
                "$DRIVE_ROOT" => home.ancestors().last().unwrap_or(&home).to_path_buf(),
                "$HOME" => home.clone(),
                "$HOME_PARENT" => home.parent().unwrap_or(&home).to_path_buf(),
                other => env.path(other),
            };
            let c = paths::check_root(&env.ctx, &path);
            json!({"case": case, "ok": c.ok, "reason": c.reason})
        })
        .collect();
    json!({ "results": results })
}

/// The log as `_undo_log` in gen_golden.py reports it: absolute paths relative to the root (except the tampered
/// log's C:\Windows entries), and whether each batch's time looks like a `YYYY-MM-DD HH:MM:SS` stamp.
fn undo_log(env: &Env) -> Vec<Value> {
    let text = fs::read_to_string(env.platform.data_dir.join("undo.json")).unwrap_or_default();
    let Ok(Value::Array(raw)) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };
    let rel_value = |v: &Value| match v.as_str() {
        Some(s) if Path::new(s).is_absolute() && !s.starts_with(r"C:\Windows") => {
            Value::from(rel(env, s))
        }
        _ => v.clone(),
    };
    raw.iter()
        .map(|b| {
            let moves: Vec<Value> = b["moves"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|m| {
                    let fields = m.as_object().into_iter().flatten();
                    Value::Object(fields.map(|(k, v)| (k.clone(), rel_value(v))).collect())
                })
                .collect();
            let stamp = b["time"].as_str().is_some_and(|t| t.chars().count() == 19);
            json!({"moves": moves, "time_is_timestamp": stamp})
        })
        .collect()
}

/// Ten days, clear of every age threshold (5 min, 3 d, 7 d); same as `OLD` in gen_golden.py.
const OLD: f64 = 10.0 * 86400.0;

fn undo_op(env: &Env, p: &Value) -> Value {
    let ctx = &env.ctx;
    let mut out = Map::new();
    let mut applies = Vec::new();
    let raw_log = p.get("raw_log");
    if let Some(raw) = raw_log {
        // A hand-written or damaged log; <root> stands for the work root.
        let root = env.root.to_string_lossy().replace('\\', r"\\");
        fs::create_dir_all(&env.platform.data_dir).unwrap();
        let text = serde_json::to_string(raw).unwrap().replace("<root>", &root);
        fs::write(env.platform.data_dir.join("undo.json"), text).unwrap();
    }
    let rounds = p["rounds"]
        .as_u64()
        .unwrap_or(if raw_log.is_some() { 0 } else { 1 });
    for _ in 0..rounds {
        let plan = plan(env, p["recent_hours"].as_u64().unwrap_or(0) as u32);
        let n = p["move_first"]
            .as_u64()
            .map_or(plan.moves.len(), |n| (n as usize).min(plan.moves.len()));
        applies.push(result(
            env,
            &sort::apply_moves(ctx, &plan.moves[..n], Some(&env.root), 0),
        ));
        for a in p["between"].as_array().into_iter().flatten() {
            env.act(a);
        }
    }
    for i in 0..p["apply_rounds"].as_u64().unwrap_or(0) {
        // One old file per round, to exercise the 20-batch cap.
        let f = env.root.join(format!("f{i:02}.txt"));
        fs::write(&f, i.to_string()).unwrap();
        let t = UNIX_EPOCH + Duration::from_secs_f64(env.platform.now - OLD);
        let times = FileTimes::new()
            .set_accessed(t)
            .set_modified(t)
            .set_created(t);
        OpenOptions::new()
            .write(true)
            .open(&f)
            .unwrap()
            .set_times(times)
            .unwrap();
        applies.push(result(
            env,
            &sort::apply_moves(ctx, &plan(env, 0).moves, Some(&env.root), 0),
        ));
    }
    if !applies.is_empty() {
        out.insert("applies".into(), applies.into());
    }
    out.insert("log_after_apply".into(), undo_log(env).into());
    out.insert("tree_after_apply".into(), env.snapshot());
    for a in p["after_apply"].as_array().into_iter().flatten() {
        env.act(a);
    }
    out.insert("can_undo".into(), undo::can_undo(ctx).into());
    out.insert("undo_count".into(), undo::undo_count(ctx).into());
    out.insert("undo".into(), result(env, &undo::undo_last(ctx)));
    out.insert("tree_after_undo".into(), env.snapshot());
    out.insert("log_len_after_undo".into(), undo_log(env).len().into());
    Value::Object(out)
}
