import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type { KeyboardShortcutApp, KeyboardShortcutsState } from "../../../../types/keyboard-shortcuts";
import { keyboardShortcutsSave, keyboardShortcutsState } from "../../lib/keyboard-shortcuts-client";
import { baseName, messageOf, pickedFolderPath } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: KeyboardShortcutsState = { settingsFolder: "", apps: [], error: null };

type Props = {
  onOpenSettings: () => void;
};

export function KeyboardShortcuts({ onOpenSettings }: Props) {
  const [state, setState] = useState<KeyboardShortcutsState>(EMPTY);
  const [adding, setAdding] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftExe, setDraftExe] = useState("");
  const [systemWide, setSystemWide] = useState(false);
  const [rowError, setRowError] = useState<string | null>(null);
  const appsRef = useRef(state.apps);
  const folderRef = useRef(state.settingsFolder);
  appsRef.current = state.apps;
  folderRef.current = state.settingsFolder;

  useEffect(() => {
    let cancelled = false;
    void keyboardShortcutsState()
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
    return () => {
      cancelled = true;
    };
  }, []);

  const problem = rowError ?? state.error;
  const rows = state.apps.flatMap((app, appIndex) =>
    app.shortcuts.map((shortcut, shortcutIndex) => ({ app, appIndex, shortcut, shortcutIndex })),
  );

  function replaceApps(apps: KeyboardShortcutApp[]) {
    appsRef.current = apps;
    setState((current) => ({ ...current, apps }));
  }

  async function persist(apps: KeyboardShortcutApp[]) {
    const saved = await keyboardShortcutsSave(apps);
    appsRef.current = saved.apps;
    setState(saved);
    setRowError(null);
    return saved;
  }

  function setEnabled(appIndex: number, shortcutIndex: number, enabled: boolean) {
    const apps = state.apps.map((entry, index) => {
      if (index !== appIndex) {
        return entry;
      }
      return {
        ...entry,
        shortcuts: entry.shortcuts.map((shortcut, index) => (index === shortcutIndex ? { ...shortcut, enabled } : shortcut)),
      };
    });
    replaceApps(apps);
    if (!folderRef.current) {
      return;
    }
    void persist(apps).catch((caught) => setRowError(messageOf(caught)));
  }

  async function addApp() {
    const name = draftName.trim();
    const exe = systemWide ? "" : baseName(draftExe.trim());
    if (!name || (!systemWide && !exe)) {
      setRowError("An application needs a name. Choose a program file, or mark it system wide.");
      return;
    }
    try {
      await persist([...appsRef.current, { id: "", name, exe, shortcuts: [] }]);
      setAdding(false);
      setDraftName("");
      setDraftExe("");
      setSystemWide(false);
    } catch (caught) {
      setRowError(messageOf(caught));
    }
  }

  async function browseExe() {
    const picked = await open({
      multiple: false,
      directory: false,
      title: "Choose the application",
      filters: [{ name: "Application", extensions: ["exe"] }],
    });
    const path = pickedFolderPath(picked);
    if (!path) {
      return;
    }
    setDraftExe(baseName(path));
    if (!draftName.trim()) {
      setDraftName(baseName(path).replace(/\.exe$/i, ""));
    }
  }

  return (
    <section id="app-shortcuts-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="app-shortcuts-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="app-shortcuts-heading" className="min-w-0">
          <h1 id="app-shortcuts-title" className="text-base font-semibold text-zinc-100">
            Keyboard Shortcuts
          </h1>
          <p id="app-shortcuts-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            Shortcuts for every application are listed together. A blank program runs in every window.
          </p>
        </div>
      </header>
      {problem ? (
        <p id="app-shortcuts-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      {state.settingsFolder.length === 0 ? (
        <div id="app-shortcuts-folder" className="flex items-center justify-between gap-3 border-b border-zinc-800 px-4 py-2">
          <p id="app-shortcuts-folder-copy" className="text-xs text-zinc-400">
            Choose a settings folder to keep changes in app-shortcuts.json. System and Excel are already listed.
          </p>
          <Button id="app-shortcuts-choose-folder" variant="outline" onClick={onOpenSettings}>
            <span id="app-shortcuts-choose-folder-label">Choose settings folder</span>
          </Button>
        </div>
      ) : null}
      {adding ? (
        <form
          id="app-shortcuts-new-app"
          className="flex flex-wrap items-end gap-2 border-b border-zinc-800 px-4 py-3"
          onSubmit={(event) => {
            event.preventDefault();
            void addApp();
          }}
        >
          <label id="app-shortcuts-new-everywhere" className="mb-1 flex items-center gap-2 pb-1 text-sm text-zinc-200">
            <input
              id="app-shortcuts-new-everywhere-input"
              type="checkbox"
              checked={systemWide}
              onChange={(event) => setSystemWide(event.target.checked)}
              className="size-4 accent-sky-500"
            />
            <span id="app-shortcuts-new-everywhere-label">System wide</span>
          </label>
          <label id="app-shortcuts-new-name-label" htmlFor="app-shortcuts-new-name" className="min-w-40 flex-1 text-xs text-zinc-400">
            Name
            <input
              id="app-shortcuts-new-name"
              value={draftName}
              onChange={(event) => setDraftName(event.target.value)}
              className="mt-1 block h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
          {systemWide ? null : (
            <label id="app-shortcuts-new-exe-label" htmlFor="app-shortcuts-new-exe" className="min-w-40 flex-1 text-xs text-zinc-400">
              Program file
              <input
                id="app-shortcuts-new-exe"
                value={draftExe}
                onChange={(event) => setDraftExe(event.target.value)}
                placeholder="EXCEL.EXE"
                className="mt-1 block h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
              />
            </label>
          )}
          {systemWide ? null : (
            <Button id="app-shortcuts-browse" type="button" variant="outline" onClick={() => void browseExe()}>
              <span id="app-shortcuts-browse-label">Browse</span>
            </Button>
          )}
          <Button id="app-shortcuts-new-save" type="submit" variant="primary">
            <span id="app-shortcuts-new-save-label">Add</span>
          </Button>
        </form>
      ) : null}
      <div id="app-shortcuts-scroll" className="min-h-0 flex-1 overflow-auto">
        <table id="app-shortcuts-table" className="w-full table-fixed border-collapse text-left text-sm">
          <colgroup id="app-shortcuts-cols">
            <col id="app-shortcuts-col-app-width" className="w-32" />
            <col id="app-shortcuts-col-program-width" className="w-32" />
            <col id="app-shortcuts-col-name-width" />
            <col id="app-shortcuts-col-hotkey-width" className="w-36" />
            <col id="app-shortcuts-col-detail-width" />
            <col id="app-shortcuts-col-enabled-width" className="w-28" />
          </colgroup>
          <thead id="app-shortcuts-head" className="sticky top-0 z-10 bg-zinc-900">
            <tr id="app-shortcuts-columns">
              <th id="app-shortcuts-col-app" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Application
              </th>
              <th id="app-shortcuts-col-program" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Program
              </th>
              <th id="app-shortcuts-col-name" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Name
              </th>
              <th id="app-shortcuts-col-hotkey" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Shortcut
              </th>
              <th id="app-shortcuts-col-detail" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                What it does
              </th>
              <th id="app-shortcuts-col-enabled" className="border-b border-zinc-800 px-3 py-2 text-xs font-medium text-zinc-400">
                Status
              </th>
            </tr>
          </thead>
          <tbody id="app-shortcuts-body">
            {rows.length === 0 ? (
              <tr id="app-shortcuts-empty">
                <td id="app-shortcuts-empty-cell" colSpan={6} className="px-3 py-8 text-sm text-zinc-500">
                  No applications yet.
                </td>
              </tr>
            ) : (
              rows.map(({ app, appIndex, shortcut, shortcutIndex }) => {
                const rowId = `app-shortcuts-row-${appIndex}-${shortcutIndex}`;
                return (
                  <tr id={rowId} key={`${app.id || appIndex}-${shortcutIndex}`} className="border-b border-zinc-800">
                    <td id={`${rowId}-app`} className="truncate px-3 py-2 text-zinc-100" title={app.name}>
                      <span id={`${rowId}-app-label`}>{app.name}</span>
                    </td>
                    <td id={`${rowId}-program`} className="truncate px-3 py-2 font-mono text-xs text-zinc-400" title={app.exe || "Everywhere"}>
                      <span id={`${rowId}-program-label`}>{app.exe || "Everywhere"}</span>
                    </td>
                    <td id={`${rowId}-name`} className="truncate px-3 py-2 text-zinc-100" title={shortcut.title}>
                      <span id={`${rowId}-name-label`}>{shortcut.title}</span>
                    </td>
                    <td id={`${rowId}-hotkey`} className="truncate px-3 py-2 font-mono text-xs text-zinc-100" title={shortcut.hotkey}>
                      <span id={`${rowId}-hotkey-label`}>{shortcut.hotkey}</span>
                    </td>
                    <td id={`${rowId}-detail`} className="truncate px-3 py-2 text-zinc-400" title={shortcut.detail}>
                      <span id={`${rowId}-detail-label`}>{shortcut.detail}</span>
                    </td>
                    <td id={`${rowId}-enabled`} className="px-3 py-1.5">
                      <button
                        id={`${rowId}-enabled-toggle`}
                        type="button"
                        aria-pressed={shortcut.enabled}
                        onClick={() => setEnabled(appIndex, shortcutIndex, !shortcut.enabled)}
                        className={
                          shortcut.enabled
                            ? "inline-flex h-7 items-center rounded-full bg-emerald-950 px-2.5 text-xs font-medium text-emerald-300"
                            : "inline-flex h-7 items-center rounded-full bg-zinc-800 px-2.5 text-xs font-medium text-zinc-400"
                        }
                      >
                        <span id={`${rowId}-enabled-label`}>{shortcut.enabled ? "Enabled" : "Disabled"}</span>
                      </button>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
