export type DateExpand = {
  id: string;
  trigger: string;
  template: string;
  enabled: boolean;
  preview: string;
};

export type DateExpandState = {
  settingsFolder: string;
  prefix: string;
  entries: DateExpand[];
  error: string | null;
};
