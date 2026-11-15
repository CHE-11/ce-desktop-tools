import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import type { AppSettings } from "../../../types/app";
import type { ExpansionState, Snippet } from "../../../types/expansion";
import { CommandPalette, type CommandItem } from "../components/CommandPalette";
import { SettingsDialog } from "../components/Dialogs/Settings/SettingsDialog";
import { UpdateDialog } from "../components/Dialogs/UpdateDialog";
import { StatusBar } from "../components/StatusBar";
import { Toolbar } from "../components/Toolbar";
import { TextExpansion } from "../components/Tools/TextExpansion";
import { applyUserThemeCss, normalizeColorMode, normalizeTheme, publishAppearance } from "../lib/appearance";
import { appSettings, themeCss } from "../lib/app-client";
import { cn } from "../lib/cn";
import { useDevFeedback } from "../lib/dev-feedback";
import { expansionState, lockVault, saveExpansions, setExpansionEnabled, unlockVault, createVaultPin } from "../lib/expansion-client";
import { messageOf } from "../lib/message";
import { TOOLS, type ToolId } from "../lib/tools";
import { publishToolbarLabels, readToolbarLabels } from "../lib/toolbar-labels";
import { useAppUpdate } from "../lib/use-app-update";
import { publishZoom } from "../lib/zoom";

const EMPTY_EXPANSION: ExpansionState = {
  settingsFolder: "",
  snippets: [],
  enabled: true,
  locked: true,
  needsPin: false,
  error: null,
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
        setExpansion(event.payload);
      }),
      listen<boolean>("expansion-enabled", (event) => {
        setExpansion((current) => ({ ...current, enabled: event.payload }));
      }),
      listen("expansion-unlock-request", () => {
        setTool("expansion");
        setFocusNonce((value) => value + 1);
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
      setExpansion(await lockVault());
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
        <aside id="app-sidebar" className="flex w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900">
          <p id="app-sidebar-label" className="px-3 pt-3 pb-1 text-[11px] font-semibold tracking-wide text-zinc-500 uppercase">
            Menu
          </p>
          <nav id="app-menu" aria-label="Tools" className="flex flex-col gap-0.5 px-2 pb-2">
            {TOOLS.map((entry) => {
              const selected = tool === entry.id;
              return (
                <button
                  id={`app-menu-${entry.id}`}
                  key={entry.id}
                  type="button"
                  aria-current={selected ? "page" : undefined}
                  onClick={() => setTool(entry.id)}
                  className={cn(
                    "rounded-md px-2.5 py-2 text-left text-sm",
                    selected ? "bg-zinc-800 font-medium text-zinc-100" : "text-zinc-300 hover:bg-zinc-800",
                  )}
                >
                  <span id={`app-menu-${entry.id}-label`}>{entry.label}</span>
                </button>
              );
            })}
          </nav>
        </aside>
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
              onEditorChange={setSnippetOpen}
            />
          ) : null}
        </main>
      </div>
      <StatusBar
        settingsFolder={expansion.settingsFolder}
        snippetCount={expansion.snippets.length}
        enabled={expansion.enabled}
        locked={expansion.locked && expansion.settingsFolder.length > 0}
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
