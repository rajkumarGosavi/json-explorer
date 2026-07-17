<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { onDragDrop, pickJsonFile } from "@/api/ipc";
import { useFileStore } from "@/stores/file";
import { formatBytes } from "@/utils/format";

const store = useFileStore();
const router = useRouter();
const dragActive = ref(false);
const showPaste = ref(false);
const pasteText = ref("");
let unlisten: (() => void) | null = null;

function explorePasted() {
  const text = pasteText.value.trim();
  if (text) void store.openText(text);
}

onMounted(async () => {
  await store.init();
  unlisten = await onDragDrop({
    onEnter: () => (dragActive.value = true),
    onLeave: () => (dragActive.value = false),
    onDrop: (paths) => {
      dragActive.value = false;
      if (paths.length > 0) void store.open(paths[0]);
    },
  });
});
onUnmounted(() => unlisten?.());

watch(
  () => store.phase,
  (phase) => {
    if (phase === "ready") void router.push({ name: "explore" });
  },
);

async function browse() {
  const path = await pickJsonFile();
  if (path) void store.open(path);
}
</script>

<template>
  <main class="open-view">
    <div class="panel">
      <img src="/logo.svg" alt="JSON Explorer" class="logo" width="72" height="72" />
      <h1>JSON Explorer</h1>
      <p class="hint">
        Drop a JSON or NDJSON file anywhere in this window, or browse for one.
        Files are indexed in place — even multi-gigabyte files open quickly.
      </p>
      <div class="open-actions">
        <Button
          label="Open file…"
          icon="pi pi-folder-open"
          :disabled="store.phase === 'indexing'"
          @click="browse"
        />
        <Button
          :label="showPaste ? 'Hide paste' : 'Paste JSON…'"
          icon="pi pi-clipboard"
          severity="secondary"
          outlined
          :disabled="store.phase === 'indexing'"
          @click="showPaste = !showPaste"
        />
      </div>

      <div v-if="showPaste" class="paste">
        <Textarea
          v-model="pasteText"
          class="paste-box mono"
          placeholder='Paste JSON or NDJSON here, e.g. {"hello": "world"}'
          :rows="8"
          spellcheck="false"
          autofocus
        />
        <Button
          label="Explore pasted JSON"
          icon="pi pi-arrow-right"
          :disabled="store.phase === 'indexing' || pasteText.trim().length === 0"
          @click="explorePasted"
        />
      </div>

      <div v-if="store.phase === 'indexing'" class="progress">
        <ProgressBar
          :mode="store.bytesTotal > 0 ? 'determinate' : 'indeterminate'"
          :value="store.progressPercent"
          :show-value="false"
          style="height: 6px"
        />
        <p class="progress-label mono">
          Indexing {{ store.path }}
          <template v-if="store.bytesTotal > 0">
            — {{ formatBytes(store.bytesDone) }} /
            {{ formatBytes(store.bytesTotal) }}
          </template>
        </p>
      </div>

      <Message
        v-else-if="store.phase === 'error' && store.error"
        severity="error"
        class="error"
      >
        <strong>Could not open {{ store.path }}</strong
        ><br />
        {{ store.error.message }}
        <template v-if="store.error.line > 0">
          (line {{ store.error.line }}, column {{ store.error.col }})
        </template>
      </Message>
    </div>

    <div v-if="dragActive" class="drop-overlay">
      <span>Drop to open</span>
    </div>
  </main>
</template>

<style scoped>
.open-view {
  position: relative;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  max-width: 34rem;
  text-align: center;
  padding: 1rem;
}

.logo {
  border-radius: 16px;
  box-shadow: 0 6px 20px rgba(37, 99, 235, 0.35);
}

h1 {
  margin: 0;
}

.hint {
  margin: 0 0 0.5rem;
  color: var(--p-text-muted-color);
}

.open-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
  justify-content: center;
}

.paste {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.paste-box {
  width: 100%;
  resize: vertical;
  font-size: 0.85rem;
}

.progress {
  width: 100%;
  margin-top: 1rem;
}

.progress-label {
  font-size: 0.8rem;
  color: var(--p-text-muted-color);
  word-break: break-all;
}

.error {
  margin-top: 1rem;
  max-width: 100%;
}

.drop-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--p-primary-color) 12%, transparent);
  border: 3px dashed var(--p-primary-color);
  border-radius: 8px;
  pointer-events: none;
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--p-primary-color);
}
</style>
