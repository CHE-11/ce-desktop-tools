import { useCallback, useEffect, useState } from "react";
import { AppWindow, Monitor, RefreshCw, Scan, Square, Video } from "lucide-react";
import type {
  RecordCameraCorner,
  RecordCursor,
  RecordEntry,
  RecordResolution,
  RecordSettings,
  RecordSource,
  RecordState,
  RecordStatus,
  RecordTarget,
} from "../../../../../types/record";
import { revealPath } from "../../../lib/app-client";
import { cn } from "../../../lib/cn";
import { messageOf } from "../../../lib/message";
import {
  chooseRecordFolder,
  minimizeMainWindow,
  onRecordError,
  onRecordSaved,
  onRecordStatus,
  recordLibrary,
  recordSave,
  recordStart,
  recordState,
  recordStatus,
  recordStop,
  recordTargets,
  restoreMainWindow,
} from "../../../lib/record-client";
import { useSettingsSection } from "../../../lib/use-settings-section";
import { Button } from "../../Button";
import { HotkeyField } from "../../HotkeyField";
import { SettingGroup, SettingRow, SETTING_SELECT } from "../../SettingRow";
import { Switch } from "../../Switch";
import { RecordLibrary } from "./RecordLibrary";
import { clock } from "./time";

const DEFAULT_HOTKEY = "Ctrl+Shift+Win+R";

const SOURCES: { id: RecordSource; label: string; icon: typeof Monitor }[] = [
  { id: "display", label: "Screen", icon: Monitor },
  { id: "window", label: "Window", icon: AppWindow },
  { id: "region", label: "Region", icon: Scan },
];

const RESOLUTIONS: { id: RecordResolution; label: string }[] = [
  { id: "720", label: "720p" },
  { id: "1080", label: "1080p" },
  { id: "1440", label: "1440p" },
  { id: "native", label: "Native (up to 4K)" },
];

const CURSORS: { id: RecordCursor; label: string }[] = [
  { id: "system", label: "System cursor" },
  { id: "circle", label: "Highlight circle" },
  { id: "hidden", label: "Hidden" },
];

const CORNERS: { id: RecordCameraCorner; label: string }[] = [
  { id: "bottom-left", label: "Bottom left" },
  { id: "bottom-right", label: "Bottom right" },
  { id: "top-left", label: "Top left" },
  { id: "top-right", label: "Top right" },
];

const HOTKEY_SOURCES: { id: RecordSource; label: string }[] = [
  { id: "display", label: "The screen under the pointer" },
  { id: "window", label: "The window in front" },
  { id: "region", label: "A region you drag" },
];

type View = "recordings" | "settings";

function settingsOf(state: RecordState) {
  return state.settings;
}

export function Record() {
  const { state, settings, error, setError, update } = useSettingsSection<RecordState, RecordSettings>({
    load: recordState,
    save: recordSave,
    settingsOf,
  });
  const [view, setView] = useState<View>("recordings");
  const [source, setSource] = useState<RecordSource>("display");
  const [targets, setTargets] = useState<RecordTarget[]>([]);
  const [target, setTarget] = useState("");
  const [status, setStatus] = useState<RecordStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [items, setItems] = useState<RecordEntry[]>([]);
  const [selected, setSelected] = useState("");

  const loadTargets = useCallback(() => {
    void recordTargets()
      .then(setTargets)
      .catch((caught: unknown) => setError(messageOf(caught)));
  }, [setError]);

  const loadLibrary = useCallback(
    (select?: string) => {
      void recordLibrary()
        .then((next) => {
          setItems(next);
          setSelected((current) => {
            const wanted = select ?? current;
            return next.some((entry) => entry.id === wanted) ? wanted : (next[0]?.id ?? "");
          });
        })
        .catch((caught: unknown) => setError(messageOf(caught)));
    },
    [setError],
  );

  useEffect(() => {
    document.title = "Record";
    loadTargets();
    loadLibrary();
    void recordStatus()
      .then(setStatus)
      .catch(() => undefined);
    const listening = [
      onRecordStatus(setStatus),
      onRecordSaved((entry) => {
        setView("recordings");
        loadLibrary(entry.id);
      }),
      onRecordError((message) => {
        setError(message);
        void restoreMainWindow();
      }),
    ];
    return () => {
      for (const pending of listening) void pending.then((stop) => stop());
    };
  }, [loadLibrary, loadTargets, setError]);

  useEffect(() => {
    if (settings) setSource(settings.source);
    // Only the first load picks the source. After that it is the user's choice here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings === null]);

  const running = status !== null && status.phase !== "idle" && status.phase !== "error";
  const screens = targets.filter((item) => item.kind === "display");
  const windows = targets.filter((item) => item.kind === "window");
  const picked = source === "window" ? windows : screens;
  const chosen = picked.some((item) => item.id === target) ? target : (picked[0]?.id ?? "");

  async function start() {
    if (!settings) return;
    setBusy(true);
    setError(null);
    try {
      await minimizeMainWindow();
      await recordStart({
        source,
        target: source === "region" ? "" : chosen,
        microphone: settings.microphone,
        systemAudio: settings.systemAudio,
        camera: settings.camera,
        countdown: settings.countdown,
        resolution: settings.resolution,
        cameraCorner: settings.cameraCorner,
        cursor: settings.cursor,
      });
    } catch (caught) {
      const message = messageOf(caught);
      if (message !== "cancelled") setError(message);
      await restoreMainWindow();
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    setBusy(true);
    try {
      await recordStop();
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  async function changeFolder() {
    if (!state) return;
    const folder = await chooseRecordFolder(state.folder);
    if (folder) update({ folder });
  }

  const startDisabled = busy || !settings || (source !== "region" && picked.length === 0);

  return (
    <section id="record-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="record-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="record-header-row" className="flex items-start justify-between gap-3">
          <div id="record-heading" className="min-w-0">
            <h1 id="record-title" className="text-base font-semibold text-zinc-100">
              Record
            </h1>
            <p id="record-summary" className="mt-1 text-sm text-zinc-400">
              Record a screen, a window, or a region with your voice and camera. The controls float on top and stay out of the video.
              {settings ? ` ${settings.hotkey} starts and stops a recording.` : ""}
            </p>
          </div>
          <div id="record-header-actions" className="flex shrink-0 items-center gap-2">
            <Button
              id="record-enabled"
              variant={settings?.enabled ? "primary" : "outline"}
              aria-pressed={settings?.enabled ?? false}
              disabled={!settings}
              onClick={() => update({ enabled: !settings?.enabled })}
            >
              {settings?.enabled ? "Shortcut on" : "Shortcut off"}
            </Button>
          </div>
        </div>
        <div id="record-start-bar" className="flex flex-wrap items-center gap-2">
          <div id="record-source" role="radiogroup" aria-label="What to record" className="inline-flex rounded-md border border-zinc-700 bg-zinc-950 p-0.5">
            {SOURCES.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  id={`record-source-${item.id}`}
                  key={item.id}
                  type="button"
                  role="radio"
                  aria-checked={source === item.id}
                  disabled={running}
                  className={cn(
                    "inline-flex h-7 items-center gap-1.5 rounded px-2.5 text-sm text-zinc-300 disabled:opacity-40",
                    source === item.id ? "bg-zinc-700 text-white" : "hover:bg-zinc-800",
                  )}
                  onClick={() => setSource(item.id)}
                >
                  <Icon id={`record-source-${item.id}-icon`} className="size-4" aria-hidden />
                  {item.label}
                </button>
              );
            })}
          </div>
          {source === "region" ? (
            <span id="record-region-note" className="text-sm text-zinc-400">
              Drag over the screen to choose the area after you press Start.
            </span>
          ) : (
            <>
              <select
                id="record-target-select"
                aria-label={source === "window" ? "Window to record" : "Screen to record"}
                className={cn(SETTING_SELECT, "max-w-80 min-w-48")}
                value={chosen}
                disabled={running}
                onChange={(event) => setTarget(event.target.value)}
              >
                {picked.length === 0 ? (
                  <option id="record-target-none" value="">
                    Nothing to record
                  </option>
                ) : null}
                {picked.map((item) => (
                  <option id={`record-target-${item.id}`} key={item.id} value={item.id}>
                    {item.label}
                  </option>
                ))}
              </select>
              <Button id="record-targets-refresh" variant="ghost" size="sm" aria-label="Refresh the list" disabled={running} onClick={loadTargets}>
                <RefreshCw id="record-targets-refresh-icon" className="size-4" aria-hidden />
              </Button>
            </>
          )}
          <div id="record-start-actions" className="ml-auto flex items-center gap-2">
            {running ? (
              <>
                <span id="record-live" className="text-sm font-semibold tabular-nums text-rose-300">
                  {status?.phase === "countdown" ? `Starting in ${status.countdown}` : `${status?.phase === "paused" ? "Paused" : "Recording"} ${clock(status?.elapsedMs ?? 0)}`}
                </span>
                <Button id="record-stop" variant="destructive" disabled={busy} onClick={() => void stop()}>
                  <Square id="record-stop-icon" className="size-3.5 fill-current" aria-hidden />
                  Stop
                </Button>
              </>
            ) : (
              <Button id="record-start" variant="primary" disabled={startDisabled} onClick={() => void start()}>
                <Video id="record-start-icon" className="size-4" aria-hidden />
                {busy ? "Starting…" : "Start recording"}
              </Button>
            )}
          </div>
        </div>
        <div id="record-views" role="tablist" aria-label="Record views" className="flex gap-1">
          {(["recordings", "settings"] as const).map((id) => (
            <button
              id={`record-view-${id}`}
              key={id}
              type="button"
              role="tab"
              aria-selected={view === id}
              className={cn("rounded-md px-2.5 py-1 text-sm", view === id ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:text-zinc-200")}
              onClick={() => setView(id)}
            >
              {id === "recordings" ? `Recordings${items.length ? ` (${items.length})` : ""}` : "Settings"}
            </button>
          ))}
        </div>
      </header>
      {error ? (
        <p id="record-error" className="border-b border-zinc-800 px-4 py-2 text-sm text-red-400">
          {error}
        </p>
      ) : null}
      {view === "recordings" ? (
        <RecordLibrary
          items={items}
          selected={selected}
          onSelect={setSelected}
          onChange={(next) => setItems((current) => current.map((entry) => (entry.id === next.id ? next : entry)))}
          onReload={() => loadLibrary()}
        />
      ) : (
        <div id="record-settings" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
          {settings && state ? (
            <>
              <SettingGroup id="record-capture" title="Capture">
                <SettingRow id="record-microphone" title="Microphone" description="Records the default microphone.">
                  <Switch id="record-microphone-switch" label="Microphone" checked={settings.microphone} onChange={(microphone) => update({ microphone })} />
                </SettingRow>
                <SettingRow id="record-system-audio" title="System audio" description="Records the sound playing on this computer.">
                  <Switch id="record-system-audio-switch" label="System audio" checked={settings.systemAudio} onChange={(systemAudio) => update({ systemAudio })} />
                </SettingRow>
                <SettingRow id="record-camera" title="Camera bubble" description="A round camera you can drag while recording and move again afterwards.">
                  <Switch id="record-camera-switch" label="Camera bubble" checked={settings.camera} onChange={(camera) => update({ camera })} />
                </SettingRow>
                <SettingRow id="record-camera-corner" title="Camera starts in">
                  <select
                    id="record-camera-corner-select"
                    aria-label="Camera starts in"
                    className={SETTING_SELECT}
                    value={settings.cameraCorner}
                    onChange={(event) => update({ cameraCorner: event.target.value as RecordCameraCorner })}
                  >
                    {CORNERS.map((corner) => (
                      <option id={`record-camera-corner-${corner.id}`} key={corner.id} value={corner.id}>
                        {corner.label}
                      </option>
                    ))}
                  </select>
                </SettingRow>
                <SettingRow id="record-countdown" title="Countdown" description="Three seconds before the recording starts.">
                  <Switch id="record-countdown-switch" label="Countdown" checked={settings.countdown} onChange={(countdown) => update({ countdown })} />
                </SettingRow>
                <SettingRow id="record-resolution" title="Resolution" description="The longest side of the video. Smaller files upload faster.">
                  <select
                    id="record-resolution-select"
                    aria-label="Resolution"
                    className={SETTING_SELECT}
                    value={settings.resolution}
                    onChange={(event) => update({ resolution: event.target.value as RecordResolution })}
                  >
                    {RESOLUTIONS.map((resolution) => (
                      <option id={`record-resolution-${resolution.id}`} key={resolution.id} value={resolution.id}>
                        {resolution.label}
                      </option>
                    ))}
                  </select>
                </SettingRow>
                <SettingRow id="record-cursor" title="Pointer in the video">
                  <select
                    id="record-cursor-select"
                    aria-label="Pointer in the video"
                    className={SETTING_SELECT}
                    value={settings.cursor}
                    onChange={(event) => update({ cursor: event.target.value as RecordCursor })}
                  >
                    {CURSORS.map((cursor) => (
                      <option id={`record-cursor-${cursor.id}`} key={cursor.id} value={cursor.id}>
                        {cursor.label}
                      </option>
                    ))}
                  </select>
                </SettingRow>
              </SettingGroup>

              <SettingGroup id="record-shortcut" title="Shortcut">
                <SettingRow id="record-hotkey" title="Start and stop" description="Starts a recording with these settings, or stops the one that is running.">
                  <HotkeyField id="record-hotkey-field" value={settings.hotkey} disabled={!settings.enabled} onChange={(hotkey) => update({ hotkey })} />
                  <Button
                    id="record-hotkey-default"
                    variant="ghost"
                    size="sm"
                    disabled={!settings.enabled || settings.hotkey === DEFAULT_HOTKEY}
                    onClick={() => update({ hotkey: DEFAULT_HOTKEY })}
                  >
                    Use default
                  </Button>
                </SettingRow>
                <SettingRow id="record-hotkey-source" title="The shortcut records">
                  <select
                    id="record-hotkey-source-select"
                    aria-label="The shortcut records"
                    className={SETTING_SELECT}
                    value={settings.source}
                    disabled={!settings.enabled}
                    onChange={(event) => update({ source: event.target.value as RecordSource })}
                  >
                    {HOTKEY_SOURCES.map((item) => (
                      <option id={`record-hotkey-source-${item.id}`} key={item.id} value={item.id}>
                        {item.label}
                      </option>
                    ))}
                  </select>
                </SettingRow>
              </SettingGroup>

              <SettingGroup id="record-storage" title="Storage">
                <SettingRow id="record-folder" title="Recordings folder" description={state.folder}>
                  <Button id="record-folder-open" variant="ghost" size="sm" onClick={() => void revealPath(state.folder).catch((caught: unknown) => setError(messageOf(caught)))}>
                    Open
                  </Button>
                  <Button id="record-folder-change" variant="outline" size="sm" onClick={() => void changeFolder()}>
                    Change…
                  </Button>
                  <Button id="record-folder-default" variant="ghost" size="sm" disabled={settings.folder === ""} onClick={() => update({ folder: "" })}>
                    Use default
                  </Button>
                </SettingRow>
              </SettingGroup>
            </>
          ) : null}
        </div>
      )}
    </section>
  );
}
