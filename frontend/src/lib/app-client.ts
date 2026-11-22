import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "../../../types/app";

export type AppRelease = {
  version: string;
  notes: string;
};

export function appVersion() {
  return getVersion();
}

export function appSettings() {
  return invoke<AppSettings>("app_settings");
}

export function saveAppSettings(input: {
  settingsFolder: string;
  fontZoom: number;
  theme: string;
  colorMode: string;
  toolbarLabels: boolean;
  runAtStartup: boolean;
}) {
  return invoke<AppSettings>("save_app_settings", input);
}

export function previewAppSettings(settingsFolder: string) {
  return invoke<AppSettings>("preview_app_settings", { settingsFolder });
}

export function patchAppSettings(patch: { fontZoom?: number; density?: string }) {
  return invoke<void>("patch_app_settings", { patch });
}

export function themeCss() {
  return invoke<string>("theme_css");
}

export function appRelease() {
  return invoke<AppRelease>("app_release");
}

export function appExecutable() {
  return invoke<string>("app_executable");
}

export function revealPath(path: string) {
  return invoke<void>("reveal_path", { path });
}

export function openUrl(url: string) {
  return invoke<void>("open_url", { url });
}
