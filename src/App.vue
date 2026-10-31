<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";

const route = useRoute();
const steps = ["目录", "规则", "扫描", "计划", "结果"];
const current = computed(() => {
  const map: Record<string, number> = {
    home: 0,
    configure: 1,
    "scan-progress": 2,
    plan: 3,
    result: 4,
  };
  return map[String(route.name)] ?? 0;
});
</script>

<template>
  <div class="shell">
    <header class="shell__header">
      <div class="shell__brand">
        <span class="shell__dot" />
        <h1>旅游照片 / 视频整理</h1>
      </div>
      <nav class="flow" aria-label="整理流程">
        <span
          v-for="(step, index) in steps"
          :key="step"
          class="flow__step"
          :class="{
            'flow__step--active': index === current,
            'flow__step--done': index < current,
          }"
        >
          <i>{{ String(index + 1).padStart(2, "0") }}</i>
          {{ step }}
        </span>
      </nav>
    </header>
    <main class="shell__main">
      <router-view />
    </main>
    <footer class="shell__footer">
      <span>本地处理 · 不覆盖 · 可撤销</span>
      <span>Rust core + Tauri</span>
    </footer>
  </div>
</template>
