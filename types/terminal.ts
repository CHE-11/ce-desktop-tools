export type ShellKind = "cmd" | "powershell" | "pwsh";

export type ShellInfo = {
  id: ShellKind;
  label: string;
  path: string;
};

export type TerminalSession = {
  id: string;
  shell: ShellKind;
  cwd: string;
  title: string;
  admin: boolean;
};

/** In-app tab, or an elevated console opened in its own window after UAC. */
export type TerminalSpawnResult =
  | { kind: "session"; session: TerminalSession }
  | { kind: "externalWindow" };

export type TerminalOutputEvent = {
  id: string;
  data: string;
};

export type TerminalExitEvent = {
  id: string;
  code: number | null;
};
