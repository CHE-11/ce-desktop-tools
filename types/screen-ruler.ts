export type MeasureStyle = "none" | "bounds" | "spacing" | "horizontal" | "vertical";

export type ExtraUnits = "none" | "inches" | "centimeters" | "millimeters";

export type ScreenRulerSettings = {
  enabled: boolean;
  hotkey: string;
  defaultMeasureStyle: MeasureStyle;
  continuousCapture: boolean;
  perChannelEdgeDetection: boolean;
  pixelTolerance: number;
  extraUnits: ExtraUnits;
  drawFeetOnCross: boolean;
  lineColor: string;
};
