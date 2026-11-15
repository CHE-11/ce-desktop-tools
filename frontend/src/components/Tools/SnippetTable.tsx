import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { FocusEvent } from "react";
import type { Snippet } from "../../../../types/expansion";
import { cn } from "../../lib/cn";
import { Button } from "../Button";

type SortKey = "trigger" | "text" | "enabled";
type SortDir = "asc" | "desc";

type Draft = {
  key: string;
  trigger: string;
  text: string;
  enabled: boolean;
  error: string | null;
};

type RowEdit = {
  trigger: string;
  text: string;
  enabled: boolean;
  error: string | null;
};

type SnippetTableProps = {
  snippets: readonly Snippet[];
  busy: boolean;
  onSave: (snippets: Snippet[]) => Promise<void>;
  onDraftChange: (open: boolean) => void;
};

export function SnippetTable({ snippets, busy, onSave, onDraftChange }: SnippetTableProps) {
  const [query, setQuery] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("trigger");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [edits, setEdits] = useState<Record<string, RowEdit>>({});
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const latest = useRef(snippets);
  const draftKeys = useRef(1);

  useEffect(() => {
    latest.current = snippets;
  }, [snippets]);

  useEffect(() => {
    onDraftChange(drafts.length > 0);
  }, [drafts.length, onDraftChange]);

  useEffect(() => {
    setEdits((current) => {
      let changed = false;
      const next = { ...current };
      for (const snippet of snippets) {
        const edit = next[snippet.id];
        if (!edit || edit.error) {
          continue;
        }
        if (edit.trigger === snippet.trigger && edit.text === snippet.text && edit.enabled === snippet.enabled) {
          delete next[snippet.id];
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [snippets]);

  const shown = useMemo(() => filterAndSort(snippets, query, sortKey, sortDir), [snippets, query, sortKey, sortDir]);

  const addRow = () => {
    const key = `draft-${draftKeys.current}`;
    draftKeys.current += 1;
    setDrafts((current) => [{ key, trigger: "", text: "", enabled: true, error: null }, ...current]);
    setExpandedId(key);
    window.setTimeout(() => {
      document.getElementById(`snippet-trigger-${key}`)?.focus();
    }, 0);
  };

  const persist = async (next: Snippet[]) => {
    const previous = latest.current;
    latest.current = next;
    try {
      await onSave(next);
    } catch (caught) {
      latest.current = previous;
      throw caught;
    }
  };

  const commitSaved = async (snippet: Snippet, edit: RowEdit) => {
    const problem = validate(edit.trigger, latest.current.filter((item) => item.id !== snippet.id));
    if (problem) {
      setEdits((current) => ({ ...current, [snippet.id]: { ...edit, error: problem } }));
      return;
    }
    if (edit.trigger === snippet.trigger && edit.text === snippet.text && edit.enabled === snippet.enabled) {
      setEdits((current) => {
        if (!current[snippet.id]) {
          return current;
        }
        const next = { ...current };
        delete next[snippet.id];
        return next;
      });
      return;
    }
    const nextSnippet: Snippet = { id: snippet.id, trigger: edit.trigger.trim(), text: edit.text, enabled: edit.enabled };
    try {
      await persist(latest.current.map((item) => (item.id === snippet.id ? nextSnippet : item)));
      setEdits((current) => {
        const next = { ...current };
        delete next[snippet.id];
        return next;
      });
    } catch (caught) {
      setEdits((current) => ({
        ...current,
        [snippet.id]: { ...edit, error: caught instanceof Error ? caught.message : "Couldn't save this row." },
      }));
    }
  };

  const commitDraft = async (draft: Draft) => {
    if (draft.trigger.trim().length === 0 && draft.text.trim().length === 0) {
      setDrafts((current) => current.filter((item) => item.key !== draft.key));
      return;
    }
    const problem = validate(draft.trigger, [...latest.current, ...draftTriggers(drafts, draft.key)]);
    if (problem) {
      setDrafts((current) => current.map((item) => (item.key === draft.key ? { ...draft, error: problem } : item)));
      return;
    }
    try {
      await persist([...latest.current, { id: "", trigger: draft.trigger.trim(), text: draft.text, enabled: draft.enabled }]);
      setDrafts((current) => current.filter((item) => item.key !== draft.key));
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "Couldn't save this row.";
      setDrafts((current) => current.map((item) => (item.key === draft.key ? { ...draft, error: message } : item)));
    }
  };

  const leaveRow = (event: FocusEvent<HTMLTableRowElement>, commit: () => void) => {
    const next = event.relatedTarget;
    if (next instanceof Node && event.currentTarget.contains(next)) {
      return;
    }
    commit();
  };

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((dir) => (dir === "asc" ? "desc" : "asc"));
      return;
    }
    setSortKey(key);
    setSortDir("asc");
  };

  return (
    <div id="snippet-table-wrap" className="flex min-h-0 flex-1 flex-col">
      <div id="snippet-table-toolbar" className="flex items-center gap-2 border-b border-zinc-800 px-3 py-2">
        <label id="snippet-search-label" htmlFor="snippet-search" className="sr-only">
          Search snippets
        </label>
        <input
          id="snippet-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search triggers and replacements"
          autoComplete="off"
          spellCheck={false}
          className="h-7 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
        <p id="snippet-table-count" className="shrink-0 text-xs text-zinc-500 tabular-nums">
          {shown.length === snippets.length
            ? `${snippets.length} ${snippets.length === 1 ? "snippet" : "snippets"}`
            : `${shown.length} of ${snippets.length}`}
        </p>
        <Button id="snippet-add-row" variant="primary" size="sm" disabled={busy} onClick={addRow}>
          <Plus id="snippet-add-row-icon" className="size-3.5" aria-hidden="true" />
          <span id="snippet-add-row-label">Add row</span>
        </Button>
      </div>
      <div id="snippet-table-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="snippet-table" className="w-full border-collapse text-left text-sm">
          <thead id="snippet-table-head" className="sticky top-0 z-10 bg-zinc-900">
            <tr id="snippet-table-columns">
              <SortHeader id="snippet-col-trigger" label="Trigger" sortKey="trigger" active={sortKey} dir={sortDir} onSort={toggleSort} className="w-44" />
              <SortHeader id="snippet-col-text" label="Replacement" sortKey="text" active={sortKey} dir={sortDir} onSort={toggleSort} />
              <SortHeader id="snippet-col-enabled" label="On" sortKey="enabled" active={sortKey} dir={sortDir} onSort={toggleSort} className="w-16" />
              <th id="snippet-col-actions" className="w-10 border-b border-zinc-800" />
            </tr>
          </thead>
          <tbody id="snippet-table-body">
            {drafts.map((draft) => (
              <tr
                id={`snippet-row-${draft.key}`}
                key={draft.key}
                className="border-b border-zinc-800 bg-sky-950/20"
                onBlur={(event) => leaveRow(event, () => void commitDraft(draft))}
              >
                <RowFields
                  id={draft.key}
                  trigger={draft.trigger}
                  text={draft.text}
                  enabled={draft.enabled}
                  error={draft.error}
                  expanded={expandedId === draft.key}
                  disabled={busy}
                  onExpand={() => setExpandedId(draft.key)}
                  onChange={(patch) => {
                    setDrafts((current) => current.map((item) => (item.key === draft.key ? { ...item, ...patch, error: null } : item)));
                  }}
                />
                <td id={`snippet-delete-cell-${draft.key}`} className="px-1 align-top">
                  <button
                    id={`snippet-delete-${draft.key}`}
                    type="button"
                    title="Remove row"
                    aria-label="Remove row"
                    disabled={busy}
                    onClick={() => setDrafts((current) => current.filter((item) => item.key !== draft.key))}
                    className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-red-300 disabled:opacity-40"
                  >
                    <Trash2 id={`snippet-delete-icon-${draft.key}`} className="size-3.5" aria-hidden="true" />
                  </button>
                </td>
              </tr>
            ))}
            {shown.map((snippet) => {
              const edit = edits[snippet.id];
              const trigger = edit?.trigger ?? snippet.trigger;
              const text = edit?.text ?? snippet.text;
              const enabled = edit?.enabled ?? snippet.enabled;
              const row: RowEdit = { trigger, text, enabled, error: edit?.error ?? null };
              return (
                <tr
                  id={`snippet-row-${domId(snippet.id)}`}
                  key={snippet.id}
                  className="border-b border-zinc-800 hover:bg-zinc-900/80"
                  onBlur={(event) => leaveRow(event, () => void commitSaved(snippet, row))}
                >
                  <RowFields
                    id={domId(snippet.id)}
                    trigger={trigger}
                    text={text}
                    enabled={enabled}
                    error={row.error}
                    expanded={expandedId === snippet.id}
                    disabled={busy}
                    onExpand={() => setExpandedId(snippet.id)}
                    onChange={(patch) => {
                      const next = { ...row, ...patch, error: null };
                      setEdits((current) => ({ ...current, [snippet.id]: next }));
                      if (patch.enabled !== undefined) {
                        void commitSaved(snippet, next);
                      }
                    }}
                  />
                  <td id={`snippet-delete-cell-${domId(snippet.id)}`} className="px-1 align-top">
                    <button
                      id={`snippet-delete-${domId(snippet.id)}`}
                      type="button"
                      title="Delete snippet"
                      aria-label={`Delete ${snippet.trigger}`}
                      disabled={busy}
                      onClick={() => void persist(latest.current.filter((item) => item.id !== snippet.id))}
                      className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-red-300 disabled:opacity-40"
                    >
                      <Trash2 id={`snippet-delete-icon-${domId(snippet.id)}`} className="size-3.5" aria-hidden="true" />
                    </button>
                  </td>
                </tr>
              );
            })}
            {shown.length === 0 && drafts.length === 0 ? (
              <tr id="snippet-table-empty">
                <td id="snippet-table-empty-cell" colSpan={4} className="px-3 py-8 text-sm text-zinc-500">
                  {snippets.length === 0 ? "No snippets yet. Add a row to start." : "No snippets match this search."}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function SortHeader({
  id,
  label,
  sortKey,
  active,
  dir,
  onSort,
  className,
}: {
  id: string;
  label: string;
  sortKey: SortKey;
  active: SortKey;
  dir: SortDir;
  onSort: (key: SortKey) => void;
  className?: string;
}) {
  const selected = active === sortKey;
  return (
    <th id={id} aria-sort={selected ? (dir === "asc" ? "ascending" : "descending") : "none"} className={cn("border-b border-zinc-800 p-0", className)}>
      <button
        id={`${id}-sort`}
        type="button"
        onClick={() => onSort(sortKey)}
        className="flex h-8 w-full items-center gap-1 px-2 text-xs font-medium text-zinc-400 hover:text-zinc-100"
      >
        <span id={`${id}-label`}>{label}</span>
        {selected ? (
          dir === "asc" ? (
            <ArrowUp id={`${id}-icon`} className="size-3" aria-hidden="true" />
          ) : (
            <ArrowDown id={`${id}-icon`} className="size-3" aria-hidden="true" />
          )
        ) : null}
      </button>
    </th>
  );
}

function RowFields({
  id,
  trigger,
  text,
  enabled,
  error,
  expanded,
  disabled,
  onExpand,
  onChange,
}: {
  id: string;
  trigger: string;
  text: string;
  enabled: boolean;
  error: string | null;
  expanded: boolean;
  disabled: boolean;
  onExpand: () => void;
  onChange: (patch: Partial<Pick<RowEdit, "trigger" | "text" | "enabled">>) => void;
}) {
  return (
    <>
      <td id={`snippet-trigger-cell-${id}`} className="px-2 py-1 align-top">
        <input
          id={`snippet-trigger-${id}`}
          value={trigger}
          disabled={disabled}
          autoComplete="off"
          spellCheck={false}
          placeholder=";sig"
          aria-invalid={error ? true : undefined}
          onFocus={onExpand}
          onChange={(event) => onChange({ trigger: event.target.value })}
          className="h-7 w-full rounded border border-transparent bg-transparent px-1 font-mono text-sm text-sky-300 outline-none focus:border-sky-500 focus:bg-zinc-950 disabled:opacity-50"
        />
        {error ? (
          <p id={`snippet-error-${id}`} className="px-1 pt-0.5 text-xs text-red-400">
            {error}
          </p>
        ) : null}
      </td>
      <td id={`snippet-text-cell-${id}`} className="px-2 py-1 align-top">
        <textarea
          id={`snippet-text-${id}`}
          value={text}
          disabled={disabled}
          rows={expanded ? 3 : 1}
          spellCheck
          title={text}
          onFocus={onExpand}
          onChange={(event) => onChange({ text: event.target.value })}
          className="w-full resize-none rounded border border-transparent bg-transparent px-1 py-1 text-sm text-zinc-200 outline-none focus:border-sky-500 focus:bg-zinc-950 disabled:opacity-50"
        />
      </td>
      <td id={`snippet-enabled-cell-${id}`} className="px-2 py-1 text-center align-top">
        <input
          id={`snippet-enabled-${id}`}
          type="checkbox"
          checked={enabled}
          disabled={disabled}
          aria-label={enabled ? "Enabled" : "Disabled"}
          onChange={(event) => onChange({ enabled: event.target.checked })}
          className="mt-1.5 size-3.5 accent-sky-500 disabled:opacity-50"
        />
      </td>
    </>
  );
}

function filterAndSort(snippets: readonly Snippet[], query: string, sortKey: SortKey, sortDir: SortDir) {
  const needle = query.trim().toLowerCase();
  const filtered = snippets.filter((snippet) => {
    if (!needle) {
      return true;
    }
    return snippet.trigger.toLowerCase().includes(needle) || snippet.text.toLowerCase().includes(needle);
  });
  const direction = sortDir === "asc" ? 1 : -1;
  return filtered.sort((left, right) => {
    if (sortKey === "enabled") {
      return (Number(left.enabled) - Number(right.enabled)) * direction;
    }
    const leftValue = sortKey === "trigger" ? left.trigger : left.text;
    const rightValue = sortKey === "trigger" ? right.trigger : right.text;
    return leftValue.localeCompare(rightValue, undefined, { sensitivity: "base", numeric: true }) * direction;
  });
}

function validate(trigger: string, snippets: readonly { id: string; trigger: string }[]) {
  const nextTrigger = trigger.trim();
  if (!nextTrigger) {
    return "A trigger is required.";
  }
  if (/[\s\u0000-\u001f]/.test(nextTrigger)) {
    return "A trigger cannot contain spaces or line breaks.";
  }
  if (nextTrigger.length > 64) {
    return "A trigger is limited to 64 characters.";
  }
  if (snippets.some((item) => item.trigger.trim() === nextTrigger)) {
    return `Trigger "${nextTrigger}" is already used.`;
  }
  return null;
}

function draftTriggers(drafts: readonly Draft[], exceptKey: string): { id: string; trigger: string }[] {
  return drafts.filter((item) => item.key !== exceptKey).map((item) => ({ id: item.key, trigger: item.trigger }));
}

function domId(id: string) {
  const clean = id.replace(/[^A-Za-z0-9_-]/g, "");
  return clean || "snippet";
}
