// Streaming search over the indexed file. Hits arrive in batches over
// search://hits as the backend walks the index; search://done reports the
// final total. A new search_start call bumps the backend's generation
// counter, which silently retires any in-flight search — see search_start
// in commands.rs — so callers don't need to cancel before re-querying.
import { onScopeDispose, ref } from "vue";
import { onSearchDone, onSearchHits, searchCancel, searchStart } from "@/api/ipc";
import type { SearchHit } from "@/types/json";

export function useSearch() {
  const query = ref("");
  const regex = ref(false);
  const caseSensitive = ref(false);
  const hits = ref<SearchHit[]>([]);
  const searching = ref(false);
  const total = ref(0);
  const truncated = ref(false);
  const error = ref<string | null>(null);

  let unlistenHits: (() => void) | null = null;
  let unlistenDone: (() => void) | null = null;

  async function ensureListeners(): Promise<void> {
    if (unlistenHits) return;
    unlistenHits = await onSearchHits((batch) => {
      hits.value.push(...batch);
    });
    unlistenDone = await onSearchDone((result) => {
      searching.value = false;
      total.value = result.total;
      truncated.value = result.truncated;
    });
  }

  function clear(): void {
    hits.value = [];
    total.value = 0;
    truncated.value = false;
    searching.value = false;
    error.value = null;
  }

  async function run(): Promise<void> {
    const q = query.value.trim();
    if (!q) {
      clear();
      return;
    }
    await ensureListeners();
    hits.value = [];
    total.value = 0;
    truncated.value = false;
    error.value = null;
    searching.value = true;
    try {
      await searchStart(q, regex.value, caseSensitive.value);
    } catch (e) {
      searching.value = false;
      error.value = String(e);
    }
  }

  async function cancel(): Promise<void> {
    if (!searching.value) return;
    searching.value = false;
    await searchCancel();
  }

  onScopeDispose(() => {
    unlistenHits?.();
    unlistenDone?.();
  });

  return {
    query,
    regex,
    caseSensitive,
    hits,
    searching,
    total,
    truncated,
    error,
    run,
    clear,
    cancel,
  };
}
