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
  border-radius: 12px;
  background: color-mix(in srgb, var(--bg-panel) 84%, transparent);
}

.stat-bar__item strong {
  display: block;
  font-family: var(--font-display);
  font-size: 24px;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}

.stat-bar__item span {
  display: block;
  margin-top: 5px;
  color: var(--ink-soft);
  font-size: 11px;
}

.stat-bar__item[data-tone="warning"] strong {
  color: var(--warm);
}

.stat-bar__item[data-tone="danger"] strong {
  color: var(--danger);
}
</style>
