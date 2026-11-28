export type RecordSource = "display" | "window" | "region";

export type RecordResolution = "720" | "1080" | "1440" | "native";

export type RecordCameraCorner = "bottom-left" | "bottom-right" | "top-left" | "top-right";

export type RecordCursor = "system" | "hidden" | "circle";

export type RecordControlsMode = "pill" | "hidden" | "settings" | "tools";

export type RecordDrawTool = "pen" | "arrow" | "rectangle" | "highlighter" | "eraser";

export type RecordSettings = {
  enabled: boolean;
  hotkey: string;
  /** Empty means Videos\CE Desktop Tools. */
  folder: string;
  microphone: boolean;
  systemAudio: boolean;
  camera: boolean;
  countdown: boolean;
  resolution: RecordResolution;
  cameraCorner: RecordCameraCorner;
  cursor: RecordCursor;
  /** What the hotkey records. */
  source: RecordSource;
};

export type RecordState = {
  settings: RecordSettings;
  /** The folder takes are saved to, with the default filled in. */
  folder: string;
};

export type RecordTarget = {
  id: string;
  kind: "display" | "window";
  label: string;
};

/** Screen pixels. */
export type ScreenRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type RecordStartRequest = {
  source: RecordSource;
  /** A monitor handle for display, or a window handle for window. */
  target: string;
  /** Screen pixels for region. Leave it out to open the region picker. */
  region?: ScreenRect | null;
  title?: string;
  microphone: boolean;
  systemAudio: boolean;
  camera: boolean;
  countdown: boolean;
  microphoneDeviceId?: string;
  resolution: RecordResolution;
  cameraCorner: RecordCameraCorner;
  cursor: RecordCursor;
};

export type RecordPhase = "idle" | "countdown" | "recording" | "paused" | "error";

export type RecordStatus = {
  phase: RecordPhase;
  elapsedMs: number;
  countdown: number;
  id: string;
  error: string | null;
  path: string | null;
  camera: boolean;
  microphone: boolean;
  micMuted: boolean;
  /** Microphone loudness, from 0 to 1. */
  micLevel: number;
  drawing: boolean;
  strokes: number;
  tool: RecordDrawTool;
  color: string;
  sourceLabel: string;
};

/** Fractions of the video frame. */
export type CameraLayout = {
  x: number;
  y: number;
  size: number;
};

export type RecordEntry = {
  id: string;
  title: string;
  createdAt: number;
  durationMs: number;
  source: string;
  sourceLabel: string;
  width: number;
  height: number;
  videoPath: string;
  cameraPath: string;
  camera: CameraLayout;
  trimStartMs: number;
  trimEndMs: number;
  hasMic: boolean;
  hasSystemAudio: boolean;
};

export type RecordUpdate = {
  id: string;
  title?: string;
  trimStartMs?: number;
  trimEndMs?: number;
  camera?: CameraLayout;
};

/** Fractions of the capture frame. */
export type RecordStrokePoint = {
  x: number;
  y: number;
};

export type RecordStroke = {
  tool: RecordDrawTool;
  color: string;
  width: number;
  points: RecordStrokePoint[];
  draft?: boolean;
};
