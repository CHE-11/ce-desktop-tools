export type AsciiCode = {
  id: string;
  trigger: string;
  text: string;
  enabled: boolean;
};

export type AsciiState = {
  settingsFolder: string;
  prefix: string;
  entries: AsciiCode[];
  error: string | null;
};
