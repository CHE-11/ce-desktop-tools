import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import type { AppSettings } from "../../types/app";
import type { ExpansionState, Snippet } from "../../types/expansion";
import { AppSidebar } from "./components/AppSidebar";
import { CommandPalette, type CommandItem } from "./components/CommandPalette";
import { SettingsDialog } from "./components/Dialogs/Settings/SettingsDialog";
import { UpdateDialog } from "./components/Dialogs/UpdateDialog";
import { StatusBar } from "./components/StatusBar";
import { Toolbar } from "./components/Toolbar/Toolbar";
import { TextExpansion } from "./components/Tools/TextExpansion";
import { CommonMisspells } from "./components/Tools/CommonMisspells";
import { DateExpander } from "./components/Tools/DateExpander";
import { AsciiCodes } from "./components/Tools/AsciiCodes";
import { Clipboard } from "./components/Tools/Clipboard";
import { KeyboardShortcuts } from "./components/Tools/KeyboardShortcuts";
import { BatchRename } from "./components/Tools/BatchRename";
import { Pantherbar } from "./components/Tools/Pantherbar";
import { Launcher } from "./components/Tools/Launcher";
import { FolderRules } from "./components/Tools/FolderRules";
import { Processes } from "./components/Tools/Processes";
import { applyUserThemeCss, normalizeColorMode, normalizeTheme, publishAppearance } from "./lib/appearance";
import { publishDensity } from "./lib/density";
import { appSettings, themeCss } from "./lib/app-client";
import { useDevFeedback } from "./lib/dev-feedback";
import { expansionState, lockVault, saveExpansions, setExpansionEnabled, unlockVault, createVaultPin } from "./lib/expansion-client";
import { messageOf } from "./lib/message";
import { isToolId, TOOLS, type ToolId } from "./lib/tools";
import { publishToolbarLabels, readToolbarLabels } from "./lib/toolbar-labels";
import { useAppUpdate } from "./lib/use-app-update";
import { publishZoom } from "./lib/zoom";

function keepUnlockedSnippets(current: ExpansionState, next: ExpansionState): ExpansionState {
  if (!next.locked || next.snippets.length > 0) {
    return next;
  }
  const visible = current.snippets.filter((snippet) => !snippet.hidden);
  if (visible.length === 0) {
    return next;
  }
  return { ...next, snippets: visible };
}

const EMPTY_EXPANSION: ExpansionState = {
  settingsFolder: "",
  snippets: [],
  enabled: true,
  locked: true,
  needsPin: false,
  error: null,
  lockSeconds: null,
};

export function HomePage() {
  const updates = useAppUpdate();
  const [tool, setTool] = useState<ToolId>("expansion");
  const [expansion, setExpansion] = useState<ExpansionState>(EMPTY_EXPANSION);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [commandsOpen, setCommandsOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [snippetOpen, setSnippetOpen] = useState(false);
  const [labels, setLabels] = useState(readToolbarLabels);
  const [focusNonce, setFocusNonce] = useState(0);

  useEffect(() => {
    document.title = "ce-desktop-tools";
  }, []);

  const applySharedSettings = useCallback((next: AppSettings) => {
    if (!next.shared) {
      return;
    }
    publishAppearance(normalizeTheme(next.theme), normalizeColorMode(next.colorMode));
    publishDensity(next.density);
    publishZoom(next.fontZoom);
    publishToolbarLabels(next.toolbarLabels);
    setLabels(next.toolbarLabels);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [nextSettings, nextExpansion, css] = await Promise.all([
          appSettings(),
          expansionState(),
          themeCss().catch(() => ""),
        ]);
        if (cancelled) {
          return;
        }
        setExpansion(nextExpansion);
        applySharedSettings(nextSettings);
        applyUserThemeCss(css);
      } catch (caught) {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [applySharedSettings]);

  useEffect(() => {
    const subscriptions = [
      listen<ExpansionState>("expansion-changed", (event) => {
        setExpansion((current) => keepUnlockedSnippets(current, event.payload));
      }),
      listen<boolean>("expansion-enabled", (event) => {
        setExpansion((current) => ({ ...current, enabled: event.payload }));
      }),
      listen("expansion-unlock-request", () => {
        setTool("expansion");
        setFocusNonce((value) => value + 1);
      }),
      listen("pantherbar-open", () => {
        setTool("pantherbar");
      }),
      listen<string>("launcher-open-tool", (event) => {
        if (isToolId(event.payload)) {
          setTool(event.payload);
        }
      }),
    ];
    return () => {
      void Promise.all(subscriptions).then((unlisten) => {
        for (const stop of unlisten) {
          stop();
        }
      });
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          return;
        }
        void expansionState()
          .then((next) => setExpansion((current) => keepUnlockedSnippets(current, next)))
          .catch((caught) => setError(messageOf(caught)));
        void themeCss()
          .then((css) => applyUserThemeCss(css))
          .catch((caught) => setError(messageOf(caught)));
      })
      .then((stop) => {
        unlisten = stop;
      });
    return () => {
      unlisten?.();
    };
  }, []);

  const saveSnippets = useCallback(async (snippets: Snippet[]) => {
    setBusy(true);
    setError(null);
    try {
      setExpansion(await saveExpansions(snippets));
    } catch (caught) {
      setError(messageOf(caught));
      throw caught;
    } finally {
      setBusy(false);
    }
  }, []);

  const lock = useCallback(async () => {
    try {
      const next = await lockVault();
      setExpansion((current) => keepUnlockedSnippets(current, next));
      setFocusNonce((value) => value + 1);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }, []);

  const submitPin = useCallback(async (pin: string, create: boolean) => {
    setBusy(true);
    try {
      setExpansion(create ? await createVaultPin(pin) : await unlockVault(pin));
    } finally {
      setBusy(false);
    }
  }, []);

  const toggleExpansion = useCallback(async () => {
    try {
      const enabled = await setExpansionEnabled(!expansion.enabled);
      setExpansion((current) => ({ ...current, enabled }));
    } catch (caught) {
      setError(messageOf(caught));
    }
  }, [expansion.enabled]);

  const commands: CommandItem[] = [
    {
      id: "toggle-expansion",
      label: expansion.enabled ? "Pause expansion" : "Resume expansion",
      group: "Tools",
      keywords: "pause resume text",
      run: () => void toggleExpansion(),
    },
    {
      id: "open-settings",
      label: "Settings",
      group: "Settings",
      keywords: "theme zoom",
      run: () => setSettingsOpen(true),
    },
    {
      id: "check-updates",
      label: "Check for updates",
      group: "Settings",
      run: () => void updates.checkForUpdate(),
    },
    ...TOOLS.map(
      (entry): CommandItem => ({
        id: `open-${entry.id}`,
        label: `Open ${entry.label}`,
        group: "Tools",
        run: () => setTool(entry.id),
      }),
    ),
  ];

  useDevFeedback({
    tool,
    settingsFolder: expansion.settingsFolder,
    snippetCount: expansion.snippets.length,
    expansionEnabled: expansion.enabled,
    expansionLocked: expansion.locked,
    needsPin: expansion.needsPin,
    expansionError: expansion.error ?? error,
    dialogs: {
      settings: settingsOpen,
      snippet: snippetOpen,
      commands: commandsOpen,
      update: updates.open,
    },
  });

  return (
    <div id="home-page" className="flex h-full min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <Toolbar
        labels={labels}
        enabled={expansion.enabled}
        canLock={expansion.settingsFolder.length > 0 && !expansion.locked}
        onToggleExpansion={() => void toggleExpansion()}
        onLock={() => void lock()}
        onSettings={() => setSettingsOpen(true)}
      />
      {error ? (
        <p id="home-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="app-body" className="flex min-h-0 flex-1">
        <AppSidebar tool={tool} locked={expansion.locked} lockSeconds={expansion.lockSeconds} onSelect={setTool} />
        <main id="tool-panel" className="flex min-h-0 min-w-0 flex-1 flex-col">
          {tool === "expansion" ? (
            <TextExpansion
              settingsFolder={expansion.settingsFolder}
              snippets={expansion.snippets}
              enabled={expansion.enabled}
              error={expansion.error}
              busy={busy}
              locked={expansion.locked}
              needsPin={expansion.needsPin}
              focusNonce={focusNonce}
              onOpenSettings={() => setSettingsOpen(true)}
              onSave={saveSnippets}
              onCreatePin={(pin) => submitPin(pin, true)}
              onUnlock={(pin) => submitPin(pin, false)}
              onLock={lock}
              onEditorChange={setSnippetOpen}
            />
          ) : null}
          {tool === "misspell" ? <CommonMisspells onOpenSettings={() => setSettingsOpen(true)} /> : null}
          {tool === "date" ? <DateExpander onOpenSettings={() => setSettingsOpen(true)} /> : null}
          {tool === "ascii" ? <AsciiCodes onOpenSettings={() => setSettingsOpen(true)} /> : null}
          {tool === "clipboard" ? (
            <Clipboard
              settingsFolder={expansion.settingsFolder}
              locked={expansion.locked}
              needsPin={expansion.needsPin}
              busy={busy}
              focusNonce={focusNonce}
              onOpenSettings={() => setSettingsOpen(true)}
              onCreatePin={(pin) => submitPin(pin, true)}
              onUnlock={(pin) => submitPin(pin, false)}
              onLock={lock}
            />
          ) : null}
          {tool === "app-shortcuts" ? <KeyboardShortcuts onOpenSettings={() => setSettingsOpen(true)} /> : null}
          {tool === "rename" ? <BatchRename /> : null}
          {tool === "pantherbar" ? <Pantherbar /> : null}
          {tool === "launcher" ? <Launcher /> : null}
          {tool === "folder-rules" ? <FolderRules onOpenSettings={() => setSettingsOpen(true)} /> : null}
          {tool === "processes" ? <Processes /> : null}
        </main>
      </div>
      <StatusBar
        settingsFolder={expansion.settingsFolder}
        updateVersion={updates.availableVersion}
        onUpdate={updates.showPending}
      />
      <CommandPalette open={commandsOpen} commands={commands} onOpenChange={setCommandsOpen} />
      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onSaved={(next) => {
          setLabels(next.toolbarLabels);
          void expansionState().then(setExpansion).catch((caught) => setError(messageOf(caught)));
          void themeCss().then(applyUserThemeCss).catch(() => undefined);
        }}
        onCheckForUpdates={() => void updates.checkForUpdate()}
        checkingUpdates={updates.checking}
      />
      <UpdateDialog
        open={updates.open}
        mode={updates.mode}
        version={updates.version}
        notes={updates.notes}
        progress={updates.progress}
        error={updates.error}
        onClose={updates.dismiss}
        onInstall={() => void updates.install()}
      />
    </div>
  );
}
