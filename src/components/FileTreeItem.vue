<script setup lang="ts">
import { computed } from "vue";
import type { PlanItem } from "../api/types";
import MediaPreview from "./MediaPreview.vue";

const props = defineProps<{ item: PlanItem; busy?: boolean }>();
const emit = defineEmits<{ toggle: [id: string, selected: boolean] }>();
const lockKeeper = computed(() => Boolean(props.item.reason?.endsWith("_keeper")));

const actionLabel = computed(() => {
  switch (props.item.action) {
    case "archive":
      return "归档";
    case "isolate":
      return "隔离";
    case "unresolved":
      return "未知日期";
    default:
      return "不处理";
  }
});

const reasonLabel = computed(() => {
  switch (props.item.reason) {
    case "byte_duplicate_keeper":
      return "字节重复组保留项";
    case "byte_duplicate":
      return "字节完全相同";
    case "similar_image_keeper":
      return "相似图保留项";
    case "similar_image":
      return "视觉相似";
    case "existing_identical":
      return "目标已有相同文件";
    case "already_organized":
      return "已在正确位置";
    case "already_organized_keeper":
      return "重复组保留项，已在正确位置";
    case "already_isolated":
      return "已在隔离区";
    case "date_unresolved":
      return "无法确定拍摄日期";
    default:
      return "已解析拍摄日期";
  }
});

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

function toggleFromRow() {
  if (props.item.action === "keep" || lockKeeper.value || props.busy) return;
  emit("toggle", props.item.id, !props.item.selected);
}
</script>

<template>
  <div
    class="file-row"
    :data-action="item.action"
    :class="{
      'file-row--grouped': item.group_id,
      'file-row--deselected': !item.selected && item.action !== 'keep',
    }"
  >
    <MediaPreview :item="item" />

    <label class="file-row__check">
      <input
        type="checkbox"
        :checked="item.selected"
        :disabled="item.action === 'keep' || lockKeeper || busy"
        @change="
          emit('toggle', item.id, ($event.target as HTMLInputElement).checked)
        "
      />
    </label>

    <div
      class="file-row__body"
      :role="item.action === 'keep' ? undefined : 'checkbox'"
      :aria-checked="item.action === 'keep' ? undefined : item.selected"
      :tabindex="item.action === 'keep' ? undefined : 0"
      @click="toggleFromRow"
      @keydown.enter.prevent="toggleFromRow"
      @keydown.space.prevent="toggleFromRow"
    >
      <div class="file-row__headline">
        <span class="action-pill" :data-action="item.action">{{ actionLabel }}</span>
        <span class="file-row__name">{{ item.source_path.split(/[\\/]/).pop() }}</span>
        <span v-if="item.group_id" class="group-pill">{{ item.group_id }}</span>
        <span class="file-row__size">{{ formatSize(item.size_bytes) }}</span>
      </div>
      <p class="file-row__reason">{{ reasonLabel }}</p>
      <p class="file-row__path">{{ item.source_path }}</p>
      <p v-if="item.target_path" class="file-row__target">→ {{ item.target_path }}</p>
    </div>
  </div>
</template>

<style scoped>
.file-row {
  position: relative;
  display: grid;
  grid-template-columns: 48px 26px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  padding: 11px 14px;
  border-bottom: 1px solid var(--line);
  background: var(--bg-panel);
  transition: background 160ms var(--ease-out);
}

.file-row:last-child {
  border-bottom: none;
}

.file-row::before {
  position: absolute;
  inset: 0 auto 0 0;
  width: 3px;
  background: transparent;
  content: "";
  transition: background 160ms var(--ease-out);
}

.file-row:hover {
  background: color-mix(in srgb, var(--bg-soft) 34%, var(--bg-panel));
}

.file-row[data-action="isolate"] {
  background: color-mix(in srgb, var(--warm-soft) 44%, var(--bg-panel));
}

.file-row[data-action="isolate"]::before {
  background: var(--warm);
}

.file-row[data-action="unresolved"] {
  background: color-mix(in srgb, var(--warm-soft) 58%, var(--bg-panel));
}

.file-row[data-action="archive"]::before {
  background: var(--accent);
}

.file-row[data-action="keep"] {
  opacity: 0.6;
}

.file-row--deselected {
  opacity: 0.62;
}

.file-row--deselected .file-row__body,
.file-row--deselected .media-preview {
  filter: grayscale(0.7);
}

.file-row__check {
  display: grid;
  place-items: center;
}

.file-row__check input {
  width: 15px;
  height: 15px;
  accent-color: var(--accent);
  cursor: pointer;
}

.file-row__check input:disabled {
  cursor: not-allowed;
}

.file-row__body {
  min-width: 0;
  cursor: pointer;
  outline: none;
  border-radius: 8px;
}

.file-row__body:focus-visible {
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.file-row__headline {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.file-row__name {
  overflow: hidden;
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-row__size {
  margin-left: auto;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 10px;
  white-space: nowrap;
}

.action-pill,
.group-pill {
  flex: none;
  padding: 2px 7px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.action-pill {
  color: var(--accent);
  background: var(--accent-soft);
  border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
}

.action-pill[data-action="isolate"] {
  color: #8a4a13;
  background: var(--warm-soft);
  border-color: color-mix(in srgb, var(--warm) 30%, transparent);
}

.action-pill[data-action="unresolved"] {
  color: #82591f;
  background: var(--warm-soft);
  border-color: color-mix(in srgb, var(--warm) 24%, transparent);
}

.action-pill[data-action="keep"] {
  color: var(--ink-soft);
  background: var(--bg-soft);
  border-color: var(--line);
}

.group-pill {
  color: var(--ink-soft);
  border: 1px solid var(--line);
  font-family: var(--font-mono);
}

.file-row__reason,
.file-row__path,
.file-row__target {
  margin: 4px 0 0;
  font-size: 11px;
  line-height: 1.45;
}

.file-row__reason {
  color: var(--ink-soft);
}

.file-row__path,
.file-row__target {
  overflow-wrap: anywhere;
  color: var(--ink-soft);
  font-family: var(--font-mono);
}

.file-row__target {
  color: var(--accent);
}
</style>
