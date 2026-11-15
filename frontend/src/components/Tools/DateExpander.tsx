import { listen } from "@tauri-apps/api/event";
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { DateExpand, DateExpandState } from "../../../../types/date-expand";
import { dateSave, dateState } from "../../lib/date-expand-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: DateExpandState = { settingsFolder: "", prefix: "", entries: [], error: null };

export function DateExpander({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [state, setState] = useState<DateExpandState>(EMPTY);
  const [draft, setDraft] = useState<DateExpand | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void dateState()
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
    const subscription = listen<DateExpandState>("date-changed", (event) => setState(event.payload));
    return () => {
      cancelled = true;
      void subscription.then((stop) => stop());
    };
  }, []);

  const problem = rowError ?? state.error;

  async function persist(entries: DateExpand[], prefix = state.prefix) {
    setState(
      await dateSave(
        entries.map(({ id, trigger, template, enabled }) => ({ id, trigger, template, enabled })),
        prefix,
      ),
    );
    setRowError(null);
  }

  return (
    <section id="date-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="date-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="date-heading" className="min-w-0">
          <h1 id="date-title" className="text-base font-semibold text-zinc-100">
            Date expander
          </h1>
          <p id="date-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            The date is filled in as soon as you finish the trigger. {state.prefix}date uses the short date. longdate uses the Windows long date. yy, MM, dd, HH, mm, and tt are year, month, day, hour, minute, and AM/PM.
          </p>
        </div>
        <Button id="date-add" variant="outline" onClick={() => setDraft({ id: "", trigger: "", template: "yyyy-MM-dd", enabled: true, preview: "" })}>
          <Plus id="date-add-icon" className="size-3.5" aria-hidden="true" />
          <span id="date-add-label">Add</span>
        </Button>
      </header>
      {problem ? (
        <p id="date-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {state.settingsFolder.length === 0 ? (
        <div id="date-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="date-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep changes in dates.json.
          </p>
          <Button id="date-choose-folder" variant="outline" onClick={onOpenSettings}>
            <span id="date-choose-folder-label">Choose settings folder</span>
          </Button>
        </div>
      ) : null}
      <div id="date-prefix-row" className="flex items-center gap-3 border-b border-zinc-800 px-4 py-2">
        <label id="date-prefix-label" htmlFor="date-prefix" className="shrink-0 text-xs font-medium text-zinc-400">
          Prefix
        </label>
        <input
          id="date-prefix"
          value={state.prefix}
          maxLength={4}
          spellCheck={false}
          aria-describedby="date-prefix-hint"
          onChange={(event) => setState((current) => ({ ...current, prefix: event.target.value }))}
          onBlur={() => {
            void persist(state.entries, state.prefix).catch((caught) => setRowError(messageOf(caught)));
          }}
          className="h-8 w-16 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-center font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
        <p id="date-prefix-hint" className="text-xs text-zinc-500">
          Typed before every trigger. Use : or . or leave it empty.
        </p>
      </div>
      <div id="date-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="date-table" className="w-full border-collapse text-sm">
          <thead id="date-head">
            <tr id="date-head-row" className="border-b border-zinc-800 text-left text-xs text-zinc-500">
              <th id="date-head-trigger" className="px-4 py-2 font-medium">Trigger</th>
              <th id="date-head-template" className="px-4 py-2 font-medium">Format</th>
              <th id="date-head-preview" className="px-4 py-2 font-medium">Now</th>
              <th id="date-head-enabled" className="w-16 px-2 py-2 font-medium">On</th>
              <th id="date-head-remove" className="w-12 px-2 py-2" />
            </tr>
          </thead>
          <tbody id="date-body">
            {draft ? (
              <DateRow
                id="date-draft"
                entry={draft}
                prefix={state.prefix}
                onChange={setDraft}
                onCommit={() => {
                  if (draft.trigger.trim().length === 0 && draft.template.trim().length === 0) {
                    setDraft(null);
                    return;
                  }
                  void persist([...state.entries, draft]).then(() => setDraft(null)).catch((caught) => setRowError(messageOf(caught)));
                }}
              />
            ) : null}
            {state.entries.map((entry) => (
              <DateRow
                key={entry.id}
                id={`date-row-${entry.id}`}
                entry={entry}
                prefix={state.prefix}
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

function DateRow({
  id,
  entry,
  prefix,
  onChange,
  onCommit,
  onDelete,
  onToggle,
}: {
  id: string;
  entry: DateExpand;
  prefix: string;
  onChange: (entry: DateExpand) => void;
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
      <td id={`${id}-template-cell`} className="px-4 py-1.5">
        <input
          id={`${id}-template`}
          value={entry.template}
          onChange={(event) => onChange({ ...entry, template: event.target.value })}
          spellCheck={false}
          className="h-8 w-full rounded-md border border-zinc-800 bg-transparent px-2 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </td>
      <td id={`${id}-preview-cell`} className="max-w-64 px-4 py-1.5">
        <span id={`${id}-preview`} className="block truncate whitespace-pre text-xs text-zinc-400" title={entry.preview}>
          {entry.preview}
        </span>
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
