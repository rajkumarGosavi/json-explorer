//! Containers-only structural index: one record per object/array, in document
//! order. Leaves are enumerated on demand by re-scanning the parent's byte
//! range (accelerated by checkpoints), never materialized up front — this is
//! what keeps a 2 GB file's index from itself costing multiple GB.

use crate::raw::{skip_literal, skip_number, skip_string, skip_ws, unescape};

pub const CHECKPOINT_STRIDE: u64 = 1024;

/// Sentinel `parent` for a `NodeRef::Leaf` describing a root-level scalar
/// document (no enclosing container) — see `RootKind::MultiDoc`/`Single` and
/// `scanner::scan_value`. Deliberately *not* `u32::MAX`: encoding packs
/// `parent` into the top 32 bits of the id, and `u32::MAX` there sets bit 63,
/// which collides with `NodeRef::encode`'s container tag bit and made every
/// such leaf decode back as `Container(child_idx)`. This sentinel's MSB is 0,
/// so it can never collide with that tag.
pub const NO_PARENT: u32 = u32::MAX >> 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

/// How the top-level document(s) look.
#[derive(Debug, Clone)]
pub enum RootKind {
    /// A single top-level JSON value (container id, or a leaf described inline).
    Single(NodeRef),
    /// NDJSON / concatenated JSON: each top-level value becomes a synthetic
    /// child of an implicit root array. `doc_starts[i]` is the byte offset of
    /// document i; `doc_refs[i]` is its NodeRef (container or leaf).
    MultiDoc {
        doc_starts: Vec<u64>,
        doc_refs: Vec<NodeRef>,
    },
}

/// Stable node handle. Containers reference the `StructuralIndex` arrays by
/// id; leaves are described inline (parent container + child position)
/// because there's no per-leaf record to point to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRef {
    Root,
    Container(u32),
    Leaf { parent: u32, child_idx: u32 },
}

impl NodeRef {
    /// Encode as a single u64 for IPC (sent to the frontend as a string).
    /// bit63=1 -> container (low 32 bits = id). bit63=0 -> leaf
    /// (bits 63..32 = parent, bits 31..0 = child_idx). Root is u64::MAX.
    pub fn encode(self) -> u64 {
        match self {
            NodeRef::Root => u64::MAX,
            NodeRef::Container(id) => (1u64 << 63) | id as u64,
            NodeRef::Leaf { parent, child_idx } => {
                ((parent as u64) << 32) | child_idx as u64
            }
        }
    }

    pub fn decode(v: u64) -> Self {
        if v == u64::MAX {
            NodeRef::Root
        } else if v & (1u64 << 63) != 0 {
            NodeRef::Container((v & 0xFFFF_FFFF) as u32)
        } else {
            NodeRef::Leaf {
                parent: (v >> 32) as u32,
                child_idx: (v & 0xFFFF_FFFF) as u32,
            }
        }
    }
}

/// A single direct child, resolved from a scan of its parent's byte range.
#[derive(Debug, Clone)]
pub struct RawChild {
    pub node: NodeRef,
    pub key: Option<String>,
    pub kind: JsonKind,
    pub value_start: u64,
    pub value_end: u64,
    pub child_count: u64, // 0 for leaves
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub key: Option<String>,
    pub index: u64,
}

impl Default for RootKind {
    fn default() -> Self {
        RootKind::Single(NodeRef::Root)
    }
}

/// Sentinel `parent` value meaning "no container parent" (root-level
/// container). Stored in `parents` instead of a signed -1 so the array can be
/// `u32`, not `i64` — halving its footprint. `u32::MAX` can never be a real
/// container id: ids are dense `0..container_count`, and reaching 4 billion
/// containers would require a >4 GB file of nothing but empty containers.
const NO_CONTAINER_PARENT: u32 = u32::MAX;

/// Flat struct-of-arrays: one entry per container (object/array), in the
/// order they were opened during the scan.
///
/// Widths are deliberately minimal: `parents`/`child_count` are `u32` because
/// container ids and child indices are already capped at `u32` everywhere
/// (see `NodeRef`). Checkpoints use a flat CSR layout rather than a
/// `Vec<Vec<u64>>` — a per-container inner `Vec` costs 24 bytes of header
/// each plus its own heap allocation, which on a file with millions of small
/// containers dominated the index (millions of tiny allocations, hundreds of
/// MB). The CSR store is two flat allocations regardless of container count.
#[derive(Debug, Default)]
pub struct StructuralIndex {
    starts: Vec<u64>,       // byte offset of '{' or '['
    is_object: Vec<bool>,
    ends: Vec<u64>,         // byte offset one past matching '}' / ']'
    parents: Vec<u32>,      // container id of parent, NO_CONTAINER_PARENT for root-level
    child_count: Vec<u32>,  // number of direct children
    // CSR checkpoint store (populated by `finalize_checkpoints`): the
    // checkpoints of container `c` are
    // `checkpoint_data[ckpt_index[c]..ckpt_index[c + 1]]`, each the byte
    // offset of every CHECKPOINT_STRIDE-th direct child in child order
    // (first entry = child 0). `ckpt_index` has `container_count + 1` entries.
    checkpoint_data: Vec<u64>,
    ckpt_index: Vec<u32>,
    // Build-time scratch: `(container, child_offset)` for every checkpoint, in
    // scan order (interleaved across containers). Compacted into the CSR
    // arrays above by `finalize_checkpoints`, then freed.
    build_ckpt_owner: Vec<u32>,
    build_ckpt_offset: Vec<u64>,
    pub root: RootKind,
}

impl StructuralIndex {
    pub(crate) fn new_container(
        &mut self,
        start: u64,
        is_object: bool,
        parent: i64,
    ) -> u32 {
        let id = self.starts.len() as u32;
        self.starts.push(start);
        self.is_object.push(is_object);
        self.ends.push(0);
        self.parents
            .push(if parent < 0 { NO_CONTAINER_PARENT } else { parent as u32 });
        self.child_count.push(0);
        id
    }

    pub(crate) fn close_container(&mut self, id: u32, end: u64) {
        self.ends[id as usize] = end;
    }

    pub(crate) fn note_child(&mut self, container: u32, child_start: u64) {
        let idx = self.child_count[container as usize];
        if idx.is_multiple_of(CHECKPOINT_STRIDE as u32) {
            self.build_ckpt_owner.push(container);
            self.build_ckpt_offset.push(child_start);
        }
        self.child_count[container as usize] = idx + 1;
    }

    /// Compact the scan-order checkpoint scratch into the CSR store. Called
    /// once after scanning completes (see `scanner::build_index_with_progress`),
    /// before the index serves any query. A counting sort groups checkpoints
    /// by owning container; within a container, scan order already equals child
    /// order (`note_child` fires in increasing child index), and the forward
    /// scatter below preserves that, so `checkpoint_data[ckpt_index[c] + j]` is
    /// container `c`'s `j`-th checkpoint — identical to the old `checkpoints[c][j]`.
    pub(crate) fn finalize_checkpoints(&mut self) {
        let n = self.starts.len();
        let mut index = vec![0u32; n + 1];
        for &c in &self.build_ckpt_owner {
            index[c as usize + 1] += 1;
        }
        for i in 0..n {
            index[i + 1] += index[i];
        }
        let mut data = vec![0u64; index[n] as usize];
        let mut cursor = index.clone(); // per-container moving write head
        for (k, &c) in self.build_ckpt_owner.iter().enumerate() {
            let w = cursor[c as usize] as usize;
            data[w] = self.build_ckpt_offset[k];
            cursor[c as usize] += 1;
        }
        self.checkpoint_data = data;
        self.ckpt_index = index;
        self.build_ckpt_owner = Vec::new();
        self.build_ckpt_offset = Vec::new();
    }

    /// Checkpoints of `container`, in child order — see `checkpoint_data`.
    fn checkpoints_of(&self, container: u32) -> &[u64] {
        let s = self.ckpt_index[container as usize] as usize;
        let e = self.ckpt_index[container as usize + 1] as usize;
        &self.checkpoint_data[s..e]
    }

    pub fn container_count(&self) -> usize {
        self.starts.len()
    }

    pub fn is_object(&self, id: u32) -> bool {
        self.is_object[id as usize]
    }

    pub fn kind_of_container(&self, id: u32) -> JsonKind {
        if self.is_object(id) {
            JsonKind::Object
        } else {
            JsonKind::Array
        }
    }

    pub fn bounds(&self, id: u32) -> (u64, u64) {
        (self.starts[id as usize], self.ends[id as usize])
    }

    pub fn child_count_of(&self, id: u32) -> u64 {
        self.child_count[id as usize] as u64
    }

    pub fn parent_of(&self, id: u32) -> Option<u32> {
        match self.parents[id as usize] {
            NO_CONTAINER_PARENT => None,
            p => Some(p),
        }
    }

    /// Nearest checkpoint at or before the requested child offset. Returns
    /// (checkpoint_child_index, byte_offset_of_that_child).
    fn nearest_checkpoint(&self, container: u32, offset: u64) -> (u64, u64) {
        let ckpts = self.checkpoints_of(container);
        let ckpt_idx = (offset / CHECKPOINT_STRIDE) as usize;
        let ckpt_idx = ckpt_idx.min(ckpts.len().saturating_sub(1));
        (
            ckpt_idx as u64 * CHECKPOINT_STRIDE,
            ckpts.get(ckpt_idx).copied().unwrap_or(self.starts[container as usize] + 1),
        )
    }

    /// Enumerate up to `limit` direct children of `container`, starting at
    /// child index `offset`. Scans forward from the nearest checkpoint.
    pub fn children(
        &self,
        buf: &[u8],
        container: u32,
        offset: u64,
        limit: u32,
    ) -> Vec<RawChild> {
        let (start_idx, start_byte) = self.nearest_checkpoint(container, offset);
        let is_obj = self.is_object(container);
        let mut pos = start_byte as usize;
        let mut idx = start_idx;
        let mut out = Vec::new();

        while idx < offset {
            if is_obj {
                // Entries here are key-value pairs: skip_entry alone only
                // consumes the key (a quoted string looks like any other
                // value to it), leaving pos stuck on ':' — skip key, ':',
                // then the value, same order as the materializing loop below.
                skip_string(buf, &mut pos);
                skip_ws(buf, &mut pos);
                pos += 1; // ':'
                skip_ws(buf, &mut pos);
            }
            skip_entry(buf, &mut pos, is_obj);
            idx += 1;
            skip_ws(buf, &mut pos);
            if pos < buf.len() && buf[pos] == b',' {
                pos += 1;
                skip_ws(buf, &mut pos);
            }
        }

        while out.len() < limit as usize && idx < self.child_count_of(container) {
            let key = if is_obj {
                let key_start = pos;
                skip_string(buf, &mut pos);
                let k = unescape(&buf[key_start + 1..pos - 1]);
                skip_ws(buf, &mut pos);
                pos += 1; // ':'
                skip_ws(buf, &mut pos);
                Some(k)
            } else {
                None
            };

            let value_start = pos as u64;
            let (node, kind, child_count) = classify_value(self, buf, container, idx, &mut pos);
            let value_end = pos as u64;

            out.push(RawChild {
                node,
                key,
                kind,
                value_start,
                value_end,
                child_count,
            });

            idx += 1;
            skip_ws(buf, &mut pos);
            if pos < buf.len() && buf[pos] == b',' {
                pos += 1;
                skip_ws(buf, &mut pos);
            }
        }

        out
    }

    /// Ancestor chain (root-first) with keys/indices, ending at `node` itself.
    pub fn path_of(&self, buf: &[u8], node: NodeRef) -> Vec<PathSegment> {
        let mut chain: Vec<(u32, u64)> = Vec::new(); // (parent_container, child_index)
        let leaf_parent_idx = match node {
            NodeRef::Root => return Vec::new(),
            // Root-level scalar document (RootKind::Single, or one entry of
            // a RootKind::MultiDoc): no enclosing container, so child_idx is
            // its position among sibling top-level documents, not a real
            // container child index.
            NodeRef::Leaf { parent, child_idx } if parent == NO_PARENT => {
                return vec![PathSegment { key: None, index: child_idx as u64 }];
            }
            NodeRef::Leaf { parent, child_idx } => Some((parent, child_idx as u64)),
            NodeRef::Container(id) => {
                let mut cur = id;
                loop {
                    match self.parent_of(cur) {
                        None => break,
                        Some(p) => {
                            let child_idx = self.child_index_of(buf, p, cur);
                            chain.push((p, child_idx));
                            cur = p;
                        }
                    }
                }
                None
            }
        };
        if let Some((parent, child_idx)) = leaf_parent_idx {
            chain.push((parent, child_idx));
            let mut cur = parent;
            loop {
                match self.parent_of(cur) {
                    None => break,
                    Some(p) => {
                        let idx = self.child_index_of(buf, p, cur);
                        chain.push((p, idx));
                        cur = p;
                    }
                }
            }
        }
        chain.reverse();
        chain
            .into_iter()
            .map(|(parent, idx)| {
                let key = if self.is_object(parent) {
                    self.key_at(buf, parent, idx)
                } else {
                    None
                };
                PathSegment { key, index: idx }
            })
            .collect()
    }

    /// Find the child index of `target` container within `parent`.
    fn child_index_of(&self, buf: &[u8], parent: u32, target: u32) -> u64 {
        let target_start = self.starts[target as usize];
        // Binary-search-ish via checkpoints, then linear scan from there.
        let ckpts = self.checkpoints_of(parent);
        let mut lo = 0usize;
        for (i, &b) in ckpts.iter().enumerate() {
            if b <= target_start {
                lo = i;
            } else {
                break;
            }
        }
        let is_obj = self.is_object(parent);
        let mut pos = ckpts.get(lo).copied().unwrap_or(self.starts[parent as usize] + 1) as usize;
        let mut idx = lo as u64 * CHECKPOINT_STRIDE;
        loop {
            if is_obj {
                skip_string(buf, &mut pos);
                skip_ws(buf, &mut pos);
                pos += 1;
                skip_ws(buf, &mut pos);
            }
            if pos as u64 == target_start {
                return idx;
            }
            skip_entry(buf, &mut pos, false);
            idx += 1;
            skip_ws(buf, &mut pos);
            if pos < buf.len() && buf[pos] == b',' {
                pos += 1;
                skip_ws(buf, &mut pos);
            }
        }
    }

    fn key_at(&self, buf: &[u8], parent: u32, child_idx: u64) -> Option<String> {
        let children = self.children(buf, parent, child_idx, 1);
        children.into_iter().next().and_then(|c| c.key)
    }

    /// Deepest container whose byte range contains `off`, plus the resolved
    /// path down to the exact node at that offset (used to map a search hit
    /// byte offset back to a tree location).
    pub fn node_at_offset(&self, buf: &[u8], off: u64) -> (NodeRef, Vec<PathSegment>) {
        match self.innermost_container_at(off) {
            None => self.root_scalar_at(buf, off),
            Some(container) => {
                // Find which direct child of `container` contains off (leaf or
                // nested container) by scanning with checkpoints.
                let count = self.child_count_of(container);
                let mut lo = 0u64;
                let hi = count;
                // Linear scan in pages of CHECKPOINT_STRIDE — good enough since
                // node_at_offset runs once per search hit, not per frame.
                while lo < hi {
                    let page = self.children(buf, container, lo, CHECKPOINT_STRIDE as u32);
                    if page.is_empty() {
                        break;
                    }
                    let mut found = None;
                    for (i, c) in page.iter().enumerate() {
                        if c.value_start <= off && off < c.value_end {
                            found = Some((lo + i as u64, c.node));
                            break;
                        }
                    }
                    if let Some((idx, node)) = found {
                        let mut path = self.path_of(buf, node);
                        // If node is itself a container spanning off deeper in,
                        // recurse isn't needed: node_at_offset's candidate search
                        // above already picked the innermost container, so this
                        // child IS that container (or off lands exactly here).
                        if path.is_empty() {
                            path.push(PathSegment {
                                key: if self.is_object(container) {
                                    page[(idx - lo) as usize].key.clone()
                                } else {
                                    None
                                },
                                index: idx,
                            });
                        }
                        return (node, path);
                    }
                    lo += page.len() as u64;
                }
                (NodeRef::Container(container), self.path_of(buf, NodeRef::Container(container)))
            }
        }
    }

    /// Innermost container whose `[start, end)` range contains `off`, or
    /// `None` if `off` isn't inside any container (e.g. a root-level scalar).
    ///
    /// `starts` is ascending — containers are assigned ids in left-to-right
    /// scan order, so a nested container always starts after its parent and
    /// before any sibling. A binary search for the last container starting
    /// at or before `off` is therefore O(log n); since JSON containers nest
    /// without partial overlap, if *any* container contains `off` it must be
    /// that one or one of its ancestors (a container that starts later than
    /// an ancestor but at/before `off` can only be nested inside it), so the
    /// walk up `parent_of` is bounded by nesting depth, not container count.
    /// This replaces an old `O(container_count)` linear scan that made
    /// search cost `O(hits * container_count)` on large files.
    fn innermost_container_at(&self, off: u64) -> Option<u32> {
        let mut lo = 0usize;
        let mut hi = self.starts.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.starts[mid] <= off {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return None; // no container starts at or before off
        }
        let mut id = (lo - 1) as u32;
        loop {
            let (s, e) = self.bounds(id);
            if s <= off && off < e {
                return Some(id);
            }
            id = self.parent_of(id)?;
        }
    }

    /// Fallback for offsets that aren't inside any container: either the
    /// whole file is a single top-level scalar (`RootKind::Single`), or
    /// `off` lands inside one of several NDJSON/concatenated-JSON top-level
    /// scalar documents (`RootKind::MultiDoc`). `doc_starts` is ascending
    /// (documents are scanned left to right), so binary search finds which
    /// doc `off` falls in, in O(log docs).
    fn root_scalar_at(&self, buf: &[u8], off: u64) -> (NodeRef, Vec<PathSegment>) {
        let RootKind::MultiDoc { doc_starts, doc_refs } = &self.root else {
            return (NodeRef::Root, Vec::new());
        };
        let idx = match doc_starts.binary_search(&off) {
            Ok(i) => i,
            Err(0) => return (NodeRef::Root, Vec::new()), // before the first doc
            Err(i) => i - 1,
        };
        let doc_ref = doc_refs[idx];
        let end = match doc_ref {
            // Containers are already handled by innermost_container_at, but
            // stay correct defensively if one ever reaches here.
            NodeRef::Container(id) => self.bounds(id).1,
            _ => leaf_value_end(buf, doc_starts[idx]),
        };
        if off < end {
            (doc_ref, vec![PathSegment { key: None, index: idx as u64 }])
        } else {
            // off is in whitespace between documents.
            (NodeRef::Root, Vec::new())
        }
    }
}

/// Whether a byte offset falls inside an object entry's key string or inside a
/// value. Used to scope search to keys vs values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetRole {
    Key,
    Value,
}

impl StructuralIndex {
    /// Classify a byte offset as landing in an object key or in a value.
    /// Array elements and root-level scalars are always `Value` (no keys).
    /// Structural bytes (`:`, whitespace) default to `Value` — they rarely
    /// match a user query and never need to count as a key hit.
    ///
    /// Runs once per search hit (not per frame), and starts scanning from the
    /// nearest checkpoint at or before `off`, so cost is bounded by
    /// `CHECKPOINT_STRIDE` entries regardless of container size.
    pub fn classify_offset(&self, buf: &[u8], off: u64) -> OffsetRole {
        let Some(container) = self.innermost_container_at(off) else {
            return OffsetRole::Value;
        };
        if !self.is_object(container) {
            return OffsetRole::Value;
        }
        let mut pos = self
            .checkpoints_of(container)
            .iter()
            .copied()
            .take_while(|&b| b <= off)
            .last()
            .unwrap_or(self.starts[container as usize] + 1) as usize;
        let end = self.bounds(container).1 as usize;
        let off = off as usize;
        skip_ws(buf, &mut pos);
        while pos < end {
            if buf.get(pos) != Some(&b'"') {
                break;
            }
            let key_start = pos;
            skip_string(buf, &mut pos);
            if off >= key_start && off < pos {
                return OffsetRole::Key;
            }
            skip_ws(buf, &mut pos);
            if buf.get(pos) == Some(&b':') {
                pos += 1;
            }
            skip_ws(buf, &mut pos);
            let value_start = pos;
            if off < value_start {
                return OffsetRole::Value; // in the `:`/whitespace gap
            }
            skip_entry(buf, &mut pos, false);
            if off < pos {
                return OffsetRole::Value; // inside this entry's value
            }
            skip_ws(buf, &mut pos);
            if buf.get(pos) == Some(&b',') {
                pos += 1;
                skip_ws(buf, &mut pos);
            } else {
                break;
            }
        }
        OffsetRole::Value
    }
}

/// End offset of the leaf value starting at `start` (a non-container node
/// has no stored bounds — its range is recovered on demand, same principle
/// as `children()` for object/array entries).
pub fn leaf_value_end(buf: &[u8], start: u64) -> u64 {
    let mut pos = start as usize;
    skip_entry(buf, &mut pos, false);
    pos as u64
}

/// Classify a scalar (non-container) value from its first byte. Only valid
/// for offsets known to point at a leaf, e.g. a `NodeRef::Leaf` or a
/// `RootKind::MultiDoc` document that isn't itself a container.
pub fn peek_scalar_kind(buf: &[u8], pos: u64) -> JsonKind {
    match buf[pos as usize] {
        b'"' => JsonKind::String,
        b't' | b'f' => JsonKind::Bool,
        b'n' => JsonKind::Null,
        _ => JsonKind::Number,
    }
}

/// Advance `pos` past one complete value (object/array/string/number/literal)
/// without recording anything — used to skip over already-counted entries.
fn skip_entry(buf: &[u8], pos: &mut usize, _in_object: bool) {
    match buf[*pos] {
        b'"' => {
            skip_string(buf, pos);
        }
        b'{' | b'[' => {
            skip_container(buf, pos);
        }
        b't' => {
            skip_literal(buf, pos, b"true");
        }
        b'f' => {
            skip_literal(buf, pos, b"false");
        }
        b'n' => {
            skip_literal(buf, pos, b"null");
        }
        _ => {
            skip_number(buf, pos);
        }
    }
}

fn skip_container(buf: &[u8], pos: &mut usize) {
    let is_obj = buf[*pos] == b'{';
    let close = if is_obj { b'}' } else { b']' };
    *pos += 1;
    skip_ws(buf, pos);
    if *pos < buf.len() && buf[*pos] == close {
        *pos += 1;
        return;
    }
    loop {
        if is_obj {
            skip_string(buf, pos);
            skip_ws(buf, pos);
            *pos += 1; // ':'
            skip_ws(buf, pos);
        }
        skip_entry(buf, pos, false);
        skip_ws(buf, pos);
        if *pos < buf.len() && buf[*pos] == b',' {
            *pos += 1;
            skip_ws(buf, pos);
        } else {
            break;
        }
    }
    if *pos < buf.len() && buf[*pos] == close {
        *pos += 1;
    }
}

/// Classify the value at `*pos` (already positioned at its first byte),
/// advancing `pos` past it. Containers are *not* re-scanned here — their
/// bounds/child_count come straight from the index built during the initial
/// pass, so this is O(1) for nested containers.
fn classify_value(
    index: &StructuralIndex,
    buf: &[u8],
    _parent: u32,
    _child_idx: u64,
    pos: &mut usize,
) -> (NodeRef, JsonKind, u64) {
    match buf[*pos] {
        b'{' | b'[' => {
            let start = *pos as u64;
            // The container was already registered during the build pass;
            // find it by start offset via linear probe over a small window is
            // wasteful, so instead containers store their own id inline —
            // handled by build_id_by_start, populated at scan time.
            let id = index
                .container_id_at(start)
                .expect("container must have been registered during scan");
            *pos = index.bounds(id).1 as usize;
            (
                NodeRef::Container(id),
                index.kind_of_container(id),
                index.child_count_of(id),
            )
        }
        b'"' => {
            skip_string(buf, pos);
            (NodeRef::Leaf { parent: _parent, child_idx: _child_idx as u32 }, JsonKind::String, 0)
        }
        b't' | b'f' => {
            let is_true = buf[*pos] == b't';
            skip_literal(buf, pos, if is_true { b"true" } else { b"false" });
            (NodeRef::Leaf { parent: _parent, child_idx: _child_idx as u32 }, JsonKind::Bool, 0)
        }
        b'n' => {
            skip_literal(buf, pos, b"null");
            (NodeRef::Leaf { parent: _parent, child_idx: _child_idx as u32 }, JsonKind::Null, 0)
        }
        _ => {
            skip_number(buf, pos);
            (NodeRef::Leaf { parent: _parent, child_idx: _child_idx as u32 }, JsonKind::Number, 0)
        }
    }
}

impl StructuralIndex {
    /// O(log n) lookup: container id whose start == the given byte offset.
    /// Backed by a sorted parallel index built once after scanning.
    pub(crate) fn container_id_at(&self, start: u64) -> Option<u32> {
        // starts[] is in scan (document) order, which for a well-formed
        // document is also ascending byte order — safe to binary search.
        match self.starts.binary_search(&start) {
            Ok(i) => Some(i as u32),
            Err(_) => None,
        }
    }
}
