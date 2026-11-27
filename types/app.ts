export type AppSettings = {
  settingsFolder: string;
  settingsFile: string;
  shared: boolean;
  warning: string | null;
  fontZoom: number;
  density: string;
  theme: string;
  colorMode: string;
  toolbarLabels: boolean;
  runAtStartup: boolean;
};

export type TerminalScript = {
  id: string;
  name: string;
  body: string;
};

export type TerminalSettings = {
  defaultShell: "cmd" | "powershell" | "pwsh";
  copyOnSelect: boolean;
  confirmMultilinePaste: boolean;
  fontSize: number;
  showScripts: boolean;
  scripts: TerminalScript[];
};
