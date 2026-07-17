use crate::index::{JsonKind, NodeRef, RootKind};
use crate::scanner::build_index;
use crate::search::search_bytes;

fn assert_kind(k: JsonKind, expected: &str) {
    let s = match k {
        JsonKind::Object => "object",
        JsonKind::Array => "array",
        JsonKind::String => "string",
        JsonKind::Number => "number",
        JsonKind::Bool => "bool",
        JsonKind::Null => "null",
    };
    assert_eq!(s, expected);
}

#[test]
fn empty_input_is_error() {
    assert!(build_index(b"").is_err());
    assert!(build_index(b"   \n\t").is_err());
}

#[test]
fn empty_object_and_array() {
    let idx = build_index(b"{}").unwrap();
    assert_eq!(idx.container_count(), 1);
    assert_eq!(idx.child_count_of(0), 0);

    let idx = build_index(b"[]").unwrap();
    assert_eq!(idx.container_count(), 1);
    assert_eq!(idx.child_count_of(0), 0);
}

#[test]
fn simple_object_children() {
    let buf = br#"{"a": 1, "b": "two", "c": true, "d": null, "e": [1,2,3]}"#;
    let idx = build_index(buf).unwrap();
    assert_eq!(idx.container_count(), 2); // outer object + inner array
    let children = idx.children(buf, 0, 0, 10);
    assert_eq!(children.len(), 5);
    assert_eq!(children[0].key.as_deref(), Some("a"));
    assert_kind(children[0].kind, "number");
    assert_eq!(children[1].key.as_deref(), Some("b"));
    assert_kind(children[1].kind, "string");
    assert_eq!(children[2].key.as_deref(), Some("c"));
    assert_kind(children[2].kind, "bool");
    assert_eq!(children[3].key.as_deref(), Some("d"));
    assert_kind(children[3].kind, "null");
    assert_eq!(children[4].key.as_deref(), Some("e"));
    assert_kind(children[4].kind, "array");
    assert_eq!(children[4].child_count, 3);
}

#[test]
fn nested_arrays_deep() {
    let mut s = String::new();
    for _ in 0..1000 {
        s.push('[');
    }
    s.push('1');
    for _ in 0..1000 {
        s.push(']');
    }
    let idx = build_index(s.as_bytes()).unwrap();
    assert_eq!(idx.container_count(), 1000);
}

#[test]
fn string_escapes() {
    let buf = "{\"a\": \"line\\nbreak\", \"b\": \"quote\\\"here\", \"c\": \"back\\\\slash\", \"d\": \"\u{e9}\"}"
        .as_bytes();
    let idx = build_index(buf).unwrap();
    let children = idx.children(buf, 0, 0, 10);
    assert_eq!(children.len(), 4);
    // Verify raw byte ranges round-trip through serde_json's own unescape as
    // an oracle: wrap the raw slice back into a JSON string literal.
    for c in &children {
        let raw = &buf[c.value_start as usize..c.value_end as usize];
        let v: serde_json::Value = serde_json::from_slice(raw).unwrap();
        assert!(v.is_string());
    }
}

#[test]
fn unicode_keys() {
    let buf = "{\"caf\u{e9}\": 1, \"\u{4e2d}\u{6587}\": 2}".as_bytes();
    let idx = build_index(buf).unwrap();
    let children = idx.children(buf, 0, 0, 10);
    assert_eq!(children[0].key.as_deref(), Some("caf\u{e9}"));
    assert_eq!(children[1].key.as_deref(), Some("\u{4e2d}\u{6587}"));
}

#[test]
fn ndjson_multi_doc() {
    let buf = b"{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n";
    let idx = build_index(buf).unwrap();
    match &idx.root {
        RootKind::MultiDoc { doc_refs, .. } => assert_eq!(doc_refs.len(), 3),
        RootKind::Single(_) => panic!("expected MultiDoc"),
    }
    assert_eq!(idx.container_count(), 3);
}

#[test]
fn invalid_truncated_object() {
    let err = build_index(b"{\"a\": 1,").unwrap_err();
    match err {
        crate::error::IndexError::Syntax { .. } => {}
        other => panic!("expected Syntax error, got {other:?}"),
    }
}

#[test]
fn invalid_trailing_garbage_in_container() {
    // Trailing comma before close is rejected.
    assert!(build_index(b"[1, 2,]").is_err());
}

#[test]
fn invalid_bad_string() {
    assert!(build_index(b"{\"a\": \"unterminated}").is_err());
}

#[test]
fn syntax_error_reports_correct_offset() {
    let buf = b"{\n  \"a\": ,\n}";
    let err = build_index(buf).unwrap_err();
    if let crate::error::IndexError::Syntax { byte_offset, line, .. } = err {
        assert_eq!(buf[byte_offset as usize], b',');
        assert_eq!(line, 2);
    } else {
        panic!("expected Syntax error");
    }
}

#[test]
fn path_of_and_node_at_offset() {
    let buf = br#"{"a": {"b": [10, 20, {"c": "deep"}]}}"#;
    let idx = build_index(buf).unwrap();
    // Locate "deep" by byte offset and confirm the resolved path.
    let deep_offset = buf.windows(4).position(|w| w == b"deep").unwrap() as u64;
    let (node, path) = idx.node_at_offset(buf, deep_offset);
    assert!(matches!(node, NodeRef::Leaf { .. }));
    let path_str: Vec<String> = path
        .iter()
        .map(|s| match &s.key {
            Some(k) => k.clone(),
            None => s.index.to_string(),
        })
        .collect();
    assert_eq!(path_str, vec!["a", "b", "2", "c"]);
}

#[test]
fn search_literal_and_regex() {
    let buf = br#"{"users": [{"name": "Alice"}, {"name": "Bob"}, {"name": "alice2"}]}"#;
    let idx = build_index(buf).unwrap();
    let mut hits = Vec::new();
    let (count, truncated) = search_bytes(buf, &idx, "Alice", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 1);
    assert!(!truncated);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "$.users[0].name");

    hits.clear();
    let (count, _) = search_bytes(buf, &idx, "alice", false, false, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 2); // case-insensitive matches Alice and alice2

    hits.clear();
    let (count, _) = search_bytes(buf, &idx, "^A", true, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 0); // no value starts the whole buffer with "A"
}

#[test]
fn search_cancel_stops_early() {
    let buf = br#"[1,1,1,1,1,1,1,1,1,1]"#;
    let idx = build_index(buf).unwrap();
    let mut seen = 0;
    let (count, _) = search_bytes(buf, &idx, "1", false, true, |_h| {
        seen += 1;
        seen < 3 // cancel after 3rd hit
    });
    assert_eq!(seen, 3);
    assert_eq!(count, 3);
}

/// Property-style test: enumerate every node in the index (recursively) for
/// a batch of small fixtures and cross-check container child counts + leaf
/// kinds against serde_json::Value as an oracle.
#[test]
fn oracle_matches_serde_json_for_small_fixtures() {
    let fixtures: &[&str] = &[
        r#"{"a":1,"b":[1,2,3],"c":{"d":null,"e":false}}"#,
        r#"[1,2,3,[4,5,[6,7]],{"x":"y"}]"#,
        r#"{"empty_obj":{},"empty_arr":[],"nested":{"a":{"b":{"c":1}}}}"#,
        r#"{"unicode":"café 中文","escapes":"a\"b\\c\nd"}"#,
    ];
    for src in fixtures {
        let buf = src.as_bytes();
        let idx = build_index(buf).unwrap();
        let oracle: serde_json::Value = serde_json::from_str(src).unwrap();
        verify_against_oracle(buf, &idx, 0, &oracle);
    }
}

#[test]
fn children_seek_past_multiple_object_entries_does_not_panic() {
    // Regression test: children()'s "seek to offset" loop used to call
    // skip_entry() directly on object entries, which only consumes the key
    // (a quoted string looks like any other value to it) and never the ':'
    // or the value — leaving `pos` stuck on ':' and corrupting all further
    // parsing. This only shows up when the requested offset isn't a
    // checkpoint boundary (a multiple of CHECKPOINT_STRIDE), i.e. almost any
    // real object with more than one key.
    let buf = br#"{"a": 1, "b": 2, "c": 3, "d": 4, "e": 5}"#;
    let idx = build_index(buf).unwrap();
    for i in 0..5u64 {
        let children = idx.children(buf, 0, i, 1);
        assert_eq!(children.len(), 1, "seeking to offset {i}");
        let expected_key = ((b'a' + i as u8) as char).to_string();
        assert_eq!(children[0].key.as_deref(), Some(expected_key.as_str()));
    }
}

#[test]
fn path_of_resolves_key_beyond_first_object_entry() {
    // path_of -> key_at -> children(buf, parent, idx, 1) with a non-zero,
    // non-checkpoint-aligned idx is exactly the call pattern search hits
    // exercise for almost every result (see the regression test above for
    // the underlying bug this used to hit).
    let buf = br#"{"a": 1, "b": 2, "c": {"needle": true}, "d": 4}"#;
    let idx = build_index(buf).unwrap();
    let needle_offset = buf.windows(6).position(|w| w == b"needle").unwrap() as u64;
    let (node, path) = idx.node_at_offset(buf, needle_offset);
    assert!(matches!(node, NodeRef::Leaf { .. }) || matches!(node, NodeRef::Container(_)));
    let path_str: Vec<String> = path
        .iter()
        .map(|s| s.key.clone().unwrap_or_else(|| s.index.to_string()))
        .collect();
    assert_eq!(path_str, vec!["c"]);
}

#[test]
fn search_across_many_sibling_object_keys_resolves_paths_without_panicking() {
    // End-to-end version of the two tests above: a top-level array of
    // objects each with several keys, searched for a value that only occurs
    // in later keys/objects, forcing path resolution through non-checkpoint
    // offsets repeatedly.
    let mut buf = String::from("[");
    for i in 0..50u32 {
        if i > 0 {
            buf.push(',');
        }
        buf.push_str(&format!(
            r#"{{"id":{i},"name":"n{i}","tag":"x","note":"marker{i}","extra":0}}"#
        ));
    }
    buf.push(']');
    let buf = buf.into_bytes();
    let idx = build_index(&buf).unwrap();
    let mut hits = Vec::new();
    let (count, _) = search_bytes(&buf, &idx, "marker", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 50);
    for (i, hit) in hits.iter().enumerate() {
        assert_eq!(hit.path, format!("$[{i}].note"));
    }
}

// --- Search edge cases found while investigating "search is slow / panics"
// reports. `node_at_offset` (index.rs) resolves a search-hit byte offset to a
// tree path by linearly scanning every container's [start, end) range, once
// per hit. These tests pin down correctness at the boundaries that scan
// touches; `examples/bench_search.rs` shows the resulting time complexity.

#[test]
fn search_hit_at_very_start_of_buffer_does_not_panic() {
    // offset 0 is less than PREVIEW_RADIUS (80), exercising the
    // saturating_sub underflow guard in build_hit.
    let buf = br#"["hi"]"#;
    let idx = build_index(buf).unwrap();
    let mut hits = Vec::new();
    let (count, _) = search_bytes(buf, &idx, "[", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 1);
    assert_eq!(hits[0].byte_offset, 0);
}

#[test]
fn search_hit_at_very_end_of_buffer_does_not_panic() {
    // Match's end (offset + len) sits at buf.len(), exercising the
    // preview-window's upper clamp.
    let buf = br#"["z"]"#;
    let idx = build_index(buf).unwrap();
    let mut hits = Vec::new();
    let (count, _) = search_bytes(buf, &idx, "]", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 1);
    assert_eq!(hits[0].byte_offset as usize, buf.len() - 1);
}

#[test]
fn search_matching_structural_characters_resolves_to_container_path() {
    // "," and ":" only ever appear as JSON syntax, never inside a value's own
    // byte range, so node_at_offset's per-child scan should fall back to the
    // enclosing container rather than panicking or matching a wrong leaf.
    let buf = br#"{"a": 1, "b": 2}"#;
    let idx = build_index(buf).unwrap();
    let mut hits = Vec::new();
    let (count, _) = search_bytes(buf, &idx, ",", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 1);
    assert_eq!(hits[0].path, "$");
}

#[test]
fn search_resolves_correct_innermost_container_among_many_nested() {
    // 500 nested arrays means node_at_offset's linear "smallest enclosing
    // range" scan has to correctly pick the *innermost* of 500 overlapping
    // ranges, not just the first one found.
    let mut s = String::new();
    for _ in 0..500 {
        s.push('[');
    }
    s.push_str("\"needle\"");
    for _ in 0..500 {
        s.push(']');
    }
    let buf = s.as_bytes();
    let idx = build_index(buf).unwrap();
    let mut hits = Vec::new();
    let (count, _) = search_bytes(buf, &idx, "needle", false, true, |h| {
        hits.push(h);
        true
    });
    assert_eq!(count, 1);
    assert!(matches!(hits[0].node, NodeRef::Leaf { .. }));
    // Path should have exactly 500 numeric segments (index 0 at every level).
    assert_eq!(hits[0].path, format!("${}", "[0]".repeat(500)));
}

#[test]
fn search_invalid_regex_returns_no_hits_instead_of_panicking() {
    let buf = br#"{"a": 1}"#;
    let idx = build_index(buf).unwrap();
    let (count, truncated) = search_bytes(buf, &idx, "(unclosed", true, true, |_h| true);
    assert_eq!(count, 0);
    assert!(!truncated);
}

#[test]
fn search_empty_query_string_does_not_panic() {
    // Frontend guards against blank queries (see useSearch.test.ts), but the
    // Rust layer should still behave sanely if ever called directly with "".
    let buf = br#"{"a": 1}"#;
    let idx = build_index(buf).unwrap();
    let (count, _) = search_bytes(buf, &idx, "", false, true, |_h| true);
    // Empty literal "matches" at every byte position (including buf.len()+1
    // positions) via memmem; just assert it terminates and doesn't panic.
    assert!(count > 0);
}

#[test]
fn ndjson_scalar_top_level_docs_resolve_to_their_doc_index() {
    // NDJSON where each line is a bare scalar (no enclosing object/array).
    // These docs aren't recorded as containers, so node_at_offset resolves
    // them via RootKind::MultiDoc's doc_starts instead (root_scalar_at).
    let buf = b"\"apple\"\n\"banana\"\n\"cherry\"\n";
    let idx = build_index(buf).unwrap();
    match &idx.root {
        RootKind::MultiDoc { doc_refs, .. } => assert_eq!(doc_refs.len(), 3),
        RootKind::Single(_) => panic!("expected MultiDoc"),
    }

    // Each doc's node must resolve to the right path AND be distinguishable
    // from the others once encoded for IPC — this used to collide, because
    // the old sentinel parent value for root-level scalars (u32::MAX) set
    // the same bit NodeRef::encode uses to tag containers, so every scalar
    // doc round-tripped back as Container(0) regardless of which doc it was.
    let mut ids = Vec::new();
    for (query, expected_idx) in [("apple", 0u64), ("banana", 1), ("cherry", 2)] {
        let mut hits = Vec::new();
        let (count, _) = search_bytes(buf, &idx, query, false, true, |h| {
            hits.push(h);
            true
        });
        assert_eq!(count, 1, "query {query:?}");
        assert_eq!(hits[0].path, format!("$[{expected_idx}]"));
        let encoded = hits[0].node.encode();
        assert_eq!(
            NodeRef::decode(encoded),
            hits[0].node,
            "encode/decode round-trip must preserve the Leaf, not collide \
             with the Container tag bit"
        );
        ids.push(encoded);
    }
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);
}

#[test]
fn root_scalar_leaf_id_does_not_collide_with_container_tag() {
    // Direct regression test for the NO_PARENT sentinel fix: a Leaf using
    // the old sentinel (u32::MAX) would encode with bit 63 set, which
    // NodeRef::decode reads as "this is a Container". NO_PARENT's MSB is 0,
    // so it must decode back as the same Leaf.
    let leaf = NodeRef::Leaf { parent: crate::index::NO_PARENT, child_idx: 7 };
    let encoded = leaf.encode();
    assert_eq!(NodeRef::decode(encoded), leaf);
    assert_ne!(
        NodeRef::decode(encoded),
        NodeRef::Container(7),
        "must not be misread as a container"
    );
}

/// Not a perf assertion (timing thresholds are flaky across machines) — this
/// is a regression guard for `StructuralIndex::innermost_container_at`
/// (index.rs): it used to be an `O(container_count)` linear scan per hit,
/// making total search cost `O(hits * container_count)`. It's now a binary
/// search plus a parent-chain walk bounded by nesting depth. Run with
/// `cargo test --release -- --ignored --nocapture` to see wall-clock time
/// stay flat as container_count grows; compare against
/// examples/bench_search.rs for a realistic file-sized measurement.
#[test]
#[ignore]
fn search_many_containers_smoke_timing() {
    use std::time::Instant;
    // Every record is its own object containing a nested array -> 2 containers
    // per record, so N records means ~2N entries in `starts`.
    let mut s = String::from(r#"{"records":["#);
    for i in 0..20_000u32 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            r#"{{"id":{i},"tags":["a","b","c"],"note":"the quick brown fox {i}"}}"#
        ));
    }
    s.push_str("]}");
    let buf = s.as_bytes();
    let idx = build_index(buf).unwrap();
    eprintln!("container_count = {}", idx.container_count());

    let start = Instant::now();
    let (count, _) = search_bytes(buf, &idx, "fox", false, true, |_h| true);
    let elapsed = start.elapsed();
    eprintln!("{count} hits over {} containers in {elapsed:?}", idx.container_count());
    assert_eq!(count, 20_000);
}

fn verify_against_oracle(
    buf: &[u8],
    idx: &crate::index::StructuralIndex,
    container: u32,
    oracle: &serde_json::Value,
) {
    match oracle {
        serde_json::Value::Object(map) => {
            let children = idx.children(buf, container, 0, idx.child_count_of(container) as u32);
            assert_eq!(children.len(), map.len());
            for (child, (k, v)) in children.iter().zip(map.iter()) {
                assert_eq!(child.key.as_deref(), Some(k.as_str()));
                if let NodeRef::Container(id) = child.node {
                    verify_against_oracle(buf, idx, id, v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            let children = idx.children(buf, container, 0, idx.child_count_of(container) as u32);
            assert_eq!(children.len(), arr.len());
            for (child, v) in children.iter().zip(arr.iter()) {
                if let NodeRef::Container(id) = child.node {
                    verify_against_oracle(buf, idx, id, v);
                }
            }
        }
        _ => {}
    }
}

#[test]
fn classify_offset_distinguishes_key_and_value() {
    use crate::index::OffsetRole;
    // bytes: {"cat":"cat food"}  — key "cat" quoted at 1..6, value string at 7..17
    let buf = br#"{"cat":"cat food"}"#;
    let idx = build_index(buf).unwrap();
    assert_eq!(idx.classify_offset(buf, 3), OffsetRole::Key); // inside the key
    assert_eq!(idx.classify_offset(buf, 9), OffsetRole::Value); // inside the value
    // Array elements have no keys — always Value.
    let arr = build_index(b"[1,2,3]").unwrap();
    assert_eq!(arr.classify_offset(b"[1,2,3]", 1), OffsetRole::Value);
}

#[test]
fn search_scope_keys_vs_values() {
    use crate::search::{search_scoped, SearchTarget};
    // "cat" appears once in a key and once in a value.
    let buf = br#"{"cat":"cat food","dog":"bark"}"#;
    let idx = build_index(buf).unwrap();

    let run = |target| {
        let mut hits = Vec::new();
        let (count, _) = search_scoped(buf, &idx, "cat", false, true, target, |h| {
            hits.push(h);
            true
        });
        (count, hits)
    };

    assert_eq!(run(SearchTarget::Both).0, 2);

    let (keys, kh) = run(SearchTarget::Keys);
    assert_eq!(keys, 1);
    assert!(kh[0].byte_offset < 6, "key hit should be in the key span");

    let (values, vh) = run(SearchTarget::Values);
    assert_eq!(values, 1);
    assert!(vh[0].byte_offset > 6, "value hit should be past the key");
}

#[test]
fn child_kinds_and_bounds_back_node_stats() {
    // One direct child of every kind, in a known order. This is exactly what
    // the get_node_stats command iterates to build its histogram + byte size.
    let buf = br#"{"o":{},"a":[],"s":"x","n":1,"b":true,"z":null}"#;
    let idx = build_index(buf).unwrap();
    let root = 0u32; // the root object is the first container scanned
    let kids = idx.children(buf, root, 0, idx.child_count_of(root) as u32);
    let kinds: Vec<_> = kids.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            JsonKind::Object,
            JsonKind::Array,
            JsonKind::String,
            JsonKind::Number,
            JsonKind::Bool,
            JsonKind::Null,
        ]
    );
    let (start, end) = idx.bounds(root);
    assert_eq!(start, 0);
    assert_eq!(end as usize, buf.len()); // byte_size spans the whole object
}
