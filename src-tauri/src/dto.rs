//! IPC DTOs. u64 ids/offsets always cross as strings — JS numbers lose
//! precision past 2^53 and packed node handles use bit 63 (see
//! json_index::NodeRef::encode).

use json_index::{JsonKind as CoreKind, NodeRef, PathSegment as CorePathSegment, RawChild};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

impl From<CoreKind> for JsonKind {
    fn from(k: CoreKind) -> Self {
        match k {
            CoreKind::Object => JsonKind::Object,
            CoreKind::Array => JsonKind::Array,
            CoreKind::String => JsonKind::String,
            CoreKind::Number => JsonKind::Number,
            CoreKind::Bool => JsonKind::Bool,
            CoreKind::Null => JsonKind::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSummary {
    pub id: String,
    pub key: Option<String>,
    pub kind: JsonKind,
    /// Truncated raw text for leaves; empty for containers.
    pub preview: String,
    /// 0 for leaves.
    pub child_count: u64,
}

const PREVIEW_MAX: usize = 160;

impl NodeSummary {
    pub fn from_raw_child(buf: &[u8], c: &RawChild) -> Self {
        let preview = if c.child_count > 0 || matches!(c.kind, CoreKind::Object | CoreKind::Array) {
            String::new()
        } else {
            let slice = &buf[c.value_start as usize..c.value_end as usize];
            let text = String::from_utf8_lossy(slice);
            if text.len() > PREVIEW_MAX {
                format!("{}…", &text[..PREVIEW_MAX])
            } else {
                text.into_owned()
            }
        };
        NodeSummary {
            id: c.node.encode().to_string(),
            key: c.key.clone(),
            kind: c.kind.into(),
            preview,
            child_count: c.child_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSegment {
    pub key: Option<String>,
    pub index: u64,
}

impl From<&CorePathSegment> for PathSegment {
    fn from(seg: &CorePathSegment) -> Self {
        PathSegment {
            key: seg.key.clone(),
            index: seg.index,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub node_id: String,
    pub path: String,
    pub preview: String,
    pub byte_offset: String,
    pub match_len: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub path: String,
    pub size_bytes: String,
    pub container_count: u64,
    pub multi_doc: bool,
    pub index_millis: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueChunk {
    pub text: String,
    pub truncated: bool,
    pub total_bytes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub bytes_done: String,
    pub bytes_total: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexErrorDto {
    pub message: String,
    pub byte_offset: String,
    pub line: u64,
    pub col: u64,
}

pub fn encode_node(node: NodeRef) -> String {
    node.encode().to_string()
}

pub fn decode_node(s: &str) -> Result<NodeRef, String> {
    let v: u64 = s.parse().map_err(|_| "invalid node id".to_string())?;
    Ok(NodeRef::decode(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_round_trips_through_string_ipc_boundary() {
        for node in [
            NodeRef::Root,
            NodeRef::Container(0),
            NodeRef::Container(u32::MAX - 1),
            NodeRef::Leaf { parent: 0, child_idx: 0 },
            NodeRef::Leaf { parent: 42, child_idx: 1_000_000 },
        ] {
            let s = encode_node(node);
            let back = decode_node(&s).unwrap();
            assert_eq!(node, back, "round trip failed for {s}");
        }
    }

    #[test]
    fn decode_node_rejects_garbage() {
        assert!(decode_node("not-a-number").is_err());
    }
}
