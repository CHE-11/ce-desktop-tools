import { invoke } from "@tauri-apps/api/core";

/** While a shortcut field is recording, global hotkeys pass through to it. */
export function setHotkeyRecording(on: boolean) {
  return invoke<void>("hotkeys_recording", { on });
}
