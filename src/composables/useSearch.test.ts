import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchHit } from "@/types/json";

const mocks = vi.hoisted(() => {
  const listeners: {
    hits?: (batch: SearchHit[]) => void;
    done?: (r: { total: number; truncated: boolean }) => void;
  } = {};
  return {
    listeners,
    searchStart: vi.fn(
      async (_q: string, _regex: boolean, _cs: boolean) => "1",
    ),
    searchCancel: vi.fn(async () => {}),
    onSearchHits: vi.fn(async (cb: (batch: SearchHit[]) => void) => {
      listeners.hits = cb;
      return () => {};
    }),
    onSearchDone: vi.fn(
      async (cb: (r: { total: number; truncated: boolean }) => void) => {
        listeners.done = cb;
        return () => {};
      },
    ),
  };
});

vi.mock("@/api/ipc", () => mocks);

import { useSearch } from "@/composables/useSearch";

function hit(nodeId: string, path: string): SearchHit {
  return { nodeId, path, preview: `"${path}"`, byteOffset: "0", matchLen: 1 };
}

describe("useSearch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does nothing for a blank query", async () => {
    const s = useSearch();
    await s.run();
    expect(mocks.searchStart).not.toHaveBeenCalled();
  });

  it("starts a search and accumulates streamed hits", async () => {
    const s = useSearch();
    s.query.value = "hello";
    await s.run();
    expect(s.searching.value).toBe(true);
    expect(mocks.searchStart).toHaveBeenCalledWith("hello", false, false, "both");

    mocks.listeners.hits!([hit("1", "$.a"), hit("2", "$.b")]);
    expect(s.hits.value).toHaveLength(2);

    mocks.listeners.done!({ total: 2, truncated: false });
    expect(s.searching.value).toBe(false);
    expect(s.total.value).toBe(2);
  });

  it("passes regex and case-sensitive flags through", async () => {
    const s = useSearch();
    s.query.value = "foo";
    s.regex.value = true;
    s.caseSensitive.value = true;
    await s.run();
    expect(mocks.searchStart).toHaveBeenCalledWith("foo", true, true, "both");
  });

  it("passes the keys/values scope through", async () => {
    const s = useSearch();
    s.query.value = "foo";
    s.target.value = "keys";
    await s.run();
    expect(mocks.searchStart).toHaveBeenCalledWith("foo", false, false, "keys");
  });

  it("re-running clears previous hits", async () => {
    const s = useSearch();
    s.query.value = "a";
    await s.run();
    mocks.listeners.hits!([hit("1", "$.a")]);
    expect(s.hits.value).toHaveLength(1);

    s.query.value = "b";
    await s.run();
    expect(s.hits.value).toHaveLength(0);
  });

  it("surfaces searchStart failures via error", async () => {
    mocks.searchStart.mockRejectedValueOnce("no file open");
    const s = useSearch();
    s.query.value = "a";
    await s.run();
    expect(s.error.value).toContain("no file open");
    expect(s.searching.value).toBe(false);
  });

  it("cancel() stops searching and calls searchCancel", async () => {
    const s = useSearch();
    s.query.value = "a";
    await s.run();
    await s.cancel();
    expect(s.searching.value).toBe(false);
    expect(mocks.searchCancel).toHaveBeenCalled();
  });

  it("clearing the query resets hit state", async () => {
    const s = useSearch();
    s.query.value = "a";
    await s.run();
    mocks.listeners.hits!([hit("1", "$.a")]);

    s.query.value = "";
    await s.run();
    expect(s.hits.value).toHaveLength(0);
    expect(mocks.searchStart).toHaveBeenCalledTimes(1);
  });

  it("next/prev cycle through hits with wrap-around", () => {
    const s = useSearch();
    s.hits.value = [hit("a", "$.a"), hit("b", "$.b"), hit("c", "$.c")];
    expect(s.currentIndex.value).toBe(-1);
    expect(s.next()?.nodeId).toBe("a");
    expect(s.currentIndex.value).toBe(0);
    expect(s.next()?.nodeId).toBe("b");
    expect(s.next()?.nodeId).toBe("c");
    expect(s.next()?.nodeId).toBe("a"); // wraps forward
    expect(s.prev()?.nodeId).toBe("c"); // wraps backward
  });

  it("next/prev are no-ops with no hits", () => {
    const s = useSearch();
    expect(s.next()).toBeNull();
    expect(s.prev()).toBeNull();
    expect(s.currentIndex.value).toBe(-1);
  });
});
