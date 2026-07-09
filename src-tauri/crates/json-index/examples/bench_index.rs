//! Times `build_index` against a generated fixture.
//! Usage: cargo run --release -p json-index --example bench_index -- <path>

use json_index::build_index;
use memmap2::Mmap;
use std::env;
use std::fs::File;
use std::time::Instant;

fn main() {
    let path = env::args().nth(1).expect("usage: bench_index <path>");
    let file = File::open(&path).expect("open fixture");
    let mmap = unsafe { Mmap::map(&file).expect("mmap fixture") };

    let start = Instant::now();
    let index = build_index(&mmap).expect("index should build");
    let elapsed = start.elapsed();

    eprintln!(
        "indexed {} bytes, {} containers in {:?} ({:.1} MB/s)",
        mmap.len(),
        index.container_count(),
        elapsed,
        mmap.len() as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}
