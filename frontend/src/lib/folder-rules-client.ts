import { invoke } from "@tauri-apps/api/core";
import type { FolderRuleActivity, FolderRulesState, WatchedFolder } from "../../../types/folder-rules";

export function folderRulesState() {
  return invoke<FolderRulesState>("folder_rules_state");
}

export function folderRulesSave(enabled: boolean, folders: WatchedFolder[]) {
  return invoke<FolderRulesState>("folder_rules_save", { enabled, folders });
}

export function folderRulesRun(folderId: string | null, dryRun: boolean) {
  return invoke<FolderRuleActivity[]>("folder_rules_run", { folderId, dryRun });
}

export function folderRulesActivity() {
  return invoke<FolderRuleActivity[]>("folder_rules_activity");
}
