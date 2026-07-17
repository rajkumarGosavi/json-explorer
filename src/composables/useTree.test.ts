import { beforeEach, describe, expect, it, vi } from "vitest";
import type { JsonKind, NodeSummary } from "@/types/json";

const mocks = vi.hoisted(() => ({
  getRoot: vi.fn<() => Promise<NodeSummary>>(),
  getChildren: vi.fn<
    (node: string, offset: number, limit: number) => Promise<NodeSummary[]>
  >(),
}));

vi.mock("@/api/ipc", () => mocks);

import { PAGE_SIZE, useTree } from "@/composables/useTree";

function node(
  id: string,
  kind: JsonKind,
  key: string | null,
  childCount = 0,
  preview = "",
): NodeSummary {
  return { id, key, kind, preview, childCount };
}

// Fixture: root object { name: "x", items: [ ...450 numbers ] }
const ROOT = node("1", "object", null, 2);
const NAME = node("10", "string", "name", 0, '"x"');
const ITEMS = node("2", "array", "items", 450);
const ITEM = (i: number) => node(`item-${i}`, "number", null, 0, String(i));

function installFixture() {
  mocks.getRoot.mockResolvedValue(ROOT);
  mocks.getChildren.mockImplementation(async (id, offset, limit) => {
    if (id === "1") return [NAME, ITEMS].slice(offset, offset + limit);
    if (id === "2") {
      const end = Math.min(offset + limit, 450);
      return Array.from({ length: end - offset }, (_, i) => ITEM(offset + i));
    }
    return [];
  });
}

describe("useTree", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    installFixture();
  });

  it("loadRoot auto-expands the root and lists its children", async () => {
    const tree = useTree();
    await tree.loadRoot();
    expect(tree.rows.value.map((r) => r.rowId)).toEqual(["1", "10", "2"]);
    expect(tree.rows.value[0].label).toBe("$");
    expect(tree.rows.value[1].label).toBe("name");
    expect(mocks.getChildren).toHaveBeenCalledWith("1", 0, PAGE_SIZE);
  });

  it("expanding a large array loads one page and shows a load-more row", async () => {
    const tree = useTree();
    await tree.loadRoot();
    await tree.toggle("2");

    const rows = tree.rows.value;
    // root + name + items + 200 elements + 1 "more" row
    expect(rows).toHaveLength(3 + PAGE_SIZE + 1);
    const more = rows[rows.length - 1];
    expect(more.type).toBe("more");
    expect(more.remaining).toBe(450 - PAGE_SIZE);
    // array elements have no keys — labels are synthesized indices
    expect(rows[3].label).toBe("[0]");
    expect(rows[3 + PAGE_SIZE - 1].label).toBe(`[${PAGE_SIZE - 1}]`);
  });

  it("loadMore pages until the child count is exhausted", async () => {
    const tree = useTree();
    await tree.loadRoot();
    await tree.toggle("2");
    await tree.loadMore("2");
    await tree.loadMore("2");

    const rows = tree.rows.value;
    expect(rows).toHaveLength(3 + 450); // no "more" row left
    expect(rows.some((r) => r.type === "more")).toBe(false);
    expect(mocks.getChildren).toHaveBeenCalledWith("2", 200, PAGE_SIZE);
    expect(mocks.getChildren).toHaveBeenCalledWith("2", 400, PAGE_SIZE);
  });

  it("collapsing hides descendants without discarding them", async () => {
    const tree = useTree();
    await tree.loadRoot();
    await tree.toggle("2");
    await tree.toggle("2"); // collapse
    expect(tree.rows.value.map((r) => r.rowId)).toEqual(["1", "10", "2"]);

    await tree.toggle("2"); // re-expand — no refetch
    expect(tree.rows.value).toHaveLength(3 + PAGE_SIZE + 1);
    // one fetch for root children + one for the array's first page only
    expect(mocks.getChildren).toHaveBeenCalledTimes(2);
  });

  it("scalar leaves are not expandable", async () => {
    const tree = useTree();
    await tree.loadRoot();
    await tree.toggle("10");
    expect(tree.rows.value).toHaveLength(3);
    expect(mocks.getChildren).toHaveBeenCalledTimes(1);
  });

  it("surfaces getRoot failures via error", async () => {
    mocks.getRoot.mockRejectedValueOnce("no file open");
    const tree = useTree();
    await tree.loadRoot();
    expect(tree.error.value).toContain("no file open");
    expect(tree.rows.value).toHaveLength(0);
  });

  it("expandToPath expands ancestors and pages an array to a deep index", async () => {
    const tree = useTree();
    await tree.loadRoot();
    const id = await tree.expandToPath([
      { key: "items", index: 0 },
      { key: null, index: 250 },
    ]);
    expect(id).toBe("item-250");
    // reaching index 250 requires paging past the first 200-element page
    expect(mocks.getChildren).toHaveBeenCalledWith("2", 200, PAGE_SIZE);
    // the target is now visible in the flattened rows
    expect(tree.rows.value.some((r) => r.rowId === "item-250")).toBe(true);
  });

  it("expandToPath returns null for a missing key", async () => {
    const tree = useTree();
    await tree.loadRoot();
    expect(await tree.expandToPath([{ key: "nope", index: 0 }])).toBeNull();
  });
});
