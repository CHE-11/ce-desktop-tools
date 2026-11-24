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
};

export type TerminalOutputEvent = {
  id: string;
  data: string;
};

export type TerminalExitEvent = {
  id: string;
  code: number | null;
};
