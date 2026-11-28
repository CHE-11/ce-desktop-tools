//! Present covers every monitor with a spotlight, a zoom lens, a laser cursor, and drawing.
//!
//! Each monitor gets a transparent stage window. Stages let clicks through except while a
//! drawing tool is armed, while a lock drag is armed, or while Alt is held for a spotlight or
//! zoom drag. The palette is its own window on the monitor that had the pointer when Present
//! opened. Stages are not hidden from screen capture, so a screen share shows the marks.

pub mod commands;
mod lens;
mod stage;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::hotkeys::{HotkeyId, usable_hotkey};

pub use lens::{lock_lens, zoom_source};

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Win+P";

static APP: OnceLock<AppHandle> = OnceLock::new();
static SETTINGS: Mutex<Option<PresentFile>> = Mutex::new(None);
static LIVE: AtomicBool = AtomicBool::new(false);
static BUSY: AtomicBool = AtomicBool::new(false);
static HOLD_ESCAPE: AtomicBool = AtomicBool::new(false);
static SESSION: Mutex<Option<LiveSession>> = Mutex::new(None);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PresentFile {
    pub enabled: bool,
    pub hotkey: String,
    /// `dot`, `ring`, `arrow`, `crosshair`, or `off`.
    pub cursor: String,
    pub color: String,
    /// One of the three stroke sizes: 0.32, 1, or 2.4.
    pub width: f32,
    /// Spotlight shade, from 0 (clear) to 0.92.
    pub dim: f32,
    /// Follow circle diameter, in CSS pixels.
    pub spread: f32,
    /// Corner radius of a highlight, in CSS pixels.
    pub radius: f32,
    /// Draws the large circle around the pointer while the cursor tool is selected.
    pub circle: bool,
}

impl Default for PresentFile {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
            cursor: "dot".into(),
            color: "#f59e0b".into(),
            width: 1.0,
            dim: 0.62,
            spread: 240.0,
            radius: 12.0,
            circle: false,
        }
    }
}

impl PresentFile {
    pub fn normalize(&mut self) {
        let fallback = Self::default();
        self.hotkey = usable_hotkey(&self.hotkey).unwrap_or_else(|| DEFAULT_HOTKEY.into());
        self.cursor = known_cursor(&self.cursor).unwrap_or(fallback.cursor);
        self.color = known_color(&self.color).unwrap_or(fallback.color);
        self.width = known_width(self.width).unwrap_or(fallback.width);
        self.dim = known_dim(self.dim).unwrap_or(fallback.dim);
        self.spread = known_spread(self.spread).unwrap_or(fallback.spread);
        self.radius = known_radius(self.radius).unwrap_or(fallback.radius);
    }
}

pub struct Present {
    app: AppHandle,
}

impl Present {
    pub fn start(app: AppHandle) -> Self {
        let _ = APP.set(app.clone());
        let present = Self { app };
        present.reload();
        present
    }

    pub fn settings(&self) -> PresentFile {
        settings()
    }

    pub fn save(&self, mut next: PresentFile) -> Result<PresentFile, String> {
        next.hotkey = usable_hotkey(&next.hotkey)
            .ok_or_else(|| "Pick a shortcut that uses Ctrl, Alt, or Win.".to_string())?;
        if next.enabled {
            crate::hotkeys::check_free(HotkeyId::Present, &next.hotkey)?;
        }
        let saved = crate::app_settings::save_present(&self.app, next)?;
        apply(saved.clone());
        Ok(saved)
    }

    pub fn reload(&self) {
        let mut file = crate::app_settings::load_present(&self.app).unwrap_or_default();
        file.normalize();
        apply(file);
    }
}

fn apply(file: PresentFile) {
    crate::hotkeys::publish(HotkeyId::Present, file.enabled, &file.hotkey);
    if let Ok(mut slot) = SETTINGS.lock() {
        *slot = Some(file);
    }
}

fn settings() -> PresentFile {
    SETTINGS
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

/// The hotkey turns Present on or off.
pub fn hotkey(app: &AppHandle) {
    if is_on() || settings().enabled {
        quick_toggle(app);
    }
}

/// Turns Present on or off from the hotkey or the launcher.
pub fn quick_toggle(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = toggle(&app) {
            eprintln!("ce-desktop-tools present: {error}");
        }
    });
}

struct LiveSession {
    tool: String,
    cursor: String,
    color: String,
    width: f32,
    dim: f32,
    radius: f32,
    spread: f32,
    circle: bool,
    placing: bool,
    clear: u32,
    spotlight: Option<PresentRect>,
    spotlight_monitor: Option<usize>,
    zoom: Option<PresentRect>,
    zoom_monitor: Option<usize>,
    /// Marks in the order they were made, as (monitor, id), so undo walks back across stages.
    marks: Vec<(usize, String)>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentState {
    pub on: bool,
    pub tool: String,
    pub cursor: String,
    pub color: String,
    pub width: f32,
    pub dim: f32,
    pub radius: f32,
    pub spread: f32,
    pub circle: bool,
    pub placing: bool,
    pub clear: u32,
    /// Fractions of the monitor named by `spotlight_monitor`.
    pub spotlight: Option<PresentRect>,
    pub spotlight_monitor: Option<usize>,
    /// Fractions of the monitor named by `zoom_monitor`.
    pub zoom: Option<PresentRect>,
    pub zoom_monitor: Option<usize>,
    pub monitors: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentPointer {
    /// CSS pixels from the top left of this stage's monitor.
    pub x: f64,
    pub y: f64,
    /// True only on the stage whose monitor holds the pointer.
    pub inside: bool,
    /// Glass of the zoom lens on this stage, as fractions of the monitor.
    pub lens: Option<PresentRect>,
    pub monitor: usize,
}

pub fn is_on() -> bool {
    LIVE.load(Ordering::Acquire)
}

/// Blocks while windows open or close. Call it off the main thread.
pub fn toggle(app: &AppHandle) -> Result<PresentState, String> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return Ok(state());
    }
    let result = if is_on() {
        close_inner(app)
    } else {
        if crate::record::running() {
            let _ = crate::record::stop_and_open(app);
        }
        open(app)
    };
    BUSY.store(false, Ordering::Release);
    result.map(|_| state())
}

pub fn open(app: &AppHandle) -> Result<(), String> {
    if is_on() {
        return Ok(());
    }
    let file = settings();
    {
        let mut slot = SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(LiveSession {
            tool: "spotlight".into(),
            cursor: file.cursor,
            color: file.color,
            width: file.width,
            dim: file.dim,
            radius: file.radius,
            spread: file.spread,
            circle: file.circle,
            placing: false,
            clear: 0,
            spotlight: None,
            spotlight_monitor: None,
            zoom: None,
            zoom_monitor: None,
            marks: Vec::new(),
        });
        sync_flags(slot.as_ref().expect("present session"));
    }
    LIVE.store(true, Ordering::Release);
    if let Err(error) = stage::open(app) {
        let _ = close_inner(app);
        return Err(error);
    }
    let _ = app.emit("present-changed", state());
    Ok(())
}

/// For app exit, on the main thread: gives the system cursor back and drops the magnifier.
pub fn shutdown() {
    LIVE.store(false, Ordering::Release);
    stage::shutdown();
}

/// Blocks while the windows close. Call it off the main thread.
pub fn close(app: &AppHandle) -> Result<(), String> {
    close_inner(app)
}

fn close_inner(app: &AppHandle) -> Result<(), String> {
    LIVE.store(false, Ordering::Release);
    HOLD_ESCAPE.store(false, Ordering::Relaxed);
    stage::close(app);
    let taken = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(live) = taken {
        remember_style(app, &live);
        let _ = app.emit("present-changed", state());
    }
    Ok(())
}

/// The next session opens with the cursor, color, and sizes this one ended with.
fn remember_style(app: &AppHandle, live: &LiveSession) {
    let mut next = settings();
    next.cursor = live.cursor.clone();
    next.color = live.color.clone();
    next.width = live.width;
    next.dim = live.dim;
    next.radius = live.radius;
    next.spread = live.spread;
    next.circle = live.circle;
    if next == settings() {
        return;
    }
    match crate::app_settings::save_present(app, next) {
        Ok(saved) => {
            if let Ok(mut slot) = SETTINGS.lock() {
                *slot = Some(saved);
            }
        }
        Err(error) => eprintln!("ce-desktop-tools present settings: {error}"),
    }
}

pub fn state() -> PresentState {
    snapshot(
        &SESSION
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

pub fn set_tool(app: &AppHandle, tool: &str) -> Result<PresentState, String> {
    let tool = known_tool(tool)?;
    mutate(app, |live| {
        if live.tool == tool && (tool == "spotlight" || tool == "cursor" || tool == "zoom") {
            let locked = if tool == "zoom" {
                live.zoom_monitor = None;
                live.zoom.take()
            } else {
                live.spotlight_monitor = None;
                live.spotlight.take()
            };
            live.placing = !(live.placing || locked.is_some());
        } else {
            live.tool = tool;
            live.placing = false;
        }
    })
}

pub fn set_cursor(app: &AppHandle, cursor: &str) -> Result<PresentState, String> {
    let cursor = known_cursor(cursor)?;
    mutate(app, |live| live.cursor = cursor)
}

pub fn set_color(app: &AppHandle, color: &str) -> Result<PresentState, String> {
    let color = known_color(color)?;
    mutate(app, |live| live.color = color)
}

pub fn set_width(app: &AppHandle, width: f32) -> Result<PresentState, String> {
    let width = known_width(width)?;
    mutate(app, |live| live.width = width)
}

pub fn set_dim(app: &AppHandle, dim: f32) -> Result<PresentState, String> {
    let dim = known_dim(dim)?;
    mutate(app, |live| live.dim = dim)
}

pub fn set_radius(app: &AppHandle, radius: f32) -> Result<PresentState, String> {
    let radius = known_radius(radius)?;
    mutate(app, |live| live.radius = radius)
}

pub fn set_spread(app: &AppHandle, spread: f32) -> Result<PresentState, String> {
    let spread = known_spread(spread)?;
    mutate(app, |live| live.spread = spread)
}

pub fn set_circle(app: &AppHandle, circle: bool) -> Result<PresentState, String> {
    mutate(app, |live| live.circle = circle)
}

/// A stage made a mark. Undo removes marks newest first, whichever monitor they are on.
pub fn push_mark(monitor: usize, id: String) {
    let mut slot = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(live) = slot.as_mut() {
        if live.marks.len() >= 2000 {
            live.marks.remove(0);
        }
        live.marks.push((monitor, id));
    }
}

/// Asks the stage that made the newest mark to remove it. A stage that already erased that
/// mark calls this again for the next one.
pub fn undo_mark(app: &AppHandle) -> Result<PresentState, String> {
    let popped = {
        let mut slot = SESSION
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let live = slot.as_mut().ok_or("Present is off.")?;
        live.marks.pop()
    };
    if let Some((monitor, id)) = popped {
        let _ = app.emit_to(
            stage::label(monitor),
            "present-undo",
            serde_json::json!({ "monitor": monitor, "id": id }),
        );
    }
    Ok(state())
}

pub fn clear_marks(app: &AppHandle) -> Result<PresentState, String> {
    mutate(app, |live| {
        live.clear = live.clear.wrapping_add(1);
        live.marks.clear();
    })
}

/// While a label or a stroke is open, Escape cancels that instead of changing the tool.
pub fn set_hold_escape(hold: bool) {
    HOLD_ESCAPE.store(hold, Ordering::Relaxed);
}

/// Stores a dragged spotlight or zoom rectangle on `monitor`. `None` ends the drag without a
/// new lock.
pub fn finish_place(
    app: &AppHandle,
    kind: &str,
    monitor: usize,
    rect: Option<PresentRect>,
) -> Result<PresentState, String> {
    if kind != "spotlight" && kind != "zoom" {
        return Err("Present can lock a spotlight or a zoom.".into());
    }
    let zoom = kind == "zoom";
    let rect = rect.and_then(clean_rect);
    mutate(app, |live| {
        live.placing = false;
        if let Some(rect) = rect {
            if zoom {
                live.zoom = Some(rect);
                live.zoom_monitor = Some(monitor);
            } else {
                live.spotlight = Some(rect);
                live.spotlight_monitor = Some(monitor);
            }
        }
    })
}

/// Leaves the current tool for the crosshair. Present and its palette stay open.
pub fn release_cursor(app: &AppHandle) -> Result<PresentState, String> {
    mutate(app, |live| {
        live.tool = "cursor".into();
        live.cursor = "crosshair".into();
        live.placing = false;
    })
}

/// True while Present is on, so the keyboard hook keeps Escape from the app in front.
pub fn escape_is_useful() -> bool {
    is_on()
}

/// Escape cancels an open label or stroke, then returns to the crosshair, then closes Present.
pub fn on_escape(app: &AppHandle) {
    if !is_on() {
        return;
    }
    if HOLD_ESCAPE.load(Ordering::Acquire) {
        let _ = app.emit("present-escape", ());
        return;
    }
    let at_rest = SESSION
        .lock()
        .ok()
        .and_then(|slot| {
            slot.as_ref().map(|live| {
                live.tool == "cursor" && live.cursor == "crosshair" && !live.placing
            })
        })
        .unwrap_or(true);
    if at_rest {
        let _ = toggle(app);
    } else {
        let _ = release_cursor(app);
    }
}

/// True while the lasso is selected, so Delete removes the selection instead of reaching the
/// app in front.
pub fn delete_is_useful() -> bool {
    if !is_on() {
        return false;
    }
    let Ok(slot) = SESSION.try_lock() else {
        return false;
    };
    slot.as_ref().is_some_and(|live| live.tool == "lasso")
}

pub fn on_delete(app: &AppHandle) {
    let _ = app.emit("present-delete", ());
}

fn mutate(app: &AppHandle, change: impl FnOnce(&mut LiveSession)) -> Result<PresentState, String> {
    let mut slot = SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(live) = slot.as_mut() else {
        return Err("Present is off.".into());
    };
    change(live);
    sync_flags(live);
    let next = snapshot(&slot);
    drop(slot);
    let _ = app.emit("present-changed", &next);
    Ok(next)
}

/// What the stage pump needs each tick, read under one lock.
#[derive(Clone)]
pub(crate) struct Live {
    pub tool: String,
    pub zoom: Option<PresentRect>,
    pub zoom_monitor: Option<usize>,
}

pub(crate) fn live() -> Option<Live> {
    let slot = SESSION.lock().ok()?;
    slot.as_ref().map(|live| Live {
        tool: live.tool.clone(),
        zoom: live.zoom,
        zoom_monitor: live.zoom_monitor,
    })
}

fn sync_flags(live: &LiveSession) {
    let draw = matches!(
        live.tool.as_str(),
        "lasso" | "highlight" | "pen" | "arrow" | "text" | "eraser"
    );
    let capture = draw || live.placing;
    let placeable = matches!(live.tool.as_str(), "spotlight" | "cursor" | "zoom");
    let follow_spot = matches!(live.tool.as_str(), "spotlight" | "cursor")
        && live.spotlight.is_none()
        && !live.placing;
    let follow_zoom = live.tool == "zoom" && live.zoom.is_none() && !live.placing;
    let fast = live.cursor != "off" || follow_spot || follow_zoom;
    let hide_pointer = live.tool == "zoom" && !capture;
    let blank = hide_pointer || (live.cursor != "off" && !capture);
    stage::set_flags(capture, placeable, fast, blank);
}

fn snapshot(slot: &Option<LiveSession>) -> PresentState {
    let monitors = stage::monitor_count();
    match slot {
        Some(live) => PresentState {
            on: true,
            tool: live.tool.clone(),
            cursor: live.cursor.clone(),
            color: live.color.clone(),
            width: live.width,
            dim: live.dim,
            radius: live.radius,
            spread: live.spread,
            circle: live.circle,
            placing: live.placing,
            clear: live.clear,
            spotlight: live.spotlight,
            spotlight_monitor: live.spotlight_monitor,
            zoom: live.zoom,
            zoom_monitor: live.zoom_monitor,
            monitors,
        },
        None => {
            let file = settings();
            PresentState {
                on: false,
                tool: "spotlight".into(),
                cursor: file.cursor,
                color: file.color,
                width: file.width,
                dim: file.dim,
                radius: file.radius,
                spread: file.spread,
                circle: file.circle,
                placing: false,
                clear: 0,
                spotlight: None,
                spotlight_monitor: None,
                zoom: None,
                zoom_monitor: None,
                monitors,
            }
        }
    }
}

fn known_tool(tool: &str) -> Result<String, String> {
    match tool {
        "none" | "lasso" | "spotlight" | "cursor" | "zoom" | "highlight" | "pen" | "arrow"
        | "text" | "eraser" => Ok(tool.to_string()),
        _ => Err("Unknown present tool.".into()),
    }
}

fn known_cursor(cursor: &str) -> Result<String, String> {
    match cursor {
        "dot" | "ring" | "arrow" | "crosshair" | "off" => Ok(cursor.to_string()),
        _ => Err("Unknown present cursor.".into()),
    }
}

fn known_spread(spread: f32) -> Result<f32, String> {
    if !spread.is_finite() {
        return Err("Present spread is a circle size in pixels.".into());
    }
    Ok(spread.clamp(64.0, 560.0))
}

fn known_radius(radius: f32) -> Result<f32, String> {
    if !radius.is_finite() {
        return Err("Present radius is a corner size in pixels.".into());
    }
    Ok(radius.clamp(0.0, 48.0))
}

fn known_dim(dim: f32) -> Result<f32, String> {
    if !dim.is_finite() {
        return Err("Present dim is a fraction from 0 to 1.".into());
    }
    Ok(dim.clamp(0.0, 0.92))
}

fn known_width(width: f32) -> Result<f32, String> {
    const CHOICES: [f32; 3] = [0.32, 1.0, 2.4];
    CHOICES
        .into_iter()
        .find(|choice| (choice - width).abs() < 0.02)
        .ok_or_else(|| "Present width is a stroke size.".into())
}

fn known_color(color: &str) -> Result<String, String> {
    let bytes = color.as_bytes();
    if bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(color.to_ascii_lowercase())
    } else {
        Err("Present color is a #rrggbb value.".into())
    }
}

fn clean_rect(rect: PresentRect) -> Option<PresentRect> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
    {
        return None;
    }
    let x = rect.x.min(rect.x + rect.width).clamp(0.0, 1.0);
    let y = rect.y.min(rect.y + rect.height).clamp(0.0, 1.0);
    let width = rect.width.abs();
    let height = rect.height.abs();
    if width.max(height) < 0.01 {
        return None;
    }
    Some(PresentRect {
        x,
        y,
        width: width.min(1.0 - x).max(0.002),
        height: height.min(1.0 - y).max(0.002),
    })
}

#[cfg(test)]
mod tests {
    use super::{PresentFile, PresentRect, clean_rect};

    #[test]
    fn clean_rect_keeps_a_thin_bar_and_drops_a_dot() {
        let bar = clean_rect(PresentRect {
            x: 0.1,
            y: 0.4,
            width: 0.5,
            height: 0.0,
        })
        .expect("bar");
        assert!(bar.width > 0.4);
        assert!(bar.height > 0.0);
        assert!(
            clean_rect(PresentRect {
                x: 0.2,
                y: 0.2,
                width: 0.004,
                height: 0.004
            })
            .is_none()
        );
    }

    #[test]
    fn an_empty_section_gets_the_defaults() {
        let mut file: PresentFile = serde_json::from_str("{}").unwrap();
        file.normalize();
        assert_eq!(file, PresentFile::default());
    }

    #[test]
    fn bad_values_fall_back() {
        let mut file: PresentFile = serde_json::from_str(
            r##"{"cursor":"laser","color":"red","width":7,"dim":2,"spread":9000,"circle":true}"##,
        )
        .unwrap();
        file.normalize();
        assert_eq!(file.cursor, "dot");
        assert_eq!(file.color, "#f59e0b");
        assert_eq!(file.width, 1.0);
        assert_eq!(file.dim, 0.92);
        assert_eq!(file.spread, 560.0);
        assert!(file.circle);
    }
}
