// Lazy tree over the paginated get_children IPC. Nodes are fetched a page at
// a time and flattened into a row list for virtual scrolling — the tree can
// have millions of children, so nothing is ever loaded eagerly.
import { computed, reactive, ref } from "vue";
import { getChildren, getRoot } from "@/api/ipc";
import type { NodeSummary, PathSegment } from "@/types/json";

export const PAGE_SIZE = 200;

interface TreeEntry {
  summary: NodeSummary;
  depth: number;
  /** Display key: object key, "[i]" for array elements, "$" for the root. */
  label: string;
  expanded: boolean;
  loading: boolean;
  /** Child node ids in document order; null = never fetched. */
  children: string[] | null;
}

export interface TreeRowModel {
  /** Unique virtual-scroller key ("<id>" or "<id>:more"). */
  rowId: string;
  nodeId: string;
  type: "node" | "more";
  depth: number;
  label: string;
  summary: NodeSummary;
  expanded: boolean;
  loading: boolean;
  /** For "more" rows: children not yet loaded. */
  remaining: number;
}

function isExpandable(s: NodeSummary): boolean {
  return s.kind === "object" || s.kind === "array" || s.childCount > 0;
}

export function useTree() {
  const entries = reactive(new Map<string, TreeEntry>());
  const rootId = ref<string | null>(null);
  const error = ref<string | null>(null);

  async function loadRoot(): Promise<void> {
    entries.clear();
    rootId.value = null;
    error.value = null;
    try {
      const root = await getRoot();
      entries.set(root.id, {
        summary: root,
        depth: 0,
        label: "$",
        expanded: false,
        loading: false,
        children: null,
      });
      rootId.value = root.id;
      if (isExpandable(root)) await toggle(root.id);
    } catch (e) {
      error.value = String(e);
    }
  }

  /** Fetch the next page of children for a node (also used as "load more"). */
  async function loadMore(id: string): Promise<void> {
    const entry = entries.get(id);
    if (!entry || entry.loading) return;
    entry.loading = true;
    try {
      const offset = entry.children?.length ?? 0;
      const page = await getChildren(id, offset, PAGE_SIZE);
      const ids: string[] = [];
      page.forEach((child, i) => {
        entries.set(child.id, {
          summary: child,
          depth: entry.depth + 1,
          label: child.key !== null ? child.key : `[${offset + i}]`,
          expanded: false,
          loading: false,
          children: null,
        });
        ids.push(child.id);
      });
      entry.children = [...(entry.children ?? []), ...ids];
    } catch (e) {
      error.value = String(e);
    } finally {
      entry.loading = false;
    }
  }

  async function toggle(id: string): Promise<void> {
    const entry = entries.get(id);
    if (!entry || !isExpandable(entry.summary)) return;
    if (entry.expanded) {
      entry.expanded = false;
      return;
    }
    entry.expanded = true;
    if (entry.children === null) await loadMore(id);
  }

  /** Resolve one path segment to a child id under `parentId`, paging the
   * parent's children as needed (by key for objects, by index for arrays).
   * Returns null if the child doesn't exist. */
  async function findChild(
    parentId: string,
    seg: PathSegment,
  ): Promise<string | null> {
    const parent = entries.get(parentId);
    if (!parent) return null;
    if (seg.key !== null) {
      for (;;) {
        const children = parent.children ?? [];
        for (const cid of children) {
          if (entries.get(cid)?.summary.key === seg.key) return cid;
        }
        const before = children.length;
        if (before >= parent.summary.childCount) return null;
        await loadMore(parentId);
        if ((parent.children?.length ?? 0) === before) return null; // no progress
      }
    }
    const target = seg.index;
    for (;;) {
      const len = parent.children?.length ?? 0;
      if (len > target || len >= parent.summary.childCount) break;
      await loadMore(parentId);
      if ((parent.children?.length ?? 0) === len) break; // no progress
    }
    return parent.children?.[target] ?? null;
  }

  /** Expand every ancestor along `segs` from the root and return the target
   * node id (or null if the path doesn't resolve). Makes the node visible in
   * `rows` so callers can select and scroll to it. */
  async function expandToPath(segs: PathSegment[]): Promise<string | null> {
    let currentId = rootId.value;
    if (currentId === null) return null;
    for (const seg of segs) {
      const entry = entries.get(currentId);
      if (!entry || !isExpandable(entry.summary)) return null;
      if (!entry.expanded) await toggle(currentId);
      const childId = await findChild(currentId, seg);
      if (childId === null) return null;
      currentId = childId;
    }
    return currentId;
  }

  const rows = computed<TreeRowModel[]>(() => {
    const out: TreeRowModel[] = [];
    const rid = rootId.value;
    if (rid === null) return out;
    const visit = (id: string) => {
      const e = entries.get(id);
      if (!e) return;
      out.push({
        rowId: id,
        nodeId: id,
        type: "node",
        depth: e.depth,
        label: e.label,
        summary: e.summary,
        expanded: e.expanded,
        loading: e.loading,
        remaining: 0,
      });
      if (e.expanded && e.children) {
        for (const c of e.children) visit(c);
        const remaining = e.summary.childCount - e.children.length;
        if (remaining > 0) {
          out.push({
            rowId: `${id}:more`,
            nodeId: id,
            type: "more",
            depth: e.depth + 1,
            label: "",
            summary: e.summary,
            expanded: false,
            loading: e.loading,
            remaining,
          });
        }
      }
    };
    visit(rid);
    return out;
  });

  return { rows, error, loadRoot, toggle, loadMore, expandToPath };
}
