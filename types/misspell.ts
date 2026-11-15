export type Misspell = {
  id: string;
  trigger: string;
  text: string;
  enabled: boolean;
};

export type MisspellState = {
  settingsFolder: string;
  entries: Misspell[];
  error: string | null;
};
