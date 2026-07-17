// Open-file lifecycle: idle → indexing → ready | error. Indexing runs on a
// backend thread; completion arrives via index:// events, so this store owns
// the (app-lifetime) event listeners and is the single source of truth for
// what file is open.
import { defineStore } from "pinia";
import * as ipc from "@/api/ipc";
import type { FileMeta, IndexError } from "@/types/json";

export type FilePhase = "idle" | "indexing" | "ready" | "error";

export const useFileStore = defineStore("file", {
  state: () => ({
    phase: "idle" as FilePhase,
    /** Path passed to the pending/last open_file call. */
    path: null as string | null,
    bytesDone: 0,
    bytesTotal: 0,
    meta: null as FileMeta | null,
    error: null as IndexError | null,
    listenersReady: false,
  }),
  getters: {
    progressPercent(state): number {
      if (state.bytesTotal <= 0) return 0;
      return Math.min(100, Math.round((state.bytesDone / state.bytesTotal) * 100));
    },
  },
  actions: {
    /**
     * Attach index event listeners once for the app lifetime. Deliberately
     * never detached — a store singleton attaching once is how we avoid the
     * duplicate-handler leak the ipc.ts comment warns about.
     */
    async init() {
      if (this.listenersReady) return;
      this.listenersReady = true;
      await Promise.all([
        ipc.onIndexProgress((p) => {
          if (this.phase !== "indexing") return;
          this.bytesDone = Number(p.bytesDone);
          this.bytesTotal = Number(p.bytesTotal);
        }),
        ipc.onIndexDone((meta) => {
          this.phase = "ready";
          this.meta = meta;
        }),
        ipc.onIndexError((err) => {
          this.phase = "error";
          this.error = err;
        }),
      ]);
    },

    async open(path: string) {
      await this.init();
      this.$patch({
        phase: "indexing",
        path,
        meta: null,
        error: null,
        bytesDone: 0,
        bytesTotal: 0,
      });
      try {
        await ipc.openFile(path);
      } catch (e) {
        // open_file itself failed (file missing / unreadable) — no
        // index://error event will arrive in this case.
        this.phase = "error";
        this.error = { message: String(e), byteOffset: "0", line: 0, col: 0 };
      }
    },

    /** Index JSON pasted directly, sharing the same index:// event lifecycle. */
    async openText(text: string) {
      await this.init();
      this.$patch({
        phase: "indexing",
        path: "(pasted JSON)",
        meta: null,
        error: null,
        bytesDone: 0,
        bytesTotal: 0,
      });
      try {
        await ipc.openText(text);
      } catch (e) {
        this.phase = "error";
        this.error = { message: String(e), byteOffset: "0", line: 0, col: 0 };
      }
    },

    async close() {
      await ipc.closeFile();
      this.$patch({
        phase: "idle",
        path: null,
        meta: null,
        error: null,
        bytesDone: 0,
        bytesTotal: 0,
      });
    },
  },
});
