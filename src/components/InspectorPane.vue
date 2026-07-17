<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { getNodeStats, getNodeValue, getPath } from "@/api/ipc";
import type { NodeStats, ValueChunk } from "@/types/json";
import { copyText } from "@/utils/clipboard";
import { formatBytes } from "@/utils/format";
import { highlightJson } from "@/utils/jsonHighlight";
import { pathToString } from "@/utils/jsonPath";

const props = defineProps<{ nodeId: string | null }>();

/** Backend caps a single chunk at 1 MB (MAX_VALUE_CAP in commands.rs). */
const MAX_VALUE_BYTES = 1024 * 1024;

const path = ref("");
const chunk = ref<ValueChunk | null>(null);
const stats = ref<NodeStats | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const pretty = ref(false);

async function load(maxBytes?: number) {
  const id = props.nodeId;
  if (id === null) return;
  loading.value = true;
  error.value = null;
  try {
    const [segs, value, nodeStats] = await Promise.all([
      getPath(id),
      getNodeValue(id, maxBytes),
      getNodeStats(id),
    ]);
    if (id !== props.nodeId) return; // selection changed while in flight
    path.value = pathToString(segs);
    chunk.value = value;
    stats.value = nodeStats;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// Reformat the raw slice with indentation; null when it isn't whole/valid JSON.
const prettyText = computed<string | null>(() => {
  if (!chunk.value || chunk.value.truncated) return null;
  try {
    return JSON.stringify(JSON.parse(chunk.value.text), null, 2);
  } catch {
    return null;
  }
});
const canPretty = computed(() => prettyText.value !== null);
const prettyHtml = computed(() =>
  prettyText.value ? highlightJson(prettyText.value) : "",
);

// Nonzero kind tallies for the stats strip.
const kindEntries = computed(() => {
  const c = stats.value?.kindCounts;
  if (!c) return [];
  return (
    [
      ["object", c.object],
      ["array", c.array],
      ["string", c.string],
      ["number", c.number],
      ["bool", c.bool],
      ["null", c.null],
    ] as const
  )
    .filter(([, n]) => n > 0)
    .map(([label, count]) => ({ label, count }));
});

watch(
  () => props.nodeId,
  () => {
    path.value = "";
    chunk.value = null;
    stats.value = null;
    error.value = null;
    void load();
  },
  { immediate: true },
);
</script>

<template>
  <div class="inspector">
    <div v-if="nodeId === null" class="empty">
      <p>Select a node to inspect its path and raw value.</p>
    </div>
    <template v-else>
      <div class="path-bar">
        <code class="path" :title="path">{{ path || "…" }}</code>
        <Button
          icon="pi pi-copy"
          size="small"
          text
          severity="secondary"
          title="Copy path"
          :disabled="!path"
          @click="copyText(path)"
        />
      </div>

      <div v-if="stats && stats.childCount > 0" class="stats mono">
        <span>{{ stats.childCount.toLocaleString() }} children</span>
        <span>·</span>
        <span>{{ formatBytes(Number(stats.byteSize)) }}</span>
        <span
          v-for="k in kindEntries"
          :key="k.label"
          class="kind-tally"
          :class="`kind-${k.label}`"
        >
          {{ k.count.toLocaleString() }} {{ k.label }}
        </span>
      </div>

      <Message v-if="error" severity="error">{{ error }}</Message>

      <template v-else-if="chunk">
        <div class="value-meta">
          <span class="mono">{{ formatBytes(Number(chunk.totalBytes)) }}</span>
          <div class="value-actions">
            <Button
              :label="pretty ? 'Raw' : 'Pretty'"
              size="small"
              text
              severity="secondary"
              :disabled="!canPretty"
              :title="canPretty ? 'Toggle pretty-print' : 'Value is truncated or not valid JSON'"
              @click="pretty = !pretty"
            />
            <Button
              icon="pi pi-copy"
              size="small"
              text
              severity="secondary"
              title="Copy value"
              @click="copyText(chunk.text)"
            />
          </div>
        </div>
        <Message v-if="chunk.truncated" severity="warn" size="small">
          Showing the first {{ formatBytes(chunk.text.length) }} of
          {{ formatBytes(Number(chunk.totalBytes)) }}.
          <Button
            v-if="chunk.text.length < MAX_VALUE_BYTES"
            label="Load up to 1 MB"
            link
            size="small"
            @click="load(MAX_VALUE_BYTES)"
          />
        </Message>
        <!-- eslint-disable-next-line vue/no-v-html — highlightJson HTML-escapes its input -->
        <pre v-if="pretty && canPretty" class="value mono" v-html="prettyHtml" />
        <pre v-else class="value mono">{{ chunk.text }}</pre>
      </template>

      <div v-else-if="loading" class="empty">
        <i class="pi pi-spinner pi-spin" />
      </div>
    </template>
  </div>
</template>

<style scoped>
.inspector {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.75rem;
  overflow: hidden;
}

.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--p-text-muted-color);
  text-align: center;
}

.path-bar {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.path {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.85rem;
  padding: 0.35rem 0.5rem;
  border-radius: 6px;
  background: var(--p-content-hover-background);
}

.stats {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--p-text-muted-color);
}

.kind-tally {
  padding: 0.05rem 0.35rem;
  border-radius: 4px;
  background: var(--p-content-hover-background);
  font-weight: 600;
}
.kind-object,
.kind-array {
  color: var(--p-primary-color);
}
.kind-string {
  color: var(--p-green-600, #3d9a50);
}
.kind-number {
  color: var(--p-orange-600, #c77b2c);
}
.kind-bool {
  color: var(--p-purple-600, #8b5cb8);
}
.kind-null {
  color: var(--p-text-muted-color);
}

.value-actions {
  display: flex;
  align-items: center;
  gap: 0.1rem;
}

/* Highlighted tokens are injected via v-html, so they need :deep to escape
   scoped-CSS attribute stamping. */
.value :deep(.json-key) {
  color: var(--p-primary-color);
}
.value :deep(.json-string) {
  color: var(--p-green-600, #3d9a50);
}
.value :deep(.json-number) {
  color: var(--p-orange-600, #c77b2c);
}
.value :deep(.json-boolean) {
  color: var(--p-purple-600, #8b5cb8);
}
.value :deep(.json-null) {
  color: var(--p-text-muted-color);
}

.value-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--p-text-muted-color);
  font-size: 0.8rem;
}

.value {
  flex: 1;
  margin: 0;
  padding: 0.5rem;
  overflow: auto;
  font-size: 0.85rem;
  border: 1px solid var(--p-content-border-color);
  border-radius: 6px;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
