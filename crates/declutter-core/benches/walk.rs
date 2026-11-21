//! Walk speed over a tree (owner: release). Target: docs/PERF.md. Run: `cargo bench -p declutter-core --bench walk`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use declutter_core::walk::walk_files;

#[path = "common.rs"]
mod common;

fn walk(c: &mut Criterion) {
    let tree = common::Tree::open();
    let data = tempfile::tempdir().expect("data dir");
    let ctx = common::ctx(data.path());
    let files = walk_files(&ctx, &tree.root, &[]).entries.len() as u64;

    let mut group = c.benchmark_group("walk");
    group.throughput(Throughput::Elements(files));
    group.sample_size(10);
    group.bench_function("walk_files", |b| b.iter(|| black_box(walk_files(&ctx, black_box(&tree.root), &[]))));
    group.finish();
}

criterion_group!(benches, walk);
criterion_main!(benches);
