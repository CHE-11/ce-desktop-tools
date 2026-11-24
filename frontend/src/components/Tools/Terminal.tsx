import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { ClipboardCopy, Copy, Eraser, FolderOpen, Minus, Plus, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { TerminalSettings } from "../../../../types/app";
import type { ShellInfo, ShellKind, TerminalSession } from "../../../../types/terminal";
import { cn } from "../../lib/cn";
import { baseName, messageOf, pickedFolderPath } from "../../lib/message";
import {
  terminalKill,
  terminalIsFolder,
  terminalList,
  terminalSettings,
  terminalSettingsSave,
  terminalShells,
  terminalSpawn,
  terminalWrite,
} from "../../lib/terminal-client";
import { Button } from "../Button";
import { PasteDialog } from "./terminal/PasteDialog";
import { changeDirectory, isMultiline, normalizePaste, pasteChunks, quotePath, type PasteMode } from "./terminal/paste";
import { TerminalView, type TerminalViewHandle } from "./terminal/TerminalView";

const DEFAULT_SETTINGS: TerminalSettings = {
  defaultShell: "powershell",
  copyOnSelect: false,
  confirmMultilinePaste: true,
  fontSize: 14,
};

const LINE_DELAY_MS = 60;
const SELECT = "h-7 rounded-md border border-zinc-700 bg-zinc-950 px-1.5 text-xs text-zinc-100";

type Tab = TerminalSession & { exited: boolean };

type Props = {
  active: boolean;
};

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function TerminalTool({ active }: Props) {
  const [shells, setShells] = useState<ShellInfo[]>([]);
  const [settings, setSettings] = useState<TerminalSettings>(DEFAULT_SETTINGS);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [newShell, setNewShell] = useState<ShellKind>("powershell");
  const [cwd, setCwd] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [paste, setPaste] = useState<{ text: string; tabId: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const views = useRef(new Map<string, TerminalViewHandle>());
  const noticeTimer = useRef<number | undefined>(undefined);
  const started = useRef(false);
  const dropTarget = useRef<Tab | null>(null);
  const [dragOver, setDragOver] = useState(false);

  const flash = useCallback((message: string) => {
    setNotice(message);
    window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(null), 2000);
  }, []);

  useEffect(() => () => window.clearTimeout(noticeTimer.current), []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const overTerminal = (position: { x: number; y: number }) => {
      const area = document.getElementById("terminal-views")?.getBoundingClientRect();
      if (!area || area.width === 0) {
        return false;
      }
      const x = position.x / window.devicePixelRatio;
      const y = position.y / window.devicePixelRatio;
      return x >= area.left && x <= area.right && y >= area.top && y <= area.bottom;
    };
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        const tab = dropTarget.current;
        if (payload.type === "leave" || !tab) {
          setDragOver(false);
          return;
        }
        const over = overTerminal(payload.position);
        if (payload.type !== "drop") {
          setDragOver(over);
          return;
        }
        setDragOver(false);
        const paths = payload.paths.map((path) => path.trim()).filter(Boolean);
        if (!over || paths.length === 0) {
          return;
        }
        void (async () => {
          try {
            if (paths.length === 1 && (await terminalIsFolder(paths[0]))) {
              await terminalWrite(tab.id, changeDirectory(paths[0], tab.shell));
            } else {
              await terminalWrite(tab.id, paths.map((path) => quotePath(path, tab.shell)).join(" "));
            }
            views.current.get(tab.id)?.focus();
          } catch (caught) {
            setError(messageOf(caught));
          }
        })();
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

  const openTab = useCallback(
    async (shell: ShellKind, folder: string | null) => {
      try {
        const session = await terminalSpawn(shell, folder, 120, 30);
        setTabs((current) => [...current, { ...session, exited: false }]);
        setActiveId(session.id);
        setError(null);
      } catch (caught) {
        setError(messageOf(caught));
      }
    },
    [],
  );

  useEffect(() => {
    if (!active || started.current) {
      return;
    }
    started.current = true;
    void (async () => {
      try {
        const [available, saved, existing] = await Promise.all([terminalShells(), terminalSettings(), terminalList()]);
        setShells(available);
        setSettings(saved);
        const preferred = available.some((shell) => shell.id === saved.defaultShell)
          ? saved.defaultShell
          : (available[0]?.id ?? "cmd");
        setNewShell(preferred);
        if (existing.length > 0) {
          setTabs(existing.map((session) => ({ ...session, exited: false })));
          setActiveId(existing[0].id);
        } else {
          await openTab(preferred, null);
        }
      } catch (caught) {
        setError(messageOf(caught));
      }
    })();
  }, [active, openTab]);

  async function updateSettings(change: Partial<TerminalSettings>) {
    const next = { ...settings, ...change };
    setSettings(next);
    try {
      setSettings(await terminalSettingsSave(next));
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function zoom(delta: number) {
    const fontSize = delta === 0 ? DEFAULT_SETTINGS.fontSize : Math.min(32, Math.max(8, settings.fontSize + delta));
    if (fontSize !== settings.fontSize) {
      void updateSettings({ fontSize });
    }
  }

  function closeTab(id: string) {
    void terminalKill(id).catch(() => undefined);
    views.current.delete(id);
    const index = tabs.findIndex((tab) => tab.id === id);
    const remaining = tabs.filter((tab) => tab.id !== id);
    setTabs(remaining);
    if (activeId === id) {
      setActiveId(remaining[Math.max(0, index - 1)]?.id ?? null);
    }
  }

  async function chooseCwd() {
    const picked = await open({ multiple: false, directory: true, title: "Start new terminals in this folder" });
    const path = pickedFolderPath(picked);
    if (path) {
      setCwd(path);
    }
  }

  async function sendPaste(tab: Tab, text: string, mode: PasteMode) {
    try {
      const chunks = pasteChunks(text, mode, tab.shell);
      for (const [index, chunk] of chunks.entries()) {
        await terminalWrite(tab.id, chunk);
        if (index < chunks.length - 1) {
          await sleep(LINE_DELAY_MS);
        }
      }
    } catch (caught) {
      setError(messageOf(caught));
    }
    views.current.get(tab.id)?.focus();
  }

  function handlePaste(tab: Tab, raw: string) {
    if (tab.exited) {
      return;
    }
    if (!isMultiline(raw)) {
      const single = normalizePaste(raw, false).replace(/\n+$/, "");
      void terminalWrite(tab.id, single).catch((caught) => setError(messageOf(caught)));
      return;
    }
    if (settings.confirmMultilinePaste) {
      setPaste({ text: raw, tabId: tab.id });
      return;
    }
    void sendPaste(tab, normalizePaste(raw, false), "lines");
  }

  const current = tabs.find((tab) => tab.id === activeId) ?? null;
  dropTarget.current = active && current && !current.exited ? current : null;
  const currentView = () => (current ? views.current.get(current.id) : undefined);
  const pasteTab = paste ? tabs.find((tab) => tab.id === paste.tabId) : undefined;

  return (
    <section id="terminal-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="terminal-tabs-bar" className="flex items-center gap-1 border-b border-zinc-800 px-2 py-1.5">
        <nav id="terminal-tabs" role="tablist" aria-label="Terminals" className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => (
            <div
              id={`terminal-tab-${tab.id}`}
              key={tab.id}
              role="tab"
              aria-selected={tab.id === activeId}
              className={cn(
                "group flex shrink-0 items-center gap-1 rounded-md pl-2 pr-1 text-xs",
                tab.id === activeId ? "row-selected" : "text-zinc-400 hover:bg-zinc-800",
              )}
            >
              {renamingId === tab.id ? (
                <input
                  id={`terminal-tab-${tab.id}-rename`}
                  className="h-6 w-32 rounded border border-zinc-700 bg-zinc-950 px-1 text-xs text-zinc-100"
                  defaultValue={tab.title}
                  autoFocus
                  onBlur={(event) => {
                    const title = event.target.value.trim() || tab.title;
                    setTabs((list) => list.map((item) => (item.id === tab.id ? { ...item, title } : item)));
                    setRenamingId(null);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.currentTarget.blur();
                    } else if (event.key === "Escape") {
                      setRenamingId(null);
                    }
                  }}
                />
              ) : (
                <button
                  id={`terminal-tab-${tab.id}-select`}
                  type="button"
                  className={cn("h-7 max-w-48 truncate", tab.exited && "text-zinc-500 line-through")}
                  title={`${tab.title} - ${tab.cwd}\nDouble-click to rename`}
                  onClick={() => setActiveId(tab.id)}
                  onDoubleClick={() => setRenamingId(tab.id)}
                >
                  {tab.title}
                </button>
              )}
              <button
                id={`terminal-tab-${tab.id}-close`}
                type="button"
                aria-label={`Close ${tab.title}`}
                className="rounded p-0.5 text-zinc-500 hover:bg-zinc-700 hover:text-zinc-100"
                onClick={() => closeTab(tab.id)}
              >
                <X id={`terminal-tab-${tab.id}-close-icon`} className="size-3" aria-hidden="true" />
              </button>
            </div>
          ))}
        </nav>
        <div id="terminal-new" className="flex shrink-0 items-center gap-1">
          <select
            id="terminal-new-shell"
            className={SELECT}
            aria-label="Shell for new tabs"
            value={newShell}
            onChange={(event) => setNewShell(event.target.value as ShellKind)}
          >
            {shells.map((shell) => (
              <option id={`terminal-new-shell-${shell.id}`} key={shell.id} value={shell.id}>
                {shell.label}
              </option>
            ))}
          </select>
          <Button
            id="terminal-new-cwd"
            variant="ghost"
            size="sm"
            title={cwd ? `New tabs start in ${cwd}. Right-click to reset.` : "Choose the starting folder for new tabs"}
            onClick={() => void chooseCwd()}
            onContextMenu={(event) => {
              event.preventDefault();
              setCwd(null);
            }}
          >
            <FolderOpen id="terminal-new-cwd-icon" className="size-3.5" aria-hidden="true" />
            <span id="terminal-new-cwd-label" className="max-w-28 truncate">
              {cwd ? baseName(cwd) : "Home"}
            </span>
          </Button>
          <Button id="terminal-new-tab" variant="secondary" size="sm" title="New tab" onClick={() => void openTab(newShell, cwd)}>
            <Plus id="terminal-new-tab-icon" className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
      </header>
      <div id="terminal-toolbar" className="flex flex-wrap items-center gap-1 border-b border-zinc-800 px-2 py-1">
        <Button
          id="terminal-copy-last"
          variant="ghost"
          size="xs"
          disabled={!current}
          title="Copy what the last command printed"
          onClick={() =>
            void currentView()?.copyLastOutput().then((copied) => {
              if (!copied) {
                flash("Couldn't find the last command's output.");
              }
            })
          }
        >
          <ClipboardCopy id="terminal-copy-last-icon" className="size-3.5" aria-hidden="true" />
          Copy last output
        </Button>
        <Button id="terminal-copy-all" variant="ghost" size="xs" disabled={!current} onClick={() => void currentView()?.copyAll()}>
          <Copy id="terminal-copy-all-icon" className="size-3.5" aria-hidden="true" />
          Copy all
        </Button>
        <Button id="terminal-clear" variant="ghost" size="xs" disabled={!current} onClick={() => currentView()?.clear()}>
          <Eraser id="terminal-clear-icon" className="size-3.5" aria-hidden="true" />
          Clear
        </Button>
        <Button id="terminal-search" variant="ghost" size="xs" disabled={!current} title="Find (Ctrl+F)" onClick={() => currentView()?.openSearch()}>
          <Search id="terminal-search-icon" className="size-3.5" aria-hidden="true" />
          Find
        </Button>
        <span id="terminal-toolbar-divider" className="mx-1 h-4 w-px bg-zinc-800" />
        <Button id="terminal-zoom-out" variant="ghost" size="xs" aria-label="Smaller text" title="Smaller text (Ctrl+-)" onClick={() => zoom(-1)}>
          <Minus id="terminal-zoom-out-icon" className="size-3.5" aria-hidden="true" />
        </Button>
        <span id="terminal-font-size" className="w-8 text-center text-xs text-zinc-400">
          {settings.fontSize}px
        </span>
        <Button id="terminal-zoom-in" variant="ghost" size="xs" aria-label="Larger text" title="Larger text (Ctrl+=)" onClick={() => zoom(1)}>
          <Plus id="terminal-zoom-in-icon" className="size-3.5" aria-hidden="true" />
        </Button>
        <span id="terminal-toolbar-divider-2" className="mx-1 h-4 w-px bg-zinc-800" />
        <label id="terminal-copy-on-select-label" className="flex items-center gap-1.5 text-xs text-zinc-300">
          <input
            id="terminal-copy-on-select"
            type="checkbox"
            checked={settings.copyOnSelect}
            onChange={(event) => void updateSettings({ copyOnSelect: event.target.checked })}
          />
          Copy on select
        </label>
        <label id="terminal-confirm-paste-label" className="ml-2 flex items-center gap-1.5 text-xs text-zinc-300">
          <input
            id="terminal-confirm-paste"
            type="checkbox"
            checked={settings.confirmMultilinePaste}
            onChange={(event) => void updateSettings({ confirmMultilinePaste: event.target.checked })}
          />
          Review multi-line paste
        </label>
        <label id="terminal-default-shell-label" className="ml-2 flex items-center gap-1.5 text-xs text-zinc-300">
          Default
          <select
            id="terminal-default-shell"
            className={SELECT}
            value={settings.defaultShell}
            onChange={(event) => void updateSettings({ defaultShell: event.target.value as ShellKind })}
          >
            {shells.map((shell) => (
              <option id={`terminal-default-shell-${shell.id}`} key={shell.id} value={shell.id}>
                {shell.label}
              </option>
            ))}
          </select>
        </label>
        {notice ? (
          <span id="terminal-notice" className="ml-auto text-xs text-emerald-400">
            {notice}
          </span>
        ) : null}
      </div>
      {error ? (
        <p id="terminal-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="terminal-views" className="relative flex min-h-0 flex-1 flex-col">
        {dragOver ? (
          <div
            id="terminal-drop-overlay"
            className="pointer-events-none absolute inset-2 z-10 flex items-center justify-center rounded-lg border-2 border-dashed border-sky-500 bg-sky-950/40 text-sm text-sky-300"
          >
            Drop a folder to go there, or a file to insert its path
          </div>
        ) : null}
        {tabs.map((tab) => (
          <TerminalView
            key={tab.id}
            ref={(handle) => {
              if (handle) {
                views.current.set(tab.id, handle);
              }
            }}
            session={tab}
            active={active && tab.id === activeId}
            settings={settings}
            onPaste={(text) => handlePaste(tab, text)}
            onExit={() => setTabs((list) => list.map((item) => (item.id === tab.id ? { ...item, exited: true } : item)))}
            onZoom={zoom}
            onCopied={flash}
          />
        ))}
        {tabs.length === 0 ? (
          <div id="terminal-empty" className="flex flex-1 flex-col items-center justify-center gap-3 text-sm text-zinc-500">
            <p id="terminal-empty-copy">No terminals are open.</p>
            <Button id="terminal-empty-new" variant="primary" onClick={() => void openTab(newShell, cwd)}>
              <Plus id="terminal-empty-new-icon" className="size-3.5" aria-hidden="true" />
              New terminal
            </Button>
          </div>
        ) : null}
      </div>
      {paste && pasteTab ? (
        <PasteDialog
          text={paste.text}
          shell={pasteTab.shell}
          onCancel={() => {
            setPaste(null);
            views.current.get(pasteTab.id)?.focus();
          }}
          onRun={(text, mode) => {
            setPaste(null);
            void sendPaste(pasteTab, text, mode);
          }}
        />
      ) : null}
    </section>
  );
}
