import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";
import type { RenameKind, RenameName, RenameRow } from "../../../../types/rename";
import { applyRename, previewRename } from "../../lib/rename-client";
import { cn } from "../../lib/cn";
import { baseName, messageOf, pickedFolderPath } from "../../lib/message";
import { Button } from "../Button";

const KINDS: { id: RenameKind; label: string }[] = [
  { id: "prefix", label: "Prefix" },
  { id: "suffix", label: "Suffix" },
  { id: "replace", label: "Replace" },
  { id: "number", label: "Number" },
];

export function BatchRename() {
  const [paths, setPaths] = useState<string[]>([]);
  const [kind, setKind] = useState<RenameKind>("prefix");
  const [text, setText] = useState("");
  const [replacement, setReplacement] = useState("");
  const [start, setStart] = useState(1);
  const [pad, setPad] = useState(2);
  const [names, setNames] = useState<RenameName[]>([]);
  const [rows, setRows] = useState<RenameRow[]>([]);
  const [applying, setApplying] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const applyingRef = useRef(applying);
  const requestRef = useRef(0);
  const refreshRef = useRef<(nextPaths: string[]) => Promise<void>>(async () => undefined);
  applyingRef.current = applying;

  const refresh = async (
    nextPaths = paths,
    nextKind = kind,
    nextText = text,
    nextStart = start,
    nextPad = pad,
    nextReplacement = replacement,
    nextNames = names,
  ) => {
    if (nextPaths.length === 0) {
      requestRef.current += 1;
      setRows([]);
      return;
    }
    const request = ++requestRef.current;
    setError(null);
    try {
      const nextRows = await previewRename({
        paths: nextPaths,
        kind: nextKind,
        text: nextText,
        replacement: nextReplacement,
        start: nextStart,
        pad: nextPad,
        names: nextNames,
      });
      if (request !== requestRef.current) {
        return;
      }
      setRows(nextRows);
    } catch (caught) {
      if (request === requestRef.current) {
        setError(messageOf(caught));
      }
    }
  };
  refreshRef.current = async (nextPaths) => {
    setNames([]);
    await refresh(nextPaths, kind, text, start, pad, replacement, []);
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (applyingRef.current || payload.type === "leave") {
          setDragOver(false);
          return;
        }
        const over = overRenameTool(payload.position);
        if (payload.type === "drop") {
          setDragOver(false);
          if (!over) {
            return;
          }
          const next = payload.paths.map((path) => path.trim()).filter((path) => path.length > 0);
          if (next.length === 0) {
            return;
          }
          setPaths(next);
          void refreshRef.current(next);
          return;
        }
        setDragOver(over);
      })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const chooseFiles = async () => {
    const picked = await open({ multiple: true, directory: false, title: "Choose files to rename" });
    const next = pickedPaths(picked);
    if (next.length === 0) {
      return;
    }
    setPaths(next);
    setNames([]);
    await refresh(next, kind, text, start, pad, replacement, []);
  };

  const chooseFolder = async () => {
    const picked = await open({ multiple: false, directory: true, title: "Choose a folder of files to rename" });
    const folder = pickedFolderPath(picked);
    if (!folder) {
      return;
    }
    const next = [folder];
    setPaths(next);
    setNames([]);
    await refresh(next, kind, text, start, pad, replacement, []);
  };

  const editName = (from: string, name: string) => {
    const nextNames = [...names.filter((item) => item.from !== from), { from, name }];
    setNames(nextNames);
    void refresh(paths, kind, text, start, pad, replacement, nextNames);
  };

  const rename = async () => {
    const ready = rows.filter((row) => row.ok);
    if (ready.length === 0) {
      return;
    }
    setApplying(true);
    setError(null);
    try {
      const done = await applyRename(ready);
      const failed = done.filter((row) => !row.ok);
      setRows(done);
      setNames([]);
      setPaths(failed.length === 0 ? [] : failed.map((row) => row.from));
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setApplying(false);
    }
  };

  const readyCount = rows.filter((row) => row.ok).length;

  return (
    <section
      id="rename-tool"
      className={cn("relative flex min-h-0 min-w-0 flex-1 flex-col", dragOver && "ring-2 ring-inset ring-sky-500")}
    >
      <header id="rename-header" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="rename-heading" className="min-w-0">
          <h1 id="rename-title" className="text-base font-semibold text-zinc-100">
            Batch rename
          </h1>
          <p id="rename-hint" className="mt-1 text-xs text-zinc-500">
            Drop files or folders here, then rename only the rows that are ready.
          </p>
        </div>
        <div id="rename-pickers" className="flex shrink-0 items-center gap-2">
          <Button id="rename-choose-files" variant="outline" disabled={applying} onClick={() => void chooseFiles()}>
            <span id="rename-choose-files-label">Choose files</span>
          </Button>
          <Button id="rename-choose-folder" variant="outline" disabled={applying} onClick={() => void chooseFolder()}>
            <span id="rename-choose-folder-label">Choose folder</span>
          </Button>
        </div>
      </header>
      <div id="rename-pattern" className="flex flex-wrap items-end gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="rename-kind" className="flex overflow-hidden rounded-md border border-zinc-700" role="group" aria-label="Rename pattern">
          {KINDS.map((option) => (
            <button
              id={`rename-kind-${option.id}`}
              key={option.id}
              type="button"
              aria-pressed={kind === option.id}
              disabled={applying}
              onClick={() => {
                setKind(option.id);
                void refresh(paths, option.id, text, start, pad, replacement, names);
              }}
              className={cn("h-8 px-3 text-sm", kind === option.id ? "bg-sky-600 text-white" : "bg-zinc-950 text-zinc-300 hover:bg-zinc-800")}
            >
              <span id={`rename-kind-${option.id}-label`}>{option.label}</span>
            </button>
          ))}
        </div>
        {kind === "number" ? (
          <div id="rename-number" className="flex items-end gap-2">
            <label id="rename-start-label" htmlFor="rename-start" className="text-xs text-zinc-400">
              Start
              <input
                id="rename-start"
                type="number"
                min={0}
                value={start}
                disabled={applying}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  setStart(next);
                  void refresh(paths, kind, text, next, pad, replacement, names);
                }}
                className="mt-1 block h-8 w-20 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
            <label id="rename-pad-label" htmlFor="rename-pad" className="text-xs text-zinc-400">
              Digits
              <input
                id="rename-pad"
                type="number"
                min={1}
                max={6}
                value={pad}
                disabled={applying}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  setPad(next);
                  void refresh(paths, kind, text, start, next, replacement, names);
                }}
                className="mt-1 block h-8 w-20 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
          </div>
        ) : kind === "replace" ? (
          <div id="rename-replace" className="flex min-w-0 flex-1 items-end gap-2">
            <label id="rename-find-label" htmlFor="rename-find" className="min-w-32 flex-1 text-xs text-zinc-400">
              Find
              <input
                id="rename-find"
                value={text}
                disabled={applying}
                onChange={(event) => {
                  setText(event.target.value);
                  void refresh(paths, kind, event.target.value, start, pad, replacement, names);
                }}
                className="mt-1 block h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
            <label id="rename-replacement-label" htmlFor="rename-replacement" className="min-w-32 flex-1 text-xs text-zinc-400">
              Replace with
              <input
                id="rename-replacement"
                value={replacement}
                disabled={applying}
                onChange={(event) => {
                  setReplacement(event.target.value);
                  void refresh(paths, kind, text, start, pad, event.target.value, names);
                }}
                className="mt-1 block h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
          </div>
        ) : (
          <label id="rename-text-label" htmlFor="rename-text" className="min-w-40 flex-1 text-xs text-zinc-400">
            {kind === "prefix" ? "Prefix" : "Suffix"}
            <input
              id="rename-text"
              value={text}
              disabled={applying}
              onChange={(event) => {
                setText(event.target.value);
                void refresh(paths, kind, event.target.value, start, pad, replacement, names);
              }}
              className="mt-1 block h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
        )}
        <Button id="rename-apply" variant="primary" disabled={applying || readyCount === 0} onClick={() => void rename()}>
          <span id="rename-apply-label">{applying ? "Working…" : `Rename ${readyCount}`}</span>
        </Button>
      </div>
      {error ? (
        <p id="rename-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="rename-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="rename-table" className="w-full border-collapse text-left text-sm">
          <thead id="rename-head" className="sticky top-0 z-10 bg-zinc-900">
            <tr id="rename-columns">
              <th id="rename-col-from" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Current name
              </th>
              <th id="rename-col-to" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                New name
              </th>
              <th id="rename-col-status" className="w-64 border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Status
              </th>
            </tr>
          </thead>
          <tbody id="rename-body">
            {rows.length === 0 ? (
              <tr id="rename-empty">
                <td id="rename-empty-cell" colSpan={3} className="px-3 py-8 text-sm text-zinc-500">
                  Drop files or a folder here, or choose them above.
                </td>
              </tr>
            ) : (
              rows.map((row, index) => (
                <tr id={`rename-row-${index}`} key={`${row.from}-${index}`} className="border-b border-zinc-800">
                  <td id={`rename-from-${index}`} className="max-w-0 truncate px-3 py-1.5 text-zinc-300" title={row.from}>
                    {baseName(row.from)}
                  </td>
                  <td id={`rename-to-${index}`} className="px-3 py-1">
                    <input
                      id={`rename-to-${index}-input`}
                      value={names.find((item) => item.from === row.from)?.name ?? baseName(row.to)}
                      disabled={applying}
                      aria-label={`New name for ${baseName(row.from)}`}
                      onChange={(event) => editName(row.from, event.target.value)}
                      className="h-7 w-full rounded-md border border-transparent bg-transparent px-1 text-sm text-zinc-100 outline-none hover:border-zinc-700 focus:border-sky-500 focus:bg-zinc-950"
                    />
                  </td>
                  <td id={`rename-status-${index}`} className={cn("px-3 py-1.5 text-xs", row.ok ? "text-emerald-400" : "text-amber-300")}>
                    {row.message}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
      {dragOver ? (
        <div id="rename-drop" className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-sky-950/40">
          <p id="rename-drop-label" className="rounded-md border border-sky-500 bg-zinc-900 px-3 py-2 text-sm text-sky-100">
            Drop files or folders to rename
          </p>
        </div>
      ) : null}
    </section>
  );
}

function overRenameTool(position: { x: number; y: number } | null) {
  if (!position) {
    return false;
  }
  const root = document.getElementById("rename-tool");
  if (!root) {
    return false;
  }
  const rect = root.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  const x = position.x / scale;
  const y = position.y / scale;
  return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
}

function pickedPaths(picked: unknown) {
  if (Array.isArray(picked)) {
    return picked.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
  }
  if (typeof picked === "string" && picked.trim()) {
    return [picked];
  }
  return [];
}
