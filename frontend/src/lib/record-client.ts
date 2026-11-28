import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  RecordControlsMode,
  RecordCameraCorner,
  RecordCursor,
  RecordEntry,
  RecordSettings,
  RecordStartRequest,
  RecordState,
  RecordStatus,
  RecordStroke,
  RecordStrokePoint,
  RecordTarget,
  RecordUpdate,
  ScreenRect,
} from "../../../types/record";

export function recordState() {
  return invoke<RecordState>("record_state");
}

export function recordSave(settings: RecordSettings) {
  return invoke<RecordState>("record_save", { settings });
}

export function recordTargets() {
  return invoke<RecordTarget[]>("record_targets");
}

export function recordPickRegion() {
  return invoke<ScreenRect | null>("record_pick_region");
}

export function recordStart(request: RecordStartRequest) {
  return invoke<RecordStatus>("record_start", { request });
}

export function recordStop() {
  return invoke<RecordEntry>("record_stop");
}

export function recordPause() {
  return invoke<RecordStatus>("record_pause");
}

export function recordResume() {
  return invoke<RecordStatus>("record_resume");
}

export function recordStatus() {
  return invoke<RecordStatus>("record_status");
}

export function recordSetMicMuted(muted: boolean) {
  return invoke<RecordStatus>("record_set_mic_muted", { muted });
}

export function recordSetControlsMode(mode: RecordControlsMode) {
  return invoke<void>("record_set_controls_mode", { mode });
}

export function recordSetCameraCorner(corner: RecordCameraCorner) {
  return invoke<void>("record_set_camera_corner", { corner });
}

export function recordSetCursor(mode: RecordCursor) {
  return invoke<void>("record_set_cursor", { mode });
}

export function recordSetCamera(on: boolean) {
  return invoke<RecordStatus>("record_set_camera", { on });
}

export function recordSetDrawing(on: boolean) {
  return invoke<RecordStatus>("record_set_drawing", { on });
}

export function recordSetStyle(tool: string, color: string) {
  return invoke<RecordStatus>("record_set_style", { tool, color });
}

export function recordErase(points: RecordStrokePoint[]) {
  return invoke<void>("record_erase", { points });
}

export function recordAddStroke(stroke: RecordStroke) {
  return invoke<void>("record_add_stroke", { stroke });
}

export function recordSetDraft(stroke: RecordStroke | null) {
  return invoke<void>("record_set_draft", { stroke });
}

export function recordClearStrokes() {
  return invoke<void>("record_clear_strokes");
}

export function recordUndoStroke() {
  return invoke<void>("record_undo_stroke");
}

export function recordAppendCamera(bytes: Uint8Array) {
  return invoke<void>("record_append_camera", { bytes: Array.from(bytes) });
}

export function recordLibrary() {
  return invoke<RecordEntry[]>("record_library");
}

export function recordUpdate(patch: RecordUpdate) {
  return invoke<RecordEntry>("record_update", { patch });
}

export function recordDelete(id: string) {
  return invoke<void>("record_delete", { id });
}

export function recordExport(id: string, path: string) {
  return invoke<string>("record_export", { id, path });
}

export function recordMediaUrl(id: string, kind: "video" | "camera") {
  return invoke<string>("record_media_url", { id, kind });
}

export function recordBeginExport(path: string) {
  return invoke<void>("record_begin_export", { path });
}

export function recordAppendExport(bytes: Uint8Array) {
  return invoke<void>("record_append_export", { bytes: Array.from(bytes) });
}

export function recordFinishExport() {
  return invoke<void>("record_finish_export");
}

export async function chooseVideoPath(title: string) {
  const name = `${title.replace(/[\\/:*?"<>|]+/g, " ").trim() || "Recording"}.mp4`;
  return save({ defaultPath: name, filters: [{ name: "MP4 video", extensions: ["mp4"] }] });
}

export async function chooseRecordFolder(current: string) {
  const picked = await open({ directory: true, defaultPath: current || undefined });
  return typeof picked === "string" ? picked : null;
}

/** Gets the app out of the picture before the region picker freezes the screen. */
export async function minimizeMainWindow() {
  try {
    await getCurrentWindow().minimize();
  } catch {
    return;
  }
  await new Promise((resolve) => window.setTimeout(resolve, 250));
}

export async function restoreMainWindow() {
  const main = getCurrentWindow();
  await main.unminimize().catch(() => undefined);
  await main.setFocus().catch(() => undefined);
}

export function onRecordStatus(handler: (status: RecordStatus) => void) {
  return listen<RecordStatus>("screen-recording", (event) => handler(event.payload));
}

export function onRecordSaved(handler: (entry: RecordEntry) => void) {
  return listen<RecordEntry>("record-saved", (event) => handler(event.payload));
}

export function onRecordError(handler: (message: string) => void) {
  return listen<string>("record-error", (event) => handler(event.payload));
}

export function onRecordDrawUndo(handler: () => void) {
  return listen("record-draw-undo", () => handler());
}

export function onRecordDrawClear(handler: () => void) {
  return listen("record-draw-clear", () => handler());
}
