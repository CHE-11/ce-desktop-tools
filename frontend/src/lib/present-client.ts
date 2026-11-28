import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { PresentCursor, PresentPointer, PresentRect, PresentSettings, PresentState, PresentTool } from "../../../types/present";

export function presentSettings() {
  return invoke<PresentSettings>("present_settings");
}

export function presentSave(settings: PresentSettings) {
  return invoke<PresentSettings>("present_save", { settings });
}

export function presentState() {
  return invoke<PresentState>("present_state");
}

export function togglePresent() {
  return invoke<PresentState>("present_toggle");
}

export function closePresent() {
  return invoke<PresentState>("present_close");
}

export function setPresentTool(tool: PresentTool) {
  return invoke<PresentState>("present_set_tool", { tool });
}

export function setPresentCursor(cursor: PresentCursor) {
  return invoke<PresentState>("present_set_cursor", { cursor });
}

export function setPresentColor(color: string) {
  return invoke<PresentState>("present_set_color", { color });
}

export function setPresentWidth(width: number) {
  return invoke<PresentState>("present_set_width", { width });
}

export function setPresentDim(dim: number) {
  return invoke<PresentState>("present_set_dim", { dim });
}

export function setPresentRadius(radius: number) {
  return invoke<PresentState>("present_set_radius", { radius });
}

export function setPresentSpread(spread: number) {
  return invoke<PresentState>("present_set_spread", { spread });
}

export function setPresentCircle(circle: boolean) {
  return invoke<PresentState>("present_set_circle", { circle });
}

export function pushPresentMark(monitor: number, id: string) {
  return invoke<void>("present_push_mark", { monitor, id });
}

export function undoPresentMark() {
  return invoke<PresentState>("present_undo");
}

export function clearPresentMarks() {
  return invoke<PresentState>("present_clear");
}

export function finishPresentPlace(kind: "spotlight" | "zoom", monitor: number, rect: PresentRect | null) {
  return invoke<PresentState>("present_finish_place", { kind, monitor, rect });
}

export function setPresentHoldEscape(hold: boolean) {
  return invoke<void>("present_hold_escape", { hold });
}

export function releasePresentCursor() {
  return invoke<PresentState>("present_release_cursor");
}

export function onPresentChanged(handler: (state: PresentState) => void) {
  return listen<PresentState>("present-changed", (event) => handler(event.payload));
}

export function onPresentPointer(handler: (pointer: PresentPointer) => void) {
  return listen<PresentPointer>("present-pointer", (event) => handler(event.payload));
}

export function onPresentEscape(handler: () => void) {
  return listen("present-escape", () => handler());
}

export function onPresentDelete(handler: () => void) {
  return listen("present-delete", () => handler());
}

export function onPresentUndo(handler: (undo: { monitor: number; id: string }) => void) {
  return listen<{ monitor: number; id: string }>("present-undo", (event) => handler(event.payload));
}
