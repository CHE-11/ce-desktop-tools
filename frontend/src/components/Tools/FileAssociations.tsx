import { open } from "@tauri-apps/plugin-dialog";
import { ExternalLink, FileCog, Pencil, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type {
  ExtensionHandler,
  FileAssociation,
  FileAssociationStatus,
  FileAssociationsState,
  FileAssociationView,
} from "../../../../types/file-associations";
import { cn } from "../../lib/cn";
import {
  fileAssocApply,
  fileAssocList,
  fileAssocLookup,
  fileAssocOpenDefaultApps,
  fileAssocPreview,
  fileAssocRemove,
} from "../../lib/file-associations-client";
import { baseName, messageOf, pickedFolderPath } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: FileAssociationsState = { settingsFolder: "", associations: [], error: null };

const INPUT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100";

const STATUS: Record<FileAssociationStatus, { label: string; className: string }> = {
  applied: { label: "Applied", className: "text-emerald-400" },
  userChoice: { label: "Windows default set by user", className: "text-amber-300" },
  notApplied: { label: "Not applied", className: "text-zinc-500" },
};

const ARG_HINTS: { token: string; detail: string }[] = [
  { token: '"%1"', detail: "The file that was opened" },
  { token: '"%L"', detail: "The long path of the file" },
  { token: "%*", detail: "Every extra argument" },
];

type Props = {
  onOpenSettings: () => void;
};

function blankDraft(): FileAssociation {
  return {
    id: "",
    extension: "",
    programPath: "",
    arguments: "",
    label: "",
    iconPath: null,
    previousProgId: null,
  };
}

export function FileAssociations({ onOpenSettings }: Props) {
  const [state, setState] = useState<FileAssociationsState>(EMPTY);
  const [draft, setDraft] = useState<FileAssociation | null>(null);
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pendingRemoveId, setPendingRemoveId] = useState<string | null>(null);
  const removeTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(removeTimer.current), []);

  useEffect(() => {
    let cancelled = false;
    void fileAssocList()
      .then((next) => {
        if (!cancelled) {
          setState(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!draft || !draft.programPath.trim()) {
      setPreview("");
      return;
    }
    let cancelled = false;
    const sample = `C:\\Users\\you\\example${draft.extension ? `.${draft.extension.replace(/^[*.]+/, "")}` : ".txt"}`;
    void fileAssocPreview(draft.programPath, draft.arguments, sample)
      .then((line) => {
        if (!cancelled) {
          setPreview(line);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [draft]);

  const noFolder = state.settingsFolder.length === 0;
  const problem = error ?? state.error;

  async function chooseProgram() {
    const picked = await open({
      multiple: false,
      title: "Choose the program that opens these files",
      filters: [{ name: "Programs", extensions: ["exe", "bat", "cmd", "com"] }],
    });
    const path = pickedFolderPath(picked);
    if (path && draft) {
      setDraft({ ...draft, programPath: path });
    }
  }

  function insertHint(token: string) {
    if (!draft) {
      return;
    }
    const current = draft.arguments.trim();
    setDraft({ ...draft, arguments: current ? `${current} ${token}` : token });
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!draft) {
      return;
    }
    setBusy(true);
    try {
      const next = await fileAssocApply(draft);
      setState(next);
      const saved = next.associations.find((item) => item.extension === normalizedExt(draft.extension));
      setDraft(null);
      setError(null);
      setNotice(
        saved?.status === "userChoice"
          ? `Saved. Windows still uses ${saved.userChoiceProgId} because a default was picked in Settings. Choose it in Default Apps or "Open with".`
          : "Saved. Explorer now opens these files with the chosen program.",
      );
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  function askRemove(id: string) {
    if (pendingRemoveId === id) {
      window.clearTimeout(removeTimer.current);
      setPendingRemoveId(null);
      void remove(id);
      return;
    }
    setPendingRemoveId(id);
    window.clearTimeout(removeTimer.current);
    removeTimer.current = window.setTimeout(() => setPendingRemoveId(null), 3000);
  }

  async function remove(id: string) {
    setBusy(true);
    try {
      setState(await fileAssocRemove(id));
      setError(null);
      setNotice("Removed. The previous handler was restored.");
      if (draft?.id === id) {
        setDraft(null);
      }
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  async function openDefaultApps() {
    try {
      await fileAssocOpenDefaultApps();
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  return (
    <section id="file-assoc-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="file-assoc-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="file-assoc-heading" className="min-w-0">
          <h1 id="file-assoc-title" className="text-base font-semibold text-zinc-100">
            File Associations
          </h1>
          <p id="file-assoc-summary" className="mt-1 text-sm text-zinc-400">
            Choose which program opens a file type, with your own command-line arguments. Changes apply to your
            Windows account only.
          </p>
        </div>
        <div id="file-assoc-header-actions" className="flex shrink-0 gap-2">
          <Button id="file-assoc-default-apps" variant="ghost" onClick={() => void openDefaultApps()}>
            <ExternalLink id="file-assoc-default-apps-icon" className="size-3.5" aria-hidden="true" />
            Default Apps
          </Button>
          <Button
            id="file-assoc-add"
            variant="primary"
            disabled={noFolder || busy}
            onClick={() => {
              setDraft(blankDraft());
              setNotice(null);
            }}
          >
            <Plus id="file-assoc-add-icon" className="size-3.5" aria-hidden="true" />
            Add association
          </Button>
        </div>
      </header>
      {noFolder ? (
        <div id="file-assoc-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="file-assoc-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep associations in file-associations.json.
          </p>
          <Button id="file-assoc-choose-folder" variant="outline" onClick={onOpenSettings}>
            Choose settings folder
          </Button>
        </div>
      ) : null}
      {problem ? (
        <p id="file-assoc-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {notice ? (
        <p id="file-assoc-notice" className="border-b border-zinc-800 px-4 py-2 text-sm text-emerald-400">
          {notice}
        </p>
      ) : null}
      <div id="file-assoc-body" className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
        {draft ? (
          <form
            id="file-assoc-editor"
            onSubmit={(event) => void submit(event)}
            className="flex flex-col gap-3 rounded-lg border border-zinc-700 bg-zinc-900 p-4"
          >
            <h2 id="file-assoc-editor-title" className="text-sm font-medium text-zinc-100">
              {draft.id ? `Edit ${draft.extension}` : "New association"}
            </h2>
            <div id="file-assoc-editor-row" className="grid grid-cols-[8rem_1fr] gap-3">
              <label id="file-assoc-ext-label" className="flex flex-col gap-1 text-xs text-zinc-400">
                Extension
                <input
                  id="file-assoc-ext"
                  className={INPUT}
                  placeholder=".log"
                  value={draft.extension}
                  autoFocus
                  onChange={(event) => setDraft({ ...draft, extension: event.target.value })}
                />
              </label>
              <label id="file-assoc-label-label" className="flex flex-col gap-1 text-xs text-zinc-400">
                Type name (optional)
                <input
                  id="file-assoc-label"
                  className={INPUT}
                  placeholder="Log file"
                  value={draft.label}
                  onChange={(event) => setDraft({ ...draft, label: event.target.value })}
                />
              </label>
            </div>
            <label id="file-assoc-program-label" className="flex flex-col gap-1 text-xs text-zinc-400">
              Program
              <div id="file-assoc-program-row" className="flex gap-2">
                <input
                  id="file-assoc-program"
                  className={cn(INPUT, "min-w-0 flex-1")}
                  placeholder="C:\Program Files\Notepad++\notepad++.exe"
                  value={draft.programPath}
                  onChange={(event) => setDraft({ ...draft, programPath: event.target.value })}
                />
                <Button id="file-assoc-program-browse" variant="outline" onClick={() => void chooseProgram()}>
                  Browse
                </Button>
              </div>
            </label>
            <label id="file-assoc-args-label" className="flex flex-col gap-1 text-xs text-zinc-400">
              Arguments
              <input
                id="file-assoc-args"
                className={cn(INPUT, "font-mono")}
                placeholder='-multiInst "%1"'
                value={draft.arguments}
                onChange={(event) => setDraft({ ...draft, arguments: event.target.value })}
              />
            </label>
            <div id="file-assoc-hints" className="flex flex-wrap items-center gap-2 text-xs text-zinc-500">
              <span id="file-assoc-hints-copy">Insert:</span>
              {ARG_HINTS.map((hint) => (
                <Button
                  id={`file-assoc-hint-${hint.token.replace(/\W/g, "")}`}
                  key={hint.token}
                  variant="secondary"
                  size="xs"
                  title={hint.detail}
                  onClick={() => insertHint(hint.token)}
                >
                  <code id={`file-assoc-hint-${hint.token.replace(/\W/g, "")}-code`}>{hint.token}</code>
                </Button>
              ))}
              <span id="file-assoc-hints-note">Without %1, the file path is added to the end.</span>
            </div>
            <label id="file-assoc-icon-label" className="flex flex-col gap-1 text-xs text-zinc-400">
              Icon (optional, defaults to the program icon)
              <input
                id="file-assoc-icon"
                className={INPUT}
                placeholder="C:\path\icon.ico,0"
                value={draft.iconPath ?? ""}
                onChange={(event) => setDraft({ ...draft, iconPath: event.target.value || null })}
              />
            </label>
            {preview ? (
              <div id="file-assoc-preview" className="flex flex-col gap-1">
                <span id="file-assoc-preview-label" className="text-xs text-zinc-500">
                  Command Explorer will run
                </span>
                <code id="file-assoc-preview-line" className="break-all rounded-md bg-zinc-950 px-2 py-1.5 font-mono text-xs text-sky-400">
                  {preview}
                </code>
              </div>
            ) : null}
            <div id="file-assoc-editor-actions" className="flex justify-end gap-2">
              <Button id="file-assoc-cancel" variant="ghost" disabled={busy} onClick={() => setDraft(null)}>
                Cancel
              </Button>
              <Button id="file-assoc-save" variant="primary" type="submit" disabled={busy}>
                Save and apply
              </Button>
            </div>
          </form>
        ) : null}
        <AssociationTable
          associations={state.associations}
          busy={busy}
          pendingRemoveId={pendingRemoveId}
          onEdit={(item) => {
            setDraft({
              id: item.id,
              extension: item.extension,
              programPath: item.programPath,
              arguments: item.arguments,
              label: item.label,
              iconPath: item.iconPath,
              previousProgId: item.previousProgId,
            });
            setNotice(null);
          }}
          onRemove={askRemove}
          onDefaultApps={() => void openDefaultApps()}
        />
        <HandlerLookup />
      </div>
    </section>
  );
}

function normalizedExt(value: string) {
  return `.${value.trim().replace(/^[*.]+/, "").toLowerCase()}`;
}

type TableProps = {
  associations: FileAssociationView[];
  busy: boolean;
  pendingRemoveId: string | null;
  onEdit: (item: FileAssociationView) => void;
  onRemove: (id: string) => void;
  onDefaultApps: () => void;
};

function AssociationTable({ associations, busy, pendingRemoveId, onEdit, onRemove, onDefaultApps }: TableProps) {
  if (associations.length === 0) {
    return (
      <p id="file-assoc-empty" className="flex items-center gap-2 text-sm text-zinc-500">
        <FileCog id="file-assoc-empty-icon" className="size-4" aria-hidden="true" />
        No associations yet. Add one to open a file type with a specific program.
      </p>
    );
  }
  return (
    <table id="file-assoc-table" className="w-full table-fixed text-left text-sm">
      <thead id="file-assoc-table-head" className="text-xs text-zinc-500">
        <tr id="file-assoc-table-head-row">
          <th id="file-assoc-head-ext" className="w-24 py-1.5 font-medium">Extension</th>
          <th id="file-assoc-head-program" className="w-1/4 py-1.5 font-medium">Program</th>
          <th id="file-assoc-head-args" className="py-1.5 font-medium">Arguments</th>
          <th id="file-assoc-head-status" className="w-44 py-1.5 font-medium">Status</th>
          <th id="file-assoc-head-actions" className="w-36 py-1.5 font-medium" />
        </tr>
      </thead>
      <tbody id="file-assoc-table-body">
        {associations.map((item) => {
          const status = STATUS[item.status];
          const confirming = pendingRemoveId === item.id;
          return (
            <tr id={`file-assoc-row-${item.id}`} key={item.id} className="border-t border-zinc-800">
              <td id={`file-assoc-row-${item.id}-ext`} className="py-2 pr-2 font-mono text-zinc-100">
                {item.extension}
              </td>
              <td id={`file-assoc-row-${item.id}-program`} className="truncate py-2 pr-2 text-zinc-200" title={item.programPath}>
                {baseName(item.programPath)}
              </td>
              <td id={`file-assoc-row-${item.id}-args`} className="truncate py-2 pr-2 font-mono text-xs text-zinc-400" title={item.command}>
                {item.arguments || <span id={`file-assoc-row-${item.id}-args-none`} className="text-zinc-600">"%1"</span>}
              </td>
              <td
                id={`file-assoc-row-${item.id}-status`}
                className={cn("py-2 pr-2 text-xs", status.className)}
                title={item.userChoiceProgId ? `Windows is using ${item.userChoiceProgId}` : undefined}
              >
                {status.label}
              </td>
              <td id={`file-assoc-row-${item.id}-actions`} className="py-2">
                <div id={`file-assoc-row-${item.id}-actions-group`} className="flex justify-end gap-1">
                  {item.status === "userChoice" ? (
                    <Button
                      id={`file-assoc-row-${item.id}-default-apps`}
                      variant="ghost"
                      size="xs"
                      title="Open Default Apps to pick this program"
                      onClick={onDefaultApps}
                    >
                      <ExternalLink id={`file-assoc-row-${item.id}-default-apps-icon`} className="size-3.5" aria-hidden="true" />
                    </Button>
                  ) : null}
                  <Button
                    id={`file-assoc-row-${item.id}-edit`}
                    variant="ghost"
                    size="xs"
                    title="Edit"
                    aria-label="Edit association"
                    disabled={busy}
                    onClick={() => onEdit(item)}
                  >
                    <Pencil id={`file-assoc-row-${item.id}-edit-icon`} className="size-3.5" aria-hidden="true" />
                  </Button>
                  <Button
                    id={`file-assoc-row-${item.id}-remove`}
                    variant={confirming ? "destructive" : "ghost"}
                    size="xs"
                    title={confirming ? "Click again to remove" : "Remove and restore the previous handler"}
                    aria-label={confirming ? "Confirm remove association" : "Remove association"}
                    disabled={busy}
                    onClick={() => onRemove(item.id)}
                  >
                    {confirming ? (
                      <span id={`file-assoc-row-${item.id}-remove-label`}>Remove?</span>
                    ) : (
                      <Trash2 id={`file-assoc-row-${item.id}-remove-icon`} className="size-3.5" aria-hidden="true" />
                    )}
                  </Button>
                </div>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

const SOURCE_LABELS: Record<ExtensionHandler["source"], string> = {
  userChoice: "Picked in Windows Settings",
  classes: "Registered by a program",
  none: "No program is registered",
};

function HandlerLookup() {
  const [extension, setExtension] = useState("");
  const [handler, setHandler] = useState<ExtensionHandler | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function lookup(event: FormEvent) {
    event.preventDefault();
    try {
      setHandler(await fileAssocLookup(extension));
      setError(null);
    } catch (caught) {
      setHandler(null);
      setError(messageOf(caught));
    }
  }

  return (
    <section id="file-assoc-lookup" className="flex flex-col gap-2 rounded-lg border border-zinc-800 p-3">
      <h2 id="file-assoc-lookup-title" className="text-sm font-medium text-zinc-100">
        What opens a file type now?
      </h2>
      <form id="file-assoc-lookup-form" className="flex gap-2" onSubmit={(event) => void lookup(event)}>
        <input
          id="file-assoc-lookup-input"
          className={cn(INPUT, "w-40")}
          placeholder=".pdf"
          value={extension}
          onChange={(event) => setExtension(event.target.value)}
        />
        <Button id="file-assoc-lookup-submit" variant="secondary" type="submit" disabled={!extension.trim()}>
          <Search id="file-assoc-lookup-submit-icon" className="size-3.5" aria-hidden="true" />
          Look up
        </Button>
      </form>
      {error ? (
        <p id="file-assoc-lookup-error" className="text-sm text-red-400">
          {error}
        </p>
      ) : null}
      {handler ? (
        <dl id="file-assoc-lookup-result" className="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-1 text-sm">
          <dt id="file-assoc-lookup-source-label" className="text-zinc-500">Source</dt>
          <dd id="file-assoc-lookup-source" className="text-zinc-200">{SOURCE_LABELS[handler.source]}</dd>
          <dt id="file-assoc-lookup-progid-label" className="text-zinc-500">ProgID</dt>
          <dd id="file-assoc-lookup-progid" className="break-all font-mono text-xs text-zinc-200">{handler.progId ?? "None"}</dd>
          <dt id="file-assoc-lookup-name-label" className="text-zinc-500">Type name</dt>
          <dd id="file-assoc-lookup-name" className="text-zinc-200">{handler.friendlyName ?? "None"}</dd>
          <dt id="file-assoc-lookup-command-label" className="text-zinc-500">Command</dt>
          <dd id="file-assoc-lookup-command" className="break-all font-mono text-xs text-sky-400">{handler.command ?? "None"}</dd>
        </dl>
      ) : null}
    </section>
  );
}
