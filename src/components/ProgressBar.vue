<script setup lang="ts">
defineProps<{
  value: number;
  label?: string;
  tone?: "accent" | "warm";
  indeterminate?: boolean;
}>();
</script>

<template>
  <div class="progress">
    <div
      class="progress__track"
      role="progressbar"
      :aria-valuenow="value"
      aria-valuemin="0"
      aria-valuemax="100"
    >
      <div
        class="progress__fill"
        :class="{
          'progress__fill--warm': tone === 'warm',
          'progress__fill--indeterminate': indeterminate,
        }"
        :style="
          indeterminate
            ? undefined
            : { width: `${Math.max(0, Math.min(100, value))}%` }
        "
      />
    </div>
    <div class="progress__meta">
      <span>{{ label }}</span>
      <strong>{{ indeterminate ? "扫描中" : `${value}%` }}</strong>
    </div>
  </div>
</template>

<style scoped>
.progress__track {
  height: 10px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--bg-soft);
}

.progress__fill {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--accent), #3dbd91);
  transition: width 240ms ease;
}

.progress__fill--indeterminate {
  width: 34%;
  animation: indeterminate 1.25s ease-in-out infinite;
}

.progress__fill--warm {
  background: linear-gradient(90deg, #c46e2d, #e1a34e);
}

@keyframes indeterminate {
  from {
    transform: translateX(-110%);
  }

  to {
    transform: translateX(300%);
  }
}

.progress__meta {
  display: flex;
  justify-content: space-between;
  margin-top: 8px;
  color: var(--ink-soft);
  font-size: 12px;
}

.progress__meta strong {
  color: var(--ink);
  font-variant-numeric: tabular-nums;
}
</style>
