<script setup lang="ts">
import { computed, ref } from "vue";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import type { PlanItem } from "../api/types";

const props = defineProps<{ item: PlanItem }>();
const open = ref(false);
const failed = ref(false);
const zoomed = ref(false);

const canUseAssetProtocol = isTauri();
const src = computed(() =>
  canUseAssetProtocol ? convertFileSrc(props.item.source_path) : "",
);
const previewLabel = computed(() => {
  switch (props.item.kind) {
    case "photo":
      return "照片";
    case "video":
      return "视频";
    case "raw":
      return "RAW";
    default:
      return "文件";
  }
});

function openPreview() {
  if (!canUseAssetProtocol || (props.item.kind !== "photo" && props.item.kind !== "video")) {
    return;
  }
  failed.value = false;
  zoomed.value = false;
  open.value = true;
}

function closePreview() {
  open.value = false;
  zoomed.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closePreview();
}
</script>

<template>
  <button
    class="media-preview"
    :class="{
      'media-preview--static': item.kind !== 'photo' && item.kind !== 'video',
      'media-preview--error': failed,
    }"
    :disabled="item.kind !== 'photo' && item.kind !== 'video'"
    :title="failed ? '无法生成预览，可继续按计划整理' : `预览${previewLabel}`"
    type="button"
    @click="openPreview"
  >
    <img
      v-if="item.kind === 'photo' && !failed"
      :src="src"
      loading="lazy"
      alt=""
      @error="failed = true"
    />
    <span v-else class="media-preview__fallback">{{ previewLabel }}</span>
  </button>

  <Teleport to="body">
    <div
      v-if="open"
      class="media-lightbox"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      @click="closePreview"
      @keydown="onKeydown"
      @vue:mounted="($el as HTMLElement).focus()"
    >
      <button class="media-lightbox__close" aria-label="关闭预览" @click="closePreview">×</button>
      <img
        v-if="item.kind === 'photo' && !failed"
        :src="src"
        :class="{ 'media-lightbox__media--zoomed': zoomed }"
        alt=""
        @click.stop="zoomed = !zoomed"
        @error="failed = true"
      />
      <video
        v-else-if="item.kind === 'video'"
        :src="src"
        controls
        autoplay
        playsinline
        @click.stop
      />
      <div v-else class="media-lightbox__message">当前文件不支持内嵌预览。</div>
    </div>
  </Teleport>
</template>

<style scoped>
.media-preview {
  display: grid;
  width: 48px;
  height: 48px;
  flex: none;
  place-items: center;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-soft);
  color: var(--ink-soft);
  cursor: zoom-in;
  padding: 0;
  transition:
    transform 140ms var(--ease-out),
    box-shadow 140ms var(--ease-out);
}

.media-preview:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px -10px rgba(35, 36, 31, 0.5);
}

.media-preview:disabled {
  cursor: default;
}

.media-preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.media-preview__fallback {
  padding: 4px;
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
}

.media-preview--error {
  color: var(--warm);
}

.media-lightbox {
  position: fixed;
  z-index: 60;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 40px;
  background: rgba(26, 24, 18, 0.86);
  backdrop-filter: blur(6px);
  cursor: zoom-out;
  outline: none;
  animation: lightbox-in 180ms var(--ease-out);
}

@keyframes lightbox-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.media-lightbox img,
.media-lightbox video {
  max-width: min(1100px, 92vw);
  max-height: 84vh;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: var(--radius);
  background: #14130f;
  box-shadow: 0 32px 90px rgba(0, 0, 0, 0.5);
}

.media-lightbox__media--zoomed {
  max-width: none;
  max-height: none;
  cursor: zoom-out;
}

.media-lightbox__close {
  position: fixed;
  top: 22px;
  right: 26px;
  display: grid;
  width: 42px;
  height: 42px;
  place-items: center;
  border: 0;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.14);
  color: white;
  font-size: 26px;
  cursor: pointer;
  transition: background 140ms var(--ease-out);
}

.media-lightbox__close:hover {
  background: rgba(255, 255, 255, 0.28);
}

.media-lightbox__message {
  padding: 16px 20px;
  border-radius: 12px;
  background: var(--bg-panel);
  color: var(--ink);
  font-size: 13px;
}
</style>
