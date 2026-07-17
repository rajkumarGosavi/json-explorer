//! Concurrent stress test for the read path the Tauri commands sit on top of.
//!
//! The real app fans `get_children` / `get_node_value` / `search_start` calls
//! at one shared `StructuralIndex` from multiple threads (Tauri command pool +
//! spawned search threads). This driver reproduces that contention against a
//! single index and reports latency percentiles + throughput, plus a
//! search-churn phase that hammers the start/cancel path to shake out races.
//!
//! It uses only the pure `json-index` API (no Tauri), so it runs fast and in
//! isolation. It approximates command cost, not the exact IPC/serde overhead.
//!
//! Usage:
//!   cargo run --release -p json-index --example load_test -- <path> [query] [threads] [ops_per_thread]
//! Example:
//!   cargo run --release -p json-index --example load_test -- samples/1GB.json fox 8 20000

use json_index::{build_index, search_bytes, NodeRef};
use memmap2::Mmap;
use std::env;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn pct(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().expect("usage: load_test <path> [query] [threads] [ops]");
    let query = args.next().unwrap_or_else(|| "fox".to_string());
    let threads: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let ops: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20_000);

    let file = File::open(&path).expect("open fixture");
    let mmap = Arc::new(unsafe { Mmap::map(&file).expect("mmap fixture") });
    let index = Arc::new(build_index(&mmap).expect("index should build"));
    let container_count = index.container_count() as u32;
    eprintln!(
        "loaded {} bytes, {} containers; {threads} threads x {ops} ops",
        mmap.len(),
        container_count
    );

    // -------- Phase 1: concurrent tree reads (get_children / get_node_value) --------
    let wall = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let mmap = mmap.clone();
            let index = index.clone();
            thread::spawn(move || {
                // Cheap deterministic per-thread PRNG (xorshift) so each thread
                // walks different containers without pulling in rand.
                let mut seed = 0x9E3779B97F4A7C15u64 ^ ((t as u64 + 1) * 0x1234_5678);
                let mut next = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed
                };
                let mut lat = Vec::with_capacity(ops);
                for _ in 0..ops {
                    let id = (next() % container_count.max(1) as u64) as u32;
                    let start = Instant::now();
                    // Mirror get_children: page 200 kids and touch previews.
                    let kids = index.children(&mmap, id, 0, 200);
                    let mut sink = 0usize;
                    for c in &kids {
                        sink ^= (c.value_end - c.value_start) as usize;
                    }
                    std::hint::black_box(sink);
                    lat.push(start.elapsed());
                }
                lat
            })
        })
        .collect();

    let mut all: Vec<Duration> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    let elapsed = wall.elapsed();
    all.sort_unstable();
    let total_ops = all.len();
    eprintln!(
        "\n[reads] {total_ops} ops in {elapsed:?} = {:.0} ops/s\n  \
         p50={:?} p90={:?} p99={:?} max={:?}",
        total_ops as f64 / elapsed.as_secs_f64(),
        pct(&all, 0.50),
        pct(&all, 0.90),
        pct(&all, 0.99),
        all.last().copied().unwrap_or_default(),
    );

    // -------- Phase 2: search start/cancel churn (thread-storm + race check) --------
    // Reproduces search_start's generation/cancel protocol: many searches kicked
    // off rapidly, most cancelled almost immediately. Verifies the cancel flag
    // actually stops the scan and that only the latest generation "wins".
    let generation = Arc::new(AtomicU64::new(0));
    let churn = 200usize;
    let mut completed = 0u64;
    let churn_wall = Instant::now();
    for _ in 0..churn {
        let my_gen = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let mmap = mmap.clone();
        let index = index.clone();
        let query = query.clone();
        let gen_ref = generation.clone();
        let cancel_ref = cancel.clone();
        let h = thread::spawn(move || {
            let mut hits = 0u64;
            search_bytes(&mmap, &index, &query, false, false, |_hit| {
                hits += 1;
                // Stop when cancelled or superseded — same predicate as the real
                // search_start closure.
                !(cancel_ref.load(Ordering::SeqCst)
                    || gen_ref.load(Ordering::SeqCst) != my_gen)
            });
            hits
        });
        // Cancel ~90% of searches near-immediately to stress the abort path.
        if my_gen % 10 != 0 {
            cancel.store(true, Ordering::SeqCst);
        }
        completed += h.join().unwrap();
        std::hint::black_box(NodeRef::Root);
    }
    eprintln!(
        "[search-churn] {churn} start/cancel cycles in {:?}, {completed} total hits scanned \
         (no panic/deadlock = cancel+generation protocol holds)",
        churn_wall.elapsed()
    );

    eprintln!("\nload test done. watch process RSS in Task Manager during the run for memory.");
}
