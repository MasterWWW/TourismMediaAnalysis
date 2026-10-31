import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyResult,
  MediaFile,
  PlanPage,
  PlanSummary,
  QuarantineDeleteResult,
  RollbackResult,
  RunStarted,
  ScanOptions,
  ScanStatus,
  ToolStatus,
} from "./types";

/** 是否运行在 Tauri 容器内（浏览器预览时 invoke 不可用）。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function assertTauri(): void {
  if (!isTauri()) {
    throw new Error("当前不在 Tauri 运行环境，请用 `pnpm tauri dev` 启动桌面应用。");
  }
}

export async function scan(options: ScanOptions): Promise<RunStarted> {
  assertTauri();
  return invoke<RunStarted>("scan", { options });
}

export async function cancelScan(runId: string): Promise<void> {
  assertTauri();
  return invoke("cancel_scan", { runId });
}

export async function getScanStatus(runId: string): Promise<ScanStatus> {
  assertTauri();
  return invoke<ScanStatus>("get_scan_status", { runId });
}

export async function getInventory(runId: string): Promise<MediaFile[]> {
  assertTauri();
  return invoke<MediaFile[]>("get_inventory", { runId });
}

export async function getPlanSummary(runId: string): Promise<PlanSummary> {
  assertTauri();
  return invoke<PlanSummary>("get_plan_summary", { runId });
}

export async function getPlanPage(
  runId: string,
  cursor: string | null,
  pageSize = 500,
): Promise<PlanPage> {
  assertTauri();
  return invoke<PlanPage>("get_plan_page", { runId, cursor, pageSize });
}

export async function setItemSelected(
  runId: string,
  itemId: string,
  selected: boolean,
): Promise<void> {
  assertTauri();
  return invoke("set_item_selected", { runId, itemId, selected });
}

export async function setAllSelected(runId: string, selected: boolean): Promise<void> {
  assertTauri();
  return invoke("set_all_selected", { runId, selected });
}

export async function applyPlan(runId: string): Promise<RunStarted> {
  assertTauri();
  return invoke<RunStarted>("apply", { runId });
}

export async function getApplyResult(runId: string): Promise<ApplyResult> {
  assertTauri();
  return invoke<ApplyResult>("get_apply_result", { runId });
}

export async function rollbackRun(runId: string): Promise<RollbackResult> {
  assertTauri();
  return invoke<RollbackResult>("rollback", { runId });
}

export async function getToolStatus(): Promise<ToolStatus> {
  assertTauri();
  return invoke<ToolStatus>("get_tool_status");
}

export async function setToolPaths(
  exiftoolPath: string | null,
  ffprobePath: string | null,
): Promise<ToolStatus> {
  assertTauri();
  return invoke<ToolStatus>("set_tool_paths", { exiftoolPath, ffprobePath });
}

export async function deleteQuarantine(
  runId: string,
): Promise<QuarantineDeleteResult> {
  assertTauri();
  return invoke<QuarantineDeleteResult>("delete_quarantine", {
    runId,
    confirmation: "DELETE_QUARANTINE",
  });
}
