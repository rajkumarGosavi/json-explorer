<script setup lang="ts">
import { ref, watch } from "vue";
import { getNodeValue, getPath } from "@/api/ipc";
import type { ValueChunk } from "@/types/json";
import { copyText } from "@/utils/clipboard";
import { formatBytes } from "@/utils/format";
import { pathToString } from "@/utils/jsonPath";

const props = defineProps<{ nodeId: string | null }>();

/** Backend caps a single chunk at 1 MB (MAX_VALUE_CAP in commands.rs). */
const MAX_VALUE_BYTES = 1024 * 1024;

const path = ref("");
const chunk = ref<ValueChunk | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

async function load(maxBytes?: number) {
  const id = props.nodeId;
  if (id === null) return;
  loading.value = true;
  error.value = null;
  try {
    const [segs, value] = await Promise.all([
      getPath(id),
      getNodeValue(id, maxBytes),
    ]);
    if (id !== props.nodeId) return; // selection changed while in flight
    path.value = pathToString(segs);
    chunk.value = value;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.nodeId,
  () => {
    path.value = "";
    chunk.value = null;
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

      <Message v-if="error" severity="error">{{ error }}</Message>

      <template v-else-if="chunk">
        <div class="value-meta">
          <span class="mono">{{ formatBytes(Number(chunk.totalBytes)) }}</span>
          <Button
            icon="pi pi-copy"
            size="small"
            text
            severity="secondary"
            title="Copy value"
            @click="copyText(chunk.text)"
          />
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
        <pre class="value mono">{{ chunk.text }}</pre>
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
