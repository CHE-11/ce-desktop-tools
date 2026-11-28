import { useEffect, useState } from "react";
import { Presentation } from "lucide-react";
import type { PresentCursor, PresentSettings, PresentState } from "../../../../../types/present";
import { cn } from "../../../lib/cn";
import { messageOf } from "../../../lib/message";
import { onPresentChanged, presentSave, presentSettings, presentState, togglePresent } from "../../../lib/present-client";
import { minimizeMainWindow } from "../../../lib/record-client";
import { useSettingsSection } from "../../../lib/use-settings-section";
import { Button } from "../../Button";
import { HotkeyField } from "../../HotkeyField";
import { SettingGroup, SettingRow, SETTING_RANGE, SETTING_SELECT } from "../../SettingRow";
import { Switch } from "../../Switch";
import { PRESENT_COLORS, PRESENT_CURSORS, PRESENT_TOOLS, PRESENT_WIDTHS } from "./present-model";

const DEFAULT_HOTKEY = "Ctrl+Shift+Win+P";
const WIDTH_LABELS = ["Thin", "Medium", "Thick"];

function settingsOf(settings: PresentSettings) {
  return settings;
}

const TOOL_HELP: Record<string, string> = {
  lasso: "Loop around marks to select them, then press Delete.",
  spotlight: "Dims everything except a circle around the pointer. Click it again to drag a fixed area.",
  cursor: "Shows a laser pointer in place of the system cursor. Clicks reach the app underneath.",
  zoom: "Magnifies the screen around the pointer. Click it again to drag a fixed area.",
  highlight: "Drag a rectangle around something.",
  pen: "Draw freehand.",
  arrow: "Drag to point at something.",
  text: "Click to type a label. Drag a label to move it.",
  eraser: "Drag over marks to remove them.",
};

export function Present() {
  const { settings, error, setError, update } = useSettingsSection<PresentSettings, PresentSettings>({
    load: presentSettings,
    save: presentSave,
    settingsOf,
  });
  const [live, setLive] = useState<PresentState | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    document.title = "Present";
    void presentState()
      .then(setLive)
      .catch(() => undefined);
    const listening = onPresentChanged(setLive);
    return () => {
      void listening.then((stop) => stop());
    };
  }, []);

  const on = live?.on ?? false;

  async function toggle() {
    setBusy(true);
    setError(null);
    try {
      if (!on) await minimizeMainWindow();
      setLive(await togglePresent());
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id="present-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="present-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="present-heading" className="min-w-0">
          <h1 id="present-title" className="text-base font-semibold text-zinc-100">
            Present
          </h1>
          <p id="present-summary" className="mt-1 text-sm text-zinc-400">
            Point, spotlight, zoom, and draw on any screen while you share it. A small toolbar floats on the screen with the pointer and is left out of
            screen shares.
            {settings ? ` ${settings.hotkey} turns it on and off.` : ""}
          </p>
        </div>
        <div id="present-header-actions" className="flex shrink-0 items-center gap-2">
          <Button
            id="present-enabled"
            variant={settings?.enabled ? "primary" : "outline"}
            aria-pressed={settings?.enabled ?? false}
            disabled={!settings}
            onClick={() => update({ enabled: !settings?.enabled })}
          >
            {settings?.enabled ? "Shortcut on" : "Shortcut off"}
          </Button>
          <Button id="present-toggle" variant={on ? "destructive" : "secondary"} disabled={busy} onClick={() => void toggle()}>
            <Presentation id="present-toggle-icon" className="size-4" aria-hidden />
            {on ? "Stop presenting" : "Start presenting"}
          </Button>
        </div>
      </header>
      {error ? (
        <p id="present-error" className="border-b border-zinc-800 px-4 py-2 text-sm text-red-400">
          {error}
        </p>
      ) : null}
      <div id="present-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
        {settings ? (
          <>
            <SettingGroup id="present-shortcut" title="Shortcut">
              <SettingRow id="present-hotkey" title="Turn Present on and off" description="Esc leaves the current tool. Esc again turns Present off.">
                <HotkeyField id="present-hotkey-field" value={settings.hotkey} disabled={!settings.enabled} onChange={(hotkey) => update({ hotkey })} />
                <Button
                  id="present-hotkey-default"
                  variant="ghost"
                  size="sm"
                  disabled={!settings.enabled || settings.hotkey === DEFAULT_HOTKEY}
                  onClick={() => update({ hotkey: DEFAULT_HOTKEY })}
                >
                  Use default
                </Button>
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="present-style" title="Starting style">
              <SettingRow id="present-cursor" title="Pointer" description="The laser pointer the cursor and spotlight tools show.">
                <select
                  id="present-cursor-select"
                  aria-label="Pointer"
                  className={SETTING_SELECT}
                  value={settings.cursor}
                  onChange={(event) => update({ cursor: event.target.value as PresentCursor })}
                >
                  {PRESENT_CURSORS.map((cursor) => (
                    <option id={`present-cursor-${cursor.id}`} key={cursor.id} value={cursor.id}>
                      {cursor.label}
                    </option>
                  ))}
                </select>
              </SettingRow>
              <SettingRow id="present-color" title="Color">
                <div id="present-color-options" role="radiogroup" aria-label="Color" className="flex items-center gap-2">
                  {PRESENT_COLORS.map((color) => (
                    <button
                      id={`present-color-${color.slice(1)}`}
                      key={color}
                      type="button"
                      role="radio"
                      aria-checked={settings.color === color}
                      aria-label={color}
                      className={cn(
                        "size-6 rounded-full ring-1 ring-white/25 ring-offset-2 ring-offset-zinc-900",
                        settings.color === color && "ring-2 ring-sky-400",
                      )}
                      style={{ background: color }}
                      onClick={() => update({ color })}
                    />
                  ))}
                </div>
              </SettingRow>
              <SettingRow id="present-width" title="Stroke">
                <div id="present-width-options" role="radiogroup" aria-label="Stroke" className="inline-flex rounded-md border border-zinc-700 bg-zinc-950 p-0.5">
                  {PRESENT_WIDTHS.map((width, index) => (
                    <button
                      id={`present-width-${String(width).replace(".", "-")}`}
                      key={width}
                      type="button"
                      role="radio"
                      aria-checked={Math.abs(settings.width - width) < 0.05}
                      className={cn(
                        "h-7 rounded px-2.5 text-sm text-zinc-300",
                        Math.abs(settings.width - width) < 0.05 ? "bg-zinc-700 text-white" : "hover:bg-zinc-800",
                      )}
                      onClick={() => update({ width })}
                    >
                      {WIDTH_LABELS[index]}
                    </button>
                  ))}
                </div>
              </SettingRow>
              <SettingRow id="present-dim" title="Spotlight dim" description={`${Math.round(settings.dim * 100)}%`}>
                <input
                  id="present-dim-input"
                  type="range"
                  aria-label="Spotlight dim"
                  min={0}
                  max={92}
                  step={1}
                  className={SETTING_RANGE}
                  value={Math.round(settings.dim * 100)}
                  onChange={(event) => update({ dim: Number(event.target.value) / 100 }, 300)}
                />
              </SettingRow>
              <SettingRow id="present-spread" title="Circle size" description={`${Math.round(settings.spread)} px`}>
                <input
                  id="present-spread-input"
                  type="range"
                  aria-label="Circle size"
                  min={80}
                  max={480}
                  step={8}
                  className={SETTING_RANGE}
                  value={Math.round(settings.spread)}
                  onChange={(event) => update({ spread: Number(event.target.value) }, 300)}
                />
              </SettingRow>
              <SettingRow id="present-circle" title="Circle around the pointer" description="Adds a ring around the cursor tool's pointer.">
                <Switch id="present-circle-switch" label="Circle around the pointer" checked={settings.circle} onChange={(circle) => update({ circle })} />
              </SettingRow>
              <SettingRow id="present-radius" title="Rectangle corners" description={`${Math.round(settings.radius)} px`}>
                <input
                  id="present-radius-input"
                  type="range"
                  aria-label="Rectangle corners"
                  min={0}
                  max={40}
                  step={1}
                  className={SETTING_RANGE}
                  value={Math.round(settings.radius)}
                  onChange={(event) => update({ radius: Number(event.target.value) }, 300)}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="present-tools" title="Tools">
              <ul id="present-tools-list" className="flex flex-col divide-y divide-zinc-800 rounded-md border border-zinc-800">
                {PRESENT_TOOLS.map((tool) => (
                  <li id={`present-tools-${tool.id}`} key={tool.id} className="flex gap-3 px-3 py-2 text-sm">
                    <span id={`present-tools-${tool.id}-name`} className="w-24 shrink-0 font-medium text-zinc-200">
                      {tool.label}
                    </span>
                    <span id={`present-tools-${tool.id}-help`} className="text-zinc-400">
                      {TOOL_HELP[tool.id]}
                    </span>
                  </li>
                ))}
              </ul>
            </SettingGroup>
          </>
        ) : null}
      </div>
    </section>
  );
}
