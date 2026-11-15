import { useEffect, useState } from "react";
import { Minus, Plus } from "lucide-react";
import { appVersion } from "../lib/app-client";
import { baseName } from "../lib/message";
import { useFontZoom } from "../lib/zoom";

type StatusBarProps = {
  settingsFolder: string;
  snippetCount: number;
  enabled: boolean;
  locked: boolean;
  updateVersion: string | null;
  onUpdate: () => void;
};

function Divider({ id }: { id: string }) {
  return <span id={id} className="h-3 w-px shrink-0 bg-zinc-700" aria-hidden="true" />;
}

export function StatusBar({ settingsFolder, snippetCount, enabled, locked, updateVersion, onUpdate }: StatusBarProps) {
  const zoom = useFontZoom();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    appVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  const folderLabel = settingsFolder ? baseName(settingsFolder) : "No settings folder";
  const snippetLabel = snippetCount === 1 ? "1 snippet" : `${snippetCount} snippets`;

  return (
    <footer
      id="status-bar"
      className="flex h-6 shrink-0 items-center gap-2 border-t border-zinc-800 bg-zinc-900 px-2 text-xs text-zinc-400 select-none"
    >
      <div id="status-bar-stats" className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
        {updateVersion ? (
          <>
            <button
              id="status-update"
              type="button"
              title={version ? `ce-desktop-tools ${version}. Update to v${updateVersion}` : `Update to v${updateVersion}`}
              onClick={onUpdate}
              className="shrink-0 rounded px-1 text-sky-300 hover:bg-zinc-800 hover:text-sky-200"
            >
              <span id="status-update-label">Update to v{updateVersion}</span>
            </button>
            <Divider id="status-sep-version" />
          </>
        ) : version ? (
          <>
            <span id="status-version" className="shrink-0 text-zinc-500 tabular-nums" title={`ce-desktop-tools ${version}`}>
              v{version}
            </span>
            <Divider id="status-sep-version" />
          </>
        ) : null}
        <span id="status-folder" className="min-w-0 truncate text-zinc-300" title={settingsFolder || "No settings folder"}>
          {folderLabel}
        </span>
        <Divider id="status-sep-snippets" />
        <span id="status-snippets" className="shrink-0 text-zinc-400">
          {snippetLabel}
        </span>
        <Divider id="status-sep-expansion" />
        <span id="status-expansion" className={locked ? "shrink-0 text-amber-300" : enabled ? "shrink-0 text-emerald-400" : "shrink-0 text-amber-300"}>
          {locked ? "Vault locked" : enabled ? "Expansion on" : "Expansion paused"}
        </span>
      </div>

      <div id="status-zoom" className="flex shrink-0 items-center">
        <button
          id="status-zoom-out"
          type="button"
          title="Smaller font (Ctrl+-)"
          aria-label="Smaller font"
          disabled={zoom.atMin}
          onClick={zoom.decrease}
          className="inline-flex size-5 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Minus id="status-zoom-out-icon" className="size-3" />
        </button>
        <button
          id="status-zoom-reset"
          type="button"
          title="Reset font size (Ctrl+0)"
          aria-label={`Font size ${zoom.zoom} percent. Reset to 100 percent.`}
          onClick={zoom.reset}
          className="inline-flex h-5 min-w-11 items-center justify-center rounded px-1 text-zinc-300 tabular-nums hover:bg-zinc-800 hover:text-zinc-100"
        >
          <span id="status-zoom-label">{zoom.zoom}%</span>
        </button>
        <button
          id="status-zoom-in"
          type="button"
          title="Larger font (Ctrl++)"
          aria-label="Larger font"
          disabled={zoom.atMax}
          onClick={zoom.increase}
          className="inline-flex size-5 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Plus id="status-zoom-in-icon" className="size-3" />
        </button>
      </div>
    </footer>
  );
}
