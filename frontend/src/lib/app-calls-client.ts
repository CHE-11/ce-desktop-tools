import { invoke } from "@tauri-apps/api/core";
import type { AppCallsSnapshot } from "../../../types/app-calls";

export function appCalls() {
  return invoke<AppCallsSnapshot>("app_calls");
}
