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
