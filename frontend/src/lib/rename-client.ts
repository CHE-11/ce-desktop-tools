import { invoke } from "@tauri-apps/api/core";
import type { RenamePlan, RenameRow } from "../../../types/rename";

export function previewRename(plan: RenamePlan) {
  return invoke<RenameRow[]>("preview_rename", { plan });
}

export function applyRename(rows: RenameRow[]) {
  return invoke<RenameRow[]>("apply_rename", { rows });
}
