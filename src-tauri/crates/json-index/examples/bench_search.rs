//! Times `search_bytes` (index build + search) against a generated fixture.
//!
//! `StructuralIndex::innermost_container_at` (index.rs) used to scan every
//! container per search hit, making total search time O(hits *
//! container_count). It's now a binary search plus a parent-chain walk
//! bounded by nesting depth. Run this against fixtures of increasing size
//! (see `gen_fixture`) with the same query and per-hit time should stay flat
//! rather than growing with container_count.
//!
//! Usage:
//!   cargo run --release -p json-index --example bench_search -- <path> <query> [--regex] [--case-sensitive]
//!
//! Example, using the repo's samples:
//!   cargo run --release -p json-index --example bench_search -- ../../../samples/5MB.json fox
//!   cargo run --release -p json-index --example bench_search -- ../../../samples/5MB.json malesuada
//!   cargo run --release -p json-index --example bench_search -- ../../../samples/1GB.json fox

use json_index::{build_index, search_bytes};
use memmap2::Mmap;
use std::env;
use std::fs::File;
use std::time::Instant;

fn main() {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .expect("usage: bench_search <path> <query> [--regex] [--case-sensitive]");
    let query = args
        .next()
        .expect("usage: bench_search <path> <query> [--regex] [--case-sensitive]");
    let flags: Vec<String> = args.collect();
    let is_regex = flags.iter().any(|f| f == "--regex");
    let case_sensitive = flags.iter().any(|f| f == "--case-sensitive");

    let file = File::open(&path).expect("open fixture");
    let mmap = unsafe { Mmap::map(&file).expect("mmap fixture") };

    let index_start = Instant::now();
    let index = build_index(&mmap).expect("index should build");
    let index_elapsed = index_start.elapsed();
    eprintln!(
        "indexed {} bytes, {} containers in {:?}",
        mmap.len(),
        index.container_count(),
        index_elapsed
    );

    let search_start = Instant::now();
    let mut hits = 0u64;
    let (total, truncated) =
        search_bytes(&mmap, &index, &query, is_regex, case_sensitive, |_hit| {
            hits += 1;
            true
        });
    let search_elapsed = search_start.elapsed();

    let per_hit = if hits > 0 {
        search_elapsed / hits as u32
    } else {
        search_elapsed
    };

    eprintln!(
        "query {query:?}: {total} hits (truncated={truncated}) in {search_elapsed:?} \
         ({per_hit:?}/hit, {} containers)",
        index.container_count()
    );
    eprintln!(
        "per-hit time should stay roughly flat across fixtures of different sizes now that \
         innermost_container_at (index.rs) is O(log containers + nesting depth) instead of \
         O(container_count); if it still grows with container_count, something regressed"
    );
}
