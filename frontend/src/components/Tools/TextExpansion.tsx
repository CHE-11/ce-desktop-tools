import { useEffect } from "react";
import type { Snippet } from "../../../../types/expansion";
import { childPath } from "../../lib/message";
import { Button } from "../Button";
import { SnippetTable } from "./SnippetTable";
import { VaultLock } from "./VaultLock";

type TextExpansionProps = {
  settingsFolder: string;
  snippets: Snippet[];
  enabled: boolean;
  error: string | null;
  busy: boolean;
  locked: boolean;
  needsPin: boolean;
  focusNonce: number;
  onOpenSettings: () => void;
  onSave: (snippets: Snippet[]) => Promise<void>;
  onCreatePin: (pin: string) => Promise<void>;
  onUnlock: (pin: string) => Promise<void>;
  onEditorChange: (open: boolean) => void;
};

export function TextExpansion({
  settingsFolder,
  snippets,
  enabled,
  error,
  busy,
  locked,
  needsPin,
  focusNonce,
  onOpenSettings,
  onSave,
  onCreatePin,
  onUnlock,
  onEditorChange,
}: TextExpansionProps) {
  useEffect(() => {
    if (settingsFolder.length === 0) {
      onEditorChange(false);
    }
  }, [onEditorChange, settingsFolder]);

  return (
    <section id="text-expansion" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div id="text-expansion-header" className="flex shrink-0 items-center justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="text-expansion-heading" className="min-w-0">
          <h1 id="text-expansion-title" className="text-base font-semibold text-zinc-100">
            Text Expansion
          </h1>
          <p id="text-expansion-file" className="mt-1 truncate font-mono text-xs text-zinc-500" title={settingsFolder || undefined}>
            {settingsFolder ? childPath(settingsFolder, "expansion.json") : "Saved in the settings folder, next to ce-desktop-tools.json"}
          </p>
        </div>
        {!settingsFolder && (
          <Button id="text-expansion-folder" variant="outline" disabled={busy} onClick={onOpenSettings}>
            <span id="text-expansion-folder-label">Choose settings folder</span>
          </Button> 
        )}
        {locked || needsPin ? (
          <VaultLock
            needsPin={needsPin}
            busy={busy}
            focusNonce={focusNonce}
            compact
            onCreate={onCreatePin}
            onUnlock={onUnlock}
          />
        ) : null}
      </div>

      {error ? (
        <p id="text-expansion-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}

      {!enabled && settingsFolder ? (
        <p id="text-expansion-paused" className="border-b border-zinc-800 px-4 py-2 text-sm text-amber-300">
          Expansion is paused. Resume it from the toolbar or the tray menu.
        </p>
      ) : null}

      {settingsFolder.length === 0 ? (
        <div id="text-expansion-empty" className="flex flex-1 flex-col items-start justify-center gap-3 px-4">
          <p id="text-expansion-empty-copy" className="max-w-md text-sm leading-5 text-zinc-400">
            Choose a settings folder. expansion.json is saved there with ce-desktop-tools.json, and typing a trigger in another Windows app replaces it.
          </p>
          <Button id="text-expansion-choose" variant="primary" disabled={busy} onClick={onOpenSettings}>
            <span id="text-expansion-choose-label">Choose settings folder</span>
          </Button>
        </div>
      ) : (
        <SnippetTable snippets={snippets} busy={busy} locked={locked} onSave={onSave} onDraftChange={onEditorChange} />
      )}
    </section>
  );
}
