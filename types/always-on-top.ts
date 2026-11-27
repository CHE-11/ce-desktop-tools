export type BorderColorMode = "windows" | "custom";

export type AlwaysOnTopSettings = {
  enabled: boolean;
  hotkey: string;
  skipInGameMode: boolean;
  showBorder: boolean;
  borderColorMode: BorderColorMode;
  borderColor: string;
  borderOpacity: number;
  borderThickness: number;
  roundCorners: boolean;
  playSound: boolean;
  excludedApps: string[];
};

export type AlwaysOnTopState = {
  settings: AlwaysOnTopSettings;
  accentColor: string;
  pinnedCount: number;
};
