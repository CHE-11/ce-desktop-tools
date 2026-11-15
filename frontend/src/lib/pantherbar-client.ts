import { invoke } from "@tauri-apps/api/core";
import type { PantherbarSave, PantherbarState } from "../../../types/pantherbar";

export function pantherbarState() {
  return invoke<PantherbarState>("pantherbar_state");
}

export function pantherbarSave(settings: PantherbarSave) {
  return invoke<PantherbarState>("pantherbar_save", { settings });
}

export function pantherbarRun(id: string) {
  return invoke<string | null>("pantherbar_run", { id });
}

export function pantherbarFit(width: number, height: number) {
  return invoke<void>("pantherbar_fit", { width, height });
}
