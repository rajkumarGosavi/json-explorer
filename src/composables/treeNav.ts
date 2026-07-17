// Pure keyboard-navigation reducer over the flattened tree rows. Kept free of
// any DOM/Vue so it's trivially unit-testable; ExplorerView applies the result
// (move the cursor, toggle a container, or load another page).
import type { TreeRowModel } from "./useTree";

export interface NavResult {
  /** rowId to move the cursor to (node or "more" row). */
  select?: string;
  /** nodeId of a container to expand/collapse. */
  toggle?: string;
  /** nodeId whose next page should load (Enter on a "more" row). */
  loadMore?: string;
}

function isContainerRow(r: TreeRowModel): boolean {
  return (
    r.type === "node" &&
    (r.summary.kind === "object" || r.summary.kind === "array")
  );
}

/**
 * Map a key press to a navigation action given the current rows and cursor.
 * `cursorRowId` is a rowId (not nodeId) so "more" rows are addressable too.
 */
export function nextAction(
  rows: TreeRowModel[],
  cursorRowId: string | null,
  key: string,
): NavResult {
  if (rows.length === 0) return {};
  const idx = cursorRowId === null ? -1 : rows.findIndex((r) => r.rowId === cursorRowId);
  const cur = idx >= 0 ? rows[idx] : null;

  switch (key) {
    case "ArrowDown":
      return { select: rows[idx < 0 ? 0 : Math.min(idx + 1, rows.length - 1)].rowId };
    case "ArrowUp":
      return { select: rows[idx <= 0 ? 0 : idx - 1].rowId };
    case "Home":
      return { select: rows[0].rowId };
    case "End":
      return { select: rows[rows.length - 1].rowId };
    case "ArrowRight": {
      if (!cur || !isContainerRow(cur)) return {};
      if (!cur.expanded) return { toggle: cur.nodeId };
      // Already open — step into the first child (the next deeper row).
      const child = rows[idx + 1];
      return child && child.depth > cur.depth ? { select: child.rowId } : {};
    }
    case "ArrowLeft": {
      if (!cur) return {};
      if (isContainerRow(cur) && cur.expanded) return { toggle: cur.nodeId };
      // Jump to the parent: nearest earlier row one level shallower.
      for (let i = idx - 1; i >= 0; i--) {
        if (rows[i].depth === cur.depth - 1) return { select: rows[i].rowId };
      }
      return {};
    }
    case "Enter":
      if (!cur) return {};
      return cur.type === "more" ? { loadMore: cur.nodeId } : { select: cur.rowId };
    default:
      return {};
  }
}
