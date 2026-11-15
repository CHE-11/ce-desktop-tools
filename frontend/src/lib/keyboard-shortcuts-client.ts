import { invoke } from "@tauri-apps/api/core";
import type { KeyboardShortcutApp, KeyboardShortcutsState } from "../../../types/keyboard-shortcuts";

export function keyboardShortcutsState() {
  return invoke<KeyboardShortcutsState>("app_shortcuts_state");
}

export function keyboardShortcutsSave(apps: KeyboardShortcutApp[]) {
  return invoke<KeyboardShortcutsState>("app_shortcuts_save", { apps });
}
