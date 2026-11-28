//! Tauri commands for Record. Anything that opens a window, waits for the encoder, or shows the
//! region picker runs on a blocking thread.

use std::path::PathBuf;

use tauri::{AppHandle, State};

use super::{
    Record, RecordFile, RecordState, RecordingEntry, RecordingTarget, RecordingUpdate, ScreenRect,
    ScreenRecordingStatus, StartRequest, Stroke, StrokePoint,
};

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn record_state(record: State<'_, Record>) -> RecordState {
    record.state()
}

#[tauri::command]
pub fn record_save(record: State<'_, Record>, settings: RecordFile) -> Result<RecordState, String> {
    record.save(settings)
}

#[tauri::command]
pub async fn record_targets() -> Result<Vec<RecordingTarget>, String> {
    blocking(|| Ok(super::list_targets())).await
}

#[tauri::command]
pub async fn record_pick_region() -> Result<Option<ScreenRect>, String> {
    blocking(|| Ok(super::pick_region())).await
}

#[tauri::command]
pub async fn record_start(
    app: AppHandle,
    request: StartRequest,
) -> Result<ScreenRecordingStatus, String> {
    blocking(move || super::start(&app, request)).await
}

#[tauri::command]
pub async fn record_stop(app: AppHandle) -> Result<RecordingEntry, String> {
    blocking(move || super::stop_and_open(&app)).await
}

#[tauri::command]
pub fn record_pause() -> Result<ScreenRecordingStatus, String> {
    super::pause()
}

#[tauri::command]
pub fn record_resume() -> Result<ScreenRecordingStatus, String> {
    super::resume()
}

#[tauri::command]
pub fn record_status() -> ScreenRecordingStatus {
    super::status()
}

#[tauri::command]
pub fn record_set_mic_muted(muted: bool) -> Result<ScreenRecordingStatus, String> {
    super::set_mic_muted(muted)
}

#[tauri::command]
pub async fn record_set_controls_mode(app: AppHandle, mode: String) -> Result<(), String> {
    blocking(move || super::set_controls_mode(&app, &mode)).await
}

#[tauri::command]
pub async fn record_set_camera_corner(app: AppHandle, corner: String) -> Result<(), String> {
    blocking(move || super::set_camera_corner(&app, &corner)).await
}

#[tauri::command]
pub fn record_set_cursor(mode: String) -> Result<(), String> {
    super::set_cursor(&mode)
}

#[tauri::command]
pub async fn record_set_camera(app: AppHandle, on: bool) -> Result<ScreenRecordingStatus, String> {
    blocking(move || super::set_camera(&app, on)).await
}

#[tauri::command]
pub async fn record_set_drawing(app: AppHandle, on: bool) -> Result<ScreenRecordingStatus, String> {
    blocking(move || super::set_drawing(&app, on)).await
}

#[tauri::command]
pub fn record_set_style(
    app: AppHandle,
    tool: String,
    color: String,
) -> Result<ScreenRecordingStatus, String> {
    super::set_style(&app, tool, color)
}

#[tauri::command]
pub fn record_erase(points: Vec<StrokePoint>) -> Result<(), String> {
    super::erase(points)
}

#[tauri::command]
pub fn record_add_stroke(stroke: Stroke) -> Result<(), String> {
    super::add_stroke(stroke)
}

#[tauri::command]
pub fn record_set_draft(stroke: Option<Stroke>) -> Result<(), String> {
    super::set_draft(stroke)
}

#[tauri::command]
pub fn record_clear_strokes(app: AppHandle) -> Result<(), String> {
    super::clear_strokes(&app)
}

#[tauri::command]
pub fn record_undo_stroke(app: AppHandle) -> Result<(), String> {
    super::undo_stroke(&app)
}

#[tauri::command]
pub fn record_append_camera(bytes: Vec<u8>) -> Result<(), String> {
    super::append_camera(&bytes)
}

#[tauri::command]
pub async fn record_library(app: AppHandle) -> Result<Vec<RecordingEntry>, String> {
    blocking(move || Ok(super::library_list(&app))).await
}

#[tauri::command]
pub fn record_update(app: AppHandle, patch: RecordingUpdate) -> Result<RecordingEntry, String> {
    super::library_update(&app, patch)
}

#[tauri::command]
pub async fn record_delete(app: AppHandle, id: String) -> Result<(), String> {
    blocking(move || super::library_delete(&app, &id)).await
}

#[tauri::command]
pub async fn record_export(app: AppHandle, id: String, path: String) -> Result<String, String> {
    blocking(move || super::library_export(&app, &id, &PathBuf::from(path))).await
}

#[tauri::command]
pub fn record_media_url(app: AppHandle, id: String, kind: String) -> Result<String, String> {
    super::media_url(&app, &id, &kind)
}

#[tauri::command]
pub fn record_begin_export(path: String) -> Result<(), String> {
    super::begin_export(&PathBuf::from(path))
}

#[tauri::command]
pub fn record_append_export(bytes: Vec<u8>) -> Result<(), String> {
    super::append_export(&bytes)
}

#[tauri::command]
pub fn record_finish_export() -> Result<(), String> {
    super::finish_export()
}
