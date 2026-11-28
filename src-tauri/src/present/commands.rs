//! Tauri commands for Present. Opening and closing waits on the main thread, so those run on
//! a blocking thread.

use tauri::{AppHandle, State};

use super::{Present, PresentFile, PresentRect, PresentState};

#[tauri::command]
pub fn present_settings(present: State<'_, Present>) -> PresentFile {
    present.settings()
}

#[tauri::command]
pub fn present_save(
    present: State<'_, Present>,
    settings: PresentFile,
) -> Result<PresentFile, String> {
    present.save(settings)
}

#[tauri::command]
pub fn present_state() -> PresentState {
    super::state()
}

#[tauri::command]
pub async fn present_toggle(app: AppHandle) -> Result<PresentState, String> {
    tauri::async_runtime::spawn_blocking(move || super::toggle(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn present_close(app: AppHandle) -> Result<PresentState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::close(&app)?;
        Ok(super::state())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn present_set_tool(app: AppHandle, tool: String) -> Result<PresentState, String> {
    super::set_tool(&app, &tool)
}

#[tauri::command]
pub fn present_set_cursor(app: AppHandle, cursor: String) -> Result<PresentState, String> {
    super::set_cursor(&app, &cursor)
}

#[tauri::command]
pub fn present_set_color(app: AppHandle, color: String) -> Result<PresentState, String> {
    super::set_color(&app, &color)
}

#[tauri::command]
pub fn present_set_width(app: AppHandle, width: f32) -> Result<PresentState, String> {
    super::set_width(&app, width)
}

#[tauri::command]
pub fn present_set_dim(app: AppHandle, dim: f32) -> Result<PresentState, String> {
    super::set_dim(&app, dim)
}

#[tauri::command]
pub fn present_set_radius(app: AppHandle, radius: f32) -> Result<PresentState, String> {
    super::set_radius(&app, radius)
}

#[tauri::command]
pub fn present_set_spread(app: AppHandle, spread: f32) -> Result<PresentState, String> {
    super::set_spread(&app, spread)
}

#[tauri::command]
pub fn present_set_circle(app: AppHandle, circle: bool) -> Result<PresentState, String> {
    super::set_circle(&app, circle)
}

#[tauri::command]
pub fn present_push_mark(monitor: usize, id: String) {
    super::push_mark(monitor, id);
}

#[tauri::command]
pub fn present_undo(app: AppHandle) -> Result<PresentState, String> {
    super::undo_mark(&app)
}

#[tauri::command]
pub fn present_clear(app: AppHandle) -> Result<PresentState, String> {
    super::clear_marks(&app)
}

#[tauri::command]
pub fn present_finish_place(
    app: AppHandle,
    kind: String,
    monitor: usize,
    rect: Option<PresentRect>,
) -> Result<PresentState, String> {
    super::finish_place(&app, &kind, monitor, rect)
}

#[tauri::command]
pub fn present_hold_escape(hold: bool) {
    super::set_hold_escape(hold);
}

#[tauri::command]
pub fn present_release_cursor(app: AppHandle) -> Result<PresentState, String> {
    super::release_cursor(&app)
}
