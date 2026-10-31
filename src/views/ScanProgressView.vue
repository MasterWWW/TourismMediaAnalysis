<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import ProgressBar from "../components/ProgressBar.vue";
import StatsBar from "../components/StatsBar.vue";
import { useScanStore } from "../stores/scan.store";

const router = useRouter();
const store = useScanStore();

const phaseLabel = computed(() => {
  switch (store.progress?.phase) {
    case "enumerating":
      return "正在枚举文件";
    case "reading_metadata":
      return "正在读取拍摄时间";
    case "hashing":
      return "正在计算文件指纹";
    case "dedup":
      return "正在构建重复分组";
    case "done":
      return "扫描与整理计划已完成";
    default:
      return "准备扫描";
  }
});

const indeterminate = computed(() => store.isScanning);

const statItems = computed(() => [
  {
    label: "已检查文件",
    value: store.progress?.processed_files ?? 0,
  },
  {
    label: "识别媒体",
    value: store.progress?.media_found ?? store.stats?.media_found ?? 0,
  },
  {
    label: "日期未知",
    value: store.progress?.unresolved ?? store.stats?.unresolved ?? 0,
    tone: "warning" as const,
  },
  {
    label: "字节重复组",
    value: store.stats?.duplicate_groups ?? 0,
  },
  {
    label: "相似图片组",
    value: store.stats?.similar_groups ?? 0,
  },
]);

watch(
  () => store.status,
  (status) => {
    if (status === "planned") {
      window.setTimeout(() => router.push("/plan"), 450);
    }
  },
  { immediate: true },
);

onMounted(() => {
  if (store.status === "idle") router.replace("/");
});

async function cancel() {
  await store.cancel();
}
</script>

<template>
  <section class="scan-page">
    <div class="scan-orbit" aria-hidden="true">
      <span />
      <span />
      <span />
    </div>

    <header class="scan-head">
      <p class="eyebrow">TRAVEL ARCHIVE / 03</p>
      <h2>{{ store.error ? "扫描遇到问题。" : "正在读完整段旅程。" }}</h2>
      <p>{{ store.error || phaseLabel }}</p>
    </header>

    <article class="card progress-card">
      <ProgressBar
        :value="store.progressPercent"
        :label="phaseLabel"
        tone="warm"
        :indeterminate="indeterminate"
      />
      <StatsBar :items="statItems" />
    </article>

    <div v-if="store.error" class="card error-panel">
      <strong>无法继续</strong>
      <p>{{ store.error }}</p>
      <button class="btn" @click="router.push('/')">返回首页</button>
    </div>

    <div v-else-if="store.canceled" class="card error-panel">
      <strong>扫描已取消</strong>
      <p>已读取的文件仍保留在当前会话中，但不会生成执行计划。</p>
      <button class="btn" @click="router.push('/')">返回首页</button>
    </div>

    <footer v-else class="scan-foot">
      <span>扫描不会修改源文件。完成后先进入计划预览。</span>
      <button class="btn btn--ghost" :disabled="!store.isScanning" @click="cancel">
        取消扫描
      </button>
    </footer>
  </section>
</template>

<style scoped>
.scan-page {
  position: relative;
  display: grid;
  gap: 24px;
  padding-top: 20px;
  animation: page-in 320ms var(--ease-out) both;
}

@keyframes page-in {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.scan-head {
  max-width: 720px;
}

.eyebrow {
  margin: 0 0 12px;
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.scan-head h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(34px, 5vw, 58px);
  letter-spacing: -0.05em;
  line-height: 1.02;
}

.scan-head > p:last-child {
  margin: 14px 0 0;
  color: var(--ink-soft);
  font-size: 13px;
}

.progress-card {
  display: grid;
  gap: 24px;
}

.scan-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--ink-soft);
  font-size: 11px;
}

.error-panel {
  display: grid;
  gap: 10px;
}

.error-panel strong {
  font-size: 16px;
}

.error-panel p {
  margin: 0;
  color: var(--ink-soft);
  font-size: 12px;
}

.error-panel .btn {
  width: fit-content;
  margin-top: 6px;
}

.scan-orbit {
  position: absolute;
  top: -38px;
  right: 3%;
  width: 180px;
  height: 180px;
  pointer-events: none;
}

.scan-orbit span {
  position: absolute;
  inset: 0;
  border: 1px solid color-mix(in srgb, var(--accent) 24%, transparent);
  border-radius: 50%;
  animation: orbit 7s linear infinite;
}

.scan-orbit span:nth-child(2) {
  inset: 28px;
  animation-direction: reverse;
  animation-duration: 5s;
}

.scan-orbit span:nth-child(3) {
  inset: 64px;
  border-color: color-mix(in srgb, var(--warm) 55%, transparent);
}

.scan-orbit span::before {
  position: absolute;
  top: -4px;
  left: 50%;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
  content: "";
}

@keyframes orbit {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 700px) {
  .scan-orbit {
    display: none;
  }

  .scan-foot {
    align-items: stretch;
    flex-direction: column;
    gap: 12px;
  }
}
</style>
