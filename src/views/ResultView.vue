<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import ProgressBar from "../components/ProgressBar.vue";
import StatsBar from "../components/StatsBar.vue";
import { useScanStore } from "../stores/scan.store";

const router = useRouter();
const store = useScanStore();
const confirmRollback = ref(false);
const busy = ref(false);
const actionError = ref<string | null>(null);
const cleanupStep = ref<0 | 1 | 2>(0);
const cleanupBusy = ref(false);

const result = computed(() => store.applyResult);
const appliedCount = computed(() => {
  const value = result.value;
  return value
    ? value.archived + value.isolated + value.unresolved
    : 0;
});

const resultStats = computed(() => [
  { label: "成功归档", value: result.value?.archived ?? 0 },
  {
    label: "移入隔离区",
    value: result.value?.isolated ?? 0,
    tone: "warning" as const,
  },
  { label: "日期未知归档", value: result.value?.unresolved ?? 0 },
  {
    label: "失败",
    value: result.value?.failed.length ?? 0,
    tone: "danger" as const,
  },
  {
    label: "可释放空间",
    value: formatSize(result.value?.freed_est_bytes ?? 0),
  },
]);

onMounted(async () => {
  if (!store.runId) {
    await router.replace("/");
    return;
  }
  if (!store.applyResult) {
    try {
      await store.loadApplyResult();
    } catch {
      if (store.status !== "applying") {
        await router.replace("/plan");
      }
    }
  }
});

async function rollback() {
  if (!confirmRollback.value) {
    confirmRollback.value = true;
    return;
  }
  busy.value = true;
  actionError.value = null;
  try {
    await store.rollback();
  } catch (error) {
    actionError.value = String(error);
  } finally {
    busy.value = false;
    confirmRollback.value = false;
  }
}

async function cleanupQuarantine() {
  if (cleanupStep.value === 0) {
    cleanupStep.value = 1;
    return;
  }
  if (cleanupStep.value === 1) {
    cleanupStep.value = 2;
    return;
  }
  cleanupBusy.value = true;
  actionError.value = null;
  try {
    await store.removeQuarantine();
    cleanupStep.value = 0;
  } catch (error) {
    actionError.value = String(error);
  } finally {
    cleanupBusy.value = false;
  }
}

function newTask() {
  store.resetAll();
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
</script>

<template>
  <section class="result-page">
    <header class="result-head">
      <p class="eyebrow">TRAVEL ARCHIVE / 05</p>
      <h2 v-if="store.status === 'applying'">正在把计划落到磁盘。</h2>
      <h2 v-else-if="store.status === 'rolled_back'">这次移动已经撤回。</h2>
      <h2 v-else>旅程已经按年月落位。</h2>
      <p v-if="store.status === 'applying'">
        请保持应用窗口开启，移动完成前不要退出。
      </p>
      <p v-else-if="store.status === 'rolled_back'">
        成功恢复 {{ store.rollbackResult?.restored ?? 0 }} 个文件。
      </p>
      <p v-else>每一笔成功移动都已写入日志，可按原路径撤回。</p>
    </header>

    <article v-if="store.status === 'applying'" class="card apply-progress">
      <ProgressBar
        :value="store.applyPercent"
        :label="`${store.applyProgress?.done ?? 0} / ${store.applyProgress?.total ?? 0}`"
      />
    </article>

    <template v-else-if="store.status === 'rolled_back'">
      <StatsBar
        :items="[
          { label: '成功恢复', value: store.rollbackResult?.restored ?? 0 },
          {
            label: '恢复失败',
            value: store.rollbackResult?.failed.length ?? 0,
            tone: 'danger',
          },
        ]"
      />
      <article class="card complete-card">
        <span class="complete-card__mark">↶</span>
        <div>
          <h3>回滚完成</h3>
          <p>文件已按日志逆序移动回执行前的位置。</p>
          <code>{{ store.rollbackResult?.journal_path }}</code>
        </div>
      </article>
    </template>

    <template v-else-if="result">
      <StatsBar :items="resultStats" />

      <div class="result-grid">
        <article class="card complete-card">
          <span class="complete-card__mark">✓</span>
          <div>
            <h3>{{ appliedCount }} 个文件处理完成</h3>
            <p>
              重复文件仍完整保留在 `_duplicates/`，确认无误后再手动清理。
            </p>
            <code>{{ result.journal_path }}</code>
          </div>
        </article>

        <article class="card rollback-card">
          <span class="rollback-card__label">安全出口</span>
          <h3>撤销本次执行</h3>
          <p>
            按日志逆序恢复所有成功移动的文件。若原路径已被占用，冲突项会列在报告中。
          </p>
          <p v-if="confirmRollback" class="confirm-note">
            再次点击将立即回滚本次全部成功移动。
          </p>
          <p v-if="actionError || store.rollbackError" class="error">
            {{ actionError || store.rollbackError }}
          </p>
          <p v-if="store.quarantineDeleteResult" class="confirm-note">
            隔离区已彻底删除，本次执行不能再自动回滚。
          </p>
          <div class="rollback-card__actions">
            <button
              v-if="confirmRollback"
              class="btn btn--ghost"
              :disabled="busy"
              @click="confirmRollback = false"
            >
              取消
            </button>
            <button
              class="btn"
              :class="{ 'btn--danger': confirmRollback }"
              :disabled="
                busy || appliedCount === 0 || Boolean(store.quarantineDeleteResult)
              "
              @click="rollback"
            >
              {{ confirmRollback ? "确认回滚" : "撤销本次执行" }}
            </button>
          </div>
        </article>
      </div>

      <article v-if="result.isolated > 0 || store.quarantineDeleteResult" class="card cleanup-card">
        <div>
          <span class="cleanup-card__label">DESTRUCTIVE / 双重确认</span>
          <h3>彻底删除隔离区</h3>
          <p v-if="store.quarantineDeleteResult">
            已删除 {{ store.quarantineDeleteResult.deleted_files }} 个文件，释放
            {{ formatSize(store.quarantineDeleteResult.freed_bytes) }}。
          </p>
          <p v-else>
            建议先打开 `_duplicates/` 人工抽查。删除后无法恢复，且本次执行将失去自动回滚保障。
          </p>
          <code v-if="store.quarantineDeleteResult">
            {{ store.quarantineDeleteResult.quarantine_path }}
          </code>
        </div>
        <div v-if="!store.quarantineDeleteResult" class="cleanup-card__actions">
          <button
            v-if="cleanupStep > 0"
            class="btn btn--ghost"
            :disabled="cleanupBusy"
            @click="cleanupStep = 0"
          >
            取消
          </button>
          <button
            class="btn"
            :class="{ 'btn--danger': cleanupStep > 0 }"
            :disabled="cleanupBusy"
            @click="cleanupQuarantine"
          >
            {{
              cleanupStep === 0
                ? "清理隔离区"
                : cleanupStep === 1
                  ? "我已抽查，继续"
                  : "最终确认：彻底删除"
            }}
          </button>
        </div>
      </article>

      <article v-if="result.failed.length" class="card failures">
        <h3>异常清单（{{ result.failed.length }}）</h3>
        <div class="failure-list">
          <div v-for="failure in result.failed" :key="failure.path + failure.code">
            <span>{{ failure.code }}</span>
            <code>{{ failure.path }}</code>
            <p>{{ failure.message }}</p>
          </div>
        </div>
      </article>
    </template>

    <article v-else class="card loading-panel">正在读取执行结果…</article>

    <footer class="result-foot">
      <button class="btn btn--ghost" @click="router.push('/plan')">
        返回计划
      </button>
      <button class="btn btn--large" @click="newTask">整理另一批文件</button>
    </footer>
  </section>
</template>

<style scoped>
.result-page {
  display: grid;
  gap: 18px;
}

.result-head {
  padding: 12px 2px 3px;
}

.eyebrow {
  margin: 0 0 10px;
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.result-head h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(30px, 4.5vw, 50px);
  letter-spacing: -0.045em;
}

.result-head > p:last-child {
  margin: 12px 0 0;
  color: var(--ink-soft);
  font-size: 12px;
}

.apply-progress {
  padding: 30px;
}

.result-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.complete-card {
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.complete-card__mark {
  display: grid;
  width: 44px;
  height: 44px;
  flex: none;
  place-items: center;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-ink);
  font-size: 20px;
}

.complete-card h3,
.rollback-card h3,
.failures h3 {
  margin: 0;
  font-size: 16px;
}

.complete-card p,
.rollback-card p {
  margin: 8px 0;
  color: var(--ink-soft);
  font-size: 11px;
  line-height: 1.6;
}

.complete-card code {
  display: block;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 9px;
  overflow-wrap: anywhere;
}

.rollback-card {
  display: grid;
  align-content: start;
}

.rollback-card__label {
  margin-bottom: 8px;
  color: var(--warm);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.1em;
}

.confirm-note {
  padding: 9px 10px;
  border-radius: 8px;
  background: var(--warm-soft);
  color: #6d3c13 !important;
}

.rollback-card__actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.cleanup-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  border-color: color-mix(in srgb, var(--danger) 22%, var(--line));
}

.cleanup-card__label {
  color: var(--danger);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.09em;
}

.cleanup-card h3 {
  margin: 7px 0;
  font-size: 16px;
}

.cleanup-card p {
  max-width: 680px;
  margin: 0;
  color: var(--ink-soft);
  font-size: 11px;
  line-height: 1.6;
}

.cleanup-card code {
  display: block;
  margin-top: 8px;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 9px;
  overflow-wrap: anywhere;
}

.cleanup-card__actions {
  display: flex;
  flex: none;
  gap: 8px;
}

.failures {
  display: grid;
  gap: 14px;
}

.failure-list {
  display: grid;
  gap: 8px;
  max-height: 280px;
  overflow: auto;
}

.failure-list > div {
  padding: 11px 12px;
  border: 1px solid color-mix(in srgb, var(--danger) 24%, var(--line));
  border-radius: 9px;
  background: color-mix(in srgb, #f9e4e1 45%, var(--bg-panel));
}

.failure-list span {
  color: var(--danger);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
}

.failure-list code {
  display: block;
  margin-top: 5px;
  font-family: var(--font-mono);
  font-size: 10px;
  overflow-wrap: anywhere;
}

.failure-list p {
  margin: 5px 0 0;
  color: var(--ink-soft);
  font-size: 10px;
}

.loading-panel {
  color: var(--ink-soft);
  font-size: 12px;
  text-align: center;
}

.result-foot {
  display: flex;
  justify-content: space-between;
  padding: 14px 0;
}

@media (max-width: 760px) {
  .result-grid {
    grid-template-columns: 1fr;
  }

  .result-foot {
    align-items: stretch;
    flex-direction: column-reverse;
    gap: 10px;
  }

  .cleanup-card {
    align-items: stretch;
    flex-direction: column;
  }

  .cleanup-card__actions {
    flex-direction: column-reverse;
  }
}
</style>
