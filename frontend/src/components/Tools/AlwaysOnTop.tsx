import { useEffect, useState } from "react";
import type { AlwaysOnTopState, BorderColorMode } from "../../../../types/always-on-top";
import { alwaysOnTopSave, alwaysOnTopState, alwaysOnTopUnpinAll } from "../../lib/always-on-top-client";
import { messageOf } from "../../lib/message";
import { useSettingsSection } from "../../lib/use-settings-section";
import { Button } from "../Button";
import { HotkeyField } from "../HotkeyField";
import { SETTING_RANGE, SETTING_SELECT, SettingGroup, SettingRow } from "../SettingRow";
import { Switch } from "../Switch";

const SLIDER_DELAY = 300;
const CHECKBOX = "size-4 accent-sky-500 disabled:opacity-40";

function settingsOf(state: AlwaysOnTopState) {
  return state.settings;
}

export function AlwaysOnTop() {
  const { state, settings, error, setError, update, accept } = useSettingsSection({
    load: alwaysOnTopState,
    save: alwaysOnTopSave,
    settingsOf,
  });
  const [excludedText, setExcludedText] = useState("");
  const savedExcluded = settings?.excludedApps.join("\n") ?? "";

  useEffect(() => {
    setExcludedText(savedExcluded);
  }, [savedExcluded]);

  async function unpinAll() {
    try {
      accept(await alwaysOnTopUnpinAll());
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function saveExcluded() {
    const excludedApps = excludedText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    update({ excludedApps });
  }

  const ready = settings !== null;
  const off = !settings?.enabled;
  const borderOff = off || !settings?.showBorder;
  const swatch = settings?.borderColorMode === "custom" ? settings.borderColor : (state?.accentColor ?? "#0078d4");

  return (
    <section id="always-on-top-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="always-on-top-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="always-on-top-header-row" className="flex items-start justify-between gap-3">
          <div id="always-on-top-heading" className="min-w-0">
            <h1 id="always-on-top-title" className="text-base font-semibold text-zinc-100">
              Always On Top
            </h1>
            <p id="always-on-top-summary" className="mt-1 text-sm text-zinc-400">
              Press the shortcut in any app to pin its window above every other window. Press it again to unpin.
            </p>
          </div>
          <div id="always-on-top-header-actions" className="flex shrink-0 items-center gap-2">
            <Button
              id="always-on-top-unpin-all"
              variant="outline"
              disabled={!state || state.pinnedCount === 0}
              onClick={() => void unpinAll()}
            >
              {state && state.pinnedCount > 0 ? `Unpin all (${state.pinnedCount})` : "Unpin all"}
            </Button>
            <Button
              id="always-on-top-enabled"
              variant={settings?.enabled ? "primary" : "outline"}
              aria-pressed={settings?.enabled ?? false}
              disabled={!ready}
              onClick={() => update({ enabled: !settings?.enabled })}
            >
              {settings?.enabled ? "Always On Top on" : "Always On Top off"}
            </Button>
          </div>
        </div>
      </header>
      <div id="always-on-top-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
        {error ? (
          <p id="always-on-top-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        {settings ? (
          <>
            <SettingGroup id="always-on-top-activation" title="Activation">
              <SettingRow
                id="always-on-top-hotkey"
                title="Activation shortcut"
                description="Pins or unpins the window in front."
              >
                <HotkeyField
                  id="always-on-top-hotkey-field"
                  value={settings.hotkey}
                  disabled={off}
                  onChange={(hotkey) => update({ hotkey })}
                />
                <Button
                  id="always-on-top-hotkey-default"
                  variant="ghost"
                  size="sm"
                  disabled={off || settings.hotkey === "Ctrl+Space"}
                  onClick={() => update({ hotkey: "Ctrl+Space" })}
                >
                  Use Ctrl+Space
                </Button>
              </SettingRow>
              <SettingRow id="always-on-top-game-mode" title="Do not activate when Game Mode is on" nested>
                <input
                  id="always-on-top-game-mode-input"
                  type="checkbox"
                  aria-label="Do not activate when Game Mode is on"
                  checked={settings.skipInGameMode}
                  disabled={off}
                  onChange={(event) => update({ skipInGameMode: event.target.checked })}
                  className={CHECKBOX}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="always-on-top-appearance" title="Appearance & behavior">
              <SettingRow id="always-on-top-border" title="Show a border around the pinned window">
                <Switch
                  id="always-on-top-border-switch"
                  label="Show a border around the pinned window"
                  checked={settings.showBorder}
                  disabled={off}
                  onChange={(showBorder) => update({ showBorder })}
                />
              </SettingRow>
              <SettingRow id="always-on-top-color-mode" title="Color mode" nested>
                <span
                  id="always-on-top-color-swatch"
                  className="size-5 rounded border border-zinc-600"
                  style={{ backgroundColor: swatch }}
                  title={swatch}
                />
                {settings.borderColorMode === "custom" ? (
                  <input
                    id="always-on-top-color-input"
                    type="color"
                    aria-label="Border color"
                    value={settings.borderColor}
                    disabled={borderOff}
                    onChange={(event) => update({ borderColor: event.target.value }, SLIDER_DELAY)}
                    className="h-8 w-10 cursor-pointer rounded-md border border-zinc-700 bg-zinc-950 p-0.5 disabled:opacity-40"
                  />
                ) : null}
                <select
                  id="always-on-top-color-mode-select"
                  aria-label="Color mode"
                  value={settings.borderColorMode}
                  disabled={borderOff}
                  onChange={(event) => update({ borderColorMode: event.target.value as BorderColorMode })}
                  className={SETTING_SELECT}
                >
                  <option id="always-on-top-color-mode-windows" value="windows">
                    Windows default
                  </option>
                  <option id="always-on-top-color-mode-custom" value="custom">
                    Custom color
                  </option>
                </select>
              </SettingRow>
              <SettingRow id="always-on-top-opacity" title="Opacity (%)" nested>
                <input
                  id="always-on-top-opacity-input"
                  type="range"
                  aria-label="Opacity"
                  min={1}
                  max={100}
                  value={settings.borderOpacity}
                  disabled={borderOff}
                  onChange={(event) => update({ borderOpacity: Number(event.target.value) }, SLIDER_DELAY)}
                  className={SETTING_RANGE}
                />
                <span id="always-on-top-opacity-value" className="w-8 text-right text-xs tabular-nums text-zinc-300">
                  {settings.borderOpacity}
                </span>
              </SettingRow>
              <SettingRow id="always-on-top-thickness" title="Thickness (px)" nested>
                <input
                  id="always-on-top-thickness-input"
                  type="range"
                  aria-label="Thickness"
                  min={1}
                  max={30}
                  value={settings.borderThickness}
                  disabled={borderOff}
                  onChange={(event) => update({ borderThickness: Number(event.target.value) }, SLIDER_DELAY)}
                  className={SETTING_RANGE}
                />
                <span id="always-on-top-thickness-value" className="w-8 text-right text-xs tabular-nums text-zinc-300">
                  {settings.borderThickness}
                </span>
              </SettingRow>
              <SettingRow id="always-on-top-round" title="Enable round corners" nested>
                <input
                  id="always-on-top-round-input"
                  type="checkbox"
                  aria-label="Enable round corners"
                  checked={settings.roundCorners}
                  disabled={borderOff}
                  onChange={(event) => update({ roundCorners: event.target.checked })}
                  className={CHECKBOX}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="always-on-top-sound" title="Sound">
              <SettingRow id="always-on-top-sound-row" title="Play a sound when pinning a window">
                <input
                  id="always-on-top-sound-input"
                  type="checkbox"
                  aria-label="Play a sound when pinning a window"
                  checked={settings.playSound}
                  disabled={off}
                  onChange={(event) => update({ playSound: event.target.checked })}
                  className={CHECKBOX}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="always-on-top-excluded" title="Excluded apps">
              <div id="always-on-top-excluded-body" className="flex flex-col gap-2 px-3 py-2.5">
                <label id="always-on-top-excluded-label" htmlFor="always-on-top-excluded-input" className="text-xs text-zinc-400">
                  These apps are never pinned. Write one program name per line, such as outlook.exe.
                </label>
                <textarea
                  id="always-on-top-excluded-input"
                  rows={4}
                  value={excludedText}
                  disabled={off}
                  onChange={(event) => setExcludedText(event.target.value)}
                  onBlur={saveExcluded}
                  className="w-full resize-y rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 font-mono text-sm text-zinc-100 disabled:opacity-40"
                />
              </div>
            </SettingGroup>

            <p id="always-on-top-own-window-note" className="text-xs text-zinc-500">
              When this window is pinned it keeps its own purple frame, the same as the pin button in the toolbar.
            </p>
          </>
        ) : null}
      </div>
    </section>
  );
}
