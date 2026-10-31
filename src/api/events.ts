import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ApplyProgress,
  ApplyResult,
  ScanDone,
  ScanError,
  ScanProgress,
} from "./types";

export interface ScanEventHandlers {
  onProgress?: (payload: ScanProgress) => void;
  onDone?: (payload: ScanDone) => void;
  onError?: (payload: ScanError) => void;
}

/** 订阅扫描事件，返回统一的取消订阅函数。 */
export async function listenScanEvents(
  handlers: ScanEventHandlers,
): Promise<UnlistenFn> {
  const unlisteners = await Promise.all([
    listen<ScanProgress>("scan://progress", (e) => handlers.onProgress?.(e.payload)),
    listen<ScanDone>("scan://done", (e) => handlers.onDone?.(e.payload)),
    listen<ScanError>("scan://error", (e) => handlers.onError?.(e.payload)),
  ]);

  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}

export interface ApplyEventHandlers {
  onProgress?: (payload: ApplyProgress) => void;
  onDone?: (payload: ApplyResult) => void;
}

export async function listenApplyEvents(
  handlers: ApplyEventHandlers,
): Promise<UnlistenFn> {
  const unlisteners = await Promise.all([
    listen<ApplyProgress>("apply://progress", (e) =>
      handlers.onProgress?.(e.payload),
    ),
    listen<ApplyResult>("apply://done", (e) => handlers.onDone?.(e.payload)),
  ]);
  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}
