import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Camera, CameraOff, ChevronDown, ChevronUp, Eraser, Highlighter, Mic, MicOff, Pause, Pencil, Play, Settings, Square, Trash2, Undo2 } from "lucide-react";
import type { RecordCameraCorner, RecordCursor, RecordDrawTool, RecordStatus } from "../../../../../types/record";
import { cn } from "../../../lib/cn";
import {
  onRecordStatus,
  recordClearStrokes,
  recordPause,
  recordResume,
  recordSave,
  recordSetCamera,
  recordSetCameraCorner,
  recordSetControlsMode,
  recordSetCursor,
  recordSetDrawing,
  recordSetMicMuted,
  recordSetStyle,
  recordState,
  recordStatus,
  recordStop,
  recordUndoStroke,
} from "../../../lib/record-client";
import { rememberDrawWidth, readDrawWidth } from "./record-draw-style";
import { clock } from "./time";

const COLORS = ["#ef4444", "#22c55e", "#f97316", "#171717", "#facc15", "#f5f5f5", "#a1a1aa"];
const TOOLS: { id: RecordDrawTool; label: string; icon: typeof Pencil }[] = [
  { id: "pen", label: "Pen", icon: Pencil },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight },
  { id: "rectangle", label: "Shape", icon: Square },
  { id: "highlighter", label: "Highlighter", icon: Highlighter },
  { id: "eraser", label: "Eraser", icon: Eraser },
];
const CORNERS: { id: RecordCameraCorner; label: string }[] = [
  { id: "top-left", label: "Top left" },
  { id: "top-right", label: "Top right" },
  { id: "bottom-left", label: "Bottom left" },
  { id: "bottom-right", label: "Bottom right" },
];
const CURSORS: { id: RecordCursor; label: string }[] = [
  { id: "system", label: "System" },
  { id: "hidden", label: "Hidden" },
  { id: "circle", label: "Circle" },
];
const EMPTY: RecordStatus = {
  phase: "countdown",
  elapsedMs: 0,
  countdown: 3,
  id: "",
  error: null,
  path: null,
  camera: false,
  microphone: true,
  micMuted: false,
  micLevel: 0,
  drawing: false,
  strokes: 0,
  tool: "pen",
  color: "#ef4444",
  sourceLabel: "",
};
const METER = [0.12, 0.28, 0.46, 0.66, 0.86];

type Prefs = { cameraCorner: RecordCameraCorner; cursor: RecordCursor };

export function RecordControls() {
  const stamp = useRef({ elapsed: 0, at: performance.now() });
  const [status, setStatus] = useState<RecordStatus>(EMPTY);
  const [tick, setTick] = useState(0);
  const [hidden, setHidden] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [prefs, setPrefs] = useState<Prefs>({ cameraCorner: "bottom-left", cursor: "system" });
  const [drawWidth, setDrawWidth] = useState(readDrawWidth);

  const apply = (next: RecordStatus) => {
    stamp.current = { elapsed: next.elapsedMs, at: performance.now() };
    setStatus(next);
  };

  useEffect(() => {
    document.title = "Recording";
    let stop = false;
    const pull = () => {
      void recordStatus()
        .then((next) => {
          if (!stop) apply(next);
        })
        .catch(() => undefined);
    };
    pull();
    const poll = window.setInterval(pull, 200);
    const clockTick = window.setInterval(() => setTick(performance.now()), 200);
    const listening = onRecordStatus((next) => {
      if (!stop) apply(next);
    });
    void recordState()
      .then((state) => setPrefs({ cameraCorner: state.settings.cameraCorner, cursor: state.settings.cursor }))
      .catch(() => undefined);
    return () => {
      stop = true;
      window.clearInterval(poll);
      window.clearInterval(clockTick);
      void listening.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const mode = hidden ? "hidden" : settingsOpen ? "settings" : status.drawing ? "tools" : "pill";
    void recordSetControlsMode(mode).catch(() => undefined);
  }, [hidden, settingsOpen, status.drawing]);

  const counting = status.phase === "countdown";
  const paused = status.phase === "paused";
  const running = status.phase === "recording";
  const shownMs = running ? stamp.current.elapsed + Math.max(0, tick - stamp.current.at) : status.elapsedMs;
  const tools = status.drawing && !hidden && !settingsOpen;

  const savePrefs = (next: Prefs) => {
    if (next.cameraCorner !== prefs.cameraCorner) void recordSetCameraCorner(next.cameraCorner).catch(() => undefined);
    if (next.cursor !== prefs.cursor) void recordSetCursor(next.cursor).catch(() => undefined);
    setPrefs(next);
    void recordState()
      .then((state) => recordSave({ ...state.settings, ...next }))
      .catch(() => undefined);
  };

  return (
    <div id="record-controls" className="flex h-full items-end justify-center overflow-visible bg-transparent p-1">
      <div id="record-controls-bar" className="relative flex w-max flex-col items-end justify-end">
        {settingsOpen && !hidden ? (
          <div id="record-controls-settings" className="absolute right-0 bottom-full z-10 mb-1 w-max rounded-lg bg-zinc-950/95 px-1 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
            <p id="record-controls-corner-label" className="px-0.5 pb-0.5 text-[9px] leading-none tracking-wide text-zinc-500 uppercase">
              Camera
            </p>
            <div id="record-controls-corners" className="grid w-max grid-cols-[max-content_max-content] gap-0.5">
              {CORNERS.map((corner) => (
                <button
                  id={`record-controls-corner-${corner.id}`}
                  key={corner.id}
                  type="button"
                  aria-pressed={prefs.cameraCorner === corner.id}
                  className={cn("h-5 rounded px-1.5 text-[9px] leading-none whitespace-nowrap hover:bg-zinc-800", prefs.cameraCorner === corner.id && "bg-zinc-700 text-white")}
                  onClick={() => savePrefs({ ...prefs, cameraCorner: corner.id })}
                >
                  {corner.label}
                </button>
              ))}
            </div>
            <p id="record-controls-cursor-label" className="px-0.5 pt-1 pb-0.5 text-[9px] leading-none tracking-wide text-zinc-500 uppercase">
              Cursor
            </p>
            <div id="record-controls-cursors" className="grid w-max grid-cols-3 gap-0.5">
              {CURSORS.map((cursor) => (
                <button
                  id={`record-controls-cursor-${cursor.id}`}
                  key={cursor.id}
                  type="button"
                  aria-pressed={prefs.cursor === cursor.id}
                  className={cn("h-5 rounded px-1.5 text-[9px] leading-none whitespace-nowrap hover:bg-zinc-800", prefs.cursor === cursor.id && "bg-zinc-700 text-white")}
                  onClick={() => savePrefs({ ...prefs, cursor: cursor.id })}
                >
                  {cursor.label}
                </button>
              ))}
            </div>
          </div>
        ) : null}
        {tools ? (
          <div id="record-controls-tools" className="absolute bottom-full left-1/2 z-10 mb-1 flex w-max -translate-x-1/2 items-center gap-0.5 rounded-full bg-zinc-950/95 px-1.5 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
            {TOOLS.map((tool) => {
              const Icon = tool.icon;
              return (
                <button
                  id={`record-controls-tool-${tool.id}`}
                  key={tool.id}
                  type="button"
                  aria-label={tool.label}
                  aria-pressed={status.tool === tool.id}
                  className={cn("inline-flex size-6 items-center justify-center rounded-full", status.tool === tool.id ? "bg-zinc-700" : "hover:bg-zinc-800")}
                  onClick={() => void recordSetStyle(tool.id, status.color).then(apply)}
                >
                  <Icon id={`record-controls-tool-${tool.id}-icon`} className="size-3" aria-hidden />
                </button>
              );
            })}
            <span id="record-controls-tool-divider" className="mx-0.5 h-4 w-px bg-white/15" />
            {COLORS.map((color) => (
              <button
                id={`record-controls-color-${color.slice(1)}`}
                key={color}
                type="button"
                aria-label={color}
                className={cn("size-3.5 rounded-full ring-1 ring-white/25 ring-offset-1 ring-offset-zinc-950", status.color === color && "ring-2 ring-sky-300")}
                style={{ background: color }}
                onClick={() => void recordSetStyle(status.tool, color).then(apply)}
              />
            ))}
            <span id="record-controls-width-divider" className="mx-0.5 h-4 w-px bg-white/15" />
            {[0, 1, 2, 3].map((size) => (
              <button
                id={`record-controls-width-${size}`}
                key={size}
                type="button"
                aria-label={["Hairline", "Thin stroke", "Medium stroke", "Thick stroke"][size]}
                aria-pressed={drawWidth === size}
                className={cn("inline-flex size-6 items-center justify-center rounded-full", drawWidth === size && "bg-zinc-700")}
                onClick={() => {
                  setDrawWidth(size);
                  rememberDrawWidth(size);
                }}
              >
                <span id={`record-controls-width-${size}-mark`} className="rounded-full bg-zinc-100" style={{ width: [3, 5, 7, 10][size], height: [3, 5, 7, 10][size] }} />
              </button>
            ))}
            <button
              id="record-controls-clear"
              type="button"
              aria-label="Clear drawing"
              className="inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800"
              onClick={() => void recordClearStrokes().catch(() => undefined)}
            >
              <Trash2 id="record-controls-clear-icon" className="size-3" aria-hidden />
            </button>
            <button
              id="record-controls-undo"
              type="button"
              aria-label="Undo"
              className="inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800"
              onClick={() => void recordUndoStroke().catch(() => undefined)}
            >
              <Undo2 id="record-controls-undo-icon" className="size-3" aria-hidden />
            </button>
          </div>
        ) : null}
        <div id="record-controls-row" className="flex w-max items-center gap-1.5 rounded-full bg-zinc-950/95 px-2 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
          <span id="record-controls-time" className="min-w-16 shrink-0 text-center text-xs font-semibold tabular-nums text-rose-300">
            {counting ? status.countdown || 3 : clock(shownMs)}
          </span>
          <button
            id="record-controls-stop"
            type="button"
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-full bg-rose-600"
            aria-label="Stop"
            onClick={() => void recordStop().catch(() => undefined)}
          >
            <Square id="record-controls-stop-icon" className="size-3 fill-current" aria-hidden />
          </button>
          {hidden ? null : (
            <>
              <button
                id="record-controls-pause"
                type="button"
                className="inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800"
                aria-label={paused ? "Resume" : "Pause"}
                disabled={counting}
                onClick={() => void (paused ? recordResume() : recordPause()).then(apply)}
              >
                {paused ? <Play id="record-controls-pause-icon" className="size-3.5" aria-hidden /> : <Pause id="record-controls-pause-icon" className="size-3.5" aria-hidden />}
              </button>
              <button
                id="record-controls-mic"
                type="button"
                className={cn("inline-flex h-7 shrink-0 items-center gap-1 rounded-full px-1.5 hover:bg-zinc-800", status.micMuted && "text-rose-300")}
                aria-label={status.micMuted ? "Unmute microphone" : "Mute microphone"}
                aria-pressed={status.micMuted}
                disabled={!status.microphone}
                onClick={() => void recordSetMicMuted(!status.micMuted).then(apply)}
              >
                <span id="record-controls-mic-meter" className="flex h-4 items-end gap-px" aria-hidden>
                  {METER.map((step, index) => (
                    <span
                      id={`record-controls-mic-bar-${index}`}
                      key={step}
                      className={cn("w-0.5 rounded-full", status.micLevel >= step ? (status.micMuted ? "bg-rose-400" : "bg-emerald-400") : "bg-zinc-700")}
                      style={{ height: `${6 + index * 2}px` }}
                    />
                  ))}
                </span>
                {status.micMuted ? <MicOff id="record-controls-mic-icon" className="size-3.5" aria-hidden /> : <Mic id="record-controls-mic-icon" className="size-3.5" aria-hidden />}
              </button>
              <button
                id="record-controls-camera"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", status.camera && "text-sky-300")}
                aria-label={status.camera ? "Hide camera" : "Show camera"}
                aria-pressed={status.camera}
                onClick={() => void recordSetCamera(!status.camera).then(apply)}
              >
                {status.camera ? <Camera id="record-controls-camera-icon" className="size-3.5" aria-hidden /> : <CameraOff id="record-controls-camera-icon" className="size-3.5" aria-hidden />}
              </button>
              <button
                id="record-controls-draw"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", status.drawing && "text-sky-300")}
                aria-label={status.drawing ? "Stop drawing" : "Draw"}
                aria-pressed={status.drawing}
                onClick={() => void recordSetDrawing(!status.drawing).then(apply)}
              >
                <Pencil id="record-controls-draw-icon" className="size-3.5" aria-hidden />
              </button>
              <button
                id="record-controls-settings-toggle"
                type="button"
                className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800", settingsOpen && "text-sky-300")}
                aria-label="Recording settings"
                aria-pressed={settingsOpen}
                onClick={() => setSettingsOpen((open) => !open)}
              >
                <Settings id="record-controls-settings-icon" className="size-3.5" aria-hidden />
              </button>
            </>
          )}
          <button
            id="record-controls-hide"
            type="button"
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-full hover:bg-zinc-800"
            aria-label={hidden ? "Show controls" : "Hide controls"}
            aria-pressed={hidden}
            onClick={() => {
              setHidden((current) => !current);
              setSettingsOpen(false);
            }}
          >
            {hidden ? <ChevronUp id="record-controls-hide-icon" className="size-3.5" aria-hidden /> : <ChevronDown id="record-controls-hide-icon" className="size-3.5" aria-hidden />}
          </button>
        </div>
      </div>
    </div>
  );
}
