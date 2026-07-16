//! Single forward pass over the raw bytes that builds a `StructuralIndex`
//! (containers only) and validates JSON structure. Runs once at file-open
//! time; all later access (children/search/path) re-scans small windows
//! using the same primitives but trusts the input (already validated here).

use crate::error::IndexError;
use crate::index::{NodeRef, RootKind, StructuralIndex, NO_PARENT};
use crate::raw::{skip_literal, skip_number, skip_string, skip_ws};

pub fn build_index(buf: &[u8]) -> Result<StructuralIndex, IndexError> {
    build_index_with_progress(buf, &mut |_pos| {})
}

/// Same as `build_index`, but calls `on_progress(current_byte_offset)` after
/// each child registered in a container — callers throttle how often that
/// translates into an actual UI update (e.g. every 64 MB).
pub fn build_index_with_progress(
    buf: &[u8],
    on_progress: &mut dyn FnMut(u64),
) -> Result<StructuralIndex, IndexError> {
    let mut index = StructuralIndex::default();
    let mut pos = 0usize;

    // Skip UTF-8 BOM if present.
    if buf.len() >= 3 && buf[0..3] == [0xEF, 0xBB, 0xBF] {
        pos = 3;
    }

    skip_ws(buf, &mut pos);
    if pos >= buf.len() {
        return Err(IndexError::Empty);
    }

    let first_ref = scan_value(buf, &mut pos, &mut index, -1, on_progress)?;

    skip_ws(buf, &mut pos);
    if pos >= buf.len() {
        index.root = RootKind::Single(first_ref);
        return Ok(index);
    }

    // More content after the first value: NDJSON / concatenated JSON.
    let mut doc_starts = vec![doc_start_of(first_ref, &index)];
    let mut doc_refs = vec![with_doc_index(first_ref, 0)];
    loop {
        skip_ws(buf, &mut pos);
        if pos >= buf.len() {
            break;
        }
        let start = pos as u64;
        let r = scan_value(buf, &mut pos, &mut index, -1, on_progress)?;
        let r = with_doc_index(r, doc_refs.len() as u32);
        doc_starts.push(start);
        doc_refs.push(r);
        skip_ws(buf, &mut pos);
        if pos >= buf.len() {
            break;
        }
    }
    index.root = RootKind::MultiDoc { doc_starts, doc_refs };
    Ok(index)
}

fn doc_start_of(node: NodeRef, index: &StructuralIndex) -> u64 {
    match node {
        NodeRef::Container(id) => index.bounds(id).0,
        _ => 0,
    }
}

/// Top-level scalar leaves come out of `scan_value` with a placeholder
/// `child_idx` of 0 (it has no way to know its position among sibling
/// top-level documents). Once we know that position, stamp it in so
/// `NodeRef::Leaf { parent: NO_PARENT, child_idx }` uniquely identifies which
/// document it is — see index.rs's `node_at_offset`/`path_of`/`NO_PARENT`.
fn with_doc_index(node: NodeRef, idx: u32) -> NodeRef {
    match node {
        NodeRef::Leaf { parent, .. } if parent == NO_PARENT => {
            NodeRef::Leaf { parent: NO_PARENT, child_idx: idx }
        }
        other => other,
    }
}

/// Scans one JSON value at `*pos`, recursing into containers so their
/// children get registered (`note_child`) and bounds get closed. Returns a
/// NodeRef describing the value (Container id, or a synthetic Leaf marker
/// with parent = the given `parent` — leaves outside any container, i.e.
/// top-level scalars, use parent = NO_PARENT as a sentinel meaning "no
/// parent"; such documents are addressed by their doc index instead, see
/// `with_doc_index`).
fn leaf_parent(parent: i64) -> u32 {
    if parent < 0 {
        NO_PARENT
    } else {
        parent as u32
    }
}

fn scan_value(
    buf: &[u8],
    pos: &mut usize,
    index: &mut StructuralIndex,
    parent: i64,
    on_progress: &mut dyn FnMut(u64),
) -> Result<NodeRef, IndexError> {
    if *pos >= buf.len() {
        return Err(syntax_err(buf, *pos, "unexpected end of input"));
    }
    match buf[*pos] {
        b'{' => scan_container(buf, pos, index, parent, true, on_progress),
        b'[' => scan_container(buf, pos, index, parent, false, on_progress),
        b'"' => {
            if !skip_string(buf, pos) {
                return Err(syntax_err(buf, *pos, "unterminated string"));
            }
            Ok(NodeRef::Leaf {
                parent: leaf_parent(parent),
                child_idx: 0,
            })
        }
        b't' => {
            if !skip_literal(buf, pos, b"true") {
                return Err(syntax_err(buf, *pos, "invalid literal"));
            }
            Ok(NodeRef::Leaf { parent: leaf_parent(parent), child_idx: 0 })
        }
        b'f' => {
            if !skip_literal(buf, pos, b"false") {
                return Err(syntax_err(buf, *pos, "invalid literal"));
            }
            Ok(NodeRef::Leaf { parent: leaf_parent(parent), child_idx: 0 })
        }
        b'n' => {
            if !skip_literal(buf, pos, b"null") {
                return Err(syntax_err(buf, *pos, "invalid literal"));
            }
            Ok(NodeRef::Leaf { parent: leaf_parent(parent), child_idx: 0 })
        }
        b'-' | b'0'..=b'9' => {
            let start = *pos;
            skip_number(buf, pos);
            if *pos == start {
                return Err(syntax_err(buf, *pos, "invalid number"));
            }
            Ok(NodeRef::Leaf { parent: leaf_parent(parent), child_idx: 0 })
        }
        _ => Err(syntax_err(buf, *pos, "unexpected character")),
    }
}

fn scan_container(
    buf: &[u8],
    pos: &mut usize,
    index: &mut StructuralIndex,
    parent: i64,
    is_object: bool,
    on_progress: &mut dyn FnMut(u64),
) -> Result<NodeRef, IndexError> {
    let start = *pos as u64;
    let id = index.new_container(start, is_object, parent);
    *pos += 1;
    skip_ws(buf, pos);

    let close = if is_object { b'}' } else { b']' };
    if *pos < buf.len() && buf[*pos] == close {
        *pos += 1;
        index.close_container(id, *pos as u64);
        return Ok(NodeRef::Container(id));
    }
    if *pos >= buf.len() {
        return Err(syntax_err(buf, *pos, "unterminated container"));
    }

    loop {
        let child_start = *pos as u64;
        if is_object {
            if buf.get(*pos) != Some(&b'"') {
                return Err(syntax_err(buf, *pos, "expected string key"));
            }
            if !skip_string(buf, pos) {
                return Err(syntax_err(buf, *pos, "unterminated key string"));
            }
            skip_ws(buf, pos);
            if buf.get(*pos) != Some(&b':') {
                return Err(syntax_err(buf, *pos, "expected ':' after key"));
            }
            *pos += 1;
            skip_ws(buf, pos);
        }

        index.note_child(id, child_start);
        scan_value(buf, pos, index, id as i64, on_progress)?;
        on_progress(*pos as u64);
        skip_ws(buf, pos);

        match buf.get(*pos) {
            Some(&b',') => {
                *pos += 1;
                skip_ws(buf, pos);
                if buf.get(*pos) == Some(&close) {
                    return Err(syntax_err(buf, *pos, "trailing comma"));
                }
            }
            Some(&c) if c == close => {
                *pos += 1;
                break;
            }
            _ => return Err(syntax_err(buf, *pos, "expected ',' or closing bracket")),
        }
    }

    index.close_container(id, *pos as u64);
    Ok(NodeRef::Container(id))
}

fn syntax_err(buf: &[u8], byte_offset: usize, message: &str) -> IndexError {
    let mut line = 1u64;
    let mut col = 1u64;
    for &b in &buf[..byte_offset.min(buf.len())] {
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    IndexError::Syntax {
        message: message.to_string(),
        byte_offset: byte_offset as u64,
        line,
        col,
    }
}
