import { MeasureStyle, ExtraUnits, ScreenRulerSettings } from "../../../../../types/screen-ruler";
import { messageOf } from "../../../lib/message";
import { screenRulerState, screenRulerSave, screenRulerOpen } from "../../../lib/screen-ruler-client";
import { useSettingsSection } from "../../../lib/use-settings-section";
import { Button } from "../../Button";
import { HotkeyField } from "../../HotkeyField";
import { SettingGroup, SettingRow, SETTING_SELECT, SETTING_RANGE } from "../../SettingRow";
import { Switch } from "../../Switch";


const SLIDER_DELAY = 300;
const DEFAULT_HOTKEY = "Ctrl+Shift+Win+M";

const STYLES: { id: MeasureStyle; label: string }[] = [
  { id: "none", label: "None" },
  { id: "bounds", label: "Bounds" },
  { id: "spacing", label: "Spacing" },
  { id: "horizontal", label: "Horizontal spacing" },
  { id: "vertical", label: "Vertical spacing" },
];

const UNITS: { id: ExtraUnits; label: string }[] = [
  { id: "none", label: "Show only pixels" },
  { id: "inches", label: "Inches" },
  { id: "centimeters", label: "Centimeters" },
  { id: "millimeters", label: "Millimeters" },
];

function settingsOf(settings: ScreenRulerSettings) {
  return settings;
}

export function ScreenRuler() {
  const { settings, error, setError, update } = useSettingsSection({
    load: screenRulerState,
    save: screenRulerSave,
    settingsOf,
  });

  async function openNow() {
    try {
      await screenRulerOpen();
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  const off = !settings?.enabled;

  return (
    <section id="screen-ruler-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="screen-ruler-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="screen-ruler-header-row" className="flex items-start justify-between gap-3">
          <div id="screen-ruler-heading" className="min-w-0">
            <h1 id="screen-ruler-title" className="text-base font-semibold text-zinc-100">
              Screen Ruler
            </h1>
            <p id="screen-ruler-summary" className="mt-1 text-sm text-zinc-400">
              Press the shortcut to open the command bar, then measure pixels on your screen. Click copies the
              measurement. Esc or a right-click closes it.
            </p>
          </div>
          <div id="screen-ruler-header-actions" className="flex shrink-0 items-center gap-2">
            <Button id="screen-ruler-open" variant="outline" disabled={off} onClick={() => void openNow()}>
              Open now
            </Button>
            <Button
              id="screen-ruler-enabled"
              variant={settings?.enabled ? "primary" : "outline"}
              aria-pressed={settings?.enabled ?? false}
              disabled={!settings}
              onClick={() => update({ enabled: !settings?.enabled })}
            >
              {settings?.enabled ? "Screen Ruler on" : "Screen Ruler off"}
            </Button>
          </div>
        </div>
      </header>
      <div id="screen-ruler-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
        {error ? (
          <p id="screen-ruler-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        {settings ? (
          <>
            <SettingGroup id="screen-ruler-activation" title="Activation">
              <SettingRow
                id="screen-ruler-hotkey"
                title="Activation shortcut"
                description="Brings up the command bar."
              >
                <HotkeyField
                  id="screen-ruler-hotkey-field"
                  value={settings.hotkey}
                  disabled={off}
                  onChange={(hotkey) => update({ hotkey })}
                />
                <Button
                  id="screen-ruler-hotkey-default"
                  variant="ghost"
                  size="sm"
                  disabled={off || settings.hotkey === DEFAULT_HOTKEY}
                  onClick={() => update({ hotkey: DEFAULT_HOTKEY })}
                >
                  Use default
                </Button>
              </SettingRow>
              <SettingRow
                id="screen-ruler-default-style"
                title="Default measure style"
                description="The command bar opens with this style already measuring."
              >
                <select
                  id="screen-ruler-default-style-select"
                  aria-label="Default measure style"
                  value={settings.defaultMeasureStyle}
                  disabled={off}
                  onChange={(event) => update({ defaultMeasureStyle: event.target.value as MeasureStyle })}
                  className={SETTING_SELECT}
                >
                  {STYLES.map((style) => (
                    <option id={`screen-ruler-default-style-${style.id}`} key={style.id} value={style.id}>
                      {style.label}
                    </option>
                  ))}
                </select>
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="screen-ruler-behavior" title="Behavior">
              <SettingRow
                id="screen-ruler-continuous"
                title="Capture screen continuously during measuring"
                description="Refreshes the screen while you measure instead of taking one screenshot."
              >
                <Switch
                  id="screen-ruler-continuous-switch"
                  label="Capture screen continuously during measuring"
                  checked={settings.continuousCapture}
                  disabled={off}
                  onChange={(continuousCapture) => update({ continuousCapture })}
                />
              </SettingRow>
              <SettingRow
                id="screen-ruler-per-channel"
                title="Per color channel edge detection"
                description="On: every color channel must stay within the tolerance. Off: the sum of the channel differences must stay within the tolerance."
              >
                <Switch
                  id="screen-ruler-per-channel-switch"
                  label="Per color channel edge detection"
                  checked={settings.perChannelEdgeDetection}
                  disabled={off}
                  onChange={(perChannelEdgeDetection) => update({ perChannelEdgeDetection })}
                />
              </SettingRow>
              <SettingRow id="screen-ruler-tolerance" title="Pixel tolerance for edge detection">
                <input
                  id="screen-ruler-tolerance-input"
                  type="range"
                  aria-label="Pixel tolerance for edge detection"
                  min={0}
                  max={255}
                  value={settings.pixelTolerance}
                  disabled={off}
                  onChange={(event) => update({ pixelTolerance: Number(event.target.value) }, SLIDER_DELAY)}
                  className={SETTING_RANGE}
                />
                <span id="screen-ruler-tolerance-value" className="w-8 text-right text-xs tabular-nums text-zinc-300">
                  {settings.pixelTolerance}
                </span>
              </SettingRow>
              <SettingRow id="screen-ruler-units" title="Extra units of measurement">
                <select
                  id="screen-ruler-units-select"
                  aria-label="Extra units of measurement"
                  value={settings.extraUnits}
                  disabled={off}
                  onChange={(event) => update({ extraUnits: event.target.value as ExtraUnits })}
                  className={SETTING_SELECT}
                >
                  {UNITS.map((unit) => (
                    <option id={`screen-ruler-units-${unit.id}`} key={unit.id} value={unit.id}>
                      {unit.label}
                    </option>
                  ))}
                </select>
              </SettingRow>
              <SettingRow id="screen-ruler-feet" title="Draw feet on cross" description="Adds feet to the end of cross lines.">
                <Switch
                  id="screen-ruler-feet-switch"
                  label="Draw feet on cross"
                  checked={settings.drawFeetOnCross}
                  disabled={off}
                  onChange={(drawFeetOnCross) => update({ drawFeetOnCross })}
                />
              </SettingRow>
              <SettingRow id="screen-ruler-line-color" title="Line color">
                <span id="screen-ruler-line-color-value" className="font-mono text-xs text-zinc-400">
                  {settings.lineColor}
                </span>
                <input
                  id="screen-ruler-line-color-input"
                  type="color"
                  aria-label="Line color"
                  value={settings.lineColor}
                  disabled={off}
                  onChange={(event) => update({ lineColor: event.target.value }, SLIDER_DELAY)}
                  className="h-8 w-10 cursor-pointer rounded-md border border-zinc-700 bg-zinc-950 p-0.5 disabled:opacity-40"
                />
              </SettingRow>
            </SettingGroup>
          </>
        ) : null}
      </div>
    </section>
  );
}
