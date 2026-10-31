import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { ApplyEventHandlers, ScanEventHandlers } from "../api/events";
import type { PlanItem, PlanSummary, ScanStats } from "../api/types";

const mocks = vi.hoisted(() => ({
  scan: vi.fn(),
  cancelScan: vi.fn(),
  getPlanSummary: vi.fn(),
  getPlanPage: vi.fn(),
  setItemSelected: vi.fn(),
  setAllSelected: vi.fn(),
  applyPlan: vi.fn(),
  getApplyResult: vi.fn(),
  rollbackRun: vi.fn(),
  deleteQuarantine: vi.fn(),
  listenScanEvents: vi.fn(),
  listenApplyEvents: vi.fn(),
}));

vi.mock("../api/commands", () => ({
  scan: mocks.scan,
  cancelScan: mocks.cancelScan,
  getPlanSummary: mocks.getPlanSummary,
  getPlanPage: mocks.getPlanPage,
  setItemSelected: mocks.setItemSelected,
  setAllSelected: mocks.setAllSelected,
  applyPlan: mocks.applyPlan,
  getApplyResult: mocks.getApplyResult,
  rollbackRun: mocks.rollbackRun,
  deleteQuarantine: mocks.deleteQuarantine,
}));

vi.mock("../api/events", () => ({
  listenScanEvents: mocks.listenScanEvents,
  listenApplyEvents: mocks.listenApplyEvents,
}));

import { useScanStore } from "./scan.store";

const stats: ScanStats = {
  total_files: 4,
  media_found: 3,
  duplicate_groups: 1,
  similar_groups: 0,
  unresolved: 1,
};

const summary: PlanSummary = {
  run_id: "run-1",
  items_total: 2,
  items_selected: 2,
  archive_count: 1,
  isolate_count: 1,
  unresolved_count: 0,
  keep_count: 0,
  freed_est_bytes: 10,
  duplicate_groups: 1,
  similar_groups: 0,
};

const planItem: PlanItem = {
  id: "a",
  source_path: "/source/a.jpg",
  target_path: "/target/2024/2024-05/a.jpg",
  action: "archive",
  reason: "dated",
  group_id: null,
  kind: "photo",
  size_bytes: 10,
  date: "2024-05-01T12:00:00Z",
  selected: true,
};

describe("scan workflow store", () => {
  let scanHandlers: ScanEventHandlers;
  let applyHandlers: ApplyEventHandlers;

  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.scan.mockResolvedValue({ run_id: "run-1" });
    mocks.cancelScan.mockResolvedValue(undefined);
    mocks.getPlanSummary.mockResolvedValue(summary);
    mocks.getPlanPage.mockResolvedValue({ items: [planItem], next_cursor: null });
    mocks.setItemSelected.mockResolvedValue(undefined);
    mocks.setAllSelected.mockResolvedValue(undefined);
    mocks.applyPlan.mockResolvedValue({ run_id: "run-1" });
    mocks.getApplyResult.mockResolvedValue(null);
    mocks.rollbackRun.mockResolvedValue({
      run_id: "run-1",
      restored: 1,
      failed: [],
      journal_path: "/journal.json",
    });
    mocks.deleteQuarantine.mockResolvedValue({
      run_id: "run-1",
      deleted_files: 1,
      freed_bytes: 10,
      quarantine_path: "/target/_duplicates",
    });
    mocks.listenScanEvents.mockImplementation(
      async (handlers: ScanEventHandlers) => {
        scanHandlers = handlers;
        return () => {};
      },
    );
    mocks.listenApplyEvents.mockImplementation(
      async (handlers: ApplyEventHandlers) => {
        applyHandlers = handlers;
        return () => {};
      },
    );
  });

  it("moves from scan completion to a loaded plan", async () => {
    const store = useScanStore();
    await store.start();
    expect(store.status).toBe("scanning");
    expect(store.runId).toBe("run-1");

    await scanHandlers.onDone?.({
      run_id: "run-1",
      stats,
      canceled: false,
    });
    await vi.waitFor(() => expect(store.status).toBe("planned"));
    expect(store.planItems).toHaveLength(1);
    expect(store.planSummary?.isolate_count).toBe(1);
  });

  it("persists item selection through the command boundary", async () => {
    const store = useScanStore();
    await store.start();
    await scanHandlers.onDone?.({ run_id: "run-1", stats, canceled: false });
    await store.toggleItem("a", false);
    expect(mocks.setItemSelected).toHaveBeenCalledWith("run-1", "a", false);
    expect(store.planItems[0].selected).toBe(false);
  });

  it("tracks apply progress and then applies the result", async () => {
    const store = useScanStore();
    await store.start();
    await store.startApply();
    expect(store.status).toBe("applying");
    applyHandlers.onProgress?.({
      run_id: "run-1",
      done: 1,
      total: 2,
      phase: "moving",
    });
    expect(store.applyPercent).toBe(50);
    applyHandlers.onDone?.({
      run_id: "run-1",
      archived: 1,
      isolated: 1,
      unresolved: 0,
      freed_est_bytes: 10,
      failed: [],
      journal_path: "/journal.json",
      canceled: false,
    });
    expect(store.status).toBe("applied");
    expect(store.applyResult?.isolated).toBe(1);
  });

  it("returns to the plan state when apply cannot start", async () => {
    mocks.applyPlan.mockRejectedValueOnce(new Error("disk unavailable"));
    const store = useScanStore();
    await store.start();
    await expect(store.startApply()).rejects.toThrow("disk unavailable");
    expect(store.status).toBe("planned");
    expect(store.applyProgress).toBeNull();
  });
});
