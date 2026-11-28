//! Color Picker.
//!
//! The hotkey freezes the screen under a magnifier. A click copies the color in
//! the default format and adds it to the history. The editor window lists the
//! history, every format, an adjust panel, and shades.

pub mod formats;
pub mod names;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::hotkeys::{HotkeyId, usable_hotkey};
use formats::Rgb;

pub const DEFAULT_HOTKEY: &str = "Shift+Win+C";
const BEHAVIORS: &[&str] = &["pickOnly", "pickThenEditor", "editorFirst"];
const HISTORY_KEY: &str = "colorPickerHistory";
const MAX_HISTORY: usize = 20;
const WINDOW_LABEL: &str = "color-picker";

static APP: OnceLock<AppHandle> = OnceLock::new();
static SETTINGS: Mutex<Option<ColorPickerFile>> = Mutex::new(None);
/// A pick started from the editor returns to the editor.
static FROM_EDITOR: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorFormat {
    pub name: String,
    pub template: String,
    pub visible: bool,
    pub built_in: bool,
}

impl Default for ColorFormat {
    fn default() -> Self {
        Self {
            name: String::new(),
            template: String::new(),
            visible: true,
            built_in: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorPickerFile {
    pub enabled: bool,
    pub hotkey: String,
    pub activation_behavior: String,
    pub default_format: String,
    pub show_color_name: bool,
    pub formats: Vec<ColorFormat>,
}

fn built_in_formats() -> Vec<ColorFormat> {
    formats::BUILT_INS
        .iter()
        .map(|format| ColorFormat {
            name: format.name.into(),
            template: format.template.into(),
            visible: format.visible,
            built_in: true,
        })
        .collect()
}

impl Default for ColorPickerFile {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
            activation_behavior: "pickOnly".into(),
            default_format: "HEX".into(),
            show_color_name: false,
            formats: built_in_formats(),
        }
    }
}

impl ColorPickerFile {
    pub fn normalize(&mut self) {
        self.hotkey = usable_hotkey(&self.hotkey).unwrap_or_else(|| DEFAULT_HOTKEY.into());
        if !BEHAVIORS.contains(&self.activation_behavior.as_str()) {
            self.activation_behavior = "pickOnly".into();
        }
        let mut seen = HashSet::new();
        let mut kept = Vec::new();
        for mut format in std::mem::take(&mut self.formats) {
            format.name = format.name.trim().to_string();
            if format.name.is_empty() || !seen.insert(format.name.to_lowercase()) {
                continue;
            }
            match formats::built_in(&format.name) {
                Some(built) => {
                    format.template = built.template.into();
                    format.built_in = true;
                }
                None => {
                    if format.template.trim().is_empty() {
                        continue;
                    }
                    format.built_in = false;
                }
            }
            kept.push(format);
        }
        for built in built_in_formats() {
            if seen.insert(built.name.to_lowercase()) {
                kept.push(built);
            }
        }
        self.formats = kept;
        if !self
            .formats
            .iter()
            .any(|format| format.name == self.default_format)
        {
            self.default_format = "HEX".into();
        }
    }

    fn default_template(&self) -> String {
        self.formats
            .iter()
            .find(|format| format.name == self.default_format)
            .map(|format| format.template.clone())
            .unwrap_or_else(|| "%Rex%Grx%Blx".into())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPickerState {
    pub settings: ColorPickerFile,
    pub history: Vec<String>,
    pub tokens: Vec<FormatToken>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatToken {
    pub token: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedColor {
    pub name: String,
    pub value: String,
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorDescription {
    pub hex: String,
    pub name: String,
    pub formats: Vec<FormattedColor>,
}

pub struct ColorPicker {
    app: AppHandle,
}

impl ColorPicker {
    pub fn start(app: AppHandle) -> Self {
        let _ = APP.set(app.clone());
        let picker = Self { app };
        picker.reload();
        picker
    }

    pub fn state(&self) -> ColorPickerState {
        ColorPickerState {
            settings: settings(),
            history: history(&self.app),
            tokens: formats::TOKEN_HELP
                .iter()
                .map(|(token, description)| FormatToken {
                    token: (*token).into(),
                    description: (*description).into(),
                })
                .collect(),
        }
    }

    pub fn save(&self, mut next: ColorPickerFile) -> Result<ColorPickerState, String> {
        next.hotkey = usable_hotkey(&next.hotkey)
            .ok_or_else(|| "Pick a shortcut that uses Ctrl, Alt, or Win.".to_string())?;
        if next.enabled {
            crate::hotkeys::check_free(HotkeyId::ColorPicker, &next.hotkey)?;
        }
        let saved = crate::app_settings::save_color_picker(&self.app, next)?;
        apply(saved);
        let state = self.state();
        let _ = self.app.emit("color-picker-changed", state.clone());
        Ok(state)
    }

    pub fn reload(&self) {
        let mut file = crate::app_settings::load_color_picker(&self.app).unwrap_or_default();
        file.normalize();
        apply(file);
    }
}

fn apply(file: ColorPickerFile) {
    crate::hotkeys::publish(HotkeyId::ColorPicker, file.enabled, &file.hotkey);
    if let Ok(mut slot) = SETTINGS.lock() {
        *slot = Some(file);
    }
}

fn settings() -> ColorPickerFile {
    SETTINGS
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

pub fn history(app: &AppHandle) -> Vec<String> {
    crate::app_settings::read_local_value(app, HISTORY_KEY)
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|hex| Rgb::from_hex(&hex).map(Rgb::hex))
        .take(MAX_HISTORY)
        .collect()
}

pub fn set_history(app: &AppHandle, colors: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let cleaned: Vec<String> = colors
        .into_iter()
        .filter_map(|hex| Rgb::from_hex(&hex).map(Rgb::hex))
        .filter(|hex| seen.insert(hex.clone()))
        .take(MAX_HISTORY)
        .collect();
    crate::app_settings::write_machine_value(app, HISTORY_KEY, json!(cleaned))?;
    let _ = app.emit("color-picker-history", cleaned.clone());
    Ok(cleaned)
}

fn remember(app: &AppHandle, color: Rgb) {
    let mut next = vec![color.hex()];
    next.extend(history(app));
    let _ = set_history(app, next);
}

pub fn describe(color: &str) -> Result<ColorDescription, String> {
    let rgb = Rgb::from_hex(color).ok_or_else(|| format!("\"{color}\" is not a #rrggbb color."))?;
    Ok(ColorDescription {
        hex: rgb.hex(),
        name: names::nearest(rgb).into(),
        formats: settings()
            .formats
            .iter()
            .map(|format| FormattedColor {
                name: format.name.clone(),
                value: formats::render(&format.template, rgb),
                visible: format.visible,
            })
            .collect(),
    })
}

pub fn render(template: &str, color: &str) -> Result<String, String> {
    let rgb = Rgb::from_hex(color).ok_or_else(|| format!("\"{color}\" is not a #rrggbb color."))?;
    Ok(formats::render(template, rgb))
}

pub fn hotkey(app: &AppHandle) {
    if !settings().enabled {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if settings().activation_behavior == "editorFirst" {
            open_editor(&app);
        } else {
            hotkey_pick(&app);
        }
    });
}

/// Picks one color on the calling thread, as the hotkey does.
pub fn hotkey_pick(app: &AppHandle) {
    FROM_EDITOR.store(false, Ordering::SeqCst);
    pick(app);
}

/// Starts a pick from the editor. The editor hides and comes back with the color.
pub fn pick_from_editor(app: &AppHandle) {
    FROM_EDITOR.store(true, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Give the editor a moment to leave the screen before the capture.
        std::thread::sleep(std::time::Duration::from_millis(150));
        pick(&app);
    });
}

fn pick(app: &AppHandle) {
    if crate::overlay::is_active() {
        crate::overlay::close_active();
        return;
    }
    #[cfg(windows)]
    overlay::run(app.clone());
    #[cfg(not(windows))]
    let _ = app;
}

fn picked(app: &AppHandle, color: Option<Rgb>) {
    let file = settings();
    let from_editor = FROM_EDITOR.swap(false, Ordering::SeqCst);
    if let Some(color) = color {
        let text = formats::render(&file.default_template(), color);
        let _ = crate::clipboard::set_clipboard_text(&text);
        remember(app, color);
        let _ = app.emit("color-picker-picked", color.hex());
    }
    if from_editor || (color.is_some() && file.activation_behavior == "pickThenEditor") {
        open_editor(app);
    }
}

pub fn open_editor(app: &AppHandle) {
    let window = match ensure_editor(app) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("ce-desktop-tools color editor did not open: {error}");
            return;
        }
    };
    let _ = window.show();
    let _ = window.unminimize();
    crate::force_foreground(&window);
    let _ = window.set_focus();
}

fn ensure_editor(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Color Picker")
    .inner_size(760.0, 560.0)
    .min_inner_size(560.0, 420.0)
    .visible(false)
    .initialization_script("window.__COLOR_PICKER__ = true")
    .build()
    .map_err(|error| format!("Couldn't open the color editor: {error}"))
}

#[cfg(windows)]
mod overlay {
    use tauri::AppHandle;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, COLORONCOLOR, HDC, SRCCOPY, SetStretchBltMode, StretchBlt,
    };

    use super::formats::{self, Rgb};
    use crate::overlay::{Input, Overlay, Reply, fill, label, outline};
    use crate::screen_capture::{Rect, Surface, monitor_at, virtual_screen};

    const MIN_ZOOM: i32 = 4;
    const MAX_ZOOM: i32 = 24;

    pub fn run(app: AppHandle) {
        let bounds = virtual_screen();
        let Some(mut capture) = Surface::new(bounds.width, bounds.height) else {
            super::picked(&app, None);
            return;
        };
        capture.grab(bounds.left, bounds.top);
        let (x, y) = crate::screen_capture::cursor();
        let picker = Picker {
            app,
            bounds,
            capture,
            cursor: (x - bounds.left, y - bounds.top),
            zoom: 10,
            result: None,
            settings: super::settings(),
        };
        crate::overlay::run(bounds, Box::new(picker));
    }

    struct Picker {
        app: AppHandle,
        bounds: Rect,
        capture: Surface,
        cursor: (i32, i32),
        zoom: i32,
        result: Option<Rgb>,
        settings: super::ColorPickerFile,
    }

    impl Picker {
        fn color(&self) -> Option<Rgb> {
            self.capture
                .rgb(self.cursor.0, self.cursor.1)
                .map(|[r, g, b]| Rgb { r, g, b })
        }

        fn scale(&self) -> f64 {
            let monitor = monitor_at(
                self.bounds.left + self.cursor.0,
                self.bounds.top + self.cursor.1,
            );
            f64::from(monitor.dpi) / 96.0
        }
    }

    impl Overlay for Picker {
        fn input(&mut self, input: Input) -> Reply {
            match input {
                Input::Move { x, y } => {
                    self.cursor = (x, y);
                    Reply::Redraw
                }
                Input::LeftDown { x, y } => {
                    self.cursor = (x, y);
                    self.result = self.color();
                    Reply::Close
                }
                Input::Wheel(delta) => {
                    let step = if delta > 0 { 2 } else { -2 };
                    self.zoom = (self.zoom + step).clamp(MIN_ZOOM, MAX_ZOOM);
                    Reply::Redraw
                }
                Input::RightDown | Input::Escape => Reply::Close,
                Input::LeftUp { .. } | Input::Tick | Input::Refresh => Reply::Idle,
            }
        }

        fn paint(&mut self, dc: HDC, width: i32, height: i32) {
            unsafe {
                let _ = BitBlt(
                    dc,
                    0,
                    0,
                    width,
                    height,
                    Some(self.capture.dc),
                    0,
                    0,
                    SRCCOPY,
                );
            }
            let scale = self.scale();
            let size = (150.0 * scale).round() as i32;
            let cells = {
                let count = (size / self.zoom).max(3);
                if count % 2 == 0 { count + 1 } else { count }
            };
            let cell = size / cells;
            let size = cell * cells;
            let gap = (24.0 * scale).round() as i32;
            let (x, y) = self.cursor;
            let mut left = x + gap;
            let mut top = y + gap;
            let swatch = (22.0 * scale).round() as i32;
            if left + size > width {
                left = x - gap - size;
            }
            if top + size + swatch > height {
                top = y - gap - size - swatch;
            }
            unsafe {
                SetStretchBltMode(dc, COLORONCOLOR);
                let _ = StretchBlt(
                    dc,
                    left,
                    top,
                    size,
                    size,
                    Some(self.capture.dc),
                    x - cells / 2,
                    y - cells / 2,
                    cells,
                    cells,
                    SRCCOPY,
                );
            }
            let center = left + (cells / 2) * cell;
            let middle = top + (cells / 2) * cell;
            outline(
                dc,
                center - 1,
                middle - 1,
                center + cell + 1,
                middle + cell + 1,
                [0, 0, 0],
                1,
            );
            outline(
                dc,
                center,
                middle,
                center + cell,
                middle + cell,
                [255, 255, 255],
                1,
            );
            let color = self.color().unwrap_or(Rgb { r: 0, g: 0, b: 0 });
            fill(
                dc,
                left,
                top + size,
                left + size,
                top + size + swatch,
                [color.r, color.g, color.b],
            );
            let border = (2.0 * scale).round().max(1.0) as i32;
            outline(
                dc,
                left - border,
                top - border,
                left + size + border,
                top + size + swatch + border,
                [24, 24, 27],
                border,
            );

            let mut lines = vec![formats::render(&self.settings.default_template(), color)];
            if self.settings.show_color_name {
                lines.push(super::names::nearest(color).to_string());
            }
            label(
                dc,
                left,
                top + size + swatch + (6.0 * scale).round() as i32,
                &lines,
                scale,
                width,
                height,
            );
        }

        fn closed(&mut self) {
            super::picked(&self.app, self.result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ColorPickerFile;

    #[test]
    fn an_empty_section_gets_all_built_in_formats() {
        let mut file: ColorPickerFile = serde_json::from_str("{}").unwrap();
        file.normalize();
        assert_eq!(file, ColorPickerFile::default());
        assert_eq!(file.formats.len(), 14);
        let visible: Vec<&str> = file
            .formats
            .iter()
            .filter(|format| format.visible)
            .map(|format| format.name.as_str())
            .collect();
        assert_eq!(visible, vec!["HEX", "RGB", "HSL"]);
    }

    #[test]
    fn order_and_custom_formats_survive_and_missing_built_ins_return() {
        let mut file: ColorPickerFile = serde_json::from_str(
            r##"{"defaultFormat":"Mine","activationBehavior":"teleport","formats":[
                {"name":"RGB","template":"tampered","visible":false,"builtIn":true},
                {"name":"Mine","template":"#%ReX%GrX%BlX","visible":true},
                {"name":"mine","template":"dupe"},
                {"name":"Empty","template":"  "}
            ]}"##,
        )
        .unwrap();
        file.normalize();
        assert_eq!(file.activation_behavior, "pickOnly");
        assert_eq!(file.formats[0].name, "RGB");
        assert_eq!(file.formats[0].template, "rgb(%Re, %Gr, %Bl)");
        assert!(!file.formats[0].visible);
        assert_eq!(file.formats[1].name, "Mine");
        assert!(!file.formats[1].built_in);
        assert_eq!(file.formats.len(), 15);
        assert_eq!(file.default_format, "Mine");

        let again = file.clone();
        file.normalize();
        assert_eq!(file, again);

        file.formats.retain(|format| format.name != "Mine");
        file.normalize();
        assert_eq!(file.default_format, "HEX");
    }
}
