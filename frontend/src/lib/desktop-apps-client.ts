import { invoke } from "@tauri-apps/api/core";
import type { DesktopApp } from "../../../types/desktop-apps";

export function desktopApps() {
  return invoke<DesktopApp[]>("desktop_apps");
}

export function desktopAppAdd(name: string, appId: string) {
  return invoke<string>("desktop_app_add", { name, appId });
}
