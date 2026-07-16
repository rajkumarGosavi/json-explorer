//! Structural indexer for huge JSON files.
//!
//! Design: the file is memory-mapped by the caller; this crate scans the raw
//! bytes once, records only containers (objects/arrays) in flat parallel
//! arrays plus child checkpoints, and materializes node data lazily from byte
//! ranges on demand. It never builds a DOM.

pub mod error;
pub mod index;
mod raw;
pub mod scanner;
pub mod search;

pub use error::IndexError;
pub use index::{
    leaf_value_end, peek_scalar_kind, JsonKind, NodeRef, PathSegment, RawChild, RootKind,
    StructuralIndex, NO_PARENT,
};
pub use scanner::{build_index, build_index_with_progress};
pub use search::{search_bytes, SearchHit};

#[cfg(test)]
mod tests;
