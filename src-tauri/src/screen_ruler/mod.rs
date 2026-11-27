//! Screen Ruler.
//!
//! The hotkey opens a small command bar. Picking a style captures the screen
//! and opens a native overlay that measures bounds, spacing, or a horizontal
//! or vertical run of similar pixels under the cursor.

pub mod measure;

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::hotkeys::{HotkeyId, usable_hotkey};

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Win+M";
const DEFAULT_LINE_COLOR: &str = "#ff4500";
const DEFAULT_TOLERANCE: u32 = 30;
const WINDOW_LABEL: &str = "screen-ruler";
const BAR_WIDTH: f64 = 248.0;
const BAR_HEIGHT: f64 = 48.0;

static APP: OnceLock<AppHandle> = OnceLock::new();
static SETTINGS: Mutex<Option<ScreenRulerFile>> = Mutex::new(None);
static STYLE: Mutex<String> = Mutex::new(String::new());

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScreenRulerFile {
    pub enabled: bool,
    pub hotkey: String,
    pub default_measure_style: String,
    pub continuous_capture: bool,
    pub per_channel_edge_detection: bool,
    pub pixel_tolerance: u32,
    pub extra_units: String,
    pub draw_feet_on_cross: bool,
    pub line_color: String,
}

impl Default for ScreenRulerFile {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
            default_measure_style: "none".into(),
            continuous_capture: false,
            per_channel_edge_detection: false,
            pixel_tolerance: DEFAULT_TOLERANCE,
            extra_units: "none".into(),
            draw_feet_on_cross: true,
            line_color: DEFAULT_LINE_COLOR.into(),
        }
    }
}

impl ScreenRulerFile {
    pub fn normalize(&mut self) {
        self.hotkey = usable_hotkey(&self.hotkey).unwrap_or_else(|| DEFAULT_HOTKEY.into());
        if !measure::STYLES.contains(&self.default_measure_style.as_str()) {
            self.default_measure_style = "none".into();
        }
        if !measure::UNITS.contains(&self.extra_units.as_str()) {
            self.extra_units = "none".into();
        }
        self.pixel_tolerance = self.pixel_tolerance.min(255);
        self.line_color = crate::overlay::parse_hex(&self.line_color)
            .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or_else(|| DEFAULT_LINE_COLOR.into());
    }
}

pub struct ScreenRuler {
    app: AppHandle,
}

impl ScreenRuler {
    pub fn start(app: AppHandle) -> Self {
        let _ = APP.set(app.clone());
        let ruler = Self { app };
        ruler.reload();
        ruler
    }

    pub fn state(&self) -> ScreenRulerFile {
        settings()
    }

    pub fn save(&self, mut next: ScreenRulerFile) -> Result<ScreenRulerFile, String> {
        next.hotkey = usable_hotkey(&next.hotkey)
            .ok_or_else(|| "Pick a shortcut that uses Ctrl, Alt, or Win.".to_string())?;
        if next.enabled {
            crate::hotkeys::check_free(HotkeyId::ScreenRuler, &next.hotkey)?;
        }
        let saved = crate::app_settings::save_screen_ruler(&self.app, next)?;
        self.apply(saved.clone());
        Ok(saved)
    }

    pub fn reload(&self) {
        let mut file = crate::app_settings::load_screen_ruler(&self.app).unwrap_or_default();
        file.normalize();
        self.apply(file);
    }

    fn apply(&self, file: ScreenRulerFile) {
        crate::hotkeys::publish(HotkeyId::ScreenRuler, file.enabled, &file.hotkey);
        if !file.enabled {
            close(&self.app);
        }
        if let Ok(mut slot) = SETTINGS.lock() {
            *slot = Some(file);
        }
        crate::overlay::refresh_active();
    }
}

fn settings() -> ScreenRulerFile {
    SETTINGS
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

pub fn style() -> String {
    let current = STYLE.lock().map(|slot| slot.clone()).unwrap_or_default();
    if current.is_empty() {
        "none".into()
    } else {
        current
    }
}

pub fn hotkey(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if bar_visible(&app) {
            close(&app);
            return;
        }
        open(&app);
    });
}

pub fn open(app: &AppHandle) {
    if let Err(error) = show_bar(app) {
        eprintln!("ce-desktop-tools screen ruler did not open: {error}");
        return;
    }
    let start = settings().default_measure_style;
    if start != "none" {
        measure(app, &start);
    } else {
        set_style(app, "none");
    }
}

/// Switches the measuring style. `none` leaves the command bar open with no overlay.
pub fn measure(app: &AppHandle, style: &str) {
    let style = if measure::STYLES.contains(&style) {
        style.to_string()
    } else {
        "none".to_string()
    };
    set_style(app, &style);
    if style == "none" {
        crate::overlay::close_active();
        return;
    }
    #[cfg(windows)]
    {
        if crate::overlay::is_active() {
            crate::overlay::refresh_active();
            return;
        }
        let app = app.clone();
        std::thread::spawn(move || overlay::run(app));
    }
}

pub fn close(app: &AppHandle) {
    set_style(app, "none");
    crate::overlay::close_active();
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

fn set_style(app: &AppHandle, style: &str) {
    if let Ok(mut slot) = STYLE.lock() {
        *slot = style.to_string();
    }
    let _ = app.emit_to(WINDOW_LABEL, "screen-ruler-style", style.to_string());
}

fn bar_visible(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn ensure_bar(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Screen Ruler")
    .inner_size(BAR_WIDTH, BAR_HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .resizable(false)
    .shadow(false)
    .initialization_script("window.__SCREEN_RULER__ = true")
    .build()
    .map_err(|error| format!("Couldn't open the Screen Ruler bar: {error}"))
}

fn show_bar(app: &AppHandle) -> Result<(), String> {
    let window = ensure_bar(app)?;
    #[cfg(windows)]
    {
        let (x, y) = crate::screen_capture::cursor();
        let monitor = crate::screen_capture::monitor_at(x, y);
        let scale = f64::from(monitor.dpi) / 96.0;
        let width = BAR_WIDTH * scale;
        let left = f64::from(monitor.work.left) + (f64::from(monitor.work.width) - width) / 2.0;
        let top = f64::from(monitor.work.top) + 16.0 * scale;
        let _ = window.set_position(tauri::PhysicalPosition::new(
            left.round() as i32,
            top.round() as i32,
        ));
        let _ = window.set_size(tauri::PhysicalSize::new(
            width.round() as u32,
            (BAR_HEIGHT * scale).round() as u32,
        ));
    }
    window.show().map_err(|error| error.to_string())?;
    raise_bar(app);
    Ok(())
}

/// Keeps the command bar above the overlay so its buttons stay clickable.
fn raise_bar(app: &AppHandle) {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        if let Ok(raw) = window.hwnd() {
            use windows::Win32::UI::WindowsAndMessaging::{
                HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
            };
            unsafe {
                let _ = SetWindowPos(
                    windows::Win32::Foundation::HWND(raw.0),
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod overlay {
    use tauri::AppHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{BitBlt, HDC, SRCCOPY};

    use super::measure::{self, Span};
    use crate::overlay::{Input, Overlay, Reply, fill, label};
    use crate::screen_capture::{Rect, Surface, monitor_at, virtual_screen};

    pub fn run(app: AppHandle) {
        let bounds = virtual_screen();
        let Some(mut capture) = Surface::new(bounds.width, bounds.height) else {
            return;
        };
        capture.grab(bounds.left, bounds.top);
        let (x, y) = crate::screen_capture::cursor();
        let ruler = Ruler {
            app,
            bounds,
            capture,
            cursor: (x - bounds.left, y - bounds.top),
            drag_start: None,
            selection: None,
            style: super::style(),
            settings: super::settings(),
        };
        crate::overlay::run(bounds, Box::new(ruler));
    }

    struct Ruler {
        app: AppHandle,
        bounds: Rect,
        capture: Surface,
        cursor: (i32, i32),
        drag_start: Option<(i32, i32)>,
        selection: Option<(i32, i32, i32, i32)>,
        style: String,
        settings: super::ScreenRulerFile,
    }

    impl Ruler {
        fn scale(&self) -> f64 {
            let monitor = monitor_at(
                self.bounds.left + self.cursor.0,
                self.bounds.top + self.cursor.1,
            );
            f64::from(monitor.dpi) / 96.0
        }

        fn dpi(&self) -> u32 {
            (self.scale() * 96.0).round() as u32
        }

        fn span(&self) -> Span {
            let (horizontal, vertical) = match self.style.as_str() {
                "horizontal" => (true, false),
                "vertical" => (false, true),
                _ => (true, true),
            };
            measure::scan(
                |x, y| self.capture.rgb(x, y),
                self.cursor.0,
                self.cursor.1,
                horizontal,
                vertical,
                self.settings.pixel_tolerance,
                self.settings.per_channel_edge_detection,
            )
        }

        fn rectangle(&self) -> Option<(i32, i32, i32, i32)> {
            if let Some((sx, sy)) = self.drag_start {
                let (cx, cy) = self.cursor;
                return Some((sx.min(cx), sy.min(cy), sx.max(cx), sy.max(cy)));
            }
            self.selection
        }

        fn lines(&self) -> Vec<String> {
            let dpi = self.dpi();
            let unit = self.settings.extra_units.as_str();
            match self.style.as_str() {
                "bounds" => match self.rectangle() {
                    Some((left, top, right, bottom)) => measure::describe(
                        Some(right - left + 1),
                        Some(bottom - top + 1),
                        dpi,
                        unit,
                    ),
                    None => vec!["Drag to measure".into()],
                },
                "horizontal" => measure::describe(Some(self.span().width()), None, dpi, unit),
                "vertical" => measure::describe(None, Some(self.span().height()), dpi, unit),
                _ => {
                    let span = self.span();
                    measure::describe(Some(span.width()), Some(span.height()), dpi, unit)
                }
            }
        }

        fn copy(&self) {
            if let Some(first) = self.lines().first() {
                if first.ends_with("px") {
                    let _ = crate::clipboard::set_clipboard_text(first);
                }
            }
        }

        fn color(&self) -> [u8; 3] {
            crate::overlay::parse_hex(&self.settings.line_color).unwrap_or([255, 69, 0])
        }
    }

    impl Overlay for Ruler {
        fn input(&mut self, input: Input) -> Reply {
            match input {
                Input::Move { x, y } => {
                    self.cursor = (x, y);
                    Reply::Redraw
                }
                Input::LeftDown { x, y } => {
                    self.cursor = (x, y);
                    if self.style == "bounds" {
                        self.drag_start = Some((x, y));
                        self.selection = None;
                    } else {
                        self.copy();
                    }
                    Reply::Redraw
                }
                Input::LeftUp { x, y } => {
                    self.cursor = (x, y);
                    if self.style == "bounds" {
                        if let Some(rect) = self.rectangle() {
                            self.selection = Some(rect);
                            self.drag_start = None;
                            self.copy();
                        }
                    }
                    Reply::Redraw
                }
                Input::RightDown | Input::Escape => Reply::Close,
                Input::Wheel(_) => Reply::Idle,
                Input::Tick => {
                    if self.settings.continuous_capture {
                        self.capture.grab(self.bounds.left, self.bounds.top);
                        Reply::Redraw
                    } else {
                        Reply::Idle
                    }
                }
                Input::Refresh => {
                    self.style = super::style();
                    self.settings = super::settings();
                    self.drag_start = None;
                    self.selection = None;
                    if self.style == "none" {
                        Reply::Close
                    } else {
                        Reply::Redraw
                    }
                }
            }
        }

        fn paint(&mut self, dc: HDC, width: i32, height: i32) {
            unsafe {
                let _ = BitBlt(dc, 0, 0, width, height, Some(self.capture.dc), 0, 0, SRCCOPY);
            }
            let scale = self.scale();
            let line = (scale.round() as i32).max(1);
            let color = self.color();
            let feet = (6.0 * scale).round() as i32;
            let (x, y) = self.cursor;
            match self.style.as_str() {
                "bounds" => {
                    if let Some((left, top, right, bottom)) = self.rectangle() {
                        crate::overlay::frame(dc, left, top, right + 1, bottom + 1, color, line);
                    }
                }
                style => {
                    let span = self.span();
                    let half = line / 2;
                    if style != "vertical" {
                        fill(dc, span.left, y - half, span.right + 1, y - half + line, color);
                        if self.settings.draw_feet_on_cross {
                            fill(dc, span.left, y - feet, span.left + line, y + feet + 1, color);
                            fill(dc, span.right + 1 - line, y - feet, span.right + 1, y + feet + 1, color);
                        }
                    }
                    if style != "horizontal" {
                        fill(dc, x - half, span.top, x - half + line, span.bottom + 1, color);
                        if self.settings.draw_feet_on_cross {
                            fill(dc, x - feet, span.top, x + feet + 1, span.top + line, color);
                            fill(dc, x - feet, span.bottom + 1 - line, x + feet + 1, span.bottom + 1, color);
                        }
                    }
                }
            }
            let offset = (16.0 * scale).round() as i32;
            label(dc, x + offset, y + offset, &self.lines(), scale, width, height);
        }

        fn tick_ms(&self) -> Option<u32> {
            Some(120)
        }

        fn shown(&mut self, _hwnd: HWND) {
            super::raise_bar(&self.app);
        }

        fn closed(&mut self) {
            if super::style() != "none" {
                super::close(&self.app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScreenRulerFile;

    #[test]
    fn an_empty_section_gets_the_defaults() {
        let mut file: ScreenRulerFile = serde_json::from_str("{}").unwrap();
        file.normalize();
        assert_eq!(file, ScreenRulerFile::default());
        assert_eq!(file.hotkey, "Ctrl+Shift+Win+M");
    }

    #[test]
    fn retired_values_fall_back() {
        let mut file: ScreenRulerFile = serde_json::from_str(
            r#"{"defaultMeasureStyle":"diagonal","extraUnits":"furlongs","pixelTolerance":900,"lineColor":"red","hotkey":"m"}"#,
        )
        .unwrap();
        file.normalize();
        assert_eq!(file.default_measure_style, "none");
        assert_eq!(file.extra_units, "none");
        assert_eq!(file.pixel_tolerance, 255);
        assert_eq!(file.line_color, "#ff4500");
        assert_eq!(file.hotkey, "Ctrl+Shift+Win+M");
    }
}
