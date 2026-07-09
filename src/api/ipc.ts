// Thin typed wrappers over Tauri invoke()/listen() — components and stores
// never import @tauri-apps/api directly, so this is the one place that knows
// about command names, event names, and the string-encoded u64 DTOs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  FileMeta,
  IndexError,
  IndexProgress,
  NodeSummary,
  PathSegment,
  SearchHit,
  ValueChunk,
} from "@/types/json";

export function openFile(path: string): Promise<void> {
  return invoke("open_file", { path });
}

export function getRoot(): Promise<NodeSummary> {
  return invoke("get_root");
}

export function getChildren(
  node: string,
  offset: number,
  limit: number,
): Promise<NodeSummary[]> {
  return invoke("get_children", { node, offset, limit });
}

export function getNodeValue(
  node: string,
  maxBytes?: number,
): Promise<ValueChunk> {
  return invoke("get_node_value", { node, maxBytes });
}

export function getPath(node: string): Promise<PathSegment[]> {
  return invoke("get_path", { node });
}

export function searchStart(
  query: string,
  regex: boolean,
  caseSensitive: boolean,
): Promise<string> {
  return invoke("search_start", { query, regex, caseSensitive });
}

export function searchCancel(): Promise<void> {
  return invoke("search_cancel");
}

export function closeFile(): Promise<void> {
  return invoke("close_file");
}

// --- Event listeners -------------------------------------------------------
// Every listener returns its Tauri unlisten fn; callers MUST invoke it on
// close/unmount. Duplicate handlers otherwise stack up across reopen cycles
// (a classic Tauri leak — see feedback_close_resources_after_use).

export function onIndexProgress(
  cb: (p: IndexProgress) => void,
): Promise<UnlistenFn> {
  return listen<IndexProgress>("index://progress", (e) => cb(e.payload));
}

export function onIndexDone(cb: (meta: FileMeta) => void): Promise<UnlistenFn> {
  return listen<FileMeta>("index://done", (e) => cb(e.payload));
}

export function onIndexError(
  cb: (err: IndexError) => void,
): Promise<UnlistenFn> {
  return listen<IndexError>("index://error", (e) => cb(e.payload));
}

export function onSearchHits(
  cb: (hits: SearchHit[]) => void,
): Promise<UnlistenFn> {
  return listen<SearchHit[]>("search://hits", (e) => cb(e.payload));
}

export function onSearchDone(
  cb: (result: { total: number; truncated: boolean }) => void,
): Promise<UnlistenFn> {
  return listen<{ total: number; truncated: boolean }>("search://done", (e) =>
    cb(e.payload),
  );
}
