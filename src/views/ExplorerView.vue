<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { RecycleScroller } from "vue-virtual-scroller";
import "vue-virtual-scroller/dist/vue-virtual-scroller.css";
import InspectorPane from "@/components/InspectorPane.vue";
import SearchPanel from "@/components/SearchPanel.vue";
import TreeRow from "@/components/TreeRow.vue";
import { useTree } from "@/composables/useTree";
import { useFileStore } from "@/stores/file";
import { formatBytes } from "@/utils/format";

const store = useFileStore();
const router = useRouter();
const { rows, error, loadRoot, toggle, loadMore } = useTree();
const selectedId = ref<string | null>(null);
const searchOpen = ref(false);

function onSearchSelect(nodeId: string) {
  selectedId.value = nodeId;
}

const fileName = computed(
  () => store.meta?.path.split(/[\\/]/).pop() ?? "",
);

onMounted(() => {
  // Deep-linking straight to /explore without an open file: bounce home.
  if (store.phase !== "ready") {
    void router.replace({ name: "open" });
    return;
  }
  void loadRoot();
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
        <RecycleScroller
          v-else
          class="scroller"
          :items="rows"
          :item-size="28"
          key-field="rowId"
        >
          <template #default="{ item }">
            <TreeRow
              :row="item"
              :selected="item.type === 'node' && item.nodeId === selectedId"
              @toggle="toggle"
              @select="(id: string) => (selectedId = id)"
              @load-more="loadMore"
            />
          </template>
        </RecycleScroller>
      </SplitterPanel>
      <SplitterPanel :size="35" :min-size="20">
        <InspectorPane :node-id="selectedId" />
      </SplitterPanel>
    </Splitter>
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

.scroller {
  height: 100%;
}
</style>
