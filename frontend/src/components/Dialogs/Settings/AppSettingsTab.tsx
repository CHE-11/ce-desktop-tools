import { FolderSearch, Loader2, RefreshCcw, Trash2 } from "lucide-react";
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
import { MAX_ZOOM, MIN_ZOOM, ZOOM_STEP } from "../../../lib/zoom";
import { Button } from "../../Button";

type AppSettingsTabProps = {
  busy: boolean;
  exePath: string;
  folder: string;
  zoom: number;
  theme: ThemeId;
  colorMode: ColorMode;
  toolbarLabels: boolean;
  runAtStartup: boolean;
  warning: string | null;
  error: string | null;
  onFolderChange: (folder: string) => void;
  onFolderCommit: () => void;
  onZoomChange: (zoom: number) => void;
  onThemeChange: (theme: ThemeId) => void;
  onColorModeChange: (colorMode: ColorMode) => void;
  onToolbarLabelsChange: (toolbarLabels: boolean) => void;
  onRunAtStartupChange: (runAtStartup: boolean) => void;
  onBrowse: () => void;
  onClearFolder: () => void;
  onRevealExe: () => void;
  onCheckForUpdates: () => void;
  checkingUpdates: boolean;
  onSubmit: () => void;
};

export function AppSettingsTab({
  busy,
  exePath,
  folder,
  zoom,
  theme,
  colorMode,
  toolbarLabels,
  runAtStartup,
  warning,
  error,
  onFolderChange,
  onFolderCommit,
  onZoomChange,
  onThemeChange,
  onColorModeChange,
  onToolbarLabelsChange,
  onRunAtStartupChange,
  onBrowse,
  onClearFolder,
  onRevealExe,
  onCheckForUpdates,
  checkingUpdates,
  onSubmit,
}: AppSettingsTabProps) {
  const settingsFile = folder.trim() ? childPath(folder.trim(), "ce-desktop-tools.json") : "";
  const themeStylesheet = folder.trim() ? childPath(folder.trim(), THEME_STYLESHEET_NAME) : "";

  return (
    <form
      id="settings-panel-app"
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <p id="app-settings-intro" className="text-sm leading-5 text-zinc-400">
        Appearance is stored in ce-desktop-tools.json when a folder is set. expansion.json is saved in that same folder.
      </p>

      <section id="app-settings-application" className="flex min-w-0 items-center gap-3 rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2">
        <span id="app-settings-exe-label" className="shrink-0 text-[11px] font-semibold tracking-wide text-zinc-500 uppercase">
          Application
        </span>
        <p id="app-settings-exe" className="min-w-0 flex-1 truncate font-mono text-xs text-zinc-200 select-all" title={exePath}>
          {exePath || "Looking up this application…"}
        </p>
        <Button
          id="app-settings-exe-reveal"
          type="button"
          size="sm"
          variant="outline"
          className="shrink-0"
          title="Show in File Explorer"
          disabled={busy || exePath.length === 0}
          onClick={onRevealExe}
        >
          <FolderSearch id="app-settings-exe-reveal-icon" className="size-3.5" aria-hidden="true" />
        </Button>
        <Button
          id="app-settings-check-updates"
          type="button"
          size="sm"
          variant="outline"
          className={cn("shrink-0", checkingUpdates && "disabled:opacity-100")}
          disabled={busy || checkingUpdates}
          aria-busy={checkingUpdates}
          onClick={onCheckForUpdates}
        >
          {checkingUpdates ? (
            <Loader2 id="app-settings-check-updates-spinner" className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <RefreshCcw id="app-settings-check-updates-icon" className="size-3.5" aria-hidden="true" />
          )}
          <span id="app-settings-check-updates-label">{checkingUpdates ? "Checking…" : "Check for updates"}</span>
        </Button>
      </section>

      <label id="app-settings-startup" className="flex items-start gap-2 text-sm text-zinc-200">
        <input
          id="app-settings-startup-input"
          type="checkbox"
          checked={runAtStartup}
          disabled={busy}
          onChange={(event) => onRunAtStartupChange(event.target.checked)}
          className="mt-0.5 size-4 accent-sky-500 disabled:opacity-50"
        />
        <span id="app-settings-startup-copy">
          <span id="app-settings-startup-text" className="block">
            Run when Windows starts
          </span>
          <span id="app-settings-startup-hint" className="mt-0.5 block text-xs leading-5 text-zinc-500">
            Starts in the tray when you sign in, so text expansion is available without opening the window.
          </span>
        </span>
      </label>

      <SettingsSection
        id="app-settings-location"
        title="Settings folder"
        hint="Leave this blank to keep appearance on this computer. A folder stores ce-desktop-tools.json and expansion.json together. Clear the folder to stop using those files without deleting them."
      >
        <label id="app-settings-folder-label" htmlFor="app-settings-folder-input" className="mb-1 block text-xs font-medium text-zinc-400">
          Folder
        </label>
        <div id="app-settings-folder-row" className="flex items-center gap-2">
          <div id="app-settings-folder-field" className="relative min-w-0 flex-1">
            <input
              id="app-settings-folder-input"
              value={folder}
              onChange={(event) => onFolderChange(event.target.value)}
              onBlur={(event) => {
                const next = event.relatedTarget;
                if (next instanceof HTMLElement && (next.id === "app-settings-browse" || next.id === "app-settings-clear")) {
                  return;
                }
                onFolderCommit();
              }}
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              placeholder="\\\\server\\share\\ce-desktop-tools"
              className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 pr-20 pl-3 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
            />
            <Button
              id="app-settings-clear"
              type="button"
              variant="ghost"
              size="xs"
              className="absolute top-1/2 right-1 -translate-y-1/2"
              onClick={onClearFolder}
              disabled={busy || folder.trim().length === 0}
            >
              <Trash2 id="app-settings-clear-icon" className="size-3.5" aria-hidden="true" />
              <span id="app-settings-clear-label">Clear</span>
            </Button>
          </div>
          <Button id="app-settings-browse" type="button" onClick={onBrowse} disabled={busy}>
            <FolderSearch id="app-settings-browse-icon" className="size-3.5" aria-hidden="true" />
            <span id="app-settings-browse-label">Browse</span>
          </Button>
        </div>
        <p id="app-settings-folder-file" className="mt-2 truncate font-mono text-xs text-zinc-500" title={settingsFile || undefined}>
          {settingsFile ? `Settings file: ${settingsFile}` : "Settings file: on this computer"}
        </p>
      </SettingsSection>

      <SettingsSection id="app-settings-appearance" title="Appearance" hint="Text size, color mode, and toolbar labels apply as you change them. Save keeps the choice.">
        <label id="app-settings-zoom-label" htmlFor="app-settings-zoom" className="mb-1 block text-xs font-medium text-zinc-400">
          Text size
        </label>
        <div id="app-settings-zoom-row" className="flex items-center gap-3">
          <input
            id="app-settings-zoom"
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
          <span id="app-settings-zoom-value" className="w-12 shrink-0 text-right text-sm text-zinc-200 tabular-nums">
            {zoom}%
          </span>
        </div>
        <p id="app-settings-zoom-hint" className="mt-1 text-xs text-zinc-500">
          Ctrl plus and Ctrl minus change this while you work. The status bar shows the current size.
        </p>

        <div id="app-settings-mode-block" className="mt-4">
          <label id="app-settings-mode-label" className="mb-1 block text-xs font-medium text-zinc-400">
            Color mode
          </label>
          <div id="app-settings-modes" className="flex overflow-hidden rounded-md border border-zinc-700" role="group" aria-labelledby="app-settings-mode-label">
            {COLOR_MODE_OPTIONS.map((option) => {
              const selected = colorMode === option.id;
              return (
                <button
                  id={`app-settings-mode-${option.id}`}
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
                  <span id={`app-settings-mode-${option.id}-label`}>{option.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        <label id="app-settings-toolbar-labels" className="mt-4 flex items-center gap-2 text-sm text-zinc-200">
          <input
            id="app-settings-toolbar-labels-input"
            type="checkbox"
            checked={toolbarLabels}
            disabled={busy}
            onChange={(event) => onToolbarLabelsChange(event.target.checked)}
            className="size-4 accent-sky-500 disabled:opacity-50"
          />
          <span id="app-settings-toolbar-labels-text">Show names on toolbar buttons</span>
        </label>

        <div id="app-settings-theme-block" className="mt-4">
          <label id="app-settings-theme-label" className="mb-1 block text-xs font-medium text-zinc-400">
            Theme
          </label>
          <div id="app-settings-themes" className="grid grid-cols-2 gap-2 sm:grid-cols-3" role="group" aria-labelledby="app-settings-theme-label">
            {THEMES.map((option) => {
              const selected = theme === option.id;
              const swatch = option.swatch[resolveMode(colorMode)];
              return (
                <button
                  id={`app-settings-theme-${option.id}`}
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
                    id={`app-settings-theme-${option.id}-swatch`}
                    className="relative h-7 w-10 shrink-0 overflow-hidden rounded border border-zinc-600"
                    style={{ background: swatch.background }}
                    aria-hidden="true"
                  >
                    <span
                      id={`app-settings-theme-${option.id}-swatch-surface`}
                      className="absolute right-1 bottom-1 h-2.5 w-4 rounded-sm"
                      style={{ background: swatch.surface }}
                    />
                    <span
                      id={`app-settings-theme-${option.id}-swatch-accent`}
                      className="absolute top-1 left-1 size-1.5 rounded-full"
                      style={{ background: swatch.accent }}
                    />
                    <span
                      id={`app-settings-theme-${option.id}-swatch-text`}
                      className="absolute top-0.5 right-1 text-[8px] leading-none font-semibold"
                      style={{ color: swatch.text }}
                    >
                      A
                    </span>
                  </span>
                  <span id={`app-settings-theme-${option.id}-label`}>{option.label}</span>
                </button>
              );
            })}
          </div>
          <p id="app-settings-theme-css-hint" className="mt-2 text-xs leading-5 text-zinc-500">
            {themeStylesheet
              ? `${themeStylesheet} is created when it is missing. The sample rules are commented out.`
              : `Choose a settings folder and ce-desktop-tools will add ${THEME_STYLESHEET_NAME} there.`}
          </p>
        </div>
      </SettingsSection>

      {warning ? (
        <p id="app-settings-warning" className="text-sm text-amber-300">
          {warning}
        </p>
      ) : null}
      {error ? (
        <p id="app-settings-error" className="text-sm text-red-400">
          {error}
        </p>
      ) : null}
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
