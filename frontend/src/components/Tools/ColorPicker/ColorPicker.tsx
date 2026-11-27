import { listen } from "@tauri-apps/api/event";
import { ArrowDown, ArrowUp, Pencil, Trash2 } from "lucide-react";
import { useState, useEffect } from "react";
import { ActivationBehavior, ColorPickerState, ColorFormat } from "../../../../../types/color-picker";
import { colorPickerState, colorPickerSave, colorPickerDescribe, colorPickerPick, colorPickerOpenEditor } from "../../../lib/color-picker-client";
import { messageOf } from "../../../lib/message";
import { useSettingsSection } from "../../../lib/use-settings-section";
import { Button } from "../../Button";
import { ColorFormatDialog } from "../../Dialogs/ColorFormatDialog";
import { HotkeyField } from "../../HotkeyField";
import { SettingGroup, SettingRow, SETTING_SELECT } from "../../SettingRow";
import { Switch } from "../../Switch";


const DEFAULT_HOTKEY = "Shift+Win+C";
const SAMPLE = "#ffe4b5";

const BEHAVIORS: { id: ActivationBehavior; label: string }[] = [
  { id: "pickOnly", label: "Only pick a color" },
  { id: "pickThenEditor", label: "Pick a color and open the editor" },
  { id: "editorFirst", label: "Open the editor" },
];

function settingsOf(state: ColorPickerState) {
  return state.settings;
}

type Editing = { index: number | null } | null;

export function ColorPicker() {
  const { state, settings, error, setError, update, accept } = useSettingsSection({
    load: colorPickerState,
    save: colorPickerSave,
    settingsOf,
  });
  const [samples, setSamples] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<Editing>(null);

  const formatKey = settings?.formats.map((format) => `${format.name}\u0000${format.template}`).join("\u0001") ?? "";

  useEffect(() => {
    if (!formatKey) {
      return;
    }
    let cancelled = false;
    void colorPickerDescribe(SAMPLE)
      .then((description) => {
        if (!cancelled) {
          setSamples(Object.fromEntries(description.formats.map((format) => [format.name, format.value])));
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [formatKey]);

  useEffect(() => {
    const subscription = listen<ColorPickerState>("color-picker-changed", (event) => accept(event.payload));
    return () => {
      void subscription.then((stop) => stop());
    };
  }, [accept]);

  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function setFormats(formats: ColorFormat[]) {
    update({ formats });
  }

  function move(index: number, offset: number) {
    if (!settings) {
      return;
    }
    const target = index + offset;
    if (target < 0 || target >= settings.formats.length) {
      return;
    }
    const next = [...settings.formats];
    [next[index], next[target]] = [next[target], next[index]];
    setFormats(next);
  }

  function saveFormat(format: ColorFormat) {
    if (!settings || !editing) {
      return;
    }
    const next = [...settings.formats];
    if (editing.index === null) {
      next.push(format);
      setFormats(next);
    } else {
      const previous = next[editing.index];
      next[editing.index] = format;
      update({
        formats: next,
        defaultFormat: settings.defaultFormat === previous.name ? format.name : settings.defaultFormat,
      });
    }
    setEditing(null);
  }

  const off = !settings?.enabled;
  const editingFormat = editing && editing.index !== null && settings ? settings.formats[editing.index] : null;

  return (
    <section id="color-picker-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="color-picker-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="color-picker-header-row" className="flex items-start justify-between gap-3">
          <div id="color-picker-heading" className="min-w-0">
            <h1 id="color-picker-title" className="text-base font-semibold text-zinc-100">
              Color Picker
            </h1>
            <p id="color-picker-summary" className="mt-1 text-sm text-zinc-400">
              Press the shortcut, then click anywhere on screen to copy that color. The mouse wheel zooms the magnifier and Esc
              cancels.
            </p>
          </div>
          <div id="color-picker-header-actions" className="flex shrink-0 items-center gap-2">
            <Button id="color-picker-pick-now" variant="outline" disabled={off} onClick={() => void run(colorPickerPick)}>
              Pick now
            </Button>
            <Button id="color-picker-open-editor" variant="outline" onClick={() => void run(colorPickerOpenEditor)}>
              Open editor
            </Button>
            <Button
              id="color-picker-enabled"
              variant={settings?.enabled ? "primary" : "outline"}
              aria-pressed={settings?.enabled ?? false}
              disabled={!settings}
              onClick={() => update({ enabled: !settings?.enabled })}
            >
              {settings?.enabled ? "Color Picker on" : "Color Picker off"}
            </Button>
          </div>
        </div>
      </header>
      <div id="color-picker-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-4">
        {error ? (
          <p id="color-picker-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        {settings ? (
          <>
            <SettingGroup id="color-picker-shortcut" title="Shortcut">
              <SettingRow
                id="color-picker-hotkey"
                title="Activation shortcut"
                description="Starts the color picker from any app."
              >
                <HotkeyField
                  id="color-picker-hotkey-field"
                  value={settings.hotkey}
                  disabled={off}
                  onChange={(hotkey) => update({ hotkey })}
                />
                <Button
                  id="color-picker-hotkey-default"
                  variant="ghost"
                  size="sm"
                  disabled={off || settings.hotkey === DEFAULT_HOTKEY}
                  onClick={() => update({ hotkey: DEFAULT_HOTKEY })}
                >
                  Use default
                </Button>
              </SettingRow>
              <SettingRow id="color-picker-behavior" title="Activation behavior">
                <select
                  id="color-picker-behavior-select"
                  aria-label="Activation behavior"
                  value={settings.activationBehavior}
                  disabled={off}
                  onChange={(event) => update({ activationBehavior: event.target.value as ActivationBehavior })}
                  className={SETTING_SELECT}
                >
                  {BEHAVIORS.map((behavior) => (
                    <option id={`color-picker-behavior-${behavior.id}`} key={behavior.id} value={behavior.id}>
                      {behavior.label}
                    </option>
                  ))}
                </select>
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="color-picker-picker" title="Picker behavior">
              <SettingRow
                id="color-picker-default-format"
                title="Default color format"
                description="This format is copied to your clipboard."
              >
                <select
                  id="color-picker-default-format-select"
                  aria-label="Default color format"
                  value={settings.defaultFormat}
                  disabled={off}
                  onChange={(event) => update({ defaultFormat: event.target.value })}
                  className={SETTING_SELECT}
                >
                  {settings.formats.map((format, index) => (
                    <option id={`color-picker-default-format-${index}`} key={format.name} value={format.name}>
                      {samples[format.name] ? `${format.name} - ${samples[format.name]}` : format.name}
                    </option>
                  ))}
                </select>
              </SettingRow>
              <SettingRow
                id="color-picker-show-name"
                title="Show color name"
                description="Shows the name of the color while picking."
              >
                <Switch
                  id="color-picker-show-name-switch"
                  label="Show color name"
                  checked={settings.showColorName}
                  disabled={off}
                  onChange={(showColorName) => update({ showColorName })}
                />
              </SettingRow>
            </SettingGroup>

            <SettingGroup id="color-picker-formats" title="Color formats">
              <SettingRow
                id="color-picker-formats-intro"
                title="Color formats"
                description="Turn formats on to show them in the editor. Reorder, edit, or delete custom formats."
              >
                <Button id="color-picker-formats-add" variant="primary" size="sm" onClick={() => setEditing({ index: null })}>
                  Add new format
                </Button>
              </SettingRow>
              {settings.formats.map((format, index) => {
                const rowId = `color-picker-format-${index}`;
                return (
                  <div id={rowId} key={format.name} className="flex items-center gap-3 px-3 py-2">
                    <div id={`${rowId}-text`} className="min-w-0 flex-1">
                      <p id={`${rowId}-name`} className="text-sm text-zinc-100">
                        {format.name}
                        {format.name === settings.defaultFormat ? (
                          <span id={`${rowId}-default`} className="ml-2 rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                            Default
                          </span>
                        ) : null}
                      </p>
                      <p id={`${rowId}-sample`} className="truncate font-mono text-xs text-zinc-400">
                        {samples[format.name] ?? format.template}
                      </p>
                    </div>
                    <Switch
                      id={`${rowId}-visible`}
                      label={`Show ${format.name}`}
                      checked={format.visible}
                      onChange={(visible) =>
                        setFormats(settings.formats.map((entry, at) => (at === index ? { ...entry, visible } : entry)))
                      }
                    />
                    <div id={`${rowId}-actions`} className="flex items-center gap-0.5">
                      <Button
                        id={`${rowId}-up`}
                        variant="ghost"
                        size="xs"
                        aria-label={`Move ${format.name} up`}
                        disabled={index === 0}
                        onClick={() => move(index, -1)}
                      >
                        <ArrowUp id={`${rowId}-up-icon`} className="size-3.5" aria-hidden="true" />
                      </Button>
                      <Button
                        id={`${rowId}-down`}
                        variant="ghost"
                        size="xs"
                        aria-label={`Move ${format.name} down`}
                        disabled={index === settings.formats.length - 1}
                        onClick={() => move(index, 1)}
                      >
                        <ArrowDown id={`${rowId}-down-icon`} className="size-3.5" aria-hidden="true" />
                      </Button>
                      <Button
                        id={`${rowId}-edit`}
                        variant="ghost"
                        size="xs"
                        aria-label={`Edit ${format.name}`}
                        disabled={format.builtIn}
                        title={format.builtIn ? "Built-in formats can be hidden or moved, not edited." : undefined}
                        onClick={() => setEditing({ index })}
                      >
                        <Pencil id={`${rowId}-edit-icon`} className="size-3.5" aria-hidden="true" />
                      </Button>
                      <Button
                        id={`${rowId}-delete`}
                        variant="ghost"
                        size="xs"
                        aria-label={`Delete ${format.name}`}
                        disabled={format.builtIn}
                        onClick={() => setFormats(settings.formats.filter((_, at) => at !== index))}
                      >
                        <Trash2 id={`${rowId}-delete-icon`} className="size-3.5" aria-hidden="true" />
                      </Button>
                    </div>
                  </div>
                );
              })}
            </SettingGroup>
          </>
        ) : null}
      </div>
      <ColorFormatDialog
        open={editing !== null}
        format={editingFormat}
        takenNames={settings?.formats.map((format) => format.name) ?? []}
        tokens={state?.tokens ?? []}
        onClose={() => setEditing(null)}
        onSave={saveFormat}
      />
    </section>
  );
}
