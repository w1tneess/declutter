//! Golden operations owned by core-B (analysis). Ports of the matching `op_*` functions in tools/gen_golden.py:
//! duplicates, large_old, leftovers, redownloads, trash_duplicates, check_dup_selection, trash_redownloads.
//! Each returns the same JSON shape as the oracle.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use declutter_core::dupes::{check_dup_selection, find_duplicates, trash_duplicates};
use declutter_core::large_old::find_large_old;
use declutter_core::leftovers::find_leftovers;
use declutter_core::model::{ActionResult, DupGroup};
use declutter_core::redownloads::{find_redownloads, trash_redownloads};
use declutter_core::walk::{self, Entry};
use declutter_core::Ctx;
use serde_json::{json, Value};

use super::common::Env;

pub fn run(op: &str, env: &Env, fx: &Value) -> Option<Value> {
    let p = &fx["params"];
    let ctx = &env.ctx;
    let rel = |s: &str| env.rel(Path::new(s));
    let paths = |v: &Value| -> Vec<PathBuf> {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|x| env.path(x.as_str().unwrap()))
            .collect()
    };
    let result = |r: &ActionResult| json!({ "count": r.count, "errors": r.errors.iter().map(|e| env.norm(e)).collect::<Vec<_>>() });
    let files = |g: &DupGroup| g.files.iter().map(|f| rel(&f.path)).collect::<Vec<_>>();
    let after_scan = || {
        p["after_scan"]
            .as_array()
            .into_iter()
            .flatten()
            .for_each(|a| env.act(a))
    };
    Some(match op {
        "duplicates" => {
            let gs = find_duplicates(ctx, &scan(ctx, &env.root), p["min_size"].as_u64().unwrap_or(1));
            json!({ "groups": gs.iter().map(|g| json!({ "size": g.size, "files": files(g) })).collect::<Vec<_>>() })
        }
        "large_old" => {
            let out = find_large_old(
                ctx,
                &scan(ctx, &env.root),
                p["min_mb"].as_f64().unwrap(),
                p["older_days"].as_u64().unwrap() as u32,
            );
            json!({ "files": out.iter().map(|f| json!({ "path": rel(&f.path), "size": f.size })).collect::<Vec<_>>() })
        }
        "leftovers" => {
            let out = find_leftovers(ctx, &scan(ctx, &env.root));
            json!({ "items": out.iter().map(|x| json!({ "path": rel(&x.path), "size": x.size, "reason": x.reason }))
                                       .collect::<Vec<_>>() })
        }
        "redownloads" => {
            let out = find_redownloads(&scan(ctx, &env.root), &paths(&p["exclude"]));
            json!({ "pairs": out.iter().map(|r| json!({
                "original": rel(&r.original.path), "copy": rel(&r.copy.path),
                "orig_size": r.original.size, "copy_size": r.copy.size
            })).collect::<Vec<_>>() })
        }
        "trash_duplicates" => {
            let groups = find_duplicates(ctx, &scan(ctx, &env.root), 1);
            after_scan();
            let removals: Vec<(DupGroup, Vec<PathBuf>)> = p["remove"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let g = &groups[r[0].as_u64().unwrap() as usize];
                    let picks = r[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| PathBuf::from(&g.files[i.as_u64().unwrap() as usize].path));
                    (g.clone(), picks.collect())
                })
                .collect();
            let res = trash_duplicates(ctx, &removals, &env.root);
            json!({ "groups": groups.iter().map(files).collect::<Vec<_>>(), "result": result(&res), "tree": env.snapshot() })
        }
        "check_dup_selection" => {
            let groups = find_duplicates(ctx, &scan(ctx, &env.root), 1);
            let results: Vec<Value> = p["selections"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sel| {
                    let chosen: HashSet<String> = paths(sel)
                        .iter()
                        .map(|x| x.to_string_lossy().into_owned())
                        .collect();
                    let (raises, message) = match check_dup_selection(&groups, &chosen) {
                        Ok(()) => (false, String::new()),
                        Err(m) => (true, m),
                    };
                    json!({ "selection": sel, "raises": raises, "message": message })
                })
                .collect();
            json!({ "results": results })
        }
        "trash_redownloads" => {
            let pairs = find_redownloads(&scan(ctx, &env.root), &[]);
            after_scan();
            let res = trash_redownloads(ctx, &pairs, &env.root, &paths(&p["removing"]));
            json!({
                "pairs": pairs.iter().map(|r| json!({ "original": rel(&r.original.path), "copy": rel(&r.copy.path) }))
                              .collect::<Vec<_>>(),
                "result": result(&res), "tree": env.snapshot()
            })
        }
        _ => return None,
    })
}

/// The real walk, as the oracle's analysis functions use internally.
fn scan(ctx: &Ctx, root: &Path) -> Vec<Entry> {
    walk::walk_files(ctx, root, &[]).entries
}
