// DTOs mirrored from src-tauri/src/dto.rs.
// All u64 ids/offsets cross IPC as strings — JS numbers lose precision past 2^53
// and packed node handles use bit 63.

export type JsonKind =
  | "object"
  | "array"
  | "string"
  | "number"
  | "bool"
  | "null";

export interface NodeSummary {
  id: string;
  key: string | null;
  kind: JsonKind;
  /** Truncated raw text for leaves; empty for containers. */
  preview: string;
  /** 0 for leaves. */
  childCount: number;
}

export interface PathSegment {
  key: string | null;
  index: number;
  nodeId: string;
}

export interface SearchHit {
  nodeId: string;
  /** JSONPath-style, e.g. $.a.b[3] */
  path: string;
  preview: string;
  byteOffset: string;
  matchLen: number;
}

export interface FileMeta {
  path: string;
  sizeBytes: string;
  containerCount: number;
  multiDoc: boolean;
  indexMillis: number;
}

export interface ValueChunk {
  text: string;
  truncated: boolean;
  totalBytes: string;
}

export interface IndexProgress {
  bytesDone: string;
  bytesTotal: string;
}

export interface IndexError {
  message: string;
  byteOffset: string;
  line: number;
  col: number;
}
