import { FolderSearch, Loader2, RefreshCcw, Trash2 } from "lucide-react";
import type { ReactNode } from "react";
import { childPath } from "../../../lib/message";
import { Button } from "../../Button";
import { ChangePin } from "./ChangePin";

type VaultSettingsTabProps = {
  busy: boolean;
  exePath: string;
  folder: string;
  runAtStartup: boolean;
  warning: string | null;
  onFolderChange: (folder: string) => void;
  onFolderCommit: () => void;
  onRunAtStartupChange: (runAtStartup: boolean) => void;
  onBrowse: () => void;
  onClearFolder: () => void;
  onRevealExe: () => void;
  onCheckForUpdates: () => void;
  checkingUpdates: boolean;
  onSubmit: () => void;
};

export function VaultSettingsTab({
  busy,
  exePath,
  folder,
  runAtStartup,
  warning,
  onFolderChange,
  onFolderCommit,
  onRunAtStartupChange,
  onBrowse,
  onClearFolder,
  onRevealExe,
  onCheckForUpdates,
  checkingUpdates,
  onSubmit,
}: VaultSettingsTabProps) {
  const settingsFile = folder.trim() ? childPath(folder.trim(), "ce-desktop-tools.json") : "";

  return (
    <form
      id="settings-panel-vault"
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <p id="vault-settings-intro" className="text-sm leading-5 text-zinc-400">
        The settings folder holds ce-desktop-tools.json and the encrypted expansion vault. Leave it blank to keep appearance on this computer only.
      </p>

      <section id="vault-settings-application" className="flex min-w-0 items-center gap-3 rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2">
        <span id="vault-settings-exe-label" className="shrink-0 text-[11px] font-semibold tracking-wide text-zinc-500 uppercase">
          Application
        </span>
        <p id="vault-settings-exe" className="min-w-0 flex-1 truncate font-mono text-xs text-zinc-200 select-all" title={exePath}>
          {exePath || "Looking up this application…"}
        </p>
        <Button
          id="vault-settings-exe-reveal"
          type="button"
          size="sm"
          variant="outline"
          className="shrink-0"
          title="Show in File Explorer"
          disabled={busy || exePath.length === 0}
          onClick={onRevealExe}
        >
          <FolderSearch id="vault-settings-exe-reveal-icon" className="size-3.5" aria-hidden="true" />
        </Button>
        <Button
          id="vault-settings-check-updates"
          type="button"
          size="sm"
          variant="outline"
          className={checkingUpdates ? "shrink-0 disabled:opacity-100" : "shrink-0"}
          disabled={busy || checkingUpdates}
          aria-busy={checkingUpdates}
          onClick={onCheckForUpdates}
        >
          {checkingUpdates ? (
            <Loader2 id="vault-settings-check-updates-spinner" className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <RefreshCcw id="vault-settings-check-updates-icon" className="size-3.5" aria-hidden="true" />
          )}
          <span id="vault-settings-check-updates-label">{checkingUpdates ? "Checking…" : "Check for updates"}</span>
        </Button>
      </section>

      <label id="vault-settings-startup" className="flex items-start gap-2 text-sm text-zinc-200">
        <input
          id="vault-settings-startup-input"
          type="checkbox"
          checked={runAtStartup}
          disabled={busy}
          onChange={(event) => onRunAtStartupChange(event.target.checked)}
          className="mt-0.5 size-4 accent-sky-500 disabled:opacity-50"
        />
        <span id="vault-settings-startup-copy">
          <span id="vault-settings-startup-text" className="block">
            Run when Windows starts
          </span>
          <span id="vault-settings-startup-hint" className="mt-0.5 block text-xs leading-5 text-zinc-500">
            Starts in the tray when you sign in, so text expansion is available without opening the window.
          </span>
        </span>
      </label>

      <SettingsSection
        id="vault-settings-location"
        title="Settings folder"
        hint="A folder stores ce-desktop-tools.json and expansion.json together. Clear the folder to stop using those files without deleting them."
      >
        <label id="vault-settings-folder-label" htmlFor="vault-settings-folder-input" className="mb-1 block text-xs font-medium text-zinc-400">
          Folder
        </label>
        <div id="vault-settings-folder-row" className="flex items-center gap-2">
          <div id="vault-settings-folder-field" className="relative min-w-0 flex-1">
            <input
              id="vault-settings-folder-input"
              value={folder}
              onChange={(event) => onFolderChange(event.target.value)}
              onBlur={(event) => {
                const next = event.relatedTarget;
                if (next instanceof HTMLElement && (next.id === "vault-settings-browse" || next.id === "vault-settings-clear")) {
                  return;
                }
                onFolderCommit();
              }}
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              placeholder="\\\\server\\share\\ce-desktop-tools"
              className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 pr-20 pl-3 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
            />
            <Button
              id="vault-settings-clear"
              type="button"
              variant="ghost"
              size="xs"
              className="absolute top-1/2 right-1 -translate-y-1/2"
              onClick={onClearFolder}
              disabled={busy || folder.trim().length === 0}
            >
              <Trash2 id="vault-settings-clear-icon" className="size-3.5" aria-hidden="true" />
              <span id="vault-settings-clear-label">Clear</span>
            </Button>
          </div>
          <Button id="vault-settings-browse" type="button" onClick={onBrowse} disabled={busy}>
            <FolderSearch id="vault-settings-browse-icon" className="size-3.5" aria-hidden="true" />
            <span id="vault-settings-browse-label">Browse</span>
          </Button>
        </div>
        <p id="vault-settings-folder-file" className="mt-2 truncate font-mono text-xs text-zinc-500" title={settingsFile || undefined}>
          {settingsFile ? `Settings file: ${settingsFile}` : "Settings file: on this computer"}
        </p>
      </SettingsSection>

      <ChangePin busy={busy} />

      {warning ? (
        <p id="vault-settings-warning" className="text-sm text-amber-300">
          {warning}
        </p>
      ) : null}
    </form>
  );
}

function SettingsSection({ id, title, hint, children }: { id: string; title: string; hint?: string; children: ReactNode }) {
  return (
    <section id={id} className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-3">
      <h3 id={`${id}-heading`} className="text-[11px] font-semibold tracking-wide text-zinc-400 uppercase">
        {title}
      </h3>
      {hint ? (
        <p id={`${id}-hint`} className="mt-1 mb-3 text-xs leading-5 text-zinc-500">
          {hint}
        </p>
      ) : null}
      {children}
    </section>
  );
}
