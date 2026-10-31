<script setup lang="ts">
export interface StatItem {
  label: string;
  value: string | number;
  tone?: "normal" | "warning" | "danger";
}

defineProps<{ items: StatItem[] }>();
</script>

<template>
  <div class="stat-bar">
    <div
      v-for="item in items"
      :key="item.label"
      class="stat-bar__item"
      :data-tone="item.tone ?? 'normal'"
    >
      <strong>{{ item.value }}</strong>
      <span>{{ item.label }}</span>
    </div>
  </div>
</template>

<style scoped>
.stat-bar {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 10px;
}

.stat-bar__item {
  padding: 14px 16px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--bg-panel);
}

.stat-bar__item strong {
  display: block;
  font-family: var(--font-sans);
  font-size: 22px;
  font-weight: 700;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}

.stat-bar__item span {
  display: block;
  margin-top: 5px;
  color: var(--ink-faint);
  font-size: 10px;
  letter-spacing: 0.04em;
}

.stat-bar__item[data-tone="warning"] strong {
  color: var(--warm);
}

.stat-bar__item[data-tone="danger"] strong {
  color: var(--danger);
}
</style>
