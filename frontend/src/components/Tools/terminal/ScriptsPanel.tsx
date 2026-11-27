import { Play, Plus, Save, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { TerminalScript } from "../../../../../types/app";
import { cn } from "../../../lib/cn";
import { Button } from "../../Button";

type Props = {
  scripts: TerminalScript[];
  canRun: boolean;
  onSave: (scripts: TerminalScript[]) => void;
  onRun: (body: string) => void;
};

function newScript(): TerminalScript {
  return {
    id: `script-${crypto.randomUUID().slice(0, 8)}`,
    name: "New script",
    body: "",
  };
}

export function ScriptsPanel({ scripts, canRun, onSave, onRun }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(scripts[0]?.id ?? null);
  const [name, setName] = useState("");
  const [body, setBody] = useState("");
  const [dirty, setDirty] = useState(false);

  const selected = scripts.find((script) => script.id === selectedId) ?? null;

  useEffect(() => {
    if (selected) {
      setName(selected.name);
      setBody(selected.body);
      setDirty(false);
      return;
    }
    if (scripts.length > 0) {
      setSelectedId(scripts[0].id);
      return;
    }
    setSelectedId(null);
    setName("");
    setBody("");
    setDirty(false);
  }, [selected, scripts]);

  function select(id: string) {
    if (dirty && selected) {
      const next = scripts.map((script) =>
        script.id === selected.id ? { ...script, name: name.trim() || "Untitled", body } : script,
      );
      onSave(next);
    }
    setSelectedId(id);
  }

  function create() {
    const script = newScript();
    const next = dirty && selected
      ? [
          ...scripts.map((item) =>
            item.id === selected.id ? { ...item, name: name.trim() || "Untitled", body } : item,
          ),
          script,
        ]
      : [...scripts, script];
    onSave(next);
    setSelectedId(script.id);
    setName(script.name);
    setBody("");
    setDirty(false);
  }

  function save() {
    if (!selected) {
      const script = { ...newScript(), name: name.trim() || "Untitled", body };
      onSave([...scripts, script]);
      setSelectedId(script.id);
      setDirty(false);
      return;
    }
    onSave(
      scripts.map((script) =>
        script.id === selected.id ? { ...script, name: name.trim() || "Untitled", body } : script,
      ),
    );
    setDirty(false);
  }

  function remove() {
    if (!selected) {
      return;
    }
    const remaining = scripts.filter((script) => script.id !== selected.id);
    onSave(remaining);
    setSelectedId(remaining[0]?.id ?? null);
    setDirty(false);
  }

  return (
    <aside
      id="terminal-scripts"
      className="flex w-72 shrink-0 flex-col border-l border-zinc-800 bg-zinc-950/40"
      aria-label="Saved scripts"
    >
      <div id="terminal-scripts-header" className="flex items-center gap-1 border-b border-zinc-800 px-2 py-1.5">
        <h2 id="terminal-scripts-title" className="flex-1 text-xs font-semibold text-zinc-200">
          Saved scripts
        </h2>
        <Button id="terminal-scripts-new" variant="ghost" size="xs" title="New script" onClick={create}>
          <Plus id="terminal-scripts-new-icon" className="size-3.5" aria-hidden="true" />
        </Button>
      </div>
      <ul id="terminal-scripts-list" className="max-h-36 shrink-0 overflow-y-auto border-b border-zinc-800 p-1">
        {scripts.length === 0 ? (
          <li id="terminal-scripts-empty" className="px-2 py-3 text-xs text-zinc-500">
            No scripts yet. Write one below and save it.
          </li>
        ) : (
          scripts.map((script) => (
            <li id={`terminal-script-item-${script.id}`} key={script.id}>
              <button
                id={`terminal-script-select-${script.id}`}
                type="button"
                className={cn(
                  "flex w-full items-center rounded-md px-2 py-1.5 text-left text-xs",
                  script.id === selectedId ? "row-selected" : "text-zinc-400 hover:bg-zinc-800",
                )}
                onClick={() => select(script.id)}
              >
                <span id={`terminal-script-name-${script.id}`} className="truncate">
                  {script.name || "Untitled"}
                </span>
              </button>
            </li>
          ))
        )}
      </ul>
      <div id="terminal-scripts-editor" className="flex min-h-0 flex-1 flex-col gap-2 p-2">
        <input
          id="terminal-scripts-name"
          className="h-7 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-xs text-zinc-100"
          placeholder="Script name"
          value={name}
          onChange={(event) => {
            setName(event.target.value);
            setDirty(true);
          }}
        />
        <textarea
          id="terminal-scripts-body"
          className="min-h-0 flex-1 resize-none rounded-md border border-zinc-700 bg-zinc-950 p-2 font-mono text-xs text-zinc-100"
          placeholder="Commands to run…"
          spellCheck={false}
          value={body}
          onChange={(event) => {
            setBody(event.target.value);
            setDirty(true);
          }}
        />
        <div id="terminal-scripts-actions" className="flex flex-wrap items-center gap-1">
          <Button id="terminal-scripts-save" variant="secondary" size="xs" disabled={!dirty} onClick={save}>
            <Save id="terminal-scripts-save-icon" className="size-3.5" aria-hidden="true" />
            Save
          </Button>
          <Button
            id="terminal-scripts-run"
            variant="primary"
            size="xs"
            disabled={!canRun || body.trim().length === 0}
            title={canRun ? "Run in the active terminal" : "Open a terminal tab first"}
            onClick={() => onRun(body)}
          >
            <Play id="terminal-scripts-run-icon" className="size-3.5" aria-hidden="true" />
            Run
          </Button>
          <Button
            id="terminal-scripts-delete"
            variant="ghost"
            size="xs"
            disabled={!selected}
            title="Delete this script"
            onClick={remove}
          >
            <Trash2 id="terminal-scripts-delete-icon" className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
      </div>
    </aside>
  );
}
