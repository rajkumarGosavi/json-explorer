//! Heap-profiles `build_index` (and optionally a full search sweep) with dhat.
//!
//! The file itself is memory-mapped, so it does NOT show up as heap — that's OS
//! page cache, not our allocations. What dhat measures here is the real cost we
//! control: the `StructuralIndex` (container arrays + checkpoints) and any
//! transient allocation during the scan. The number to watch is
//! `peak heap bytes / file bytes` — how much RAM the index adds on top of the
//! mapped file. Track it as fixtures grow (100MB -> 1GB -> 4GB); it should stay
//! a roughly constant fraction.
//!
//! Usage:
//!   cargo run --release -p json-index --example mem_index -- <path> [query]
//!
//! Writes `dhat-heap.json` in the working dir. View at
//! https://nnethercote.github.io/dh_view/dh_view.html (drag the file in), or
//! just read the peak-heap line printed to stderr on exit.

use json_index::{build_index, search_bytes};
use memmap2::Mmap;
use std::env;
use std::fs::File;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    // dhat installs its allocator hooks for the lifetime of this guard and
    // dumps dhat-heap.json when it drops at end of main.
    let _profiler = dhat::Profiler::new_heap();

    let mut args = env::args().skip(1);
    let path = args.next().expect("usage: mem_index <path> [query]");
    let query = args.next();

    let file = File::open(&path).expect("open fixture");
    let mmap = unsafe { Mmap::map(&file).expect("mmap fixture") };

    let index = build_index(&mmap).expect("index should build");
    eprintln!(
        "indexed {} bytes, {} containers",
        mmap.len(),
        index.container_count()
    );

    // Optional: run a search too, so the profile also captures per-hit
    // allocation (preview/path strings) under load.
    if let Some(q) = query {
        let mut hits = 0u64;
        let (total, truncated) = search_bytes(&mmap, &index, &q, false, false, |_hit| {
            hits += 1;
            true
        });
        eprintln!("query {q:?}: {total} hits (truncated={truncated})");
    }

    // index + mmap drop here; dhat prints the peak-heap summary on profiler drop.
}
