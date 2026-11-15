export type Snippet = {
  id: string;
  trigger: string;
  text: string;
  enabled: boolean;
  hidden: boolean;
};

export type ExpansionState = {
  settingsFolder: string;
  snippets: Snippet[];
  enabled: boolean;
  locked: boolean;
  needsPin: boolean;
  error: string | null;
};
