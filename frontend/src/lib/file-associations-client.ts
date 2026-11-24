import { invoke } from "@tauri-apps/api/core";
import type { ExtensionHandler, FileAssociation, FileAssociationsState } from "../../../types/file-associations";

export function fileAssocList() {
  return invoke<FileAssociationsState>("file_assoc_list");
}

export function fileAssocApply(association: FileAssociation) {
  return invoke<FileAssociationsState>("file_assoc_apply", { association });
}

export function fileAssocRemove(id: string) {
  return invoke<FileAssociationsState>("file_assoc_remove", { id });
}

export function fileAssocLookup(extension: string) {
  return invoke<ExtensionHandler>("file_assoc_lookup", { extension });
}

export function fileAssocPreview(programPath: string, args: string, sample: string) {
  return invoke<string>("file_assoc_preview", { programPath, arguments: args, sample });
}

export function fileAssocOpenDefaultApps() {
  return invoke<void>("file_assoc_open_default_apps");
}
