import {
  Activity,
  Binary,
  CalendarClock,
  Calculator,
  Cat,
  Clipboard,
  Eraser,
  FileCog,
  FilePen,
  Files,
  FolderSync,
  Globe,
  Keyboard,
  MonitorDown,
  Lock,
  Pin,
  Pipette,
  RulerDimensionLine,
  Search,
  SpellCheck,
  SquareTerminal,
  Type,
  Unlock,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "../lib/cn";
import { TOOLS, type ToolId } from "../lib/tools";
import { ColorPickerAction } from "./Tools/Actions/ColorPicker.action";
import { RestartExplorer } from "./Tools/Actions/RestartExplorer.action";
import { RunMegaClip } from "./Tools/Actions/RunMegaClip.action";
import { ScreenRulerAction } from "./Tools/Actions/ScreenRuler.action";
import { StayAwake } from "./Tools/Actions/StayAwake.action";

const TOOL_ICONS: Record<ToolId, LucideIcon> = {
  expansion: Type,
  clipboard: Clipboard,
  "clip-cleaner": Eraser,
  misspell: SpellCheck,
  date: CalendarClock,
  ascii: Binary,
  calculator: Calculator,
  "app-shortcuts": Keyboard,
  rename: FilePen,
  pantherbar: Cat,
  launcher: Search,
  "folder-rules": FolderSync,
  processes: Activity,
  "network-explorer": Globe,
  "file-tools": Files,
  "add-to-desktop": MonitorDown,
  "file-associations": FileCog,
  terminal: SquareTerminal,
  "always-on-top": Pin,
  "screen-ruler": RulerDimensionLine,
  "color-picker": Pipette,
};

const MENU_CHILDREN = {
  expansion: ["misspell", "date", "ascii"],
} as const satisfies Partial<Record<ToolId, readonly ToolId[]>>;

const CHILD_IDS = new Set<ToolId>(Object.values(MENU_CHILDREN).flat());

function childrenOf(parent: ToolId) {
  return MENU_CHILDREN[parent as keyof typeof MENU_CHILDREN] ?? [];
}

function groupOpen(parent: ToolId, current: ToolId) {
  const children = childrenOf(parent);
  return children.length > 0 && (current === parent || (children as readonly ToolId[]).includes(current));
}

const SIDEBAR_WIDTH_KEY = "ce-desktop-tools.sidebar-width";
const DEFAULT_WIDTH = 224;
const MIN_WIDTH = 160;
const MAX_WIDTH = 480;
const KEYBOARD_STEP = 16;

const TOP_LEVEL_TOOLS = TOOLS.filter((entry) => !CHILD_IDS.has(entry.id));
const EXPANSION_TOOL = TOP_LEVEL_TOOLS.find((entry) => entry.id === "expansion")!;
const OTHER_TOOLS = TOP_LEVEL_TOOLS.filter((entry) => entry.id !== "expansion").sort((a, b) =>
  a.label.localeCompare(b.label, undefined, { sensitivity: "base" }),
);

type AppSidebarProps = {
  tool: ToolId;
  locked: boolean;
  lockSeconds: number | null;
  onSelect: (id: ToolId) => void;
};

type MenuEntryProps = {
  entry: (typeof TOOLS)[number];
  tool: ToolId;
  locked: boolean;
  lockMinutes: number | null;
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

function MenuEntry({ entry, tool, locked, lockMinutes, onSelect }: MenuEntryProps) {
  const selected = tool === entry.id;
  const Icon = TOOL_ICONS[entry.id];
  const open = groupOpen(entry.id, tool);
  const children = open ? TOOLS.filter((child) => (childrenOf(entry.id) as readonly ToolId[]).includes(child.id)) : [];

  return (
    <div id={`app-menu-${entry.id}-group`} className="flex flex-col gap-0.5">
      <button
        id={`app-menu-${entry.id}`}
        type="button"
        aria-current={selected ? "page" : undefined}
        aria-expanded={childrenOf(entry.id).length > 0 ? open : undefined}
        onClick={() => onSelect(entry.id)}
        className={cn(
          "flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-xs",
          selected
            ? "bg-zinc-800 font-medium text-zinc-100"
            : open
              ? "text-zinc-100 hover:bg-zinc-800"
              : "text-zinc-300 hover:bg-zinc-800",
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
      {children.length > 0 ? (
        <div
          id={`app-menu-${entry.id}-children`}
          role="group"
          aria-label={`${entry.label} tools`}
          className="ml-4 flex flex-col gap-0.5 border-l border-zinc-700 pl-1.5"
        >
          {children.map((child) => {
            const childSelected = tool === child.id;
            const ChildIcon = TOOL_ICONS[child.id];
            return (
              <button
                id={`app-menu-${child.id}`}
                key={child.id}
                type="button"
                aria-current={childSelected ? "page" : undefined}
                onClick={() => onSelect(child.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2.5 py-1 text-left text-xs",
                  childSelected
                    ? "bg-zinc-800 font-medium text-zinc-100"
                    : "text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <ChildIcon
                  id={`app-menu-${child.id}-icon`}
                  className="size-4 shrink-0"
                  aria-hidden="true"
                />
                <span id={`app-menu-${child.id}-label`} className="min-w-0 flex-1 truncate">
                  {child.label}
                </span>
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
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
          <MenuEntry
            entry={EXPANSION_TOOL}
            tool={tool}
            locked={locked}
            lockMinutes={lockMinutes}
            onSelect={onSelect}
          />
          <div id="app-menu-divider" role="separator" className="my-1.5 border-t border-zinc-800" />
          {OTHER_TOOLS.map((entry) => (
            <MenuEntry
              key={entry.id}
              entry={entry}
              tool={tool}
              locked={locked}
              lockMinutes={lockMinutes}
              onSelect={onSelect}
            />
          ))}
        </nav>

        <div id="app-actions" aria-label="Actions" className="flex shrink-0 flex-col gap-2 border-t border-zinc-800 px-2 py-2">
          <StayAwake />
          <ColorPickerAction />
          <ScreenRulerAction />
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
