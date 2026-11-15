import { listen } from "@tauri-apps/api/event";
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { Misspell, MisspellState } from "../../../../types/misspell";
import { misspellSave, misspellState } from "../../lib/misspell-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: MisspellState = { settingsFolder: "", entries: [], error: null };

export function CommonMisspells({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [state, setState] = useState<MisspellState>(EMPTY);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<Misspell | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void misspellState()
      .then((next) => {
        if (!cancelled) {
          setState(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setState((current) => ({ ...current, error: messageOf(caught) }));
        }
      });
    const subscription = listen<MisspellState>("misspell-changed", (event) => setState(event.payload));
    return () => {
      cancelled = true;
      void subscription.then((stop) => stop());
    };
  }, []);

  const shown = state.entries.filter((entry) => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) {
      return true;
    }
    return entry.trigger.toLowerCase().includes(needle) || entry.text.toLowerCase().includes(needle);
  });
  const problem = rowError ?? state.error;

  async function persist(entries: Misspell[]) {
    setState(await misspellSave(entries));
    setRowError(null);
  }

  return (
    <section id="misspell-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="misspell-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="misspell-heading" className="min-w-0">
          <h1 id="misspell-title" className="text-base font-semibold text-zinc-100">
            Common misspells
          </h1>
          <p id="misspell-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            In any other app, type the word and press space. dont becomes don't, and the space stays. Dont becomes Don't. A period, comma, or other punctuation does the same. The starting list also covers other missing apostrophes, such as doesnt and youre, and frequent typos such as teh and recieve.
          </p>
        </div>
        <Button id="misspell-add" variant="outline" onClick={() => setDraft({ id: "", trigger: "", text: "", enabled: true })}>
          <Plus id="misspell-add-icon" className="size-3.5" aria-hidden="true" />
          <span id="misspell-add-label">Add</span>
        </Button>
      </header>
      {problem ? (
        <p id="misspell-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {state.settingsFolder.length === 0 ? (
        <div id="misspell-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="misspell-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep changes in misspells.json.
          </p>
          <Button id="misspell-choose-folder" variant="outline" onClick={onOpenSettings}>
            <span id="misspell-choose-folder-label">Choose settings folder</span>
          </Button>
        </div>
      ) : null}
      <div id="misspell-search-row" className="border-b border-zinc-800 px-4 py-2">
        <input
          id="misspell-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search"
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </div>
      <div id="misspell-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="misspell-table" className="w-full border-collapse text-sm">
          <thead id="misspell-head">
            <tr id="misspell-head-row" className="border-b border-zinc-800 text-left text-xs text-zinc-500">
              <th id="misspell-head-trigger" className="px-4 py-2 font-medium">Typed</th>
              <th id="misspell-head-text" className="px-4 py-2 font-medium">Replacement</th>
              <th id="misspell-head-enabled" className="w-16 px-2 py-2 font-medium">On</th>
              <th id="misspell-head-remove" className="w-12 px-2 py-2" />
            </tr>
          </thead>
          <tbody id="misspell-body">
            {draft ? (
              <EditRow
                id="misspell-draft"
                entry={draft}
                onChange={setDraft}
                onCommit={() => {
                  if (draft.trigger.trim().length === 0 && draft.text.trim().length === 0) {
                    setDraft(null);
                    return;
                  }
                  void persist([...state.entries, draft]).then(() => setDraft(null)).catch((caught) => setRowError(messageOf(caught)));
                }}
              />
            ) : null}
            {shown.map((entry) => (
              <EditRow
                key={entry.id}
                id={`misspell-row-${entry.id}`}
                entry={entry}
                onChange={(next) => {
                  setState((current) => ({
                    ...current,
                    entries: current.entries.map((item) => (item.id === entry.id ? next : item)),
                  }));
                }}
                onCommit={() => {
                  const next = state.entries.find((item) => item.id === entry.id) ?? entry;
                  void persist(state.entries.map((item) => (item.id === entry.id ? next : item))).catch((caught) => setRowError(messageOf(caught)));
                }}
                onToggle={(enabled) => {
                  const entries = state.entries.map((item) => (item.id === entry.id ? { ...item, enabled } : item));
                  setState((current) => ({ ...current, entries }));
                  void persist(entries).catch((caught) => setRowError(messageOf(caught)));
                }}
                onDelete={() => {
                  void persist(state.entries.filter((item) => item.id !== entry.id)).catch((caught) => setRowError(messageOf(caught)));
                }}
              />
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function EditRow({
  id,
  entry,
  onChange,
  onCommit,
  onDelete,
  onToggle,
}: {
  id: string;
  entry: Misspell;
  onChange: (entry: Misspell) => void;
  onCommit: () => void;
  onDelete?: () => void;
  onToggle?: (enabled: boolean) => void;
}) {
  return (
    <tr
      id={id}
      className="border-b border-zinc-900"
      onBlur={(event) => {
        const next = event.relatedTarget;
        if (next instanceof Node && event.currentTarget.contains(next)) {
          return;
        }
        onCommit();
      }}
    >
      <td id={`${id}-trigger-cell`} className="px-4 py-1.5">
        <input
          id={`${id}-trigger`}
          value={entry.trigger}
          onChange={(event) => onChange({ ...entry, trigger: event.target.value })}
          spellCheck={false}
          className="h-8 w-full rounded-md border border-zinc-800 bg-transparent px-2 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </td>
      <td id={`${id}-text-cell`} className="px-4 py-1.5">
        <input
          id={`${id}-text`}
          value={entry.text}
          onChange={(event) => onChange({ ...entry, text: event.target.value })}
          className="h-8 w-full rounded-md border border-zinc-800 bg-transparent px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </td>
      <td id={`${id}-enabled-cell`} className="px-2 py-1.5 text-center">
        <input
          id={`${id}-enabled`}
          type="checkbox"
          checked={entry.enabled}
          onChange={(event) => {
            const enabled = event.target.checked;
            onChange({ ...entry, enabled });
            onToggle?.(enabled);
          }}
          className="size-4 accent-sky-500"
        />
      </td>
      <td id={`${id}-remove-cell`} className="px-2 py-1.5">
        {onDelete ? (
          <Button id={`${id}-remove`} variant="ghost" size="xs" onClick={onDelete}>
            <Trash2 id={`${id}-remove-icon`} className="size-3.5" aria-hidden="true" />
          </Button>
        ) : null}
      </td>
    </tr>
  );
}
