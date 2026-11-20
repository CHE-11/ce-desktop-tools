import { Lock, Pause, Pin, Play, Settings } from "lucide-react";
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
  onTogglePin: () => void;
  onLock: () => void;
  onSettings: () => void;
};

export function Toolbar({ labels, enabled, pinned, canLock, onToggleExpansion, onTogglePin, onLock, onSettings }: ToolbarProps) {
  return (
    <header id="app-toolbar" className="flex h-12 shrink-0 items-center gap-2 border-b border-zinc-800 bg-zinc-900 px-3">
      <div id="app-toolbar-title" className="mr-auto min-w-0">
        <p id="app-toolbar-name" className="truncate text-sm font-semibold text-zinc-100">
          ce-desktop-tools
        </p>
      </div>
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
        <Button id="toolbar-lock" variant="ghost" size="sm" title="Lock the vault" onClick={onLock}>
          <Lock id="toolbar-lock-icon" className="size-4" aria-hidden="true" />
          {labels ? <span id="toolbar-lock-label">Lock</span> : null}
        </Button>
      ) : null}
      <StylesMenu labels={labels} />
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
        {labels ? <span id="toolbar-pin-label">On top</span> : null}
      </Button>
      <Button id="toolbar-settings" variant="ghost" size="sm" title="Settings" onClick={onSettings}>
        <Settings id="toolbar-settings-icon" className="size-4" aria-hidden="true" />
        {labels ? <span id="toolbar-settings-label">Settings</span> : null}
      </Button>
    </header>
  );
}
