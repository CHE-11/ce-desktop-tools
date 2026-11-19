import {
  Activity,
  Binary,
  CalendarClock,
  Ruler,
  Cat,
  Clipboard,
  FilePen,
  Files,
  FolderSync,
  Keyboard,
  Lock,
  Search,
  SpellCheck,
  Type,
  Unlock,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "../lib/cn";
import { TOOLS, type ToolId } from "../lib/tools";
import { RestartExplorer } from "./Actions/RestartExplorer.action";
import { RunMegaClip } from "./Actions/RunMegaClip.action";

const TOOL_ICONS: Record<ToolId, LucideIcon> = {
  expansion: Type,
  clipboard: Clipboard,
  misspell: SpellCheck,
  date: CalendarClock,
  ascii: Binary,
  units: Ruler,
  "app-shortcuts": Keyboard,
  rename: FilePen,
  pantherbar: Cat,
  launcher: Search,
  "folder-rules": FolderSync,
  processes: Activity,
  "file-tools": Files,
};

const SIDEBAR_WIDTH_KEY = "ce-desktop-tools.sidebar-width";
const DEFAULT_WIDTH = 224;
const MIN_WIDTH = 160;
const MAX_WIDTH = 480;
const KEYBOARD_STEP = 16;

type AppSidebarProps = {
  tool: ToolId;
  locked: boolean;
  lockSeconds: number | null;
  onSelect: (id: ToolId) => void;
};

function useLockMinutes(locked: boolean, lockSeconds: number | null) {
  const [minutes, setMinutes] = useState<number | null>(null);

  useEffect(() => {
    if (locked || lockSeconds == null) {
      setMinutes(null);
      return;
    }
    const endsAt = Date.now() + lockSeconds * 1000;
    const tick = () => {
      const left = endsAt - Date.now();
      setMinutes(left <= 0 ? 1 : Math.ceil(left / 60_000));
    };
    tick();
    const id = window.setInterval(tick, 1000);
    return () => window.clearInterval(id);
  }, [locked, lockSeconds]);

  return minutes;
}

function clampWidth(value: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(value)));
}

function readSidebarWidth() {
  try {
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    if (!raw) {
      return DEFAULT_WIDTH;
    }
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? clampWidth(parsed) : DEFAULT_WIDTH;
  } catch {
    return DEFAULT_WIDTH;
  }
}

function writeSidebarWidth(width: number) {
  try {
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(width));
  } catch {
    // The resized width still applies for this session.
  }
}

export function AppSidebar({ tool, locked, lockSeconds, onSelect }: AppSidebarProps) {
  const lockMinutes = useLockMinutes(locked, lockSeconds);
  const [width, setWidth] = useState(readSidebarWidth);
  const widthRef = useRef(width);
  const drag = useRef<{ pointerId: number; startX: number; startWidth: number } | null>(null);

  const applyWidth = useCallback((next: number) => {
    const clamped = clampWidth(next);
    widthRef.current = clamped;
    setWidth(clamped);
    return clamped;
  }, []);

  const onPointerDown = useCallback((event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startWidth: widthRef.current,
    };
  }, []);

  const onPointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const active = drag.current;
      if (!active || active.pointerId !== event.pointerId) {
        return;
      }
      applyWidth(active.startWidth + event.clientX - active.startX);
    },
    [applyWidth],
  );

  const endDrag = useCallback((event: PointerEvent<HTMLDivElement>) => {
    const active = drag.current;
    if (!active || active.pointerId !== event.pointerId) {
      return;
    }
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    writeSidebarWidth(widthRef.current);
  }, []);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const next =
        event.key === "ArrowLeft"
          ? widthRef.current - KEYBOARD_STEP
          : event.key === "ArrowRight"
            ? widthRef.current + KEYBOARD_STEP
            : event.key === "Home"
              ? MIN_WIDTH
              : event.key === "End"
                ? MAX_WIDTH
                : null;
      if (next === null) {
        return;
      }
      event.preventDefault();
      writeSidebarWidth(applyWidth(next));
    },
    [applyWidth],
  );

  const onDoubleClick = useCallback(() => {
    writeSidebarWidth(applyWidth(DEFAULT_WIDTH));
  }, [applyWidth]);

  return (
    <div id="app-sidebar-shell" style={{ width }} className="relative z-10 flex shrink-0">
      <aside id="app-sidebar" className="flex min-h-0 min-w-0 flex-1 flex-col justify-between border-r border-zinc-800 bg-zinc-900">
        <nav id="app-menu" aria-label="Pages" className="flex min-h-0 flex-col gap-0.5 overflow-y-auto px-2 pt-2">
          {TOOLS.map((entry) => {
            const selected = tool === entry.id;
            const Icon = TOOL_ICONS[entry.id];
            return (
              <button
                id={`app-menu-${entry.id}`}
                key={entry.id}
                type="button"
                aria-current={selected ? "page" : undefined}
                onClick={() => onSelect(entry.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm",
                  selected ? "bg-zinc-800 font-medium text-zinc-100" : "text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <Icon id={`app-menu-${entry.id}-icon`} className="size-4 shrink-0" aria-hidden="true" />
                <span id={`app-menu-${entry.id}-label`} className="min-w-0 flex-1 truncate">
                  {entry.label}
                </span>
                {entry.lockable ? (
                  locked ? (
                    <Lock
                      id={`app-menu-${entry.id}-lock`}
                      className="size-3.5 shrink-0 text-amber-300"
                      aria-label="Locked"
                    />
                  ) : (
                    <>
                      {lockMinutes != null ? (
                        <span
                          id={`app-menu-${entry.id}-lock-minutes`}
                          className="shrink-0 text-xs tabular-nums text-zinc-400"
                          title={`${lockMinutes} minutes until the vault locks`}
                        >
                          {lockMinutes} min
                        </span>
                      ) : null}
                      <Unlock
                        id={`app-menu-${entry.id}-lock`}
                        className="size-3.5 shrink-0 text-zinc-500"
                        aria-label="Unlocked"
                      />
                    </>
                  )
                ) : null}
              </button>
            );
          })}
        </nav>

        <div id="app-actions" aria-label="Actions" className="flex shrink-0 flex-col gap-2 border-t border-zinc-800 px-2 py-2">
          <RunMegaClip />
          <RestartExplorer />
        </div>
      </aside>
      <div
        id="app-sidebar-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize sidebar"
        aria-valuemin={MIN_WIDTH}
        aria-valuemax={MAX_WIDTH}
        aria-valuenow={width}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onKeyDown={onKeyDown}
        onDoubleClick={onDoubleClick}
        className="group absolute inset-y-0 -right-1.5 z-10 w-3 cursor-col-resize touch-none focus-visible:outline focus-visible:outline-sky-500"
      >
        <span
          id="app-sidebar-resize-grip"
          className="pointer-events-none absolute inset-y-0 left-1.5 w-px bg-transparent group-hover:bg-sky-500 group-focus-visible:bg-sky-500"
        />
      </div>
    </div>
  );
}
