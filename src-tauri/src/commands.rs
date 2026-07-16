use crate::dto::{
    decode_node, encode_node, FileMeta, IndexErrorDto, IndexProgress, NodeSummary, PathSegment,
    SearchHitDto, ValueChunk,
};
use crate::state::{AppState, Session};
use json_index::{
    build_index_with_progress, leaf_value_end, peek_scalar_kind, search_bytes, IndexError,
    JsonKind, NodeRef, RootKind, NO_PARENT,
};
use memmap2::Mmap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

const PROGRESS_STEP_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CHILDREN_PAGE: u32 = 200;
const DEFAULT_VALUE_CAP: u64 = 64 * 1024;
const MAX_VALUE_CAP: u64 = 1024 * 1024;

#[tauri::command]
pub async fn open_file(path: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let file = File::open(&path).map_err(|e| format!("could not open file: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("could not map file: {e}"))?;
    let mmap = Arc::new(mmap);
    let size = mmap.len() as u64;

    // Clear any previously open file immediately; the new session is only
    // published to AppState once indexing finishes (see below), so the
    // frontend must wait for `index://done`/`index://error` before issuing
    // get_root/get_children.
    state.replace_session(None);
    let app_for_thread = app.clone();

    std::thread::spawn(move || {
        // Fresh handle into AppState — safe across threads because AppHandle
        // internally holds an Arc to shared app state.
        let app_state = app_for_thread.state::<AppState>();
        let mut last_reported = 0u64;
        let start = Instant::now();
        let result = build_index_with_progress(&mmap, &mut |pos| {
            if pos.saturating_sub(last_reported) >= PROGRESS_STEP_BYTES {
                last_reported = pos;
                let _ = app_for_thread.emit(
                    "index://progress",
                    IndexProgress {
                        bytes_done: pos.to_string(),
                        bytes_total: size.to_string(),
                    },
                );
            }
        });

        match result {
            Ok(index) => {
                let container_count = index.container_count() as u64;
                let multi_doc = matches!(index.root, RootKind::MultiDoc { .. });
                let session = Session {
                    mmap: mmap.clone(),
                    index: Arc::new(index),
                    path: PathBuf::from(&path),
                    search_generation: Arc::new(AtomicU64::new(0)),
                    search_cancel: Arc::new(AtomicBool::new(false)),
                };
                app_state.replace_session(Some(session));
                let _ = app_for_thread.emit(
                    "index://done",
                    FileMeta {
                        path,
                        size_bytes: size.to_string(),
                        container_count,
                        multi_doc,
                        index_millis: start.elapsed().as_millis() as u64,
                    },
                );
            }
            Err(err) => {
                let dto = match err {
                    IndexError::Syntax {
                        message,
                        byte_offset,
                        line,
                        col,
                    } => IndexErrorDto {
                        message,
                        byte_offset: byte_offset.to_string(),
                        line,
                        col,
                    },
                    IndexError::Empty => IndexErrorDto {
                        message: "file is empty or contains only whitespace".into(),
                        byte_offset: "0".into(),
                        line: 1,
                        col: 1,
                    },
                };
                let _ = app_for_thread.emit("index://error", dto);
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn get_root(state: State<'_, AppState>) -> Result<NodeSummary, String> {
    let guard = state.session.read();
    let session = guard.as_ref().ok_or("no file open")?;
    let buf: &[u8] = &session.mmap;
    let index = &session.index;

    let summary = match &index.root {
        RootKind::Single(NodeRef::Container(id)) => NodeSummary {
            id: encode_node(NodeRef::Container(*id)),
            key: None,
            kind: index.kind_of_container(*id).into(),
            preview: String::new(),
            child_count: index.child_count_of(*id),
        },
        RootKind::Single(_) => {
            // Whole file is a single scalar value (rare, but valid JSON).
            let mut start = 0usize;
            while start < buf.len() && buf[start].is_ascii_whitespace() {
                start += 1;
            }
            let end = leaf_value_end(buf, start as u64) as usize;
            let kind = peek_scalar_kind(buf, start as u64);
            let text = String::from_utf8_lossy(&buf[start..end]);
            NodeSummary {
                id: encode_node(NodeRef::Root),
                key: None,
                kind: kind.into(),
                preview: text.into_owned(),
                child_count: 0,
            }
        }
        RootKind::MultiDoc { doc_refs, .. } => NodeSummary {
            id: encode_node(NodeRef::Root),
            key: None,
            kind: JsonKind::Array.into(),
            preview: String::new(),
            child_count: doc_refs.len() as u64,
        },
    };
    Ok(summary)
}

#[tauri::command]
pub fn get_children(
    node: String,
    offset: u64,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<NodeSummary>, String> {
    let guard = state.session.read();
    let session = guard.as_ref().ok_or("no file open")?;
    let buf: &[u8] = &session.mmap;
    let index = &session.index;
    let node_ref = decode_node(&node)?;
    let limit = limit.min(MAX_CHILDREN_PAGE);

    match node_ref {
        NodeRef::Container(id) => Ok(index
            .children(buf, id, offset, limit)
            .iter()
            .map(|c| NodeSummary::from_raw_child(buf, c))
            .collect()),
        NodeRef::Leaf { .. } => Ok(Vec::new()),
        NodeRef::Root => match &index.root {
            RootKind::MultiDoc { doc_refs, doc_starts } => {
                let end = (offset + limit as u64).min(doc_refs.len() as u64);
                let mut out = Vec::new();
                for i in offset..end {
                    let doc_ref = doc_refs[i as usize];
                    let summary = match doc_ref {
                        NodeRef::Container(cid) => NodeSummary {
                            id: encode_node(doc_ref),
                            key: None,
                            kind: index.kind_of_container(cid).into(),
                            preview: String::new(),
                            child_count: index.child_count_of(cid),
                        },
                        _ => {
                            let start = doc_starts[i as usize];
                            let end = leaf_value_end(buf, start);
                            let kind = peek_scalar_kind(buf, start);
                            let text = String::from_utf8_lossy(
                                &buf[start as usize..end as usize],
                            );
                            NodeSummary {
                                id: encode_node(doc_ref),
                                key: None,
                                kind: kind.into(),
                                preview: text.into_owned(),
                                child_count: 0,
                            }
                        }
                    };
                    out.push(summary);
                }
                Ok(out)
            }
            _ => Ok(Vec::new()),
        },
    }
}

#[tauri::command]
pub fn get_node_value(
    node: String,
    max_bytes: Option<u32>,
    state: State<'_, AppState>,
) -> Result<ValueChunk, String> {
    let guard = state.session.read();
    let session = guard.as_ref().ok_or("no file open")?;
    let buf: &[u8] = &session.mmap;
    let index = &session.index;
    let node_ref = decode_node(&node)?;
    let cap = (max_bytes.unwrap_or(DEFAULT_VALUE_CAP as u32) as u64).min(MAX_VALUE_CAP);

    let (start, end) = match node_ref {
        NodeRef::Container(id) => index.bounds(id),
        // Root-level scalar document in a MultiDoc (NDJSON) root: no
        // enclosing container to look up via children(), child_idx is its
        // position among sibling top-level documents instead.
        NodeRef::Leaf { parent, child_idx } if parent == NO_PARENT => match &index.root {
            RootKind::MultiDoc { doc_starts, .. } => {
                let start = *doc_starts.get(child_idx as usize).ok_or("node not found")?;
                let end = leaf_value_end(buf, start);
                (start, end)
            }
            _ => return Err("node not found".to_string()),
        },
        NodeRef::Leaf { parent, child_idx } => {
            let children = index.children(buf, parent, child_idx as u64, 1);
            let c = children.first().ok_or("node not found")?;
            (c.value_start, c.value_end)
        }
        NodeRef::Root => {
            let mut start = 0usize;
            while start < buf.len() && buf[start].is_ascii_whitespace() {
                start += 1;
            }
            let end = leaf_value_end(buf, start as u64);
            (start as u64, end)
        }
    };

    let total = end - start;
    let truncated = total > cap;
    let slice_end = start + total.min(cap);
    let text = String::from_utf8_lossy(&buf[start as usize..slice_end as usize]).into_owned();
    Ok(ValueChunk {
        text,
        truncated,
        total_bytes: total.to_string(),
    })
}

#[tauri::command]
pub fn get_path(node: String, state: State<'_, AppState>) -> Result<Vec<PathSegment>, String> {
    let guard = state.session.read();
    let session = guard.as_ref().ok_or("no file open")?;
    let buf: &[u8] = &session.mmap;
    let index = &session.index;
    let node_ref = decode_node(&node)?;
    Ok(index.path_of(buf, node_ref).iter().map(PathSegment::from).collect())
}

#[tauri::command]
pub fn search_start(
    query: String,
    regex: bool,
    case_sensitive: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (mmap, index, generation, cancel, search_id) = {
        let guard = state.session.read();
        let session = guard.as_ref().ok_or("no file open")?;
        session.search_cancel.store(false, Ordering::SeqCst);
        let generation = session.search_generation.fetch_add(1, Ordering::SeqCst) + 1;
        (
            session.mmap.clone(),
            session.index.clone(),
            session.search_generation.clone(),
            session.search_cancel.clone(),
            generation,
        )
    };

    std::thread::spawn(move || {
        let mut batch: Vec<SearchHitDto> = Vec::with_capacity(50);
        let flush = |app: &AppHandle, batch: &mut Vec<SearchHitDto>| {
            if !batch.is_empty() {
                let _ = app.emit("search://hits", std::mem::take(batch));
            }
        };

        let (total, truncated) = search_bytes(&mmap, &index, &query, regex, case_sensitive, |hit| {
            if cancel.load(Ordering::SeqCst) || generation.load(Ordering::SeqCst) != search_id {
                return false;
            }
            batch.push(SearchHitDto {
                node_id: encode_node(hit.node),
                path: hit.path,
                preview: hit.preview,
                byte_offset: hit.byte_offset.to_string(),
                match_len: hit.match_len,
            });
            if batch.len() >= 50 {
                flush(&app, &mut batch);
            }
            true
        });
        flush(&app, &mut batch);

        if generation.load(Ordering::SeqCst) == search_id {
            let _ = app.emit(
                "search://done",
                serde_json::json!({ "total": total, "truncated": truncated }),
            );
        }
    });

    Ok(search_id.to_string())
}

#[tauri::command]
pub fn search_cancel(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.session.read();
    if let Some(session) = guard.as_ref() {
        session.search_cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn close_file(state: State<'_, AppState>) -> Result<(), String> {
    state.replace_session(None);
    Ok(())
}
