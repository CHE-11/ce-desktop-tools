import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowDown, ArrowUp, FolderOpen, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type {
  FolderRule,
  FolderRuleAction,
  FolderRuleActivity,
  FolderRuleConflict,
  FolderRuleStep,
  FolderRulesState,
  WatchedFolder,
} from "../../../../types/folder-rules";
import { cn } from "../../lib/cn";
import { folderRulesActivity, folderRulesRun, folderRulesSave, folderRulesState } from "../../lib/folder-rules-client";
import { baseName, messageOf, pickedFolderPath } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: FolderRulesState = { settingsFolder: "", enabled: true, folders: [], error: null };

const ACTIONS: { id: FolderRuleAction; label: string }[] = [
  { id: "move", label: "Move to folder" },
  { id: "copy", label: "Copy to folder" },
  { id: "rename", label: "Rename in place" },
  { id: "recycle", label: "Send to Recycle Bin" },
];

const CONFLICTS: { id: FolderRuleConflict; label: string }[] = [
  { id: "number", label: "Add a number" },
  { id: "skip", label: "Skip the file" },
  { id: "overwrite", label: "Replace it" },
];

const OUTCOME_STYLES: Record<string, string> = {
  done: "text-emerald-400",
  planned: "text-sky-400",
  skipped: "text-zinc-400",
  error: "text-red-400",
};

const INPUT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100";

type Props = {
  onOpenSettings: () => void;
};

function newId(prefix: string) {
  return `${prefix}-${crypto.randomUUID().slice(0, 8)}`;
}

function newStep(): FolderRuleStep {
  return {
    id: newId("step"),
    action: "move",
    target: "",
    renamePattern: "{name}",
    onConflict: "number",
  };
}

function newRule(): FolderRule {
  return {
    id: newId("rule"),
    name: "New rule",
    enabled: true,
    match: { extensions: [], nameContains: "", olderThanDays: 0, minSizeKb: 0 },
    steps: [newStep()],
  };
}

function timeOf(at: number) {
  return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function FolderRules({ onOpenSettings }: Props) {
  const [state, setState] = useState<FolderRulesState>(EMPTY);
  const [folders, setFolders] = useState<WatchedFolder[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<FolderRuleActivity[] | null>(null);
  const [activity, setActivity] = useState<FolderRuleActivity[]>([]);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([folderRulesState(), folderRulesActivity()])
      .then(([next, log]) => {
        if (cancelled) {
          return;
        }
        setState(next);
        setFolders(next.folders);
        setSelectedId(next.folders[0]?.id ?? null);
        setActivity(log);
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    const subscription = listen<FolderRuleActivity>("folder-rules-activity", (event) => {
      setActivity((current) => [event.payload, ...current].slice(0, 200));
    });
    return () => {
      cancelled = true;
      void subscription.then((stop) => stop());
    };
  }, []);

  const selected = useMemo(() => folders.find((folder) => folder.id === selectedId) ?? null, [folders, selectedId]);
  const noFolder = state.settingsFolder.length === 0;
  const problem = error ?? state.error;

  function edit(next: WatchedFolder[]) {
    setFolders(next);
    setDirty(true);
    setPreview(null);
    setNotice(null);
  }

  function editSelected(change: (folder: WatchedFolder) => WatchedFolder) {
    if (!selected) {
      return;
    }
    edit(folders.map((folder) => (folder.id === selected.id ? change(folder) : folder)));
  }

  function editRule(ruleId: string, change: (rule: FolderRule) => FolderRule) {
    editSelected((folder) => ({
      ...folder,
      rules: folder.rules.map((rule) => (rule.id === ruleId ? change(rule) : rule)),
    }));
  }

  function moveRule(index: number, offset: number) {
    editSelected((folder) => {
      const rules = [...folder.rules];
      const target = index + offset;
      if (target < 0 || target >= rules.length) {
        return folder;
      }
      [rules[index], rules[target]] = [rules[target], rules[index]];
      return { ...folder, rules };
    });
  }

  async function save(enabled = state.enabled, next = folders) {
    setBusy(true);
    try {
      const saved = await folderRulesSave(enabled, next);
      setState(saved);
      setFolders(saved.folders);
      if (!saved.folders.some((folder) => folder.id === selectedId)) {
        setSelectedId(saved.folders[0]?.id ?? null);
      }
      setDirty(false);
      setError(null);
      setNotice("Saved. Watched folders are being watched now.");
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  function discard() {
    setFolders(state.folders);
    setDirty(false);
    setError(null);
    setPreview(null);
    if (!state.folders.some((folder) => folder.id === selectedId)) {
      setSelectedId(state.folders[0]?.id ?? null);
    }
  }

  async function addFolder() {
    const picked = await open({ multiple: false, directory: true, title: "Choose a folder to watch" });
    const path = pickedFolderPath(picked);
    if (!path) {
      return;
    }
    if (folders.some((folder) => folder.path.toLowerCase() === path.toLowerCase())) {
      setError("That folder is already in the list.");
      return;
    }
    const folder: WatchedFolder = { id: newId("folder"), path, enabled: true, rules: [] };
    edit([...folders, folder]);
    setSelectedId(folder.id);
  }

  async function chooseTarget(ruleId: string, stepId: string) {
    const picked = await open({ multiple: false, directory: true, title: "Choose the destination folder" });
    const path = pickedFolderPath(picked);
    if (path) {
      editRule(ruleId, (rule) => ({
        ...rule,
        steps: rule.steps.map((step) => (step.id === stepId ? { ...step, target: path } : step)),
      }));
    }
  }

  async function run(dryRun: boolean) {
    if (!selected) {
      return;
    }
    setBusy(true);
    setNotice(null);
    try {
      const rows = await folderRulesRun(selected.id, dryRun);
      if (dryRun) {
        setPreview(rows);
      } else {
        setPreview(null);
        const done = rows.filter((row) => row.outcome === "done").length;
        setNotice(done === 0 ? "No files matched a rule." : `Handled ${done} file${done === 1 ? "" : "s"}.`);
      }
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id="folder-rules-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="folder-rules-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="folder-rules-heading" className="min-w-0">
          <h1 id="folder-rules-title" className="text-base font-semibold text-zinc-100">
            Folder Rules
          </h1>
          <p id="folder-rules-summary" className="mt-1 text-sm text-zinc-400">
            When a file lands in a watched folder, the first matching rule moves, copies, renames, or recycles it.
            Downloads still in progress are left alone.
          </p>
        </div>
        <Button
          id="folder-rules-enabled"
          variant={state.enabled ? "primary" : "outline"}
          aria-pressed={state.enabled}
          disabled={busy || noFolder || dirty}
          title={dirty ? "Save or discard your changes first" : undefined}
          onClick={() => void save(!state.enabled, state.folders)}
        >
          {state.enabled ? "Watching on" : "Watching off"}
        </Button>
      </header>
      {noFolder ? (
        <div id="folder-rules-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="folder-rules-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep rules in folder-rules.json.
          </p>
          <Button id="folder-rules-choose-folder" variant="outline" onClick={onOpenSettings}>
            Choose settings folder
          </Button>
        </div>
      ) : null}
      {problem ? (
        <p id="folder-rules-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {notice ? (
        <p id="folder-rules-notice" className="border-b border-zinc-800 px-4 py-2 text-sm text-emerald-400">
          {notice}
        </p>
      ) : null}
      {dirty ? (
        <div id="folder-rules-dirty" className="flex items-center justify-between gap-3 border-b border-zinc-800 bg-zinc-900 px-4 py-2">
          <p id="folder-rules-dirty-copy" className="text-sm text-amber-300">
            You have unsaved changes.
          </p>
          <div id="folder-rules-dirty-actions" className="flex gap-2">
            <Button id="folder-rules-discard" variant="ghost" size="sm" disabled={busy} onClick={discard}>
              Discard
            </Button>
            <Button id="folder-rules-save" variant="primary" size="sm" disabled={busy || noFolder} onClick={() => void save()}>
              Save
            </Button>
          </div>
        </div>
      ) : null}
      <div id="folder-rules-body" className="flex min-h-0 flex-1">
        <aside id="folder-rules-folders" className="flex w-64 shrink-0 flex-col gap-2 overflow-y-auto border-r border-zinc-800 p-3">
          <div id="folder-rules-folders-heading" className="flex items-center justify-between">
            <h2 id="folder-rules-folders-title" className="text-sm font-medium text-zinc-100">
              Watched folders
            </h2>
            <Button id="folder-rules-add-folder" variant="ghost" size="xs" disabled={noFolder} onClick={() => void addFolder()}>
              <Plus id="folder-rules-add-folder-icon" className="size-3.5" aria-hidden="true" />
              Add
            </Button>
          </div>
          {folders.length === 0 ? (
            <p id="folder-rules-folders-empty" className="text-sm text-zinc-500">
              Add a folder, such as Downloads, to start.
            </p>
          ) : (
            <ul id="folder-rules-folder-list" className="flex flex-col gap-1">
              {folders.map((folder) => (
                <li id={`folder-rules-folder-${folder.id}`} key={folder.id}>
                  <button
                    id={`folder-rules-folder-${folder.id}-select`}
                    type="button"
                    onClick={() => {
                      setSelectedId(folder.id);
                      setPreview(null);
                    }}
                    title={folder.path}
                    className={cn(
                      "flex w-full flex-col rounded-md px-2 py-1.5 text-left",
                      folder.id === selectedId ? "row-selected" : "hover:bg-zinc-800",
                    )}
                  >
                    <span id={`folder-rules-folder-${folder.id}-name`} className={cn("truncate text-sm", folder.enabled ? "text-zinc-100" : "text-zinc-500")}>
                      {baseName(folder.path) || folder.path}
                    </span>
                    <span id={`folder-rules-folder-${folder.id}-meta`} className="truncate text-xs text-zinc-500">
                      {folder.rules.length} rule{folder.rules.length === 1 ? "" : "s"}
                      {folder.enabled ? "" : " · paused"}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </aside>
        <div id="folder-rules-main" className="flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
          {selected ? (
            <>
              <div id="folder-rules-selected" className="flex flex-wrap items-center gap-2">
                <FolderOpen id="folder-rules-selected-icon" className="size-4 text-zinc-400" aria-hidden="true" />
                <span id="folder-rules-selected-path" className="min-w-0 flex-1 truncate text-sm text-zinc-200" title={selected.path}>
                  {selected.path}
                </span>
                <label id="folder-rules-selected-enabled-label" className="flex items-center gap-1.5 text-sm text-zinc-300">
                  <input
                    id="folder-rules-selected-enabled"
                    type="checkbox"
                    checked={selected.enabled}
                    onChange={(event) => editSelected((folder) => ({ ...folder, enabled: event.target.checked }))}
                  />
                  Watch
                </label>
                <Button
                  id="folder-rules-preview"
                  variant="outline"
                  size="sm"
                  disabled={busy || dirty || selected.rules.length === 0}
                  title={dirty ? "Save first" : undefined}
                  onClick={() => void run(true)}
                >
                  Preview
                </Button>
                <Button
                  id="folder-rules-run"
                  variant="secondary"
                  size="sm"
                  disabled={busy || dirty || selected.rules.length === 0}
                  title={dirty ? "Save first" : undefined}
                  onClick={() => void run(false)}
                >
                  Run now
                </Button>
                <Button
                  id="folder-rules-remove-folder"
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    edit(folders.filter((folder) => folder.id !== selected.id));
                    setSelectedId(folders.find((folder) => folder.id !== selected.id)?.id ?? null);
                  }}
                >
                  <Trash2 id="folder-rules-remove-folder-icon" className="size-3.5" aria-hidden="true" />
                  Remove folder
                </Button>
              </div>
              {preview ? <PreviewTable rows={preview} /> : null}
              <div id="folder-rules-rules" className="flex flex-col gap-3">
                {selected.rules.length === 0 ? (
                  <p id="folder-rules-rules-empty" className="text-sm text-zinc-500">
                    This folder has no rules yet.
                  </p>
                ) : null}
                {selected.rules.map((rule, index) => (
                  <RuleEditor
                    key={rule.id}
                    rule={rule}
                    first={index === 0}
                    last={index === selected.rules.length - 1}
                    onChange={(change) => editRule(rule.id, change)}
                    onMove={(offset) => moveRule(index, offset)}
                    onRemove={() =>
                      editSelected((folder) => ({ ...folder, rules: folder.rules.filter((each) => each.id !== rule.id) }))
                    }
                    onBrowse={(stepId) => void chooseTarget(rule.id, stepId)}
                  />
                ))}
                <div id="folder-rules-add-rule-row">
                  <Button
                    id="folder-rules-add-rule"
                    variant="outline"
                    size="sm"
                    onClick={() => editSelected((folder) => ({ ...folder, rules: [...folder.rules, newRule()] }))}
                  >
                    <Plus id="folder-rules-add-rule-icon" className="size-3.5" aria-hidden="true" />
                    Add rule
                  </Button>
                </div>
              </div>
            </>
          ) : (
            <p id="folder-rules-none-selected" className="text-sm text-zinc-500">
              Choose or add a watched folder.
            </p>
          )}
          <ActivityLog rows={activity} />
        </div>
      </div>
    </section>
  );
}

type RuleEditorProps = {
  rule: FolderRule;
  first: boolean;
  last: boolean;
  onChange: (change: (rule: FolderRule) => FolderRule) => void;
  onMove: (offset: number) => void;
  onRemove: () => void;
  onBrowse: (stepId: string) => void;
};

function RuleEditor({ rule, first, last, onChange, onMove, onRemove, onBrowse }: RuleEditorProps) {
  const [extensions, setExtensions] = useState(rule.match.extensions.join(", "));
  const base = `folder-rules-rule-${rule.id}`;

  useEffect(() => {
    setExtensions(rule.match.extensions.join(", "));
  }, [rule.id, rule.match.extensions]);

  function commitExtensions(text: string) {
    const list = text
      .split(/[,;\s]+/)
      .map((part) => part.trim().replace(/^\*?\./, "").toLowerCase())
      .filter((part, index, all) => part.length > 0 && all.indexOf(part) === index);
    onChange((current) => ({ ...current, match: { ...current.match, extensions: list } }));
  }

  function numberOf(value: string) {
    const parsed = Number.parseInt(value, 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  }

  function editStep(stepId: string, change: (step: FolderRuleStep) => FolderRuleStep) {
    onChange((current) => ({
      ...current,
      steps: current.steps.map((step) => (step.id === stepId ? change(step) : step)),
    }));
  }

  function moveStep(index: number, offset: number) {
    onChange((current) => {
      const steps = [...current.steps];
      const target = index + offset;
      if (target < 0 || target >= steps.length) {
        return current;
      }
      [steps[index], steps[target]] = [steps[target], steps[index]];
      return { ...current, steps };
    });
  }

  return (
    <article id={base} className={cn("flex flex-col gap-3 rounded-lg border border-zinc-800 bg-zinc-900 p-3", !rule.enabled && "opacity-60")}>
      <div id={`${base}-header`} className="flex items-center gap-2">
        <input
          id={`${base}-enabled`}
          type="checkbox"
          checked={rule.enabled}
          aria-label="Rule enabled"
          onChange={(event) => onChange((current) => ({ ...current, enabled: event.target.checked }))}
        />
        <label id={`${base}-name-label`} htmlFor={`${base}-name`} className="sr-only">
          Rule name
        </label>
        <input
          id={`${base}-name`}
          value={rule.name}
          onChange={(event) => onChange((current) => ({ ...current, name: event.target.value }))}
          className={cn(INPUT, "min-w-0 flex-1 font-medium")}
        />
        <Button id={`${base}-up`} variant="ghost" size="xs" disabled={first} title="Move up" onClick={() => onMove(-1)}>
          <ArrowUp id={`${base}-up-icon`} className="size-3.5" aria-hidden="true" />
        </Button>
        <Button id={`${base}-down`} variant="ghost" size="xs" disabled={last} title="Move down" onClick={() => onMove(1)}>
          <ArrowDown id={`${base}-down-icon`} className="size-3.5" aria-hidden="true" />
        </Button>
        <Button id={`${base}-remove`} variant="ghost" size="xs" title="Delete rule" onClick={onRemove}>
          <Trash2 id={`${base}-remove-icon`} className="size-3.5" aria-hidden="true" />
        </Button>
      </div>
      <fieldset id={`${base}-match`} className="grid grid-cols-2 gap-x-4 gap-y-2 lg:grid-cols-4">
        <legend id={`${base}-match-legend`} className="mb-1 text-xs font-medium tracking-wide text-zinc-500 uppercase">
          When the file
        </legend>
        <label id={`${base}-extensions-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
          Has extension
          <input
            id={`${base}-extensions`}
            value={extensions}
            placeholder="pdf, docx"
            onChange={(event) => setExtensions(event.target.value)}
            onBlur={(event) => commitExtensions(event.target.value)}
            className={INPUT}
          />
        </label>
        <label id={`${base}-name-contains-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
          Name contains (* and ? work)
          <input
            id={`${base}-name-contains`}
            value={rule.match.nameContains}
            placeholder="invoice"
            onChange={(event) =>
              onChange((current) => ({ ...current, match: { ...current.match, nameContains: event.target.value } }))
            }
            className={INPUT}
          />
        </label>
        <label id={`${base}-older-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
          Older than (days)
          <input
            id={`${base}-older`}
            type="number"
            min={0}
            value={rule.match.olderThanDays || ""}
            placeholder="Any age"
            onChange={(event) =>
              onChange((current) => ({ ...current, match: { ...current.match, olderThanDays: numberOf(event.target.value) } }))
            }
            className={INPUT}
          />
        </label>
        <label id={`${base}-size-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
          At least (KB)
          <input
            id={`${base}-size`}
            type="number"
            min={0}
            value={rule.match.minSizeKb || ""}
            placeholder="Any size"
            onChange={(event) =>
              onChange((current) => ({ ...current, match: { ...current.match, minSizeKb: numberOf(event.target.value) } }))
            }
            className={INPUT}
          />
        </label>
      </fieldset>
      <fieldset id={`${base}-do`} className="flex flex-col gap-3">
        <legend id={`${base}-do-legend`} className="text-xs font-medium tracking-wide text-zinc-500 uppercase">
          Then
        </legend>
        {rule.steps.map((step, index) => {
          const stepId = `${base}-step-${step.id}`;
          const needsTarget = step.action === "move" || step.action === "copy";
          return (
            <div id={stepId} key={step.id} className={cn("flex flex-wrap items-end gap-x-4 gap-y-2", index > 0 && "border-t border-zinc-800 pt-3")}>
              <label id={`${stepId}-action-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
                Action
                <select
                  id={`${stepId}-action`}
                  value={step.action}
                  onChange={(event) =>
                    editStep(step.id, (current) => ({ ...current, action: event.target.value as FolderRuleAction }))
                  }
                  className={INPUT}
                >
                  {ACTIONS.map((action) => (
                    <option id={`${stepId}-action-${action.id}`} key={action.id} value={action.id}>
                      {action.label}
                    </option>
                  ))}
                </select>
              </label>
              {needsTarget ? (
                <label id={`${stepId}-target-label`} className="flex min-w-64 flex-1 flex-col gap-1 text-xs text-zinc-400">
                  Destination ({"{yyyy}"} and other date pictures work)
                  <span id={`${stepId}-target-row`} className="flex gap-2">
                    <input
                      id={`${stepId}-target`}
                      value={step.target}
                      placeholder="D:\Sorted\PDFs"
                      onChange={(event) => editStep(step.id, (current) => ({ ...current, target: event.target.value }))}
                      className={cn(INPUT, "min-w-0 flex-1")}
                    />
                    <Button id={`${stepId}-target-browse`} variant="outline" size="sm" className="h-8" onClick={() => onBrowse(step.id)}>
                      Browse
                    </Button>
                  </span>
                </label>
              ) : null}
              {step.action === "rename" ? (
                <label id={`${stepId}-pattern-label`} className="flex min-w-64 flex-1 flex-col gap-1 text-xs text-zinc-400">
                  New name ({"{name}"}, {"{ext}"}, {"{yyyy-MM-dd}"})
                  <input
                    id={`${stepId}-pattern`}
                    value={step.renamePattern}
                    placeholder="{yyyy-MM-dd} {name}"
                    onChange={(event) => editStep(step.id, (current) => ({ ...current, renamePattern: event.target.value }))}
                    className={INPUT}
                  />
                </label>
              ) : null}
              {step.action !== "recycle" ? (
                <label id={`${stepId}-conflict-label`} className="flex flex-col gap-1 text-xs text-zinc-400">
                  If the name is taken
                  <select
                    id={`${stepId}-conflict`}
                    value={step.onConflict}
                    onChange={(event) =>
                      editStep(step.id, (current) => ({ ...current, onConflict: event.target.value as FolderRuleConflict }))
                    }
                    className={INPUT}
                  >
                    {CONFLICTS.map((conflict) => (
                      <option id={`${stepId}-conflict-${conflict.id}`} key={conflict.id} value={conflict.id}>
                        {conflict.label}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}
              <div id={`${stepId}-order`} className="flex gap-1">
                <Button id={`${stepId}-up`} variant="ghost" size="xs" disabled={index === 0} title="Move step up" onClick={() => moveStep(index, -1)}>
                  <ArrowUp id={`${stepId}-up-icon`} className="size-3.5" aria-hidden="true" />
                </Button>
                <Button
                  id={`${stepId}-down`}
                  variant="ghost"
                  size="xs"
                  disabled={index === rule.steps.length - 1}
                  title="Move step down"
                  onClick={() => moveStep(index, 1)}
                >
                  <ArrowDown id={`${stepId}-down-icon`} className="size-3.5" aria-hidden="true" />
                </Button>
                <Button
                  id={`${stepId}-remove`}
                  variant="ghost"
                  size="xs"
                  disabled={rule.steps.length === 1}
                  title={rule.steps.length === 1 ? "A rule needs a then step" : "Remove step"}
                  onClick={() => onChange((current) => ({ ...current, steps: current.steps.filter((each) => each.id !== step.id) }))}
                >
                  <Trash2 id={`${stepId}-remove-icon`} className="size-3.5" aria-hidden="true" />
                </Button>
              </div>
            </div>
          );
        })}
        <div id={`${base}-add-step-row`}>
          <Button
            id={`${base}-add-step`}
            variant="outline"
            size="sm"
            disabled={rule.steps.length >= 8}
            title={rule.steps.length >= 8 ? "A rule can have 8 steps" : undefined}
            onClick={() => onChange((current) => ({ ...current, steps: [...current.steps, newStep()] }))}
          >
            <Plus id={`${base}-add-step-icon`} className="size-3.5" aria-hidden="true" />
            Add then
          </Button>
        </div>
      </fieldset>
    </article>
  );
}

function resultOf(row: FolderRuleActivity) {
  if (row.outcome === "error") {
    return row.detail;
  }
  if (!row.to) {
    return row.action === "recycle" || row.action.endsWith(", recycle") ? "Recycle Bin" : row.detail;
  }
  return row.to;
}

function activityDetail(row: FolderRuleActivity) {
  if (row.outcome === "error") {
    return row.detail;
  }
  if (row.to && row.detail) {
    return `${row.detail} · ${baseName(row.to)}`;
  }
  return row.detail || (row.to ? baseName(row.to) : "");
}

function PreviewTable({ rows }: { rows: FolderRuleActivity[] }) {
  return (
    <section id="folder-rules-preview-table" className="flex flex-col gap-2 rounded-lg border border-sky-900 bg-zinc-900 p-3">
      <h2 id="folder-rules-preview-title" className="text-sm font-medium text-zinc-100">
        Preview: {rows.length === 0 ? "no files match" : `${rows.length} file${rows.length === 1 ? "" : "s"}`}
      </h2>
      {rows.length > 0 ? (
        <table id="folder-rules-preview-grid" className="w-full table-fixed text-left text-sm">
          <thead id="folder-rules-preview-head" className="text-xs text-zinc-500">
            <tr id="folder-rules-preview-head-row">
              <th id="folder-rules-preview-head-file" className="w-1/3 py-1 font-medium">File</th>
              <th id="folder-rules-preview-head-rule" className="w-1/6 py-1 font-medium">Rule</th>
              <th id="folder-rules-preview-head-to" className="py-1 font-medium">Result</th>
            </tr>
          </thead>
          <tbody id="folder-rules-preview-body">
            {rows.map((row, index) => (
              <tr id={`folder-rules-preview-row-${index}`} key={`${row.file}-${index}`} className="border-t border-zinc-800">
                <td id={`folder-rules-preview-row-${index}-file`} className="truncate py-1 pr-2 text-zinc-200" title={row.file}>
                  {row.file}
                </td>
                <td id={`folder-rules-preview-row-${index}-rule`} className="truncate py-1 pr-2 text-zinc-400">
                  {row.rule}
                </td>
                <td
                  id={`folder-rules-preview-row-${index}-to`}
                  className={cn("truncate py-1", OUTCOME_STYLES[row.outcome])}
                  title={row.detail || row.to}
                >
                  {resultOf(row)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </section>
  );
}

function ActivityLog({ rows }: { rows: FolderRuleActivity[] }) {
  return (
    <section id="folder-rules-activity" className="mt-auto flex flex-col gap-2 border-t border-zinc-800 pt-4">
      <h2 id="folder-rules-activity-title" className="text-sm font-medium text-zinc-100">
        Activity this session
      </h2>
      {rows.length === 0 ? (
        <p id="folder-rules-activity-empty" className="text-sm text-zinc-500">
          Nothing has been handled yet.
        </p>
      ) : (
        <ul id="folder-rules-activity-list" className="flex max-h-56 flex-col gap-1 overflow-y-auto">
          {rows.map((row, index) => (
            <li id={`folder-rules-activity-${index}`} key={`${row.at}-${index}`} className="flex items-baseline gap-3 text-sm">
              <span id={`folder-rules-activity-${index}-time`} className="shrink-0 font-mono text-xs text-zinc-500">
                {timeOf(row.at)}
              </span>
              <span id={`folder-rules-activity-${index}-outcome`} className={cn("w-16 shrink-0 text-xs", OUTCOME_STYLES[row.outcome])}>
                {row.outcome}
              </span>
              <span id={`folder-rules-activity-${index}-file`} className="min-w-0 flex-1 truncate text-zinc-200" title={row.to || row.file}>
                {row.file}
                <span id={`folder-rules-activity-${index}-detail`} className="text-zinc-500">
                  {" "}
                  · {row.rule} · {activityDetail(row)}
                </span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
