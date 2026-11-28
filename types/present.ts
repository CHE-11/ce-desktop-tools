export type PresentTool =
  | "none"
  | "lasso"
  | "spotlight"
  | "cursor"
  | "zoom"
  | "highlight"
  | "pen"
  | "arrow"
  | "text"
  | "eraser";

export type PresentCursor = "dot" | "ring" | "arrow" | "crosshair" | "off";

export type PresentSettings = {
  enabled: boolean;
  hotkey: string;
  cursor: PresentCursor;
  color: string;
  /** One of the three stroke sizes: 0.32, 1, or 2.4. */
  width: number;
  /** Spotlight shade, from 0 (clear) to 0.92. */
  dim: number;
  /** Follow circle diameter, in CSS pixels. */
  spread: number;
  /** Corner radius of a highlight, in CSS pixels. */
  radius: number;
  circle: boolean;
};

/** Fractions of one monitor. */
export type PresentRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PresentState = {
  on: boolean;
  tool: PresentTool;
  cursor: PresentCursor;
  color: string;
  width: number;
  dim: number;
  radius: number;
  spread: number;
  circle: boolean;
  placing: boolean;
  /** Goes up by one each time every stage should clear its marks. */
  clear: number;
  spotlight: PresentRect | null;
  spotlightMonitor: number | null;
  zoom: PresentRect | null;
  zoomMonitor: number | null;
  monitors: number;
};

export type PresentPointer = {
  /** CSS pixels from the top left of this stage's monitor. */
  x: number;
  y: number;
  /** True only on the stage whose monitor holds the pointer. */
  inside: boolean;
  /** Glass of the zoom lens on this stage, as fractions of the monitor. */
  lens: PresentRect | null;
  monitor: number;
};
