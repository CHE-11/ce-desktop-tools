export type AppSettings = {
  settingsFolder: string;
  settingsFile: string;
  shared: boolean;
  warning: string | null;
  fontZoom: number;
  theme: string;
  colorMode: string;
  toolbarLabels: boolean;
  runAtStartup: boolean;
};
