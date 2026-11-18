export type LauncherResultKind = "calc" | "unit" | "app" | "tool" | "action" | "process" | "info";

export type LauncherResult = {
  id: string;
  kind: LauncherResultKind;
  title: string;
  subtitle: string;
  icon: string;
};

export type LauncherState = {
  enabled: boolean;
  hotkey: string;
  appCount: number;
};

export type LauncherSave = {
  enabled: boolean;
  hotkey: string;
};
