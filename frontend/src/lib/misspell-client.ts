import { invoke } from "@tauri-apps/api/core";
import type { Misspell, MisspellState } from "../../../types/misspell";

export function misspellState() {
  return invoke<MisspellState>("misspell_state");
}

export function misspellSave(entries: Misspell[]) {
  return invoke<MisspellState>("misspell_save", { entries });
}
