import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import type { LauncherSave, LauncherState } from "../../../../types/launcher";
import { chordOf } from "../../lib/hotkey";
import { launcherRefreshApps, launcherSave, launcherState } from "../../lib/launcher-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const EXAMPLES = [
  { id: "app", query: "chrome", detail: "Opens a Start menu app or Store app." },
  { id: "tool", query: "folder rules", detail: "Opens a tool in this window." },
  { id: "calc", query: "(1200 * 1.08) / 12", detail: "Calculates. Enter copies the answer." },
  { id: "power", query: "2^10, sqrt(2), 5!", detail: "Powers, functions, and factorials." },
  { id: "base", query: "255 to hex", detail: "Converts between hex, binary, octal, and decimal." },
  { id: "unit", query: "5 km to mi", detail: "Length, mass, volume, area, speed, time, data, and temperature." },
  { id: "temp", query: "72 f to c", detail: "Temperatures use the correct offset." },
  { id: "kill", query: "kill notepad", detail: "Lists matching processes. Enter closes one." },
  { id: "port", query: "port 3000", detail: "Shows what is using a port and lets you close it." },
  { id: "port-range", query: "port 30**", detail: "Each * is any one digit, so this covers ports 3000 to 3099." },
];

const EMPTY: LauncherState = { enabled: true, hotkey: "Win+Space", appCount: 0 };

export function Launcher() {
  const [state, setState] = useState<LauncherState>(EMPTY);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const hotkeyRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let cancelled = false;
    void launcherState()
      .then((next) => {
        if (!cancelled) {
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

  async function persist(patch: Partial<LauncherSave>) {
    try {
      const saved = await launcherSave({
        enabled: patch.enabled ?? state.enabled,
        hotkey: patch.hotkey ?? state.hotkey,
      });
      setState(saved);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
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
    const chord = chordOf(event);
    if (!chord) {
      return;
    }
    setRecording(false);
    void persist({ hotkey: chord });
  }

  async function refreshApps() {
    try {
      await launcherRefreshApps();
      window.setTimeout(() => {
        void launcherState().then(setState).catch(() => undefined);
      }, 3000);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  const usesWinSpace = state.hotkey === "Win+Space";

  return (
    <section id="launcher-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="launcher-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="launcher-header-row" className="flex items-start justify-between gap-3">
          <div id="launcher-heading" className="min-w-0">
            <h1 id="launcher-title" className="text-base font-semibold text-zinc-100">
              Launcher
            </h1>
            <p id="launcher-summary" className="mt-1 text-sm text-zinc-400">
              Press the hotkey anywhere to search apps, open tools, calculate, convert units, or close a process.
            </p>
          </div>
          <Button
            id="launcher-enabled"
            variant={state.enabled ? "primary" : "outline"}
            aria-pressed={state.enabled}
            disabled={!ready}
            onClick={() => void persist({ enabled: !state.enabled })}
          >
            {state.enabled ? "Launcher on" : "Launcher off"}
          </Button>
        </div>
        <div id="launcher-hotkey" className="flex flex-wrap items-center gap-2">
          <span id="launcher-hotkey-label" className="text-sm text-zinc-300">
            Hotkey
          </span>
          <button
            id="launcher-hotkey-field"
            ref={hotkeyRef}
            type="button"
            aria-pressed={recording}
            disabled={!ready}
            onClick={() => {
              setRecording(true);
              hotkeyRef.current?.focus();
            }}
            onKeyDown={onHotkeyKey}
            onBlur={() => setRecording(false)}
            className="h-8 min-w-36 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-left text-sm text-zinc-100"
          >
            {recording ? "Press a shortcut" : state.hotkey}
          </button>
          <Button
            id="launcher-hotkey-default"
            variant="ghost"
            size="sm"
            disabled={!ready || usesWinSpace}
            onClick={() => void persist({ hotkey: "Win+Space" })}
          >
            Use Win+Space
          </Button>
          <Button
            id="launcher-hotkey-alt"
            variant="ghost"
            size="sm"
            disabled={!ready || state.hotkey === "Alt+Space"}
            onClick={() => void persist({ hotkey: "Alt+Space" })}
          >
            Use Alt+Space
          </Button>
        </div>
        {usesWinSpace ? (
          <p id="launcher-language-note" className="text-xs text-amber-300">
            Windows uses Win+Space to switch keyboard languages. While the launcher owns it, switch languages with
            Alt+Shift or the taskbar language button, or pick another hotkey.
          </p>
        ) : null}
      </header>
      <div id="launcher-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
        {error ? (
          <p id="launcher-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        <section id="launcher-examples" className="flex flex-col gap-2">
          <h2 id="launcher-examples-title" className="text-sm font-medium text-zinc-100">
            What you can type
          </h2>
          <ul id="launcher-examples-list" className="flex flex-col divide-y divide-zinc-800 rounded-md border border-zinc-800">
            {EXAMPLES.map((example) => (
              <li id={`launcher-example-${example.id}`} key={example.id} className="flex items-center gap-4 px-3 py-2">
                <code id={`launcher-example-${example.id}-query`} className="w-48 shrink-0 font-mono text-sm text-sky-400">
                  {example.query}
                </code>
                <span id={`launcher-example-${example.id}-detail`} className="text-sm text-zinc-400">
                  {example.detail}
                </span>
              </li>
            ))}
          </ul>
        </section>
        <section id="launcher-apps" className="flex flex-col gap-2">
          <h2 id="launcher-apps-title" className="text-sm font-medium text-zinc-100">
            Apps
          </h2>
          <div id="launcher-apps-row" className="flex flex-wrap items-center gap-3">
            <p id="launcher-apps-count" className="text-sm text-zinc-400">
              {state.appCount > 0
                ? `${state.appCount} Start menu apps are indexed. The list refreshes every 10 minutes.`
                : "Start menu apps are still being indexed."}
            </p>
            <Button id="launcher-apps-refresh" variant="outline" size="sm" onClick={() => void refreshApps()}>
              Refresh now
            </Button>
          </div>
        </section>
      </div>
    </section>
  );
}
