import { getCurrentWindow } from "@tauri-apps/api/window";
import { Lock, Maximize, Minimize, Pause, Pin, Play, Settings, Unlock, X } from "lucide-react";
import type { MouseEvent } from "react";
import { launcherShow } from "../../lib/launcher-client";
import { Button } from "../Button";
import { Kbd } from "../Kbd";
import { StylesMenu } from "./StylesMenu";

type ToolbarProps = {
  labels: boolean;
  enabled: boolean;
  pinned: boolean;
  canLock: boolean;
  onToggleExpansion: () => void;
  onUnlock: () => void;
  onTogglePin: () => void;
  onLock: () => void;
  onSettings: () => void;
  onMaximize: () => void;
  onClose: () => void;
  maximized: boolean;
};

function onToolbarMouseDown(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) {
    return;
  }
  const target = event.target;
  if (!(target instanceof Element) || target.closest("button, a, input, textarea, select, [role='menu']")) {
    return;
  }
  event.preventDefault();
  void getCurrentWindow().startDragging();
}

export function Toolbar({ labels, enabled, pinned, canLock, onToggleExpansion, onUnlock, onTogglePin, onLock, onSettings, onMaximize, onClose, maximized }: ToolbarProps) {
  return (
    <header
      id="app-toolbar"
      className="flex h-12 shrink-0 cursor-default items-center justify-between gap-2 border-b border-zinc-800 bg-zinc-900 px-3 select-none"
      onMouseDown={onToolbarMouseDown}
    >

      <Button
        id="toolbar-launcher"
        variant="ghost"
        className="flex flex-col justify-center items-center gap-0.5 min-h-fit py-0.5"
        title="Open the launcher"
        onClick={() => void launcherShow()}
      >
        <div className="flex items-center gap-1">
          <Kbd id="toolbar-launcher-win" aria-label="Windows" className="size-5 min-w-[25px]">
            <svg id="toolbar-launcher-win-icon" viewBox="0 0 16 16" className="size-3" aria-hidden="true">
              <path fill="currentColor" d="M0 2.4 6.6 1.5v5.8H0zm7.3-1L16 0v7.3H7.3zM0 8.6h6.6v5.8L0 13.5zm7.3 0H16V16l-8.7-1.2z" />
            </svg>
          </Kbd>
          <Kbd id="toolbar-launcher-space" aria-label="Space" className="size-5 min-w-[40px]">Space</Kbd>
        </div>
        <p id="toolbar-launcher-label" className="text-[0.6rem] font-medium text-zinc-100 opacity-40">
          Open Launcher
        </p>
      </Button>

      <div className="flex items-center gap-2">
        <Button
          id="toolbar-expansion"
          variant={enabled ? "secondary" : "outline"}
          size="sm"
          title={enabled ? "Pause expansion" : "Resume expansion"}
          aria-pressed={enabled}
          onClick={onToggleExpansion}
        >
          {enabled ? (
            <Pause id="toolbar-expansion-icon" className="size-4" aria-hidden="true" />
          ) : (
            <Play id="toolbar-expansion-icon" className="size-4" aria-hidden="true" />
          )}
          {labels ? <span id="toolbar-expansion-label">{enabled ? "Pause" : "Resume"}</span> : null}
        </Button>

        {canLock ? (
          <Button id="toolbar-lock" variant="ghost" size="sm" title="Unlock the vault" onClick={onLock}>  
            <Unlock id="toolbar-unlock-icon" className="size-4 text-emerald-400" aria-hidden="true" />
            {labels ? <span id="toolbar-unlock-label" className="text-emerald-400">Lock Vault</span> : null}
          </Button>
        ) : (
          <Button id="toolbar-lock" variant="ghost" size="sm" title="Lock the vault" onClick={onUnlock}>
            <Lock id="toolbar-lock-icon" className="size-4 text-red-400" aria-hidden="true" />
            {labels ? <span id="toolbar-lock-label" className="text-zinc-100">Unlock Vault</span> : null}
          </Button>
        )}
    
        <StylesMenu labels={labels} />

        <Button id="toolbar-settings" variant="ghost" size="sm" title="Settings" onClick={onSettings}>
          <Settings id="toolbar-settings-icon" className="size-4" aria-hidden="true" />
          {labels ? <span id="toolbar-settings-label">Settings</span> : null}
        </Button>

      </div>

      <div className="flex items-center gap-2">
        <Button
          id="toolbar-pin"
          variant={pinned ? "outline" : "ghost"}
          size="sm"
          className={pinned ? "border-violet-500 bg-violet-950 text-violet-100 hover:bg-violet-900" : undefined}
          title={pinned ? "Stop keeping this window on top" : "Keep this window on top"}
          aria-pressed={pinned}
          onClick={onTogglePin}
        >
          <Pin id="toolbar-pin-icon" className={pinned ? "size-4 text-violet-300" : "size-4"} aria-hidden="true" />
        </Button>

        <Button id="toolbar-maximize" variant="ghost" size="sm" title={maximized ? "Unmaximize" : "Maximize"} onClick={onMaximize}>
          {maximized ? (
            <Minimize id="toolbar-maximize-icon-unmaximized" className="size-4" aria-hidden="true" />
          ) : (
            <Maximize id="toolbar-maximize-icon-maximized" className="size-4" aria-hidden="true" />
          )}
        </Button>

        <Button id="toolbar-close" variant="ghost" size="sm" title="Close" onClick={onClose}>
          <X id="toolbar-close-icon" className="size-4" aria-hidden="true" />
        </Button>
      </div>
    </header>
  );
}
