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
      <span class="dupe__chevron" aria-hidden="true">›</span>
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
          :class="{
            'dupe-file--keeper': item.action !== 'isolate',
            'dupe-file--off': item.action === 'isolate' && !item.selected,
          }"
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
  border-radius: var(--radius-sm);
  background: var(--bg-panel);
  transition: border-color 160ms var(--ease-out);
}

.dupe + .dupe {
  margin-top: 10px;
}

.dupe summary {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 11px 12px;
  color: var(--ink-soft);
  font-size: 11px;
  cursor: pointer;
  list-style: none;
  user-select: none;
}

.dupe summary::-webkit-details-marker {
  display: none;
}

.dupe__chevron {
  color: var(--warm);
  font-size: 13px;
  font-weight: 700;
  transform: rotate(90deg);
  transition: transform 180ms var(--ease-out);
}

.dupe:not([open]) .dupe__chevron {
  transform: rotate(0deg);
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
  position: relative;
  display: grid;
  gap: 8px;
  min-width: 0;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--warm) 30%, var(--line));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--warm-soft) 38%, var(--bg-panel));
  transition:
    opacity 160ms var(--ease-out),
    box-shadow 160ms var(--ease-out);
}

.dupe-file:hover {
  box-shadow: var(--shadow-card);
}

.dupe-file--keeper {
  border-color: color-mix(in srgb, var(--accent) 32%, var(--line));
  background: color-mix(in srgb, var(--accent-soft) 48%, var(--bg-panel));
}

.dupe-file--keeper::after {
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 2px 6px;
  border-radius: 999px;
  background: var(--accent-strong);
  color: var(--accent-ink);
  content: "保留";
  font-size: 9px;
  font-weight: 700;
}

.dupe-file--off {
  opacity: 0.55;
}

.dupe-file--off :deep(.media-preview) {
  filter: grayscale(0.8);
}

.dupe-file__choice {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 7px;
  border-radius: 8px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  color: var(--ink-soft);
  font-size: 11px;
  cursor: pointer;
  transition: border-color 150ms var(--ease-out), color 150ms var(--ease-out);
}

.dupe-file__choice input {
  accent-color: var(--accent);
}

.dupe-file__choice:hover {
  border-color: var(--accent);
  color: var(--accent-strong);
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
