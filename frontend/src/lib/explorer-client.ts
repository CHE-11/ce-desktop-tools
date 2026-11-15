import { invoke } from "@tauri-apps/api/core";

export function restartExplorer() {
  return invoke<void>("restart_explorer");
}
