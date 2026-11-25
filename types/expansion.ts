export type Snippet = {
  id: string;
  trigger: string;
  text: string;
  enabled: boolean;
  hidden: boolean;
};

/** What the Windows keyboard hook has been doing. Holds no keys. Times are Unix milliseconds. */
export type ExpansionHookHealth = {
  installs: number;
  lastInstallError: string | null;
  lastRehookReason: "start" | "timer" | "session" | "resume" | "missing" | "deadHook" | "stall" | null;
  lastRehookMs: number | null;
  lastHookKeyMs: number | null;
};

export type ExpansionState = {
  settingsFolder: string;
  snippets: Snippet[];
  enabled: boolean;
  locked: boolean;
  needsPin: boolean;
  error: string | null;
  lockSeconds: number | null;
  hook?: ExpansionHookHealth | null;
};
