import { listen } from "@tauri-apps/api/event";
import { Copy, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { AsciiCode, AsciiState } from "../../../../types/ascii";
import { asciiSave, asciiState, copyText } from "../../lib/ascii-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: AsciiState = { settingsFolder: "", prefix: ":", entries: [], error: null };

export function AsciiCodes({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [state, setState] = useState<AsciiState>(EMPTY);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState<AsciiCode | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void asciiState()
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
    const subscription = listen<AsciiState>("ascii-changed", (event) => setState(event.payload));
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

  async function persist(entries: AsciiCode[], prefix = state.prefix) {
    setState(await asciiSave(entries, prefix));
    setRowError(null);
  }

  async function copyInsert(id: string, text: string) {
    try {
      await copyText(text);
      setCopiedId(id);
      window.setTimeout(() => setCopiedId((current) => (current === id ? null : current)), 1200);
      setRowError(null);
    } catch (caught) {
      setRowError(messageOf(caught));
    }
  }

  return (
    <section id="ascii-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="ascii-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="ascii-heading" className="min-w-0">
          <h1 id="ascii-title" className="text-base font-semibold text-zinc-100">
            ASCII codes
          </h1>
          <p id="ascii-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            The symbol is inserted as soon as you finish the trigger. {state.prefix}1/4 becomes ¼, and you can add your own.
          </p>
        </div>
        <Button id="ascii-add" variant="outline" onClick={() => setDraft({ id: "", trigger: "", text: "", enabled: true })}>
          <Plus id="ascii-add-icon" className="size-3.5" aria-hidden="true" />
          <span id="ascii-add-label">Add</span>
        </Button>
      </header>
      {problem ? (
        <p id="ascii-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {state.settingsFolder.length === 0 ? (
        <div id="ascii-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="ascii-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep changes in ascii.json.
          </p>
          <Button id="ascii-choose-folder" variant="outline" onClick={onOpenSettings}>
            <span id="ascii-choose-folder-label">Choose settings folder</span>
          </Button>
        </div>
      ) : null}
      <div id="ascii-prefix-row" className="flex items-center gap-3 border-b border-zinc-800 px-4 py-2">
        <label id="ascii-prefix-label" htmlFor="ascii-prefix" className="shrink-0 text-xs font-medium text-zinc-400">
          Prefix
        </label>
        <input
          id="ascii-prefix"
          value={state.prefix}
          maxLength={4}
          spellCheck={false}
          aria-describedby="ascii-prefix-hint"
          onChange={(event) => setState((current) => ({ ...current, prefix: event.target.value }))}
          onBlur={() => {
            void persist(state.entries, state.prefix).catch((caught) => setRowError(messageOf(caught)));
          }}
          className="h-8 w-16 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-center font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
        <p id="ascii-prefix-hint" className="text-xs text-zinc-500">
          Typed before every trigger. Use : or . or leave it empty.
        </p>
      </div>
      <div id="ascii-search-row" className="border-b border-zinc-800 px-4 py-2">
        <input
          id="ascii-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search"
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </div>
      <div id="ascii-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="ascii-table" className="w-full border-collapse text-sm">
          <thead id="ascii-head">
            <tr id="ascii-head-row" className="border-b border-zinc-800 text-left text-xs text-zinc-500">
              <th id="ascii-head-trigger" className="px-4 py-2 font-medium">Trigger</th>
              <th id="ascii-head-text" className="px-4 py-2 font-medium">Inserts</th>
              <th id="ascii-head-enabled" className="w-16 px-2 py-2 font-medium">On</th>
              <th id="ascii-head-actions" className="w-20 px-2 py-2" />
            </tr>
          </thead>
          <tbody id="ascii-body">
            {draft ? (
              <AsciiRow
                id="ascii-draft"
                entry={draft}
                prefix={state.prefix}
                copied={copiedId === "ascii-draft"}
                onCopy={() => void copyInsert("ascii-draft", draft.text)}
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
              <AsciiRow
                key={entry.id}
                id={`ascii-row-${entry.id}`}
                entry={entry}
                prefix={state.prefix}
                copied={copiedId === entry.id}
                onCopy={() => void copyInsert(entry.id, entry.text)}
                onChange={(next) => {
                  setState((current) => ({
                    ...current,
                    entries: current.entries.map((item) => (item.id === entry.id ? next : item)),
                  }));
                }}
                onCommit={() => {
                  const next = state.entries.find((item) => item.id === entry.id) ?? entry;
                  void persist(state.entries.map((item) => (item.id === entry.id ? next : item))).catch((caught) =>
                    setRowError(messageOf(caught)),
                  );
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

function AsciiRow({
  id,
  entry,
  prefix,
  copied,
  onChange,
  onCommit,
  onCopy,
  onDelete,
  onToggle,
}: {
  id: string;
  entry: AsciiCode;
  prefix: string;
  copied: boolean;
  onChange: (entry: AsciiCode) => void;
  onCommit: () => void;
  onCopy: () => void;
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
        <div id={`${id}-trigger-row`} className="flex items-center gap-1">
          {prefix ? (
            <span id={`${id}-prefix`} className="shrink-0 font-mono text-sm text-zinc-500">
              {prefix}
            </span>
          ) : null}
          <input
            id={`${id}-trigger`}
            value={entry.trigger}
            onChange={(event) => onChange({ ...entry, trigger: event.target.value })}
            spellCheck={false}
            className="h-8 w-full rounded-md border border-zinc-800 bg-transparent px-2 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
          />
        </div>
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
          onChange={(event) => onToggle?.(event.target.checked)}
          className="size-4 accent-sky-500"
        />
      </td>
      <td id={`${id}-actions-cell`} className="px-2 py-1.5">
        <div id={`${id}-actions`} className="flex items-center justify-end gap-1">
          <Button
            id={`${id}-copy`}
            variant="ghost"
            size="xs"
            title={copied ? "Copied" : "Copy insert"}
            aria-label={copied ? "Copied insert" : "Copy insert"}
            disabled={entry.text.length === 0}
            onClick={onCopy}
          >
            <Copy id={`${id}-copy-icon`} className="size-3.5" aria-hidden="true" />
          </Button>
          {onDelete ? (
            <Button id={`${id}-remove`} variant="ghost" size="xs" title="Remove" onClick={onDelete}>
              <Trash2 id={`${id}-remove-icon`} className="size-3.5" aria-hidden="true" />
            </Button>
          ) : null}
        </div>
      </td>
    </tr>
  );
}
