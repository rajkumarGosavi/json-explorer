<script setup lang="ts">
import ContextMenu from "primevue/contextmenu";
import type { MenuItem } from "primevue/menuitem";
import { computed, nextTick, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { RecycleScroller } from "vue-virtual-scroller";
import "vue-virtual-scroller/dist/vue-virtual-scroller.css";
import { getNodeValue, getPath } from "@/api/ipc";
import InspectorPane from "@/components/InspectorPane.vue";
import SearchPanel from "@/components/SearchPanel.vue";
import TreeRow from "@/components/TreeRow.vue";
import { nextAction } from "@/composables/treeNav";
import { useTree } from "@/composables/useTree";
import { useFileStore } from "@/stores/file";
import { copyText } from "@/utils/clipboard";
import { formatBytes } from "@/utils/format";
import { pathToString } from "@/utils/jsonPath";

/** Matches MAX_VALUE_CAP in commands.rs — a copied subtree/value is capped. */
const MAX_VALUE_BYTES = 1024 * 1024;

const store = useFileStore();
const router = useRouter();
const { rows, error, loadRoot, toggle, loadMore } = useTree();
const selectedId = ref<string | null>(null);
/** Keyboard cursor, tracked by rowId so "more" rows are addressable too. */
const cursorRowId = ref<string | null>(null);
const searchOpen = ref(false);

const treePanel = ref<HTMLElement | null>(null);
const scrollerRef = ref<{ scrollToItem?: (index: number) => void } | null>(null);
const menuRef = ref<InstanceType<typeof ContextMenu> | null>(null);
const contextNodeId = ref<string | null>(null);

function onSelect(nodeId: string) {
  selectedId.value = nodeId;
  cursorRowId.value = nodeId; // a node row's rowId equals its nodeId
}

function onSearchSelect(nodeId: string) {
  selectedId.value = nodeId;
  cursorRowId.value = nodeId;
}

const fileName = computed(
  () => store.meta?.path.split(/[\\/]/).pop() ?? "",
);

// --- Right-click copy menu -------------------------------------------------

const menuItems: MenuItem[] = [
  { label: "Copy key", icon: "pi pi-key", command: () => void copyKey() },
  { label: "Copy path", icon: "pi pi-sitemap", command: () => void copyPath() },
  { label: "Copy value", icon: "pi pi-copy", command: () => void copyValue() },
];

function onContext(nodeId: string, event: MouseEvent) {
  contextNodeId.value = nodeId;
  menuRef.value?.show(event);
}

async function copyKey() {
  const id = contextNodeId.value;
  const row = rows.value.find((r) => r.nodeId === id && r.type === "node");
  if (row) await copyText(row.label);
}

async function copyPath() {
  const id = contextNodeId.value;
  if (id === null) return;
  await copyText(pathToString(await getPath(id)));
}

async function copyValue() {
  const id = contextNodeId.value;
  if (id === null) return;
  const chunk = await getNodeValue(id, MAX_VALUE_BYTES);
  await copyText(chunk.text);
}

// --- Keyboard navigation ---------------------------------------------------

function rowIndex(rowId: string): number {
  return rows.value.findIndex((r) => r.rowId === rowId);
}

async function applyNav(rowId: string) {
  cursorRowId.value = rowId;
  const row = rows.value.find((r) => r.rowId === rowId);
  if (row && row.type === "node") selectedId.value = row.nodeId;
  await nextTick();
  const i = rowIndex(rowId);
  if (i >= 0) scrollerRef.value?.scrollToItem?.(i);
}

async function onKeydown(e: KeyboardEvent) {
  if (e.key === "/") {
    if (!searchOpen.value) {
      searchOpen.value = true;
      e.preventDefault();
    }
    return;
  }
  if (e.key === "Escape") {
    if (searchOpen.value) searchOpen.value = false;
    else selectedId.value = null;
    return;
  }
  const result = nextAction(rows.value, cursorRowId.value, e.key);
  if (result.select) await applyNav(result.select);
  else if (result.toggle) await toggle(result.toggle);
  else if (result.loadMore) await loadMore(result.loadMore);
  else return;
  e.preventDefault();
}

onMounted(async () => {
  // Deep-linking straight to /explore without an open file: bounce home.
  if (store.phase !== "ready") {
    void router.replace({ name: "open" });
    return;
  }
  await loadRoot();
  if (cursorRowId.value === null && rows.value.length > 0) {
    cursorRowId.value = rows.value[0].rowId;
  }
  treePanel.value?.focus();
});

async function closeFile() {
  await store.close();
  void router.push({ name: "open" });
}
</script>

<template>
  <main class="explorer-view">
    <header class="topbar">
      <div class="file-info">
        <i class="pi pi-file" />
        <span class="name" :title="store.meta?.path">{{ fileName }}</span>
        <Tag v-if="store.meta?.multiDoc" value="NDJSON" severity="info" />
        <span class="meta mono">
          {{ formatBytes(Number(store.meta?.sizeBytes ?? 0)) }} ·
          {{ (store.meta?.containerCount ?? 0).toLocaleString() }} containers ·
          indexed in {{ store.meta?.indexMillis ?? 0 }} ms
        </span>
      </div>
      <div class="actions">
        <Button
          icon="pi pi-search"
          :severity="searchOpen ? 'primary' : 'secondary'"
          text
          size="small"
          title="Search"
          @click="searchOpen = !searchOpen"
        />
        <Button
          label="Close"
          icon="pi pi-times"
          severity="secondary"
          text
          size="small"
          @click="closeFile"
        />
      </div>
    </header>

    <Message v-if="error" severity="error" class="tree-error">{{
      error
    }}</Message>

    <Splitter v-else class="body" layout="horizontal">
      <SplitterPanel :size="65" :min-size="30" class="tree-panel">
        <SearchPanel
          v-if="searchOpen"
          @select="onSearchSelect"
          @close="searchOpen = false"
        />
        <div
          v-else
          ref="treePanel"
          class="tree-focus"
          tabindex="0"
          @keydown="onKeydown"
        >
          <RecycleScroller
            ref="scrollerRef"
            class="scroller"
            :items="rows"
            :item-size="28"
            key-field="rowId"
          >
            <template #default="{ item }">
              <TreeRow
                :row="item"
                :selected="item.rowId === cursorRowId"
                @toggle="toggle"
                @select="onSelect"
                @load-more="loadMore"
                @context="onContext"
              />
            </template>
          </RecycleScroller>
        </div>
      </SplitterPanel>
      <SplitterPanel :size="35" :min-size="20">
        <InspectorPane :node-id="selectedId" />
      </SplitterPanel>
    </Splitter>

    <ContextMenu ref="menuRef" :model="menuItems" />
  </main>
</template>

<style scoped>
.explorer-view {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.4rem 0.75rem;
  border-bottom: 1px solid var(--p-content-border-color);
}

.file-info {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.file-info .name {
  font-weight: 600;
  white-space: nowrap;
}

.file-info .meta {
  color: var(--p-text-muted-color);
  font-size: 0.8rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.actions {
  display: flex;
  align-items: center;
  gap: 0.15rem;
}

.tree-error {
  margin: 1rem;
}

.body {
  flex: 1;
  min-height: 0;
  border: none;
  border-radius: 0;
}

.tree-panel {
  overflow: hidden;
}

.tree-focus {
  height: 100%;
  outline: none;
}

.scroller {
  height: 100%;
}
</style>
