import { invoke } from "@tauri-apps/api/core";
import type { AlwaysOnTopSettings, AlwaysOnTopState } from "../../../types/always-on-top";

export function alwaysOnTopState() {
  return invoke<AlwaysOnTopState>("always_on_top_state");
}

export function alwaysOnTopSave(settings: AlwaysOnTopSettings) {
  return invoke<AlwaysOnTopState>("always_on_top_save", { settings });
}

export function alwaysOnTopUnpinAll() {
  return invoke<AlwaysOnTopState>("always_on_top_unpin_all");
}
