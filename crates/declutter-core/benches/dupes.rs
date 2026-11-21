//! Duplicate hashing over a tree (owner: release). Target: docs/PERF.md.
//! Run: `cargo bench -p declutter-core --bench dupes`.
//! "cold_cache" starts from an empty hash cache on every iteration (the OS file cache is warm after the first);
//! "warm_cache" is the rescan after an action, where unchanged files must not be hashed again.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use declutter_core::dupes::find_duplicates;
use declutter_core::walk::walk_files;

#[path = "common.rs"]
mod common;

fn dupes(c: &mut Criterion) {
    let tree = common::Tree::open();
    let data = tempfile::tempdir().expect("data dir");
    let entries = walk_files(&common::ctx(data.path()), &tree.root, &[]).entries;

    let mut group = c.benchmark_group("dupes");
    group.throughput(Throughput::Elements(entries.len() as u64));
    group.sample_size(10);

    group.bench_function("cold_cache", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().expect("data dir");
                let ctx = common::ctx(dir.path());
                (dir, ctx)
            },
            |(dir, ctx)| (find_duplicates(&ctx, black_box(&entries), 1), dir),
            BatchSize::PerIteration,
        )
    });

    let warm = common::ctx(data.path());
    find_duplicates(&warm, &entries, 1); // fills the cache
    group.bench_function("warm_cache", |b| b.iter(|| black_box(find_duplicates(&warm, black_box(&entries), 1))));
    group.finish();
}

criterion_group!(benches, dupes);
criterion_main!(benches);
