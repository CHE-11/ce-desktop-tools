import type { ReactNode } from "react";
import {
  COLOR_MODE_OPTIONS,
  resolveMode,
  THEME_STYLESHEET_NAME,
  THEMES,
  type ColorMode,
  type ThemeId,
} from "../../../lib/appearance";
import { cn } from "../../../lib/cn";
import { childPath } from "../../../lib/message";
import { DENSITY_OPTIONS, MAX_DENSITY, MIN_DENSITY, DENSITY_STEP } from "../../../lib/density";
import { MAX_ZOOM, MIN_ZOOM, ZOOM_STEP } from "../../../lib/zoom";

type AppearanceSettingsTabProps = {
  busy: boolean;
  folder: string;
  zoom: number;
  density: number;
  theme: ThemeId;
  colorMode: ColorMode;
  toolbarLabels: boolean;
  onZoomChange: (zoom: number) => void;
  onDensityChange: (density: number) => void;
  onThemeChange: (theme: ThemeId) => void;
  onColorModeChange: (colorMode: ColorMode) => void;
  onToolbarLabelsChange: (toolbarLabels: boolean) => void;
  onSubmit: () => void;
};

export function AppearanceSettingsTab({
  busy,
  folder,
  zoom,
  density,
  theme,
  colorMode,
  toolbarLabels,
  onZoomChange,
  onDensityChange,
  onThemeChange,
  onColorModeChange,
  onToolbarLabelsChange,
  onSubmit,
}: AppearanceSettingsTabProps) {
  const themeStylesheet = folder.trim() ? childPath(folder.trim(), THEME_STYLESHEET_NAME) : "";

  return (
    <form
      id="settings-panel-appearance"
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <p id="appearance-settings-intro" className="text-sm leading-5 text-zinc-400">
        Text size, spacing, color mode, and toolbar labels apply as you change them. Save keeps the choice in the settings folder when one is set.
      </p>

      <SettingsSection id="appearance-settings-layout" title="Layout" hint="Fine-tune text size and spacing. Spacing scales padding and gaps across the app, including the sidebar.">
        <label id="appearance-settings-zoom-label" htmlFor="appearance-settings-zoom" className="mb-1 block text-xs font-medium text-zinc-400">
          Text size
        </label>
        <div id="appearance-settings-zoom-row" className="flex items-center gap-3">
          <input
            id="appearance-settings-zoom"
            type="range"
            min={MIN_ZOOM}
            max={MAX_ZOOM}
            step={ZOOM_STEP}
            value={zoom}
            disabled={busy}
            aria-valuemin={MIN_ZOOM}
            aria-valuemax={MAX_ZOOM}
            aria-valuenow={zoom}
            onChange={(event) => onZoomChange(Number(event.target.value))}
            className="w-full accent-sky-500 disabled:opacity-50"
          />
          <span id="appearance-settings-zoom-value" className="w-12 shrink-0 text-right text-sm text-zinc-200 tabular-nums">
            {zoom}%
          </span>
        </div>
        <p id="appearance-settings-zoom-hint" className="mt-1 text-xs text-zinc-500">
          Ctrl plus and Ctrl minus change this while you work. Range {MIN_ZOOM}%–{MAX_ZOOM}% in {ZOOM_STEP}% steps.
        </p>

        <label
          id="appearance-settings-density-label"
          htmlFor="appearance-settings-density"
          className="mt-4 mb-1 block text-xs font-medium text-zinc-400"
        >
          Sidebar & spacing
        </label>
        <div id="appearance-settings-density-row" className="flex items-center gap-3">
          <input
            id="appearance-settings-density"
            type="range"
            min={MIN_DENSITY}
            max={MAX_DENSITY}
            step={DENSITY_STEP}
            value={density}
            disabled={busy}
            aria-valuemin={MIN_DENSITY}
            aria-valuemax={MAX_DENSITY}
            aria-valuenow={density}
            onChange={(event) => onDensityChange(Number(event.target.value))}
            className="w-full accent-sky-500 disabled:opacity-50"
          />
          <span id="appearance-settings-density-value" className="w-12 shrink-0 text-right text-sm text-zinc-200 tabular-nums">
            {density}%
          </span>
        </div>
        <div id="appearance-settings-density-presets" className="mt-2 flex flex-wrap gap-1.5" role="group" aria-label="Spacing presets">
          {DENSITY_OPTIONS.map((option) => {
            const selected = density === option.id;
            return (
              <button
                id={`appearance-settings-density-preset-${option.id}`}
                key={option.id}
                type="button"
                disabled={busy}
                aria-pressed={selected}
                onClick={() => onDensityChange(option.id)}
                className={cn(
                  "h-7 rounded-md border px-2.5 text-xs disabled:opacity-50",
                  selected
                    ? "border-sky-500 bg-sky-600/20 text-sky-200"
                    : "border-zinc-700 bg-zinc-950 text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <span id={`appearance-settings-density-preset-${option.id}-label`}>{option.label}</span>
              </button>
            );
          })}
        </div>
        <p id="appearance-settings-density-hint" className="mt-1 text-xs text-zinc-500">
          Lower is tighter. Range {MIN_DENSITY}%–{MAX_DENSITY}% in {DENSITY_STEP}% steps.
        </p>
      </SettingsSection>

      <SettingsSection id="appearance-settings-theme" title="Theme" hint="Color mode and palette apply immediately. Save keeps the choice.">
        <label id="appearance-settings-mode-label" className="mb-1 block text-xs font-medium text-zinc-400">
          Color mode
        </label>
        <div id="appearance-settings-modes" className="flex overflow-hidden rounded-md border border-zinc-700" role="group" aria-labelledby="appearance-settings-mode-label">
          {COLOR_MODE_OPTIONS.map((option) => {
            const selected = colorMode === option.id;
            return (
              <button
                id={`appearance-settings-mode-${option.id}`}
                key={option.id}
                type="button"
                aria-pressed={selected}
                disabled={busy}
                onClick={() => onColorModeChange(option.id)}
                className={cn(
                  "h-8 flex-1 text-sm disabled:opacity-50",
                  selected ? "bg-sky-600 text-white" : "bg-zinc-950 text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <span id={`appearance-settings-mode-${option.id}-label`}>{option.label}</span>
              </button>
            );
          })}
        </div>

        <label id="appearance-settings-toolbar-labels" className="mt-4 flex items-center gap-2 text-sm text-zinc-200">
          <input
            id="appearance-settings-toolbar-labels-input"
            type="checkbox"
            checked={toolbarLabels}
            disabled={busy}
            onChange={(event) => onToolbarLabelsChange(event.target.checked)}
            className="size-4 accent-sky-500 disabled:opacity-50"
          />
          <span id="appearance-settings-toolbar-labels-text">Show names on toolbar buttons</span>
        </label>

        <div id="appearance-settings-theme-block" className="mt-4">
          <label id="appearance-settings-theme-label" className="mb-1 block text-xs font-medium text-zinc-400">
            Theme
          </label>
          <div id="appearance-settings-themes" className="grid grid-cols-2 gap-2 sm:grid-cols-3" role="group" aria-labelledby="appearance-settings-theme-label">
            {THEMES.map((option) => {
              const selected = theme === option.id;
              const swatch = option.swatch[resolveMode(colorMode)];
              return (
                <button
                  id={`appearance-settings-theme-${option.id}`}
                  key={option.id}
                  type="button"
                  aria-pressed={selected}
                  disabled={busy}
                  onClick={() => onThemeChange(option.id)}
                  className={cn(
                    "flex items-center gap-2.5 rounded-md border px-2.5 py-1.5 text-left text-sm disabled:opacity-50",
                    selected ? "border-sky-500 bg-zinc-800 text-zinc-100" : "border-zinc-700 bg-zinc-950 text-zinc-300 hover:bg-zinc-800",
                  )}
                >
                  <span
                    id={`appearance-settings-theme-${option.id}-swatch`}
                    className="relative h-7 w-10 shrink-0 overflow-hidden rounded border border-zinc-600"
                    style={{ background: swatch.background }}
                    aria-hidden="true"
                  >
                    <span
                      id={`appearance-settings-theme-${option.id}-swatch-surface`}
                      className="absolute right-1 bottom-1 h-2.5 w-4 rounded-sm"
                      style={{ background: swatch.surface }}
                    />
                    <span
                      id={`appearance-settings-theme-${option.id}-swatch-accent`}
                      className="absolute top-1 left-1 size-1.5 rounded-full"
                      style={{ background: swatch.accent }}
                    />
                    <span
                      id={`appearance-settings-theme-${option.id}-swatch-text`}
                      className="absolute top-0.5 right-1 text-[8px] leading-none font-semibold"
                      style={{ color: swatch.text }}
                    >
                      A
                    </span>
                  </span>
                  <span id={`appearance-settings-theme-${option.id}-label`}>{option.label}</span>
                </button>
              );
            })}
          </div>
          <p id="appearance-settings-theme-css-hint" className="mt-2 text-xs leading-5 text-zinc-500">
            {themeStylesheet
              ? `${themeStylesheet} is created when it is missing. The sample rules are commented out.`
              : `Choose a settings folder and ce-desktop-tools will add ${THEME_STYLESHEET_NAME} there.`}
          </p>
        </div>
      </SettingsSection>
    </form>
  );
}

function SettingsSection({ id, title, hint, children }: { id: string; title: string; hint?: string; children: ReactNode }) {
  return (
    <section id={id} className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-3">
      <h3 id={`${id}-heading`} className="text-[11px] font-semibold tracking-wide text-zinc-400 uppercase">
        {title}
      </h3>
      {hint ? (
        <p id={`${id}-hint`} className="mt-1 mb-3 text-xs leading-5 text-zinc-500">
          {hint}
        </p>
      ) : null}
      {children}
    </section>
  );
}
