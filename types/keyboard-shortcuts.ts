export type KeyboardShortcut = {
  id: string;
  title: string;
  hotkey: string;
  detail: string;
  action: string;
  enabled: boolean;
};

export type KeyboardShortcutApp = {
  id: string;
  name: string;
  exe: string;
  shortcuts: KeyboardShortcut[];
};

export type KeyboardShortcutsState = {
  settingsFolder: string;
  apps: KeyboardShortcutApp[];
  error: string | null;
};
