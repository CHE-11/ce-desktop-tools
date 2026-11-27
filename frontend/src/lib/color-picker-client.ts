import { invoke } from "@tauri-apps/api/core";
import type { ColorDescription, ColorPickerSettings, ColorPickerState } from "../../../types/color-picker";

export function colorPickerState() {
  return invoke<ColorPickerState>("color_picker_state");
}

export function colorPickerSave(settings: ColorPickerSettings) {
  return invoke<ColorPickerState>("color_picker_save", { settings });
}

export function colorPickerDescribe(color: string) {
  return invoke<ColorDescription>("color_picker_describe", { color });
}

export function colorPickerRender(template: string, color: string) {
  return invoke<string>("color_picker_render", { template, color });
}

export function colorPickerSetHistory(history: string[]) {
  return invoke<string[]>("color_picker_set_history", { history });
}

export function colorPickerPick() {
  return invoke<void>("color_picker_pick");
}

export function colorPickerOpenEditor() {
  return invoke<void>("color_picker_open_editor");
}
