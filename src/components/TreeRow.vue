<script setup lang="ts">
import { computed } from "vue";
import type { TreeRowModel } from "@/composables/useTree";
import type { JsonKind } from "@/types/json";

const props = defineProps<{ row: TreeRowModel; selected: boolean }>();
const emit = defineEmits<{
  toggle: [nodeId: string];
  select: [nodeId: string];
  loadMore: [nodeId: string];
  context: [nodeId: string, event: MouseEvent];
}>();

const KIND_GLYPHS: Record<JsonKind, string> = {
  object: "{}",
  array: "[]",
  string: '""',
  number: "#",
  bool: "tf",
  null: "∅",
};

const isContainer = computed(
  () =>
    props.row.type === "node" &&
    (props.row.summary.kind === "object" || props.row.summary.kind === "array"),
);

function onClick() {
  if (props.row.type === "more") {
    emit("loadMore", props.row.nodeId);
    return;
  }
  emit("select", props.row.nodeId);
}

function onCaret() {
  if (isContainer.value) emit("toggle", props.row.nodeId);
}

function onContext(event: MouseEvent) {
  if (props.row.type !== "node") return;
  emit("context", props.row.nodeId, event);
}
</script>

<template>
  <div
    class="tree-row mono"
    :class="{ selected, more: row.type === 'more' }"
    :style="{ paddingLeft: `${row.depth * 16 + 8}px` }"
    @click="onClick"
    @dblclick="onCaret"
    @contextmenu.prevent="onContext"
  >
    <template v-if="row.type === 'node'">
      <span class="caret" @click.stop="onCaret">
        <i
          v-if="isContainer"
          class="pi"
          :class="row.expanded ? 'pi-chevron-down' : 'pi-chevron-right'"
        />
      </span>
      <span class="kind" :class="`kind-${row.summary.kind}`">
        {{ KIND_GLYPHS[row.summary.kind] }}
      </span>
      <span class="label">{{ row.label }}</span>
      <span v-if="row.summary.preview" class="preview">{{
        row.summary.preview
      }}</span>
      <span v-else-if="isContainer" class="count">{{
        row.summary.childCount.toLocaleString()
      }}</span>
      <i v-if="row.loading" class="pi pi-spinner pi-spin loading" />
    </template>
    <template v-else>
      <span class="caret" />
      <span class="load-more">
        <i v-if="row.loading" class="pi pi-spinner pi-spin" />
        Load more… ({{ row.remaining.toLocaleString() }} remaining)
      </span>
    </template>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  height: 28px;
  padding-right: 8px;
  font-size: 0.85rem;
  white-space: nowrap;
  overflow: hidden;
  cursor: pointer;
  user-select: none;
}

.tree-row:hover {
  background: var(--p-content-hover-background);
}

.tree-row.selected {
  background: var(--p-highlight-background);
  color: var(--p-highlight-color);
}

.caret {
  flex: 0 0 1rem;
  display: inline-flex;
  justify-content: center;
}

.caret .pi {
  font-size: 0.7rem;
  color: var(--p-text-muted-color);
}

.kind {
  flex: 0 0 auto;
  font-size: 0.7rem;
  font-weight: 700;
  opacity: 0.9;
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

.label {
  flex: 0 0 auto;
  font-weight: 600;
}

.preview {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--p-text-muted-color);
}

.count {
  color: var(--p-text-muted-color);
  font-size: 0.75rem;
}

.loading {
  font-size: 0.75rem;
  color: var(--p-text-muted-color);
}

.load-more {
  color: var(--p-primary-color);
  font-size: 0.8rem;
}
</style>
