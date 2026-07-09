// M0 smoke test — proves the Vitest harness runs. Real store tests land in M3/M4.
import { describe, expect, it } from "vitest";
import type { NodeSummary } from "./json";

describe("test harness", () => {
  it("runs with typed DTOs", () => {
    const node: NodeSummary = {
      id: "0",
      key: null,
      kind: "object",
      preview: "",
      childCount: 2,
    };
    expect(node.kind).toBe("object");
  });
});
