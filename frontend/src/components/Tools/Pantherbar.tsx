import { getCurrentWindow } from "@tauri-apps/api/window";
import { GripVertical, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import type { PantherbarSave, PantherbarState } from "../../../../types/pantherbar";
import { hotkeyName } from "../../lib/hotkey";
import { pantherbarSave, pantherbarState } from "../../lib/pantherbar-client";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";
import { PantherbarIcon } from "./PantherbarIcon";

const GROUPS = [
  { id: "clipboard", label: "Clipboard" },
  { id: "text", label: "Text" },
  { id: "search", label: "Search" },
  { id: "tools", label: "Tools" },
];

const EMPTY: PantherbarState = {
  enabled: false,
  installed: [],
  usingDefaults: true,
  custom: [],
  excludedApps: [],
  catalog: [],
  icons: [],
  defaults: [],
  lastApp: "",
  hotkey: "",
};

function saveOf(state: PantherbarState, patch: Partial<PantherbarSave> = {}): PantherbarSave {
  return {
    enabled: patch.enabled ?? state.enabled,
    installed: patch.installed ?? state.installed,
    custom: patch.custom ?? state.custom,
    excludedApps: patch.excludedApps ?? state.excludedApps,
    hotkey: patch.hotkey ?? state.hotkey,
  };
}

function domId(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-");
}

function describe(state: PantherbarState, id: string) {
  const catalog = state.catalog.find((entry) => entry.id === id);
  if (catalog) {
    return catalog;
  }
  const custom = state.custom.find((action) => action.id === id);
  if (custom) {
    return { id, name: custom.name, icon: custom.icon, group: "custom" };
  }
  return { id, name: id, icon: "sparkles", group: "custom" };
}

export function Pantherbar() {
  const [state, setState] = useState<PantherbarState>(EMPTY);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [appName, setAppName] = useState("");
  const [recording, setRecording] = useState(false);
  const hotkeyRef = useRef<HTMLButtonElement>(null);
  const stateRef = useRef(state);
  const queue = useRef(Promise.resolve());
  const dragIndex = useRef<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  stateRef.current = state;

  useEffect(() => {
    let cancelled = false;
    void pantherbarState()
      .then((next) => {
        if (!cancelled) {
          stateRef.current = next;
          setState(next);
          setReady(true);
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
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          return;
        }
        const run = queue.current.then(async () => {
          const next = await pantherbarState();
          stateRef.current = next;
          setState(next);
        });
        queue.current = run.then(
          () => undefined,
          () => undefined,
        );
      })
      .then((stop) => {
        unlisten = stop;
      });
    return () => {
      unlisten?.();
    };
  }, []);

  function persist(patch: Partial<PantherbarSave>) {
    if (!ready) {
      return Promise.resolve();
    }
    const run = queue.current.then(async () => {
      const saved = await pantherbarSave(saveOf(stateRef.current, patch));
      stateRef.current = saved;
      setState(saved);
      setError(null);
      return saved;
    });
    queue.current = run.then(
      () => undefined,
      () => undefined,
    );
    return run.catch((caught) => {
      setError(messageOf(caught));
      throw caught;
    });
  }

  function moveInstalled(to: number) {
    const from = dragIndex.current;
    dragIndex.current = null;
    if (from === null || from === to) {
      return;
    }
    const installed = [...stateRef.current.installed];
    const [item] = installed.splice(from, 1);
    if (!item) {
      return;
    }
    installed.splice(to, 0, item);
    void persist({ installed });
  }

  function removeInstalled(id: string) {
    void persist({ installed: stateRef.current.installed.filter((entry) => entry !== id) });
  }

  function armHotkey() {
    setRecording(true);
    hotkeyRef.current?.focus();
  }

  function onHotkeyKey(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (!recording) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setRecording(false);
      return;
    }
    if (event.key === "Control" || event.key === "Alt" || event.key === "Shift" || event.key === "Meta") {
      return;
    }
    const key = hotkeyName(event.code);
    if (!key) {
      return;
    }
    const parts: string[] = [];
    if (event.ctrlKey) {
      parts.push("Ctrl");
    }
    if (event.altKey) {
      parts.push("Alt");
    }
    if (event.shiftKey) {
      parts.push("Shift");
    }
    if (event.metaKey) {
      parts.push("Win");
    }
    parts.push(key);
    setRecording(false);
    void persist({ hotkey: parts.join("+") });
  }

  function addExcluded() {
    const name = appName.trim();
    if (!name) {
      setError("Enter the program name, such as notepad.exe.");
      return;
    }
    setAppName("");
    void persist({ excludedApps: [...stateRef.current.excludedApps, name] });
  }

  return (
    <section id="pantherbar-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="pantherbar-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="pantherbar-header-row" className="flex items-start justify-between gap-3">
        <div id="pantherbar-heading" className="min-w-0">
          <h1 id="pantherbar-title" className="text-base font-semibold text-zinc-100">
            Pantherbar
          </h1>
          <p id="pantherbar-summary" className="mt-1 text-sm text-zinc-400">
            The switch shows the bar when you select text. A hotkey still opens it whenever a shortcut is set.
          </p>
        </div>
        <Button
          id="pantherbar-enabled"
          variant={state.enabled ? "primary" : "outline"}
          aria-pressed={state.enabled}
          disabled={!ready}
          onClick={() => void persist({ enabled: !state.enabled })}
        >
          {state.enabled ? "Selected text on" : "Selected text off"}
        </Button>
        </div>
        <div id="pantherbar-hotkey" className="flex flex-wrap items-center gap-2">
          <span id="pantherbar-hotkey-label" className="text-sm text-zinc-300">
            Hotkey
          </span>
          <button
            id="pantherbar-hotkey-field"
            ref={hotkeyRef}
            type="button"
            aria-pressed={recording}
            disabled={!ready}
            onClick={armHotkey}
            onKeyDown={onHotkeyKey}
            onBlur={() => setRecording(false)}
            className="h-8 min-w-36 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-left text-sm text-zinc-100"
          >
            {recording ? "Press a shortcut" : state.hotkey || "None"}
          </button>
          <Button id="pantherbar-hotkey-clear" variant="ghost" size="sm" disabled={!ready || !state.hotkey} onClick={() => void persist({ hotkey: "" })}>
            Clear
          </Button>
        </div>
      </header>
      <div id="pantherbar-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-hidden px-4 py-4">
        {error ? (
          <p id="pantherbar-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        {!ready && !error ? (
          <p id="pantherbar-loading" className="text-sm text-zinc-500">
            Loading Pantherbar.
          </p>
        ) : null}
        {ready ? (
        <>
        <div id="pantherbar-columns" className="grid min-h-0 flex-1 grid-cols-2 grid-rows-[minmax(0,1fr)] gap-6">
        <section id="pantherbar-installed" className="col-start-2 row-start-1 flex min-h-0 flex-col gap-2 overflow-y-auto">
          <div id="pantherbar-installed-heading" className="flex items-center justify-between gap-3">
            <h2 id="pantherbar-installed-title" className="text-sm font-medium text-zinc-100">
              On the bar
            </h2>
            <Button id="pantherbar-reset" variant="ghost" size="sm" disabled={!ready} onClick={() => void persist({ installed: state.defaults })}>
              Reset
            </Button>
          </div>
          {state.installed.length === 0 ? (
            <p id="pantherbar-installed-empty" className="text-sm text-zinc-500">
              The bar is empty. Add an action from the catalog.
            </p>
          ) : (
            <ul id="pantherbar-installed-list" className="flex flex-col gap-1">
              {state.installed.map((id, index) => {
                const action = describe(state, id);
                return (
                  <li
                    id={`pantherbar-row-${id}`}
                    key={id}
                    data-index={index}
                    className={cn(
                      "flex items-center gap-2 rounded-md border bg-zinc-900 px-2 py-1.5",
                      dropIndex === index && dragIndex.current !== index ? "border-sky-500" : "border-zinc-800",
                      dragIndex.current === index && dropIndex !== null && "opacity-50",
                    )}
                  >
                    <span
                      id={`pantherbar-row-${id}-grip`}
                      className="shrink-0 cursor-grab touch-none text-zinc-500 active:cursor-grabbing"
                      onPointerDown={(event) => {
                        event.preventDefault();
                        event.currentTarget.setPointerCapture(event.pointerId);
                        dragIndex.current = index;
                        setDropIndex(index);
                      }}
                      onPointerMove={(event) => {
                        if (dragIndex.current === null) {
                          return;
                        }
                        const row = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("#pantherbar-installed-list > li");
                        if (row?.dataset.index) {
                          setDropIndex(Number(row.dataset.index));
                        }
                      }}
                      onPointerUp={() => {
                        const to = dropIndex;
                        setDropIndex(null);
                        if (to !== null) {
                          moveInstalled(to);
                        }
                      }}
                      onPointerCancel={() => {
                        dragIndex.current = null;
                        setDropIndex(null);
                      }}
                    >
                      <GripVertical id={`pantherbar-row-${id}-grip-icon`} className="size-4" aria-hidden="true" />
                    </span>
                    <PantherbarIcon id={`pantherbar-row-${id}-icon`} name={action.icon} className="size-4 shrink-0 text-zinc-300" />
                    <span id={`pantherbar-row-${id}-name`} className="min-w-0 flex-1 truncate text-sm text-zinc-100">
                      {action.name}
                    </span>
                    <Button id={`pantherbar-row-${id}-remove`} variant="ghost" size="xs" title="Remove" onClick={() => removeInstalled(id)}>
                      <Trash2 id={`pantherbar-row-${id}-remove-icon`} className="size-3.5" aria-hidden="true" />
                    </Button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
        <section id="pantherbar-catalog" className="col-start-1 row-start-1 flex min-h-0 flex-col gap-4 overflow-y-auto">
          <h2 id="pantherbar-catalog-title" className="text-sm font-medium text-zinc-100">
            Catalog
          </h2>
          {GROUPS.map((group) => {
            const entries = state.catalog.filter((entry) => entry.group === group.id && !state.installed.includes(entry.id));
            if (entries.length === 0) {
              return null;
            }
            return (
              <div id={`pantherbar-group-${group.id}`} key={group.id} className="flex flex-col gap-2">
                <h3 id={`pantherbar-group-${group.id}-title`} className="text-xs font-medium tracking-wide text-zinc-500 uppercase">
                  {group.label}
                </h3>
                <div id={`pantherbar-group-${group.id}-list`} className="flex flex-wrap gap-2">
                  {entries.map((entry) => (
                    <Button
                      id={`pantherbar-add-${entry.id}`}
                      key={entry.id}
                      variant="outline"
                      size="sm"
                      onClick={() => void persist({ installed: [...stateRef.current.installed, entry.id] })}
                    >
                      <PantherbarIcon id={`pantherbar-add-${entry.id}-icon`} name={entry.icon} className="size-3.5" />
                      {entry.name}
                    </Button>
                  ))}
                </div>
              </div>
            );
          })}
        </section>
        </div>
        <section id="pantherbar-excluded" className="flex shrink-0 flex-col gap-2">
          <h2 id="pantherbar-excluded-title" className="text-sm font-medium text-zinc-100">
            Excluded apps
          </h2>
          <p id="pantherbar-excluded-summary" className="text-sm text-zinc-400">
            Pantherbar stays hidden while one of these programs is in front.
          </p>
          <div id="pantherbar-excluded-form" className="flex flex-wrap items-center gap-2">
            <label id="pantherbar-excluded-label" htmlFor="pantherbar-excluded-input" className="sr-only">
              Program name
            </label>
            <input
              id="pantherbar-excluded-input"
              value={appName}
              placeholder="notepad.exe"
              onChange={(event) => setAppName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  addExcluded();
                }
              }}
              className="h-8 w-56 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100"
            />
            <Button id="pantherbar-excluded-add" variant="secondary" size="sm" onClick={addExcluded}>
              Exclude
            </Button>
            <Button
              id="pantherbar-excluded-last"
              variant="outline"
              size="sm"
              disabled={!state.lastApp}
              onClick={() => void persist({ excludedApps: [...stateRef.current.excludedApps, stateRef.current.lastApp] })}
            >
              {state.lastApp ? `Exclude ${state.lastApp}` : "Exclude the last app"}
            </Button>
          </div>
          {state.excludedApps.length > 0 ? (
            <ul id="pantherbar-excluded-list" className="flex flex-col gap-1">
              {state.excludedApps.map((app) => (
                <li id={`pantherbar-excluded-${domId(app)}`} key={app} className="flex items-center gap-2 text-sm text-zinc-300">
                  <span id={`pantherbar-excluded-${domId(app)}-name`} className="min-w-0 flex-1 truncate">
                    {app}
                  </span>
                  <Button
                    id={`pantherbar-excluded-${domId(app)}-remove`}
                    variant="ghost"
                    size="xs"
                    onClick={() =>
                      void persist({
                        excludedApps: stateRef.current.excludedApps.filter((entry) => entry !== app),
                      })
                    }
                  >
                    Remove
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}
        </section>
        </>
        ) : null}
      </div>
    </section>
  );
}
