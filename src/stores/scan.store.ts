import { computed, ref } from "vue";
import { defineStore } from "pinia";
import type {
  ApplyProgress,
  ApplyResult,
  DateSource,
  PlanItem,
  PlanSummary,
  QuarantineDeleteResult,
  RollbackResult,
  ScanOptions,
  ScanProgress,
  ScanStats,
} from "../api/types";
import {
  applyPlan,
  cancelScan,
  deleteQuarantine,
  getApplyResult,
  getPlanPage,
  getPlanSummary,
  rollbackRun,
  scan as scanCommand,
  setAllSelected,
  setItemSelected,
} from "../api/commands";
import { listenApplyEvents, listenScanEvents } from "../api/events";

export type WorkflowStatus =
  | "idle"
  | "scanning"
  | "scanned"
  | "planned"
  | "applying"
  | "applied"
  | "rolling_back"
  | "rolled_back"
  | "error";

const DEFAULT_DATE_ORDER: DateSource[] = [
  "exif_date_time_original",
  "exif_create_date",
  "video_metadata",
  "file_birth_time",
  "file_mtime",
  "filename",
];

function defaultOptions(): ScanOptions {
  return {
    source_dirs: [],
    target_dir: "",
    recursive: true,
    follow_symlinks: false,
    include_raw: true,
    include_heic: true,
    date_priority: [...DEFAULT_DATE_ORDER],
    dedup: {
      byte: true,
      similar_images: { hamming_threshold: 8 },
      similar_videos: null,
    },
    naming: { prefix_date: false },
  };
}

export const useScanStore = defineStore("scan", () => {
  const options = ref<ScanOptions>(defaultOptions());
  const runId = ref<string | null>(null);
  const status = ref<WorkflowStatus>("idle");
  const progress = ref<ScanProgress | null>(null);
  const stats = ref<ScanStats | null>(null);
  const error = ref<string | null>(null);
  const canceled = ref(false);

  const planSummary = ref<PlanSummary | null>(null);
  const planItems = ref<PlanItem[]>([]);
  const planLoading = ref(false);
  const applyProgress = ref<ApplyProgress | null>(null);
  const applyResult = ref<ApplyResult | null>(null);
  const rollbackResult = ref<RollbackResult | null>(null);
  const rollbackError = ref<string | null>(null);
  const quarantineDeleteResult = ref<QuarantineDeleteResult | null>(null);

  let unlistenScan: (() => void) | null = null;
  let unlistenApply: (() => void) | null = null;
  let scanListenersReady: Promise<void> | null = null;
  let applyListenersReady: Promise<void> | null = null;

  async function loadPlan(): Promise<void> {
    const id = runId.value;
    if (!id) throw new Error("尚未开始扫描");
    planLoading.value = true;
    try {
      planSummary.value = await getPlanSummary(id);
      const all: PlanItem[] = [];
      let cursor: string | null = null;
      do {
        const page = await getPlanPage(id, cursor, 500);
        all.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor);
      planItems.value = all;
      if (status.value === "scanned") status.value = "planned";
    } finally {
      planLoading.value = false;
    }
  }

  function ensureScanListeners(): Promise<void> {
    if (!scanListenersReady) {
      scanListenersReady = (async () => {
        unlistenScan = await listenScanEvents({
          onProgress: (p) => {
            if (runId.value && p.run_id !== runId.value) return;
            if (!runId.value) runId.value = p.run_id;
            progress.value = p;
            if (p.phase !== "done") status.value = "scanning";
          },
          onDone: async (d) => {
            if (runId.value && d.run_id !== runId.value) return;
            runId.value = d.run_id;
            stats.value = d.stats;
            canceled.value = d.canceled;
            status.value = "scanned";
            try {
              if (!d.canceled) await loadPlan();
            } catch (e) {
              error.value = String(e);
              status.value = "error";
            }
          },
          onError: (e) => {
            if (runId.value && e.run_id !== runId.value) return;
            runId.value = e.run_id;
            error.value = e.message;
            status.value = "error";
          },
        });
      })();
    }
    return scanListenersReady;
  }

  function ensureApplyListeners(): Promise<void> {
    if (!applyListenersReady) {
      applyListenersReady = (async () => {
        unlistenApply = await listenApplyEvents({
          onProgress: (p) => {
            if (!runId.value || p.run_id !== runId.value) return;
            applyProgress.value = p;
          },
          onDone: (r) => {
            if (!runId.value || r.run_id !== runId.value) return;
            applyResult.value = r;
            applyProgress.value = {
              run_id: r.run_id,
              done: r.archived + r.isolated + r.unresolved + r.failed.length,
              total: r.archived + r.isolated + r.unresolved + r.failed.length,
              phase: "done",
            };
            status.value = "applied";
          },
        });
      })();
    }
    return applyListenersReady;
  }

  async function start(): Promise<string> {
    error.value = null;
    stats.value = null;
    progress.value = null;
    canceled.value = false;
    planSummary.value = null;
    planItems.value = [];
    applyProgress.value = null;
    applyResult.value = null;
    rollbackResult.value = null;
    rollbackError.value = null;
    quarantineDeleteResult.value = null;
    status.value = "scanning";

    await ensureScanListeners();
    const started = await scanCommand(options.value);
    runId.value = started.run_id;
    return started.run_id;
  }

  async function cancel(): Promise<void> {
    if (runId.value) await cancelScan(runId.value);
  }

  async function toggleItem(itemId: string, selected: boolean): Promise<void> {
    const id = runId.value;
    if (!id) return;
    await setItemSelected(id, itemId, selected);
    const item = planItems.value.find((candidate) => candidate.id === itemId);
    if (item) item.selected = selected;
    planSummary.value = await getPlanSummary(id);
  }

  async function selectAll(selected: boolean): Promise<void> {
    const id = runId.value;
    if (!id) return;
    await setAllSelected(id, selected);
    for (const item of planItems.value) {
      if (item.action !== "keep") item.selected = selected;
    }
    planSummary.value = await getPlanSummary(id);
  }

  async function startApply(): Promise<void> {
    const id = runId.value;
    if (!id) throw new Error("尚未开始扫描");
    await ensureApplyListeners();
    status.value = "applying";
    applyProgress.value = { run_id: id, done: 0, total: 0, phase: "preparing" };
    try {
      await applyPlan(id);
    } catch (error) {
      status.value = "planned";
      applyProgress.value = null;
      throw error;
    }
  }

  async function loadApplyResult(): Promise<void> {
    if (!runId.value) return;
    applyResult.value = await getApplyResult(runId.value);
    if (applyResult.value) status.value = "applied";
  }

  async function rollback(): Promise<void> {
    const id = runId.value;
    if (!id) throw new Error("没有可撤销的任务");
    rollbackError.value = null;
    status.value = "rolling_back";
    try {
      rollbackResult.value = await rollbackRun(id);
      status.value = "rolled_back";
    } catch (e) {
      rollbackError.value = String(e);
      status.value = "applied";
      throw e;
    }
  }

  async function removeQuarantine(): Promise<void> {
    const id = runId.value;
    if (!id) throw new Error("没有可清理的任务");
    try {
      quarantineDeleteResult.value = await deleteQuarantine(id);
    } catch (error) {
      rollbackError.value = String(error);
      throw error;
    }
  }

  function resetRun(): void {
    runId.value = null;
    status.value = "idle";
    progress.value = null;
    stats.value = null;
    error.value = null;
    canceled.value = false;
    planSummary.value = null;
    planItems.value = [];
    applyProgress.value = null;
    applyResult.value = null;
    rollbackResult.value = null;
    rollbackError.value = null;
    quarantineDeleteResult.value = null;
  }

  function resetAll(): void {
    resetRun();
    options.value = defaultOptions();
  }

  function dispose(): void {
    unlistenScan?.();
    unlistenApply?.();
    unlistenScan = null;
    unlistenApply = null;
    scanListenersReady = null;
    applyListenersReady = null;
  }

  const isScanning = computed(() => status.value === "scanning");
  const isApplying = computed(() => status.value === "applying");
  const progressPercent = computed(() => {
    const p = progress.value;
    if (!p || p.total_files === 0) return 0;
    return Math.min(100, Math.round((p.processed_files / p.total_files) * 100));
  });
  const applyPercent = computed(() => {
    const p = applyProgress.value;
    if (!p || p.total === 0) return status.value === "applied" ? 100 : 0;
    return Math.min(100, Math.round((p.done / p.total) * 100));
  });

  return {
    options,
    runId,
    status,
    progress,
    stats,
    error,
    canceled,
    planSummary,
    planItems,
    planLoading,
    applyProgress,
    applyResult,
    rollbackResult,
    rollbackError,
    quarantineDeleteResult,
    isScanning,
    isApplying,
    progressPercent,
    applyPercent,
    start,
    cancel,
    loadPlan,
    toggleItem,
    selectAll,
    startApply,
    loadApplyResult,
    rollback,
    removeQuarantine,
    resetRun,
    resetAll,
    dispose,
  };
});
