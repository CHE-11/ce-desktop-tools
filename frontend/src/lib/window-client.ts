import { invoke } from "@tauri-apps/api/core";

export function alwaysOnTop() {
  return invoke<boolean>("always_on_top");
}

export function setAlwaysOnTop(on: boolean) {
  return invoke<boolean>("set_always_on_top", { on });
}
