import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TerminalSettings } from "../../../types/app";
import type {
  ShellInfo,
  ShellKind,
  TerminalExitEvent,
  TerminalOutputEvent,
  TerminalSession,
  TerminalSpawnResult,
} from "../../../types/terminal";

export function terminalShells() {
  return invoke<ShellInfo[]>("terminal_shells");
}

export function terminalSpawn(
  shell: ShellKind,
  cwd: string | null,
  cols: number,
  rows: number,
  admin = false,
) {
  return invoke<TerminalSpawnResult>("terminal_spawn", { shell, cwd, cols, rows, admin });
}

export function terminalWrite(id: string, data: string) {
  return invoke<void>("terminal_write", { id, data });
}

export function terminalResize(id: string, cols: number, rows: number) {
  return invoke<void>("terminal_resize", { id, cols, rows });
}

export function terminalKill(id: string) {
  return invoke<void>("terminal_kill", { id });
}

export function terminalList() {
  return invoke<TerminalSession[]>("terminal_list");
}

export function terminalClipboardText() {
  return invoke<string>("terminal_clipboard_text");
}

export function terminalIsFolder(path: string) {
  return invoke<boolean>("terminal_is_folder", { path });
}

export function terminalSettings() {
  return invoke<TerminalSettings>("terminal_settings");
}

export function terminalSettingsSave(settings: TerminalSettings) {
  return invoke<TerminalSettings>("terminal_settings_save", { settings });
}

type OutputSink = (data: string) => void;
type ExitSink = (code: number | null) => void;

const outputSinks = new Map<string, OutputSink>();
const exitSinks = new Map<string, ExitSink>();
const pendingOutput = new Map<string, string[]>();
let started = false;

function ensureListening() {
  if (started) {
    return;
  }
  started = true;
  void listen<TerminalOutputEvent>("terminal-output", ({ payload }) => {
    const sink = outputSinks.get(payload.id);
    if (sink) {
      sink(payload.data);
      return;
    }
    const queued = pendingOutput.get(payload.id) ?? [];
    queued.push(payload.data);
    pendingOutput.set(payload.id, queued);
  });
  void listen<TerminalExitEvent>("terminal-exit", ({ payload }) => {
    exitSinks.get(payload.id)?.(payload.code);
  });
}

ensureListening();

/** Routes a session's output to `onOutput`, including output that arrived before it attached. */
export function attachTerminal(id: string, onOutput: OutputSink, onExit: ExitSink) {
  outputSinks.set(id, onOutput);
  exitSinks.set(id, onExit);
  for (const data of pendingOutput.get(id) ?? []) {
    onOutput(data);
  }
  pendingOutput.delete(id);
  return () => {
    outputSinks.delete(id);
    exitSinks.delete(id);
  };
}
