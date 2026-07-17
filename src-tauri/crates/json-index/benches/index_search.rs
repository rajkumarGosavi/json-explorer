//! Statistical benchmarks for `build_index` and `search_bytes`.
//!
//! Eyeball benches (examples/bench_index, bench_search) print one number and
//! leave you guessing whether a delta is real. Criterion runs many samples,
//! reports mean + confidence interval, and — via `target/criterion` history —
//! flags regressions/improvements between runs. Use this to gate perf work.
//!
//! Point it at a real fixture (default is a small generated one so `cargo bench`
//! works out of the box):
//!   cargo run --release -p json-index --example gen_fixture -- 200mb bench.json
//!   BENCH_FILE=bench.json cargo bench -p json-index
//!
//! Override the search query with BENCH_QUERY (default "fox", which
//! gen_fixture plants in every record's "note").

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use json_index::{build_index, search_bytes};
use memmap2::Mmap;
use std::fs::File;
use std::hint::black_box;
use std::io::Write;

/// Load BENCH_FILE via mmap, or synthesize a small in-memory fixture so the
/// bench is runnable with zero setup. Returns owned bytes to keep lifetimes
/// simple for both paths.
fn load_bytes() -> Vec<u8> {
    // Empty string counts as unset (the Makefile always exports BENCH_FILE,
    // blank when no FILE= was given).
    if let Some(path) = std::env::var("BENCH_FILE").ok().filter(|p| !p.is_empty()) {
        let file = File::open(&path).expect("open BENCH_FILE");
        let mmap = unsafe { Mmap::map(&file).expect("mmap BENCH_FILE") };
        return mmap.to_vec();
    }
    // ~8 MB synthetic fixture mirroring gen_fixture's shape.
    let mut buf = Vec::with_capacity(9_000_000);
    buf.extend_from_slice(b"{\"records\":[");
    let mut i: u64 = 0;
    while buf.len() < 8_000_000 {
        if i > 0 {
            buf.push(b',');
        }
        write!(
            buf,
            "{{\"id\":{i},\"name\":\"item-{i}\",\"tags\":[\"a\",\"b\",\"c\"],\
             \"active\":{},\"note\":\"the quick brown fox jumps over the lazy dog {i}\"}}",
            i % 2 == 0
        )
        .unwrap();
        i += 1;
    }
    buf.extend_from_slice(b"]}");
    buf
}

fn benches(c: &mut Criterion) {
    let bytes = load_bytes();
    let query = std::env::var("BENCH_QUERY").unwrap_or_else(|_| "fox".to_string());

    let mut group = c.benchmark_group("json-index");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    // Pure indexing cost. new fresh index each iteration (BatchSize::PerIteration
    // avoids reusing a warmed allocation and skews).
    group.bench_function("build_index", |b| {
        b.iter_batched(
            || (),
            |_| black_box(build_index(black_box(&bytes)).expect("index builds")),
            BatchSize::PerIteration,
        );
    });

    // Search cost only — index built once outside the timed loop, so this
    // isolates the search hot path from indexing.
    let index = build_index(&bytes).expect("index builds");
    group.bench_function("search_bytes", |b| {
        b.iter(|| {
            let mut hits = 0u64;
            let (total, _trunc) = search_bytes(
                black_box(&bytes),
                black_box(&index),
                black_box(&query),
                false,
                false,
                |_hit| {
                    hits += 1;
                    true
                },
            );
            black_box((total, hits))
        });
    });

    group.finish();
}

criterion_group!(g, benches);
criterion_main!(g);
