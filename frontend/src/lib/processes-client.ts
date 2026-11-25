import { invoke } from "@tauri-apps/api/core";
import type { FileLocker, PortOwner, ProcessIcon, ProcessInfo } from "../../../types/processes";

export function processList() {
  return invoke<ProcessInfo[]>("process_list");
}

/** Icons for executable paths. Paths that have no icon are left out. */
export function processIcons(paths: string[]) {
  return invoke<ProcessIcon[]>("process_icons", { paths });
}

export function processKill(pid: number, tree: boolean) {
  return invoke<string>("process_kill", { pid, tree });
}

/** `port` is a number such as `3000`, or a pattern such as `30**` where each `*` is one digit. */
export function portOwners(port: string) {
  return invoke<PortOwner[]>("port_owners", { port });
}

export function fileLockers(path: string) {
  return invoke<FileLocker[]>("file_lockers", { path });
}

export function formatBytes(bytes: number) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}
