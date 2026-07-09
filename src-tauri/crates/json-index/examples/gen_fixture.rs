//! Generates large JSON fixture files for indexing benchmarks.
//! Usage: cargo run --release -p json-index --example gen_fixture -- <size> [path]
//! <size> like "2gb", "500mb", or a raw byte count.

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn parse_size(s: &str) -> u64 {
    let s = s.to_lowercase();
    if let Some(n) = s.strip_suffix("gb") {
        n.parse::<u64>().unwrap() * 1_000_000_000
    } else if let Some(n) = s.strip_suffix("mb") {
        n.parse::<u64>().unwrap() * 1_000_000
    } else {
        s.parse::<u64>().unwrap()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_bytes = parse_size(args.get(1).map(String::as_str).unwrap_or("100mb"));
    let path = args.get(2).cloned().unwrap_or_else(|| "fixture.json".to_string());

    let file = File::create(&path).expect("create fixture file");
    let mut w = BufWriter::new(file);
    let start = Instant::now();

    write!(w, "{{\"records\":[").unwrap();
    let mut written = 12u64;
    let mut i: u64 = 0;
    loop {
        if i > 0 {
            w.write_all(b",").unwrap();
            written += 1;
        }
        let entry = format!(
            "{{\"id\":{i},\"name\":\"item-{i}\",\"tags\":[\"a\",\"b\",\"c\"],\"active\":{},\"score\":{:.3},\"note\":\"the quick brown fox jumps over the lazy dog {i}\"}}",
            i.is_multiple_of(2),
            (i % 1000) as f64 / 7.0,
        );
        w.write_all(entry.as_bytes()).unwrap();
        written += entry.len() as u64;
        i += 1;
        if written >= target_bytes {
            break;
        }
    }
    write!(w, "]}}").unwrap();
    w.flush().unwrap();

    eprintln!(
        "wrote {path} ({written} bytes, {i} records) in {:?}",
        start.elapsed()
    );
}
