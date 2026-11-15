import { listen } from "@tauri-apps/api/event";
import { Lock, Pin, PinOff, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { ClipboardState } from "../../../../types/clipboard";
import { clipboardCopy, clipboardDelete, clipboardSetPinned, clipboardState } from "../../lib/clipboard-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";
import { VaultLock } from "./VaultLock";

type ClipboardProps = {
  settingsFolder: string;
  locked: boolean;
  needsPin: boolean;
  busy: boolean;
  focusNonce: number;
  onOpenSettings: () => void;
  onCreatePin: (pin: string) => Promise<void>;
  onUnlock: (pin: string) => Promise<void>;
};

export function Clipboard({ settingsFolder, locked, needsPin, busy, focusNonce, onOpenSettings, onCreatePin, onUnlock }: ClipboardProps) {
  const [history, setHistory] = useState<ClipboardState>({ locked: true, clips: [], error: null });
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void clipboardState()
      .then((next) => {
        if (!cancelled) {
          setHistory(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    const subscription = listen<ClipboardState>("clipboard-changed", (event) => {
      setHistory(event.payload);
    });
    return () => {
      cancelled = true;
      void subscription.then((stop) => stop());
    };
  }, []);

  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return history.clips;
    }
    return history.clips.filter((clip) => clip.text.toLowerCase().includes(needle));
  }, [history.clips, query]);

  return (
    <section id="clipboard-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="clipboard-header" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="clipboard-heading" className="min-w-0">
          <h1 id="clipboard-title" className="text-base font-semibold text-zinc-100">
            Clipboard
          </h1>
          <p id="clipboard-file" className="mt-1 text-xs text-zinc-500">
            Recent text copies, saved in clipboard.json next to the settings file.
          </p>
        </div>
        {settingsFolder.length === 0 ? (
          <Button id="clipboard-folder" variant="outline" disabled={busy} onClick={onOpenSettings}>
            <span id="clipboard-folder-label">Choose settings folder</span>
          </Button>
        ) : null}

        {locked || needsPin ? (
          <VaultLock
            needsPin={needsPin}
            busy={busy}
            focusNonce={focusNonce}
            compact
            pending={locked && !needsPin}
            onCreate={onCreatePin}
            onUnlock={onUnlock}
          />
        ) : null}

      </header>
      {history.error || error ? (
        <p id="clipboard-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {history.error || error}
        </p>
      ) : null}
      {settingsFolder.length === 0 ? (
        <div id="clipboard-empty-folder" className="flex flex-1 flex-col items-start justify-center gap-3 px-4">
          <p id="clipboard-empty-folder-copy" className="max-w-md text-sm leading-5 text-zinc-400">
            Choose a settings folder. Clipboard history uses the same PIN as text expansion.
          </p>
          <Button id="clipboard-choose" variant="primary" disabled={busy} onClick={onOpenSettings}>
            <span id="clipboard-choose-label">Choose settings folder</span>
          </Button>
        </div>
      ) : locked || needsPin ? (
        <div className="flex flex-col gap-2 w-full h-full items-center justify-center">
          <Lock className="size-10 text-zinc-400" />
          <h2>Clipboard Locked</h2>
          <p id="clipboard-locked-message" className="text-sm text-zinc-400">
            Enter the PIN to unlock it.
          </p>
        </div>
      ) : (
      <>
      <div id="clipboard-toolbar" className="flex items-center gap-2 border-b border-zinc-800 px-3 py-2">
        <label id="clipboard-search-label" htmlFor="clipboard-search" className="sr-only">
          Search clips
        </label>
        <input
          id="clipboard-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search copied text"
          autoComplete="off"
          spellCheck={false}
          className="h-7 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
        <p id="clipboard-count" className="shrink-0 text-xs text-zinc-500 tabular-nums">
          {shown.length === history.clips.length ? `${history.clips.length} clips` : `${shown.length} of ${history.clips.length}`}
        </p>
      </div>
      <div id="clipboard-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="clipboard-table" className="w-full border-collapse text-left text-sm">
          <thead id="clipboard-table-head" className="sticky top-0 z-10 bg-zinc-900">
            <tr id="clipboard-columns">
              <th id="clipboard-col-text" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Text
              </th>
              <th id="clipboard-col-actions" className="w-20 border-b border-zinc-800" />
            </tr>
          </thead>
          <tbody id="clipboard-body">
            {shown.length === 0 ? (
              <tr id="clipboard-empty">
                <td id="clipboard-empty-cell" colSpan={2} className="px-3 py-8 text-sm text-zinc-500">
                  {history.clips.length === 0 ? "Copies will show up here." : "No clips match this search."}
                </td>
              </tr>
            ) : (
              shown.map((clip, index) => {
                const rowId = domId(clip.id) || `row-${index}`;
                const preview = clip.text.replace(/\s+/g, " ");
                return (
                  <tr id={`clipboard-row-${rowId}`} key={clip.id} className="border-b border-zinc-800 hover:bg-zinc-900/80">
                    <td id={`clipboard-text-cell-${rowId}`} className="px-1 py-1">
                      <button
                        id={`clipboard-copy-${rowId}`}
                        type="button"
                        title={clip.text}
                        onClick={() => void clipboardCopy(clip.id).catch((caught) => setError(messageOf(caught)))}
                        className="block w-full truncate rounded px-2 py-1 text-left text-sm text-zinc-200 hover:bg-zinc-800"
                      >
                        <span id={`clipboard-copy-label-${rowId}`}>{preview}</span>
                      </button>
                    </td>
                    <td id={`clipboard-actions-${rowId}`} className="px-1 py-1">
                      <div id={`clipboard-actions-row-${rowId}`} className="flex items-center justify-end">
                        <button
                          id={`clipboard-pin-${rowId}`}
                          type="button"
                          title={clip.pinned ? "Unpin" : "Pin"}
                          aria-pressed={clip.pinned}
                          onClick={() => void clipboardSetPinned(clip.id, !clip.pinned).then(setHistory).catch((caught) => setError(messageOf(caught)))}
                          className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                        >
                          {clip.pinned ? (
                            <Pin id={`clipboard-pin-icon-${rowId}`} className="size-3.5 text-sky-300" aria-hidden="true" />
                          ) : (
                            <PinOff id={`clipboard-pin-icon-${rowId}`} className="size-3.5" aria-hidden="true" />
                          )}
                        </button>
                        <button
                          id={`clipboard-delete-${rowId}`}
                          type="button"
                          title="Delete"
                          aria-label="Delete clip"
                          onClick={() => void clipboardDelete(clip.id).then(setHistory).catch((caught) => setError(messageOf(caught)))}
                          className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-red-300"
                        >
                          <Trash2 id={`clipboard-delete-icon-${rowId}`} className="size-3.5" aria-hidden="true" />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
      </>
      )}
    </section>
  );
}

function domId(id: string) {
  return id.replace(/[^A-Za-z0-9_-]/g, "");
}
