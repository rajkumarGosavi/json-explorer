//! Ripgrep-style search directly over the raw file bytes — never over a DOM.
//! Each match's byte offset is later mapped to a tree path via
//! `StructuralIndex::node_at_offset`.

use crate::index::{NodeRef, StructuralIndex};
use memchr::memmem;
use regex::bytes::RegexBuilder;

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub node: NodeRef,
    pub path: String,
    pub preview: String,
    pub byte_offset: u64,
    pub match_len: u32,
}

const PREVIEW_RADIUS: usize = 80;
const MAX_HITS: usize = 10_000;

pub fn search_bytes(
    buf: &[u8],
    index: &StructuralIndex,
    query: &str,
    is_regex: bool,
    case_sensitive: bool,
    mut on_hit: impl FnMut(SearchHit) -> bool, // return false to cancel
) -> (usize, bool) {
    let mut count = 0usize;
    let mut truncated = false;

    let mut emit = |offset: usize, len: usize| -> bool {
        let hit = build_hit(buf, index, offset as u64, len as u32);
        count += 1;
        if count >= MAX_HITS {
            truncated = true;
        }
        let keep_going = on_hit(hit);
        keep_going && count < MAX_HITS
    };

    if is_regex {
        let re = match RegexBuilder::new(query)
            .case_insensitive(!case_sensitive)
            .build()
        {
            Ok(r) => r,
            Err(_) => return (0, false),
        };
        for m in re.find_iter(buf) {
            if !emit(m.start(), m.len()) {
                break;
            }
        }
    } else if case_sensitive {
        let finder = memmem::Finder::new(query.as_bytes());
        let mut start = 0usize;
        // An empty query matches even at buf.len() itself, so `start` can
        // walk one past the end; buf.get(..) turns that into a clean loop
        // exit instead of an out-of-bounds slice panic.
        while let Some(pos) = buf.get(start..).and_then(|s| finder.find(s)) {
            let abs = start + pos;
            if !emit(abs, query.len()) {
                break;
            }
            start = abs + query.len().max(1);
        }
    } else {
        // Case-insensitive literal search: regex engine handles this cleanly
        // without a manual lowercasing pass over the whole buffer.
        let escaped = regex::escape(query);
        let re = RegexBuilder::new(&escaped)
            .case_insensitive(true)
            .build()
            .expect("escaped literal is always a valid regex");
        for m in re.find_iter(buf) {
            if !emit(m.start(), m.len()) {
                break;
            }
        }
    }

    (count, truncated)
}

fn build_hit(buf: &[u8], index: &StructuralIndex, offset: u64, len: u32) -> SearchHit {
    let (node, path) = index.node_at_offset(buf, offset);
    let start = offset.saturating_sub(PREVIEW_RADIUS as u64) as usize;
    let end = ((offset + len as u64) as usize + PREVIEW_RADIUS).min(buf.len());
    let preview = String::from_utf8_lossy(&buf[start..end]).into_owned();
    let path_str = format_path(&path);
    SearchHit {
        node,
        path: path_str,
        preview,
        byte_offset: offset,
        match_len: len,
    }
}

fn format_path(segments: &[crate::index::PathSegment]) -> String {
    let mut s = String::from("$");
    for seg in segments {
        match &seg.key {
            Some(k) => {
                s.push('.');
                s.push_str(k);
            }
            None => {
                s.push('[');
                s.push_str(&seg.index.to_string());
                s.push(']');
            }
        }
    }
    s
}
