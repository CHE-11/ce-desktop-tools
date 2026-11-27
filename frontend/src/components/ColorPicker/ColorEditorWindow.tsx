import { listen } from "@tauri-apps/api/event";
import { Check, Copy, Pipette, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { ColorDescription, ColorPickerState } from "../../../../types/color-picker";
import { copyText } from "../../lib/ascii-client";
import { colorPickerDescribe, colorPickerPick, colorPickerSetHistory, colorPickerState } from "../../lib/color-picker-client";
import { hexToHsv, hsvToHex, normalizeHex, readableOn, shade, type Hsv } from "../../lib/color";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const FALLBACK = "#ffe4b5";
const SHADES = [-0.6, -0.4, -0.2, 0, 0.2, 0.4, 0.6];

/** The Color Picker editor. It runs in its own window. */
export function ColorEditorWindow() {
  const [history, setHistory] = useState<string[]>([]);
  const [color, setColor] = useState(FALLBACK);
  const [description, setDescription] = useState<ColorDescription | null>(null);
  const [hexText, setHexText] = useState(FALLBACK);
  const [hsv, setHsv] = useState<Hsv>(() => hexToHsv(FALLBACK));
  const [copied, setCopied] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [formatsVersion, setFormatsVersion] = useState(0);

  useEffect(() => {
    document.title = "Color Picker";
  }, []);

  const applyState = useCallback((state: ColorPickerState) => {
    setHistory(state.history);
    setFormatsVersion((value) => value + 1);
  }, []);

  useEffect(() => {
    void colorPickerState()
      .then((state) => {
        applyState(state);
        if (state.history[0]) {
          setColor(state.history[0]);
        }
      })
      .catch((caught) => setError(messageOf(caught)));
    const subscriptions = [
      listen<string[]>("color-picker-history", (event) => setHistory(event.payload)),
      listen<string>("color-picker-picked", (event) => setColor(event.payload)),
      listen<ColorPickerState>("color-picker-changed", (event) => applyState(event.payload)),
    ];
    return () => {
      void Promise.all(subscriptions).then((stops) => stops.forEach((stop) => stop()));
    };
  }, [applyState]);

  useEffect(() => {
    setHexText(color);
    setHsv((current) => (hsvToHex(current) === color ? current : hexToHsv(color)));
    let cancelled = false;
    void colorPickerDescribe(color)
      .then((next) => {
        if (!cancelled) {
          setDescription(next);
        }
      })
      .catch((caught) => setError(messageOf(caught)));
    return () => {
      cancelled = true;
    };
  }, [color, formatsVersion]);

  async function saveHistory(next: string[]) {
    try {
      setHistory(await colorPickerSetHistory(next));
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  async function copy(name: string, value: string) {
    try {
      await copyText(value);
      setCopied(name);
      window.setTimeout(() => setCopied((current) => (current === name ? null : current)), 1200);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function adjust(patch: Partial<Hsv>) {
    const next = { ...hsv, ...patch };
    setHsv(next);
    setColor(hsvToHex(next));
  }

  function commitHex() {
    const next = normalizeHex(hexText);
    if (next) {
      setColor(next);
    } else {
      setHexText(color);
    }
  }

  const visible = description?.formats.filter((format) => format.visible) ?? [];
  const inHistory = history.includes(color);

  return (
    <div id="color-editor" className="flex h-full min-h-0 bg-zinc-950 text-zinc-100">
      <aside id="color-editor-history" className="flex w-44 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900">
        <div id="color-editor-history-header" className="flex items-center justify-between px-3 py-2">
          <h2 id="color-editor-history-title" className="text-xs font-medium text-zinc-300">
            History
          </h2>
          <Button
            id="color-editor-history-clear"
            variant="ghost"
            size="xs"
            disabled={history.length === 0}
            onClick={() => void saveHistory([])}
          >
            Clear
          </Button>
        </div>
        <ul id="color-editor-history-list" className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-2">
          {history.length === 0 ? (
            <li id="color-editor-history-empty" className="px-1 py-2 text-xs text-zinc-500">
              Picked colors show here.
            </li>
          ) : null}
          {history.map((entry, index) => (
            <li id={`color-editor-history-${index}`} key={entry} className="group flex items-center gap-1">
              <button
                id={`color-editor-history-${index}-select`}
                type="button"
                onClick={() => setColor(entry)}
                className={cn(
                  "flex min-w-0 flex-1 items-center gap-2 rounded-md px-1.5 py-1 text-left font-mono text-xs",
                  entry === color ? "bg-zinc-800 text-zinc-100" : "text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <span
                  id={`color-editor-history-${index}-swatch`}
                  className="size-5 shrink-0 rounded border border-zinc-600"
                  style={{ backgroundColor: entry }}
                />
                {entry}
              </button>
              <button
                id={`color-editor-history-${index}-delete`}
                type="button"
                aria-label={`Remove ${entry} from history`}
                onClick={() => void saveHistory(history.filter((item) => item !== entry))}
                className="invisible rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-100 group-hover:visible"
              >
                <X id={`color-editor-history-${index}-delete-icon`} className="size-3" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      </aside>

      <main id="color-editor-main" className="flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
        {error ? (
          <p id="color-editor-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        <section id="color-editor-current" className="flex items-stretch gap-4">
          <div
            id="color-editor-swatch"
            className="flex h-28 w-40 shrink-0 flex-col justify-end rounded-lg border border-zinc-700 p-2 text-xs"
            style={{ backgroundColor: color, color: readableOn(color) }}
          >
            <span id="color-editor-swatch-hex" className="font-mono">
              {color}
            </span>
            <span id="color-editor-swatch-name">{description?.name ?? ""}</span>
          </div>
          <div id="color-editor-current-actions" className="flex min-w-0 flex-1 flex-col gap-2">
            <div id="color-editor-current-buttons" className="flex flex-wrap gap-2">
              <Button id="color-editor-pick" variant="primary" onClick={() => void colorPickerPick()}>
                <Pipette id="color-editor-pick-icon" className="size-4" aria-hidden="true" />
                Pick a color
              </Button>
              <Button
                id="color-editor-save"
                variant="outline"
                disabled={inHistory}
                onClick={() => void saveHistory([color, ...history])}
              >
                {inHistory ? "In history" : "Add to history"}
              </Button>
              {inHistory ? (
                <Button
                  id="color-editor-remove"
                  variant="ghost"
                  onClick={() => void saveHistory(history.filter((item) => item !== color))}
                >
                  <Trash2 id="color-editor-remove-icon" className="size-4" aria-hidden="true" />
                  Remove
                </Button>
              ) : null}
            </div>
            <label id="color-editor-hex-label" htmlFor="color-editor-hex" className="flex items-center gap-2 text-xs text-zinc-400">
              Hex
              <input
                id="color-editor-hex"
                value={hexText}
                onChange={(event) => setHexText(event.target.value)}
                onBlur={commitHex}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    commitHex();
                  }
                }}
                className="h-8 w-28 rounded-md border border-zinc-700 bg-zinc-950 px-2 font-mono text-sm text-zinc-100"
              />
            </label>
          </div>
        </section>

        <section id="color-editor-adjust" className="flex flex-col gap-2">
          <h2 id="color-editor-adjust-title" className="text-sm font-medium text-zinc-100">
            Adjust
          </h2>
          {(
            [
              { key: "h", label: "Hue", max: 359 },
              { key: "s", label: "Saturation", max: 100 },
              { key: "v", label: "Value", max: 100 },
            ] as const
          ).map((slider) => (
            <label
              id={`color-editor-adjust-${slider.key}`}
              key={slider.key}
              htmlFor={`color-editor-adjust-${slider.key}-input`}
              className="flex items-center gap-3 text-xs text-zinc-400"
            >
              <span id={`color-editor-adjust-${slider.key}-label`} className="w-20">
                {slider.label}
              </span>
              <input
                id={`color-editor-adjust-${slider.key}-input`}
                type="range"
                min={0}
                max={slider.max}
                value={hsv[slider.key]}
                onChange={(event) => adjust({ [slider.key]: Number(event.target.value) })}
                className="flex-1 accent-sky-500"
              />
              <span id={`color-editor-adjust-${slider.key}-value`} className="w-8 text-right tabular-nums text-zinc-300">
                {hsv[slider.key]}
              </span>
            </label>
          ))}
        </section>

        <section id="color-editor-shades" className="flex flex-col gap-2">
          <h2 id="color-editor-shades-title" className="text-sm font-medium text-zinc-100">
            Shades
          </h2>
          <div id="color-editor-shades-row" className="flex overflow-hidden rounded-md border border-zinc-700">
            {SHADES.map((amount, index) => {
              const value = amount === 0 ? color : shade(color, amount);
              return (
                <button
                  id={`color-editor-shade-${index}`}
                  key={amount}
                  type="button"
                  title={value}
                  aria-label={`Use ${value}`}
                  onClick={() => setColor(value)}
                  className={cn("h-10 flex-1", amount === 0 && "ring-2 ring-inset ring-sky-500")}
                  style={{ backgroundColor: value }}
                />
              );
            })}
          </div>
        </section>

        <section id="color-editor-formats" className="flex flex-col gap-2">
          <h2 id="color-editor-formats-title" className="text-sm font-medium text-zinc-100">
            Formats
          </h2>
          <ul id="color-editor-formats-list" className="flex flex-col divide-y divide-zinc-800 rounded-md border border-zinc-800">
            {visible.map((format, index) => (
              <li id={`color-editor-format-${index}`} key={format.name} className="flex items-center gap-3 px-3 py-1.5">
                <span id={`color-editor-format-${index}-name`} className="w-20 shrink-0 text-xs text-zinc-400">
                  {format.name}
                </span>
                <code id={`color-editor-format-${index}-value`} className="min-w-0 flex-1 truncate font-mono text-sm text-zinc-100">
                  {format.value}
                </code>
                <Button
                  id={`color-editor-format-${index}-copy`}
                  variant="ghost"
                  size="xs"
                  aria-label={`Copy ${format.name}`}
                  onClick={() => void copy(format.name, format.value)}
                >
                  {copied === format.name ? (
                    <Check id={`color-editor-format-${index}-copied-icon`} className="size-3.5 text-emerald-400" aria-hidden="true" />
                  ) : (
                    <Copy id={`color-editor-format-${index}-copy-icon`} className="size-3.5" aria-hidden="true" />
                  )}
                </Button>
              </li>
            ))}
          </ul>
          <p id="color-editor-formats-note" className="text-xs text-zinc-500">
            Choose which formats show here on the Color Picker page in the main window.
          </p>
        </section>
      </main>
    </div>
  );
}
