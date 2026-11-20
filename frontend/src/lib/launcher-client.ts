import { invoke } from "@tauri-apps/api/core";
import type { LauncherResult, LauncherSave, LauncherState } from "../../../types/launcher";

export function launcherState() {
  return invoke<LauncherState>("launcher_state");
}

export function launcherSave(settings: LauncherSave) {
  return invoke<LauncherState>("launcher_save", { settings });
}

export function launcherQuery(text: string) {
  return invoke<LauncherResult[]>("launcher_query", { text });
}

export function launcherRun(id: string) {
  return invoke<string | null>("launcher_run", { id });
}

export function launcherHide() {
  return invoke<void>("launcher_hide");
}

export function launcherShow() {
  return invoke<void>("launcher_show");
}

export function launcherRefreshApps() {
  return invoke<void>("launcher_refresh_apps");
}
