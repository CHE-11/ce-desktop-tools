export type FolderRuleAction = "move" | "copy" | "recycle" | "rename";

export type FolderRuleConflict = "number" | "skip" | "overwrite";

export type FolderRuleMatch = {
  extensions: string[];
  nameContains: string;
  olderThanDays: number;
  minSizeKb: number;
};

export type FolderRule = {
  id: string;
  name: string;
  enabled: boolean;
  match: FolderRuleMatch;
  action: FolderRuleAction;
  target: string;
  renamePattern: string;
  onConflict: FolderRuleConflict;
};

export type WatchedFolder = {
  id: string;
  path: string;
  enabled: boolean;
  rules: FolderRule[];
};

export type FolderRulesState = {
  settingsFolder: string;
  enabled: boolean;
  folders: WatchedFolder[];
  error: string | null;
};

export type FolderRuleOutcome = "done" | "planned" | "skipped" | "error";

export type FolderRuleActivity = {
  at: number;
  folder: string;
  file: string;
  rule: string;
  action: string;
  outcome: FolderRuleOutcome;
  to: string;
  detail: string;
};
