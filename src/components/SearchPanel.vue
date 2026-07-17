<script setup lang="ts">
import { watch } from "vue";
import { useSearch } from "@/composables/useSearch";
import type { SearchHit } from "@/types/json";

const emit = defineEmits<{ select: [nodeId: string]; close: [] }>();

// Must match .result's fixed height in the <style> block below — VirtualScroller
// positions rows by this size instead of measuring the DOM, so a mismatch here
// causes visible gaps/overlap once results scroll (harmless for the few dozen
// hits a typical query returns, but real for the 10,000-hit truncated case).
const ROW_HEIGHT = 44;

const {
  query,
  regex,
  caseSensitive,
  target,
  hits,
  searching,
  total,
  truncated,
  error,
  currentIndex,
  next,
  prev,
  run,
  cancel,
} = useSearch();

let debounceHandle: ReturnType<typeof setTimeout> | undefined;
watch([query, regex, caseSensitive, target], () => {
  clearTimeout(debounceHandle);
  debounceHandle = setTimeout(() => void run(), 250);
});

function onSelect(hit: SearchHit) {
  currentIndex.value = hits.value.indexOf(hit);
  emit("select", hit.nodeId);
}

function goNext() {
  const hit = next();
  if (hit) emit("select", hit.nodeId);
}

function goPrev() {
  const hit = prev();
  if (hit) emit("select", hit.nodeId);
}

function onClose() {
  void cancel();
  emit("close");
}
</script>

<template>
  <div class="search-panel">
    <div class="search-bar">
      <IconField class="query-field">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          placeholder="Search values and keys…"
          size="small"
          autofocus
          class="query-input"
        />
      </IconField>
      <Button
        icon="pi pi-code"
        size="small"
        text
        :severity="regex ? 'primary' : 'secondary'"
        title="Regex"
        @click="regex = !regex"
      />
      <Button
        label="Aa"
        size="small"
        text
        :severity="caseSensitive ? 'primary' : 'secondary'"
        title="Case sensitive"
        @click="caseSensitive = !caseSensitive"
      />
      <div class="scope">
        <Button
          v-for="opt in (['both', 'keys', 'values'] as const)"
          :key="opt"
          :label="opt === 'both' ? 'All' : opt === 'keys' ? 'Keys' : 'Values'"
          size="small"
          text
          :severity="target === opt ? 'primary' : 'secondary'"
          :title="`Search ${opt === 'both' ? 'keys and values' : opt}`"
          @click="target = opt"
        />
      </div>
      <Button
        icon="pi pi-times"
        size="small"
        text
        severity="secondary"
        title="Close search"
        @click="onClose"
      />
    </div>

    <div class="status mono">
      <i v-if="searching" class="pi pi-spinner pi-spin" />
      <span v-if="error" class="error">{{ error }}</span>
      <template v-else-if="query.trim() && !searching">
        <span>
          {{ total.toLocaleString() }} match<template v-if="total !== 1">es</template>
          <template v-if="truncated"> (truncated)</template>
        </span>
        <span v-if="hits.length" class="hit-nav">
          <Button
            icon="pi pi-chevron-up"
            size="small"
            text
            severity="secondary"
            title="Previous match"
            @click="goPrev"
          />
          <span class="hit-pos">{{ currentIndex >= 0 ? currentIndex + 1 : "–" }} / {{ hits.length.toLocaleString() }}</span>
          <Button
            icon="pi pi-chevron-down"
            size="small"
            text
            severity="secondary"
            title="Next match"
            @click="goNext"
          />
        </span>
      </template>
    </div>

    <div class="results">
      <VirtualScroller
        v-if="hits.length"
        :items="hits"
        :itemSize="ROW_HEIGHT"
        class="results-scroller"
      >
        <template #item="{ item }: { item: SearchHit }">
          <div class="result mono" @click="onSelect(item)">
            <div class="path">{{ item.path }}</div>
            <div class="preview">{{ item.preview }}</div>
          </div>
        </template>
      </VirtualScroller>
      <div v-if="!searching && query.trim() && hits.length === 0" class="empty">
        No matches.
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.5rem;
  border-bottom: 1px solid var(--p-content-border-color);
}

.query-field {
  flex: 1;
  min-width: 0;
}

.query-input {
  width: 100%;
}

.status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.4rem;
  padding: 0.1rem 0.6rem;
  font-size: 0.75rem;
  color: var(--p-text-muted-color);
  min-height: 1.6rem;
}

.scope {
  display: flex;
  align-items: center;
}

.hit-nav {
  display: flex;
  align-items: center;
  gap: 0.15rem;
}

.hit-pos {
  min-width: 3.5rem;
  text-align: center;
}

.status .error {
  color: var(--p-red-600, #d33);
}

.results {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.results-scroller {
  flex: 1;
  width: 100%;
}

.result {
  height: 44px; /* keep in sync with ROW_HEIGHT in <script setup> */
  box-sizing: border-box;
  padding: 0.35rem 0.6rem;
  display: flex;
  flex-direction: column;
  justify-content: center;
  cursor: pointer;
  border-bottom: 1px solid var(--p-content-border-color);
}

.result:hover {
  background: var(--p-content-hover-background);
}

.result .path {
  font-size: 0.8rem;
  font-weight: 600;
}

.result .preview {
  font-size: 0.78rem;
  color: var(--p-text-muted-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty {
  padding: 1rem;
  text-align: center;
  color: var(--p-text-muted-color);
  font-size: 0.85rem;
}
</style>
