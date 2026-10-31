<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useScanStore } from "../stores/scan.store";

const router = useRouter();
const store = useScanStore();
const formError = ref<string | null>(null);

async function pickSources() {
  try {
    const picked = await open({ directory: true, multiple: true });
    const list = Array.isArray(picked) ? picked : picked ? [picked] : [];
    store.options.source_dirs = Array.from(
      new Set([...store.options.source_dirs, ...list]),
    );
  } catch (error) {
    formError.value = String(error);
  }
}

async function pickTarget() {
  try {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") store.options.target_dir = picked;
  } catch (error) {
    formError.value = String(error);
  }
}

function removeSource(dir: string) {
  store.options.source_dirs = store.options.source_dirs.filter(
    (candidate) => candidate !== dir,
  );
}

const canContinue = computed(
  () =>
    store.options.source_dirs.length > 0 &&
    store.options.target_dir.length > 0,
);

function next() {
  if (!canContinue.value) return;
  if (store.status !== "idle") store.resetRun();
  router.push("/configure");
}
</script>

<template>
  <section class="home">
    <div class="hero">
      <p class="eyebrow">TRAVEL ARCHIVE / 01</p>
      <h2>先把散落的旅程收进一个入口。</h2>
      <p class="hero__copy">
        扫描只读取文件，不会移动或删除任何内容。整理计划生成后，你可以逐项确认。
      </p>
    </div>

    <div class="home__grid">
      <article class="card">
        <div class="card__heading">
          <div>
            <span class="step-number">01</span>
            <h3>源目录</h3>
          </div>
          <button class="btn btn--ghost" @click="pickSources">添加目录</button>
        </div>
        <ul class="source-list">
          <li v-for="dir in store.options.source_dirs" :key="dir">
            <span class="source-list__icon">↳</span>
            <span class="source-list__path">{{ dir }}</span>
            <button title="移除" @click="removeSource(dir)">×</button>
          </li>
          <li v-if="store.options.source_dirs.length === 0" class="empty-state">
            可以添加多个来源文件夹，重复扫描会自动归并。
          </li>
        </ul>
      </article>

      <article class="card">
        <div class="card__heading">
          <div>
            <span class="step-number">02</span>
            <h3>目标根目录</h3>
          </div>
          <button class="btn btn--ghost" @click="pickTarget">选择目录</button>
        </div>
        <div class="target-box" :class="{ 'target-box--empty': !store.options.target_dir }">
          <span class="target-box__shape">Y/M</span>
          <p>{{ store.options.target_dir || "尚未选择目标目录" }}</p>
          <small>将创建 YEAR / YEAR-MONTH / 原文件名的两级结构</small>
        </div>
      </article>
    </div>

    <footer class="page-actions">
      <p v-if="formError" class="error">{{ formError }}</p>
      <button class="btn btn--large" :disabled="!canContinue" @click="next">
        配置扫描规则
      </button>
    </footer>
  </section>
</template>

<style scoped>
.home {
  display: grid;
  gap: 24px;
}

.hero {
  padding: 12px 2px 4px;
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

.eyebrow {
  margin: 0 0 12px;
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.hero h2 {
  max-width: 780px;
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(30px, 5vw, 54px);
  font-weight: 600;
  letter-spacing: -0.045em;
  line-height: 1.05;
}

.hero__copy {
  max-width: 640px;
  margin: 18px 0 0;
  color: var(--ink-soft);
  font-size: 14px;
  line-height: 1.7;
}

.home__grid {
  display: grid;
  grid-template-columns: 1.08fr 0.92fr;
  gap: 18px;
}

.card__heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 18px;
}

.card__heading > div {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.card__heading h3 {
  margin: 0;
  font-size: 16px;
}

.step-number {
  color: var(--warm);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
}

.source-list {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.source-list li:not(.empty-state) {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr) 24px;
  align-items: center;
  gap: 8px;
  padding: 11px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-inset);
  transition: border-color 150ms var(--ease-out);
}

.source-list li:not(.empty-state):hover {
  border-color: var(--line-strong);
}

.source-list__icon {
  color: var(--accent);
}

.source-list__path {
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-list button {
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--ink-soft);
  font-size: 17px;
  cursor: pointer;
  transition: color 150ms var(--ease-out), background 150ms var(--ease-out);
}

.source-list button:hover {
  background: var(--bg-soft);
  color: var(--danger);
}

.empty-state {
  padding: 26px 16px;
  border: 1px dashed var(--line-strong);
  border-radius: 10px;
  color: var(--ink-soft);
  font-size: 12px;
  text-align: center;
}

.target-box {
  min-height: 151px;
  padding: 22px;
  border-radius: var(--radius-sm);
  background:
    linear-gradient(145deg, color-mix(in srgb, var(--accent-soft) 62%, var(--bg-panel)), var(--bg-panel));
  border: 1px solid color-mix(in srgb, var(--accent) 20%, var(--line));
}

.target-box--empty {
  background: var(--bg-soft);
  border-style: dashed;
}

.target-box__shape {
  display: inline-grid;
  width: 54px;
  height: 54px;
  place-items: center;
  margin-bottom: 14px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-ink);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
}

.target-box p {
  margin: 0 0 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  overflow-wrap: anywhere;
}

.target-box small {
  color: var(--ink-soft);
  font-size: 11px;
}

.page-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 18px;
}

.page-actions .error {
  margin-right: auto;
}

@media (max-width: 760px) {
  .home__grid {
    grid-template-columns: 1fr;
  }

  .page-actions {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
