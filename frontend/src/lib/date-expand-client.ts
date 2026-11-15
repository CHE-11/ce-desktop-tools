import { invoke } from "@tauri-apps/api/core";
import type { DateExpand, DateExpandState } from "../../../types/date-expand";

export function dateState() {
  return invoke<DateExpandState>("date_state");
}

export function dateSave(entries: Array<Pick<DateExpand, "id" | "trigger" | "template" | "enabled">>, prefix: string) {
  return invoke<DateExpandState>("date_save", { entries, prefix });
}
