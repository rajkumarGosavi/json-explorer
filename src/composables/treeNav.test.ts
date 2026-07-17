import { describe, expect, it } from "vitest";
import type { JsonKind, NodeSummary } from "@/types/json";
import type { TreeRowModel } from "@/composables/useTree";
import { nextAction } from "@/composables/treeNav";

function summary(kind: JsonKind, childCount = 0): NodeSummary {
  return { id: "x", key: null, kind, preview: "", childCount };
}

function row(
  rowId: string,
  kind: JsonKind | "more",
  depth: number,
  expanded = false,
  remaining = 0,
): TreeRowModel {
  const type = kind === "more" ? "more" : "node";
  return {
    rowId,
    nodeId: rowId.replace(/:more$/, ""),
    type,
    depth,
    label: rowId,
    summary: summary(kind === "more" ? "array" : kind, kind === "more" ? 10 : 0),
    expanded,
    loading: false,
    remaining,
  };
}

// Fixture (expanded tree):
//   1  object  depth0 expanded
//   10 object  depth1 collapsed
//   2  array   depth1 expanded
//   i0 number  depth2
//   2:more     depth2 (5 remaining)
const ROWS: TreeRowModel[] = [
  row("1", "object", 0, true),
  row("10", "object", 1, false),
  row("2", "array", 1, true),
  row("i0", "number", 2),
  row("2:more", "more", 2, false, 5),
];

describe("nextAction", () => {
  it("moves the cursor down and up by one row", () => {
    expect(nextAction(ROWS, "1", "ArrowDown")).toEqual({ select: "10" });
    expect(nextAction(ROWS, "10", "ArrowUp")).toEqual({ select: "1" });
  });

  it("clamps at the ends", () => {
    expect(nextAction(ROWS, "2:more", "ArrowDown")).toEqual({ select: "2:more" });
    expect(nextAction(ROWS, "1", "ArrowUp")).toEqual({ select: "1" });
  });

  it("jumps to first/last with Home/End", () => {
    expect(nextAction(ROWS, "2", "Home")).toEqual({ select: "1" });
    expect(nextAction(ROWS, "2", "End")).toEqual({ select: "2:more" });
  });

  it("ArrowRight expands a collapsed container", () => {
    expect(nextAction(ROWS, "10", "ArrowRight")).toEqual({ toggle: "10" });
  });

  it("ArrowRight steps into the first child when already expanded", () => {
    expect(nextAction(ROWS, "2", "ArrowRight")).toEqual({ select: "i0" });
  });

  it("ArrowRight does nothing on a leaf", () => {
    expect(nextAction(ROWS, "i0", "ArrowRight")).toEqual({});
  });

  it("ArrowLeft collapses an expanded container", () => {
    expect(nextAction(ROWS, "2", "ArrowLeft")).toEqual({ toggle: "2" });
  });

  it("ArrowLeft jumps to the parent from a leaf or collapsed node", () => {
    expect(nextAction(ROWS, "i0", "ArrowLeft")).toEqual({ select: "2" });
    expect(nextAction(ROWS, "10", "ArrowLeft")).toEqual({ select: "1" });
  });

  it("Enter loads more on a 'more' row and selects otherwise", () => {
    expect(nextAction(ROWS, "2:more", "Enter")).toEqual({ loadMore: "2" });
    expect(nextAction(ROWS, "10", "Enter")).toEqual({ select: "10" });
  });

  it("ignores unknown keys and empty rows", () => {
    expect(nextAction(ROWS, "1", "x")).toEqual({});
    expect(nextAction([], null, "ArrowDown")).toEqual({});
  });
});
