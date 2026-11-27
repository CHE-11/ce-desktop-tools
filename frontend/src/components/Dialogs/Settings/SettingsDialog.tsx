import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type { AppSettings } from "../../../../../types/app";
import {
  applyAppearance,
  normalizeColorMode,
  normalizeTheme,
  publishAppearance,
  readAppearance,
  type ColorMode,
  type ThemeId,
} from "../../../lib/appearance";
import { appExecutable, appSettings, appVersion, previewAppSettings, revealPath, saveAppSettings } from "../../../lib/app-client";
import { cn } from "../../../lib/cn";
import { messageOf, pickedFolderPath } from "../../../lib/message";
import { normalizeDensity, publishDensity, readDensity } from "../../../lib/density";
import { publishToolbarLabels, readToolbarLabels } from "../../../lib/toolbar-labels";
import { publishZoom, readZoom } from "../../../lib/zoom";
import { Button } from "../../Button";
import { Dialog } from "../Dialog";
import { AppearanceSettingsTab } from "./AppearanceSettingsTab";
import { VaultSettingsTab } from "./VaultSettingsTab";

type SettingsDialogProps = {
  open: boolean;
  onClose: () => void;
  onSaved: (settings: AppSettings) => void;
  onCheckForUpdates: () => void;
  checkingUpdates: boolean;
};

type SettingsTab = "vault" | "appearance";

type SettingsSnapshot = {
  folder: string;
  zoom: number;
  density: number;
  theme: ThemeId;
  colorMode: ColorMode;
  toolbarLabels: boolean;
  runAtStartup: boolean;
};

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "vault", label: "Vault Settings" },
  { id: "appearance", label: "Appearance" },
];

function sameSettings(left: SettingsSnapshot, right: SettingsSnapshot) {
  return (
    left.folder === right.folder &&
    left.zoom === right.zoom &&
    left.density === right.density &&
    left.theme === right.theme &&
    left.colorMode === right.colorMode &&
    left.toolbarLabels === right.toolbarLabels &&
    left.runAtStartup === right.runAtStartup
  );
}

export function SettingsDialog({ open, onClose, onSaved, onCheckForUpdates, checkingUpdates }: SettingsDialogProps) {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [tab, setTab] = useState<SettingsTab>("vault");
  const [exePath, setExePath] = useState("");
  const [version, setVersion] = useState("");
  const [folder, setFolder] = useState("");
  const [zoom, setZoom] = useState(readZoom);
  const [density, setDensity] = useState(readDensity);
  const [theme, setTheme] = useState<ThemeId>(readAppearance().theme);
  const [colorMode, setColorMode] = useState<ColorMode>(readAppearance().colorMode);
  const [toolbarLabels, setToolbarLabels] = useState(readToolbarLabels);
  const [runAtStartup, setRunAtStartup] = useState(false);
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<SettingsSnapshot | null>(null);
  const opened = useRef({
    theme: readAppearance().theme,
    colorMode: readAppearance().colorMode,
    zoom: readZoom(),
    density: readDensity(),
    toolbarLabels: readToolbarLabels(),
  });

  useEffect(() => {
    if (!open) {
      return;
    }
    let cancelled = false;
    setLoading(true);
    setLoaded(null);
    setError(null);
    setTab("vault");
    const appearance = readAppearance();
    opened.current = {
      theme: appearance.theme,
      colorMode: appearance.colorMode,
      zoom: readZoom(),
      density: readDensity(),
      toolbarLabels: readToolbarLabels(),
    };
    void (async () => {
      try {
        const [settings, executable, currentVersion] = await Promise.all([
          appSettings(),
          appExecutable().catch(() => ""),
          appVersion().catch(() => ""),
        ]);
        if (cancelled) {
          return;
        }
        const snapshot: SettingsSnapshot = settings.shared
          ? {
              folder: settings.settingsFolder,
              zoom: settings.fontZoom,
              density: normalizeDensity(settings.density),
              theme: normalizeTheme(settings.theme),
              colorMode: normalizeColorMode(settings.colorMode),
              toolbarLabels: settings.toolbarLabels,
              runAtStartup: settings.runAtStartup,
            }
          : {
              folder: settings.settingsFolder,
              zoom: readZoom(),
              density: readDensity(),
              theme: appearance.theme,
              colorMode: appearance.colorMode,
              toolbarLabels: readToolbarLabels(),
              runAtStartup: settings.runAtStartup,
            };
        setFolder(snapshot.folder);
        setWarning(settings.warning);
        setExePath(executable);
        setVersion(currentVersion);
        setRunAtStartup(snapshot.runAtStartup);
        setTheme(snapshot.theme);
        setColorMode(snapshot.colorMode);
        setZoom(snapshot.zoom);
        setDensity(snapshot.density);
        setToolbarLabels(snapshot.toolbarLabels);
        setLoaded(snapshot);
        if (settings.shared) {
          publishAppearance(snapshot.theme, snapshot.colorMode);
          publishZoom(snapshot.zoom);
          publishDensity(snapshot.density);
          publishToolbarLabels(snapshot.toolbarLabels);
        }
      } catch (caught) {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  const restore = () => {
    publishAppearance(opened.current.theme, opened.current.colorMode);
    publishZoom(opened.current.zoom);
    publishDensity(opened.current.density);
    publishToolbarLabels(opened.current.toolbarLabels);
    applyAppearance(opened.current.theme, opened.current.colorMode);
  };

  const requestClose = () => {
    if (saving) {
      return;
    }
    restore();
    onClose();
  };

  const commitFolder = async (nextFolder: string) => {
    const trimmed = nextFolder.trim();
    if (!trimmed) {
      setWarning(null);
      return;
    }
    try {
      const preview = await previewAppSettings(trimmed);
      setWarning(preview.warning);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  };

  const browse = async () => {
    const picked = await openFolder({ directory: true, multiple: false, title: "Choose the settings folder" });
    const next = pickedFolderPath(picked);
    if (!next) {
      return;
    }
    setFolder(next);
    await commitFolder(next);
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const saved = await saveAppSettings({
        settingsFolder: folder.trim(),
        fontZoom: zoom,
        density: String(density),
        theme,
        colorMode,
        toolbarLabels,
        runAtStartup,
      });
      publishAppearance(saved.theme, saved.colorMode);
      publishZoom(saved.fontZoom);
      publishDensity(saved.density);
      publishToolbarLabels(saved.toolbarLabels);
      onSaved(saved);
      onClose();
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setSaving(false);
    }
  };

  const busy = loading || saving;
  const dirty =
    loaded !== null &&
    !sameSettings(loaded, { folder, zoom, density, theme, colorMode, toolbarLabels, runAtStartup });

  const tabs = (
    <div id="settings-tabs" role="tablist" aria-label="Settings sections" className="flex gap-1">
      {TABS.map((entry) => {
        const selected = tab === entry.id;
        return (
          <button
            id={`settings-tab-${entry.id}`}
            key={entry.id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={`settings-panel-${entry.id}`}
            disabled={busy}
            onClick={() => setTab(entry.id)}
            className={cn(
              "-mb-px border-b-2 px-3 py-3 text-sm disabled:opacity-50",
              selected
                ? "border-sky-500 text-zinc-100"
                : "border-transparent text-zinc-400 hover:text-zinc-200",
            )}
          >
            <span id={`settings-tab-${entry.id}-label`}>{entry.label}</span>
          </button>
        );
      })}
    </div>
  );

  return (
    <Dialog
      id="settings-dialog"
      ariaLabel="Settings"
      open={open}
      onClose={requestClose}
      panelClassName="max-w-3xl"
      header={tabs}
      headerActions={
        dirty ? (
          <>
            <Button id="app-settings-cancel" variant="ghost" onClick={requestClose} disabled={saving}>
              <span id="app-settings-cancel-label">Cancel</span>
            </Button>
            <Button id="app-settings-save" variant="primary" onClick={() => void save()} disabled={busy}>
              <span id="app-settings-save-label">{saving ? "Saving…" : "Save"}</span>
            </Button>
          </>
        ) : null
      }
    >
      {loading ? (
        <p id="settings-loading" className="text-sm text-zinc-400">
          Loading settings…
        </p>
      ) : (
        <>
          {tab === "vault" ? (
            <VaultSettingsTab
              busy={busy}
              exePath={exePath}
              folder={folder}
              runAtStartup={runAtStartup}
              warning={warning}
              onFolderChange={setFolder}
              onFolderCommit={() => void commitFolder(folder)}
              onRunAtStartupChange={setRunAtStartup}
              onBrowse={() => void browse()}
              onClearFolder={() => {
                setFolder("");
                setWarning(null);
              }}
              onRevealExe={() => void revealPath(exePath).catch((caught) => setError(messageOf(caught)))}
              onCheckForUpdates={onCheckForUpdates}
              checkingUpdates={checkingUpdates}
              onSubmit={() => void save()}
            />
          ) : (
            <AppearanceSettingsTab
              busy={busy}
              folder={folder}
              zoom={zoom}
              density={density}
              theme={theme}
              colorMode={colorMode}
              toolbarLabels={toolbarLabels}
              onZoomChange={(next) => setZoom(publishZoom(next))}
              onDensityChange={(next) => setDensity(publishDensity(next))}
              onThemeChange={(next) => {
                setTheme(next);
                publishAppearance(next, colorMode);
              }}
              onColorModeChange={(next) => {
                setColorMode(next);
                publishAppearance(theme, next);
              }}
              onToolbarLabelsChange={(next) => setToolbarLabels(publishToolbarLabels(next))}
              onSubmit={() => void save()}
            />
          )}

          {error ? (
            <p id="settings-error" className="mt-3 text-sm text-red-400">
              {error}
            </p>
          ) : null}

          {version ? (
            <p id="settings-version" className="mt-3 text-xs text-zinc-500">
              Version {version}
            </p>
          ) : null}
        </>
      )}
    </Dialog>
  );
}
