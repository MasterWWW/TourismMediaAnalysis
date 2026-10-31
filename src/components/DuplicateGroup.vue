<script setup lang="ts">
import { computed } from "vue";
import type { PlanItem } from "../api/types";
import MediaPreview from "./MediaPreview.vue";

const props = defineProps<{
  groupId: string;
  items: PlanItem[];
  busy?: boolean;
}>();

const emit = defineEmits<{ toggle: [id: string, selected: boolean] }>();

const keeper = computed(() =>
  props.items.find((item) => item.reason?.endsWith("_keeper")),
);
const duplicates = computed(() =>
  props.items
    .filter((item) => item.action === "isolate")
    .sort((a, b) => a.source_path.localeCompare(b.source_path)),
);
const comparedItems = computed(() => [
  ...(keeper.value ? [keeper.value] : []),
  ...duplicates.value,
]);
const selectedDuplicates = computed(
  () => duplicates.value.filter((item) => item.selected).length,
);

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(1)} ${units[index]}`;
}
</script>

<template>
  <details class="dupe" open>
    <summary>
      <span class="dupe__id">{{ groupId }}</span>
      <span>{{ items.length }} 个文件</span>
      <span class="dupe__count">已选隔离 {{ selectedDuplicates }}/{{ duplicates.length }}</span>
    </summary>
    <div class="dupe__body">
      <div class="dupe-compare">
        <div
          v-for="item in comparedItems"
          :key="item.id"
          class="dupe-file"
          :class="{ 'dupe-file--keeper': item.action !== 'isolate' }"
        >
          <MediaPreview :item="item" />
          <label class="dupe-file__choice">
            <input
              type="checkbox"
              :checked="item.selected"
              :disabled="busy || item.action !== 'isolate'"
              @change="
                emit(
                  'toggle',
                  item.id,
                  ($event.target as HTMLInputElement).checked,
                )
              "
            />
            <span>{{ item.action === "isolate" ? "隔离" : "保留" }}</span>
          </label>
          <div class="dupe-file__meta">
            <strong>
              {{ item.source_path.split(/[\\/]/).pop() }}
            </strong>
            <span>{{ formatSize(item.size_bytes) }}</span>
          </div>
          <code>{{ item.source_path }}</code>
        </div>
      </div>
    </div>
  </details>
</template>

<style scoped>
.dupe {
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--bg-panel);
}

.dupe + .dupe {
  margin-top: 10px;
}

.dupe summary {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 12px;
  color: var(--ink-soft);
  font-size: 11px;
  cursor: pointer;
  list-style: none;
}

.dupe summary::-webkit-details-marker {
  display: none;
}

.dupe__id {
  color: var(--ink);
  font-family: var(--font-mono);
  font-weight: 600;
}

.dupe__count {
  margin-left: auto;
  color: var(--warm);
}

.dupe__body {
  padding: 0 12px 12px;
}

.dupe-compare {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 10px;
}

.dupe-file {
  display: grid;
  gap: 8px;
  min-width: 0;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--warm) 30%, var(--line));
  border-radius: 10px;
  background: color-mix(in srgb, var(--warm-soft) 38%, var(--bg-panel));
}

.dupe-file--keeper {
  border-color: color-mix(in srgb, var(--accent) 32%, var(--line));
  background: color-mix(in srgb, var(--accent-soft) 48%, var(--bg-panel));
}

.dupe-file__choice {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 7px;
  border-radius: 8px;
  background: var(--bg-panel);
  color: var(--ink-soft);
  font-size: 11px;
  cursor: pointer;
}

.dupe-file__choice input {
  accent-color: var(--accent);
}

.dupe-file--keeper .dupe-file__choice {
  color: var(--accent);
  font-weight: 700;
}

.dupe-file__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.dupe-file__meta strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dupe-file__meta span {
  flex: none;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 10px;
}

.dupe-file code {
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 9px;
  overflow-wrap: anywhere;
}
</style>
