import { Lock, Pause, Pin, Play, Settings } from "lucide-react";
import { Button } from "../Button";
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
