import { invoke } from "@tauri-apps/api/core";
import type { ClipCleaner, ClipCleanerPreview, ClipboardState } from "../../../types/clipboard";

export function clipboardState() {
  return invoke<ClipboardState>("clipboard_state");
}

export function clipboardDelete(id: string) {
  return invoke<ClipboardState>("clipboard_delete", { id });
}

export function clipboardCopy(id: string) {
  return invoke<void>("clipboard_copy", { id });
}

export function clipboardClear() {
  return invoke<ClipboardState>("clipboard_clear");
}

export function clipboardClearWindows() {
  return invoke<void>("clipboard_clear_windows");
}

export function clipboardCleanerState() {
  return invoke<ClipCleaner>("clipboard_cleaner_state");
}

export function clipboardCleanerSave(cleaner: ClipCleaner) {
  return invoke<ClipCleaner>("clipboard_cleaner_save", { cleaner });
}

export function clipboardCleanerPreview(text: string, cleaner: ClipCleaner) {
  return invoke<ClipCleanerPreview>("clipboard_cleaner_preview", { text, cleaner });
}
