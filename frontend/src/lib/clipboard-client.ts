import { invoke } from "@tauri-apps/api/core";
import type { ClipboardState } from "../../../types/clipboard";

export function clipboardState() {
  return invoke<ClipboardState>("clipboard_state");
}

export function clipboardSetPinned(id: string, pinned: boolean) {
  return invoke<ClipboardState>("clipboard_set_pinned", { id, pinned });
}

export function clipboardDelete(id: string) {
  return invoke<ClipboardState>("clipboard_delete", { id });
}

export function clipboardCopy(id: string) {
  return invoke<void>("clipboard_copy", { id });
}
