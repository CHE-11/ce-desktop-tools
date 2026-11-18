import { invoke } from "@tauri-apps/api/core";

export function megaclipRunning() {
  return invoke<boolean>("megaclip_running");
}

export function runMegaClip() {
  return invoke<void>("run_megaclip");
}

export function stopMegaClip() {
  return invoke<void>("stop_megaclip");
}
