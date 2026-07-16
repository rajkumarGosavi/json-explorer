import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { FileMeta, IndexError, IndexProgress } from "@/types/json";

const mocks = vi.hoisted(() => {
  const listeners: {
    progress?: (p: IndexProgress) => void;
    done?: (m: FileMeta) => void;
    error?: (e: IndexError) => void;
  } = {};
  return {
    listeners,
    openFile: vi.fn(async (_path: string) => {}),
    closeFile: vi.fn(async () => {}),
    onIndexProgress: vi.fn(async (cb: (p: IndexProgress) => void) => {
      listeners.progress = cb;
      return () => {};
    }),
    onIndexDone: vi.fn(async (cb: (m: FileMeta) => void) => {
      listeners.done = cb;
      return () => {};
    }),
    onIndexError: vi.fn(async (cb: (e: IndexError) => void) => {
      listeners.error = cb;
      return () => {};
    }),
  };
});

vi.mock("@/api/ipc", () => mocks);

import { useFileStore } from "@/stores/file";

const META: FileMeta = {
  path: "C:\\data\\big.json",
  sizeBytes: "2147483648",
  containerCount: 12345,
  multiDoc: false,
  indexMillis: 812,
};

describe("file store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("starts idle", () => {
    const store = useFileStore();
    expect(store.phase).toBe("idle");
    expect(store.meta).toBeNull();
  });

  it("open() enters indexing and invokes open_file", async () => {
    const store = useFileStore();
    await store.open("C:\\data\\big.json");
    expect(store.phase).toBe("indexing");
    expect(store.path).toBe("C:\\data\\big.json");
    expect(mocks.openFile).toHaveBeenCalledWith("C:\\data\\big.json");
  });

  it("progress events update byte counters only while indexing", async () => {
    const store = useFileStore();
    await store.open("x.json");
    mocks.listeners.progress!({ bytesDone: "1024", bytesTotal: "4096" });
    expect(store.bytesDone).toBe(1024);
    expect(store.progressPercent).toBe(25);

    mocks.listeners.done!(META);
    mocks.listeners.progress!({ bytesDone: "9999", bytesTotal: "9999" });
    expect(store.bytesDone).toBe(1024); // stale event after done is ignored
  });

  it("done event moves to ready with metadata", async () => {
    const store = useFileStore();
    await store.open("x.json");
    mocks.listeners.done!(META);
    expect(store.phase).toBe("ready");
    expect(store.meta).toEqual(META);
  });

  it("error event moves to error with details", async () => {
    const store = useFileStore();
    await store.open("bad.json");
    mocks.listeners.error!({
      message: "unexpected token",
      byteOffset: "17",
      line: 2,
      col: 5,
    });
    expect(store.phase).toBe("error");
    expect(store.error?.line).toBe(2);
  });

  it("open_file rejection becomes an error phase", async () => {
    mocks.openFile.mockRejectedValueOnce("could not open file: not found");
    const store = useFileStore();
    await store.open("missing.json");
    expect(store.phase).toBe("error");
    expect(store.error?.message).toContain("not found");
  });

  it("close() resets state and invokes close_file", async () => {
    const store = useFileStore();
    await store.open("x.json");
    mocks.listeners.done!(META);
    await store.close();
    expect(store.phase).toBe("idle");
    expect(store.meta).toBeNull();
    expect(mocks.closeFile).toHaveBeenCalled();
  });

  it("attaches event listeners exactly once across opens", async () => {
    const store = useFileStore();
    await store.open("a.json");
    await store.open("b.json");
    expect(mocks.onIndexDone).toHaveBeenCalledTimes(1);
    expect(mocks.onIndexProgress).toHaveBeenCalledTimes(1);
  });
});
