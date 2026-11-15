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
import { messageOf, pickedFolderPath } from "../../../lib/message";
import { publishDensity, readDensity } from "../../../lib/density";
import { publishToolbarLabels, readToolbarLabels } from "../../../lib/toolbar-labels";
import { publishZoom, readZoom } from "../../../lib/zoom";
import { Button } from "../../Button";
import { ChangePin } from "../../Tools/ChangePin";
import { Dialog } from "../Dialog";
import { AppSettingsTab } from "./AppSettingsTab";

type SettingsDialogProps = {
  open: boolean;
  onClose: () => void;
  onSaved: (settings: AppSettings) => void;
  onCheckForUpdates: () => void;
  checkingUpdates: boolean;
};

type SettingsSnapshot = {
  folder: string;
  zoom: number;
  theme: ThemeId;
  colorMode: ColorMode;
  toolbarLabels: boolean;
  runAtStartup: boolean;
};

function sameSettings(left: SettingsSnapshot, right: SettingsSnapshot) {
  return (
    left.folder === right.folder &&
    left.zoom === right.zoom &&
    left.theme === right.theme &&
    left.colorMode === right.colorMode &&
    left.toolbarLabels === right.toolbarLabels &&
    left.runAtStartup === right.runAtStartup
  );
}

export function SettingsDialog({ open, onClose, onSaved, onCheckForUpdates, checkingUpdates }: SettingsDialogProps) {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [exePath, setExePath] = useState("");
  const [version, setVersion] = useState("");
  const [folder, setFolder] = useState("");
  const [zoom, setZoom] = useState(readZoom);
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
              theme: normalizeTheme(settings.theme),
              colorMode: normalizeColorMode(settings.colorMode),
              toolbarLabels: settings.toolbarLabels,
              runAtStartup: settings.runAtStartup,
            }
          : {
              folder: settings.settingsFolder,
              zoom: readZoom(),
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
        setToolbarLabels(snapshot.toolbarLabels);
        setLoaded(snapshot);
        if (settings.shared) {
          publishAppearance(snapshot.theme, snapshot.colorMode);
          publishZoom(snapshot.zoom);
          publishDensity(settings.density);
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
    !sameSettings(loaded, { folder, zoom, theme, colorMode, toolbarLabels, runAtStartup });

  return (
    <Dialog
      id="settings-dialog"
      title="Settings"
      open={open}
      onClose={requestClose}
      panelClassName="max-w-3xl"
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
        <AppSettingsTab
          busy={busy}
          exePath={exePath}
          folder={folder}
          zoom={zoom}
          theme={theme}
          colorMode={colorMode}
          toolbarLabels={toolbarLabels}
          runAtStartup={runAtStartup}
          warning={warning}
          error={error}
          onFolderChange={setFolder}
          onFolderCommit={() => void commitFolder(folder)}
          onZoomChange={(next) => setZoom(publishZoom(next))}
          onThemeChange={(next) => {
            setTheme(next);
            publishAppearance(next, colorMode);
          }}
          onColorModeChange={(next) => {
            setColorMode(next);
            publishAppearance(theme, next);
          }}
          onToolbarLabelsChange={(next) => setToolbarLabels(publishToolbarLabels(next))}
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
        <div id="settings-change-pin" className="mt-3">
          <ChangePin busy={busy} />
        </div>
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
