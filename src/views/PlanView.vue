<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import type { PlanItem } from "../api/types";
import DuplicateGroup from "../components/DuplicateGroup.vue";
import FileTreeItem from "../components/FileTreeItem.vue";
import StatsBar from "../components/StatsBar.vue";
import { useScanStore } from "../stores/scan.store";

interface MonthBucket {
  key: string;
  label: string;
  items: PlanItem[];
}

interface YearBucket {
  key: string;
  label: string;
  count: number;
  months: MonthBucket[];
}

const router = useRouter();
const store = useScanStore();
const filter = ref<"all" | "duplicates" | "unresolved">("all");
const confirming = ref(false);
const actionError = ref<string | null>(null);
const busy = ref(false);
const monthLimits = ref<Record<string, number>>({});

const filteredItems = computed(() => {
  if (filter.value === "duplicates") {
    return store.planItems.filter(
      (item) => item.group_id || item.action === "isolate",
    );
  }
  if (filter.value === "unresolved") {
    return store.planItems.filter((item) => item.action === "unresolved");
  }
  return store.planItems;
});

const tree = computed<YearBucket[]>(() => {
  const years = new Map<string, Map<string, MonthBucket>>();
  for (const item of filteredItems.value) {
    const parsed = item.date ? new Date(item.date) : null;
    const year = parsed ? String(parsed.getUTCFullYear()) : "未确定";
    const month = parsed
      ? `${year}-${String(parsed.getUTCMonth() + 1).padStart(2, "0")}`
      : "unresolved";
    if (!years.has(year)) years.set(year, new Map());
    const months = years.get(year)!;
    if (!months.has(month)) {
      months.set(month, {
        key: month,
        label: month === "unresolved" ? "日期未知" : month,
        items: [],
      });
    }
    months.get(month)!.items.push(item);
  }

  return [...years.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([year, monthMap]) => {
      const months = [...monthMap.values()]
        .sort((a, b) => a.key.localeCompare(b.key))
        .map((month) => ({
          ...month,
          items: month.items.sort((a, b) =>
            a.source_path.localeCompare(b.source_path),
          ),
        }));
      return {
        key: year,
        label: year === "未确定" ? year : `${year} 年`,
        count: months.reduce((sum, month) => sum + month.items.length, 0),
        months,
      };
    });
});

const duplicateGroups = computed(() => {
  const groups = new Map<string, PlanItem[]>();
  for (const item of store.planItems) {
    if (!item.group_id) continue;
    const list = groups.get(item.group_id) ?? [];
    list.push(item);
    groups.set(item.group_id, list);
  }
  return [...groups.entries()]
    .filter(([, items]) => items.length > 1)
    .sort(([a], [b]) => a.localeCompare(b));
});

const stats = computed(() => [
  {
    label: "已选择处理",
    value: store.planSummary?.items_selected ?? 0,
  },
  {
    label: "归档",
    value: store.planSummary?.archive_count ?? 0,
  },
  {
    label: "隔离重复",
    value: store.planSummary?.isolate_count ?? 0,
    tone: "warning" as const,
  },
  {
    label: "日期未知",
    value: store.planSummary?.unresolved_count ?? 0,
  },
  {
    label: "预计腾出",
    value: formatSize(store.planSummary?.freed_est_bytes ?? 0),
  },
]);

onMounted(async () => {
  if (!store.runId) {
    await router.replace("/");
    return;
  }
  if (store.planItems.length === 0) {
    try {
      await store.loadPlan();
    } catch (error) {
      actionError.value = String(error);
    }
  }
});

async function toggle(itemId: string, selected: boolean) {
  busy.value = true;
  actionError.value = null;
  try {
    await store.toggleItem(itemId, selected);
  } catch (error) {
    actionError.value = String(error);
  } finally {
    busy.value = false;
  }
}

async function selectAll(selected: boolean) {
  busy.value = true;
  actionError.value = null;
  try {
    await store.selectAll(selected);
  } catch (error) {
    actionError.value = String(error);
  } finally {
    busy.value = false;
  }
}

async function execute() {
  if (!confirming.value) {
    confirming.value = true;
    return;
  }
  busy.value = true;
  actionError.value = null;
  try {
    await store.startApply();
    await router.push("/result");
  } catch (error) {
    actionError.value = String(error);
    confirming.value = false;
  } finally {
    busy.value = false;
  }
}

async function backToConfigure() {
  if (busy.value) return;
  confirming.value = false;
  await router.push("/configure");
}

function abandonRun() {
  if (busy.value) return;
  confirming.value = false;
  store.resetRun();
  router.push("/");
}

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

function visibleItems(month: MonthBucket): PlanItem[] {
  const limit = monthLimits.value[month.key] ?? 60;
  return month.items.slice(0, limit);
}

function showMore(month: MonthBucket) {
  const limit = monthLimits.value[month.key] ?? 60;
  monthLimits.value[month.key] = limit + 100;
}
</script>

<template>
  <section class="plan-page">
    <header class="plan-head">
      <div>
        <p class="eyebrow">TRAVEL ARCHIVE / 04</p>
        <h2>先看一眼每一张会去哪里。</h2>
        <p>这是 dry-run 预览，尚未移动任何文件。</p>
      </div>
      <div class="plan-head__side">
        <div class="plan-head__meta">
          <span>RUN</span>
          <code>{{ store.runId }}</code>
        </div>
        <div class="plan-head__actions">
          <button class="btn btn--ghost" :disabled="busy" @click="backToConfigure">
            返回修改规则
          </button>
          <button class="btn btn--ghost" :disabled="busy" @click="abandonRun">
            放弃本次会话
          </button>
        </div>
      </div>
    </header>

    <StatsBar :items="stats" />

    <div class="plan-toolbar card">
      <div class="filter-tabs">
        <button :class="{ active: filter === 'all' }" @click="filter = 'all'">
          全部 {{ store.planItems.length }}
        </button>
        <button
          :class="{ active: filter === 'duplicates' }"
          @click="filter = 'duplicates'"
        >
          重复与相似
        </button>
        <button
          :class="{ active: filter === 'unresolved' }"
          @click="filter = 'unresolved'"
        >
          日期未知
        </button>
      </div>
      <div class="selection-actions">
        <button :disabled="busy" @click="selectAll(true)">全选</button>
        <button :disabled="busy" @click="selectAll(false)">全不选</button>
      </div>
    </div>

    <div v-if="store.planLoading" class="card loading-panel">
      正在加载整理计划…
    </div>

    <div v-else class="plan-grid">
      <article class="card tree-card">
        <div class="tree-card__head">
          <div>
            <h3>归档树</h3>
            <p>展开年份和月份查看来源与目标。</p>
          </div>
          <span class="dry-badge">DRY RUN</span>
        </div>

        <div v-if="tree.length === 0" class="empty-plan">
          当前筛选条件下没有文件。
        </div>

        <details
          v-for="(year, yearIndex) in tree"
          :key="year.key"
          class="year-group"
          :open="yearIndex === 0"
        >
          <summary>
            <span class="year-group__marker" />
            <strong>{{ year.label }}</strong>
            <span>{{ year.count }} 个文件</span>
          </summary>
          <details
            v-for="month in year.months"
            :key="month.key"
            class="month-group"
            open
          >
            <summary>
              <span>{{ month.label }}</span>
              <span>{{ month.items.length }}</span>
            </summary>
            <FileTreeItem
              v-for="item in visibleItems(month)"
              :key="item.id"
              :item="item"
              :busy="busy"
              @toggle="toggle"
            />
            <button
              v-if="visibleItems(month).length < month.items.length"
              class="show-more"
              @click="showMore(month)"
            >
              继续显示剩余 {{ month.items.length - visibleItems(month).length }} 个
            </button>
          </details>
        </details>
      </article>

      <aside class="plan-side">
        <article class="card">
          <h3>重复分组</h3>
          <p class="side-note">
            每组保留一项归档，其余进入 `_duplicates/`。可对照预览，勾选要隔离的文件。
          </p>
          <div v-if="duplicateGroups.length" class="dupe-list">
            <DuplicateGroup
              v-for="[groupId, items] in duplicateGroups.slice(0, 30)"
              :key="groupId"
              :group-id="groupId"
              :items="items"
              :busy="busy"
              @toggle="toggle"
            />
            <p v-if="duplicateGroups.length > 30" class="side-note">
              另有 {{ duplicateGroups.length - 30 }} 组未展开。
            </p>
          </div>
          <p v-else class="empty-plan">没有发现重复分组。</p>
        </article>

        <article class="card commit-card">
          <span class="commit-card__label">将执行</span>
          <strong>{{ store.planSummary?.items_selected ?? 0 }} 个文件</strong>
          <p>
            其中归档 {{ store.planSummary?.archive_count ?? 0 }} 个，隔离
            {{ store.planSummary?.isolate_count ?? 0 }} 个。
          </p>
          <p v-if="confirming" class="confirm-note">
            再次点击将真实移动文件；每次移动都会写入可撤销日志。
          </p>
          <p v-if="actionError" class="error">{{ actionError }}</p>
          <div class="commit-card__actions">
            <button
              v-if="confirming"
              class="btn btn--ghost"
              :disabled="busy"
              @click="confirming = false"
            >
              再检查一下
            </button>
            <button
              class="btn"
              :class="{ 'btn--danger': confirming }"
              :disabled="
                busy ||
                store.status === 'applied' ||
                store.status === 'rolled_back' ||
                !(store.planSummary?.items_selected ?? 0)
              "
              @click="execute"
            >
              {{
                store.status === "applied"
                  ? "本次已执行"
                  : confirming
                    ? "确认并执行"
                    : "执行整理"
              }}
            </button>
          </div>
        </article>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.plan-page {
  display: grid;
  gap: 18px;
}

.plan-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 24px;
  padding: 10px 2px 4px;
}

.eyebrow {
  margin: 0 0 10px;
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.plan-head h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(28px, 4vw, 46px);
  letter-spacing: -0.045em;
}

.plan-head p:last-child {
  margin: 10px 0 0;
  color: var(--ink-soft);
  font-size: 12px;
}

.plan-head__meta {
  max-width: 280px;
  text-align: right;
}

.plan-head__side {
  display: grid;
  justify-items: end;
  gap: 10px;
}

.plan-head__meta span {
  display: block;
  margin-bottom: 4px;
  color: var(--ink-soft);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.plan-head__meta code {
  display: block;
  overflow: hidden;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
}

.plan-head__actions {
  display: flex;
  gap: 8px;
}

.plan-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px;
}

.filter-tabs {
  display: flex;
  gap: 4px;
}

.filter-tabs button,
.selection-actions button {
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--ink-soft);
  font-size: 11px;
  cursor: pointer;
}

.filter-tabs button {
  padding: 8px 11px;
}

.filter-tabs button.active {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 700;
}

.selection-actions {
  display: flex;
  gap: 4px;
}

.selection-actions button {
  padding: 8px 9px;
  border: 1px solid var(--line);
}

.selection-actions button:disabled {
  opacity: 0.35;
}

.plan-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.65fr) minmax(280px, 0.75fr);
  align-items: start;
  gap: 16px;
}

.tree-card {
  padding: 0;
  overflow: hidden;
}

.tree-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 20px;
  border-bottom: 1px solid var(--line);
}

.tree-card h3,
.plan-side h3 {
  margin: 0;
  font-size: 15px;
}

.tree-card__head p,
.side-note {
  margin: 5px 0 0;
  color: var(--ink-soft);
  font-size: 10px;
  line-height: 1.55;
}

.dry-badge {
  padding: 5px 8px;
  border: 1px solid color-mix(in srgb, var(--warm) 45%, var(--line));
  border-radius: 999px;
  color: var(--warm);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.year-group > summary {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  background: var(--bg-soft);
  font-size: 12px;
  cursor: pointer;
  list-style: none;
}

.year-group > summary span:last-child {
  margin-left: auto;
  color: var(--ink-soft);
  font-size: 10px;
}

.year-group__marker {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
}

.month-group > summary {
  display: flex;
  justify-content: space-between;
  padding: 10px 18px 10px 34px;
  border-bottom: 1px solid var(--line);
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 10px;
  cursor: pointer;
  list-style: none;
}

.month-group > summary::-webkit-details-marker,
.year-group > summary::-webkit-details-marker {
  display: none;
}

.show-more {
  width: 100%;
  padding: 10px;
  border: 0;
  border-bottom: 1px solid var(--line);
  background: var(--bg-soft);
  color: var(--accent);
  font-size: 10px;
  cursor: pointer;
}

.plan-side {
  display: grid;
  gap: 16px;
}

.dupe-list {
  max-height: 440px;
  margin-top: 14px;
  overflow: auto;
}

.empty-plan {
  padding: 28px 18px;
  color: var(--ink-soft);
  font-size: 11px;
  text-align: center;
}

.loading-panel {
  color: var(--ink-soft);
  font-size: 12px;
  text-align: center;
}

.commit-card {
  display: grid;
  gap: 8px;
  background:
    linear-gradient(150deg, color-mix(in srgb, var(--accent-soft) 72%, white), var(--bg-panel));
}

.commit-card__label {
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.1em;
}

.commit-card strong {
  font-family: var(--font-display);
  font-size: 30px;
  letter-spacing: -0.03em;
}

.commit-card p {
  margin: 0;
  color: var(--ink-soft);
  font-size: 11px;
  line-height: 1.55;
}

.confirm-note {
  padding: 9px 10px;
  border-radius: 8px;
  background: var(--warm-soft);
  color: #6d3c13 !important;
}

.commit-card__actions {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.commit-card__actions .btn {
  flex: 1;
}

@media (max-width: 900px) {
  .plan-grid {
    grid-template-columns: 1fr;
  }

  .plan-side {
    grid-row: 1;
  }
}

@media (max-width: 680px) {
  .plan-head,
  .plan-toolbar {
    align-items: stretch;
    flex-direction: column;
  }

  .plan-head__meta {
    text-align: left;
  }

  .plan-head__side {
    text-align: left;
    justify-items: start;
  }

  .filter-tabs {
    overflow-x: auto;
  }
}
</style>
