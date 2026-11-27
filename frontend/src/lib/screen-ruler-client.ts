import { invoke } from "@tauri-apps/api/core";
import type { MeasureStyle, ScreenRulerSettings } from "../../../types/screen-ruler";

export function screenRulerState() {
  return invoke<ScreenRulerSettings>("screen_ruler_state");
}

export function screenRulerSave(settings: ScreenRulerSettings) {
  return invoke<ScreenRulerSettings>("screen_ruler_save", { settings });
}

export function screenRulerStyle() {
  return invoke<MeasureStyle>("screen_ruler_style");
}

export function screenRulerMeasure(style: MeasureStyle) {
  return invoke<void>("screen_ruler_measure", { style });
}

export function screenRulerOpen() {
  return invoke<void>("screen_ruler_open");
}

export function screenRulerClose() {
  return invoke<void>("screen_ruler_close");
}
