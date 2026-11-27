//! Global launcher.
//!
//! A hotkey (Win+Space by default) opens a borderless search window. Typing
//! finds Start menu apps, this app's tools and actions, a calculator answer, a
//! unit conversion, or the `kill <name>` and `port <n>` process commands.

pub mod apps;
pub mod calc;
pub mod calculator;
mod fuzzy;
pub mod units;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::hotkeys::{HotkeyId, usable_hotkey};

pub const DEFAULT_HOTKEY: &str = "Win+Space";
const WINDOW_LABEL: &str = "launcher";
const WIDTH: f64 = 680.0;
const HEIGHT: f64 = 440.0;
const MAX_RESULTS: usize = 30;
const MAX_PROCESS_RESULTS: usize = 12;

static VISIBLE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherFile {
    pub enabled: bool,
    pub hotkey: String,
}

impl Default for LauncherFile {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
        }
    }
}

impl LauncherFile {
    pub fn normalize(&mut self) {
        self.hotkey = usable_hotkey(&self.hotkey).unwrap_or_else(|| DEFAULT_HOTKEY.into());
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherState {
    pub enabled: bool,
    pub hotkey: String,
    pub app_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSave {
    pub enabled: bool,
    pub hotkey: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
}

impl LauncherResult {
    fn new(id: String, kind: &str, title: String, subtitle: String, icon: &str) -> Self {
        Self {
            id,
            kind: kind.into(),
            title,
            subtitle,
            icon: icon.into(),
        }
    }
}

struct Entry {
    id: &'static str,
    title: &'static str,
    keywords: &'static str,
}

const TOOLS: &[Entry] = &[
    Entry {
        id: "expansion",
        title: "Text Expansion",
        keywords: "snippets",
    },
    Entry {
        id: "clipboard",
        title: "Clipboard",
        keywords: "history",
    },
    Entry {
        id: "misspell",
        title: "Common misspells",
        keywords: "spelling",
    },
    Entry {
        id: "date",
        title: "Date expander",
        keywords: "time",
    },
    Entry {
        id: "ascii",
        title: "ASCII codes",
        keywords: "symbols",
    },
    Entry {
        id: "calculator",
        title: "Calculator & Units",
        keywords: "calculate math tape convert units",
    },
    Entry {
        id: "app-shortcuts",
        title: "Keyboard Shortcuts",
        keywords: "hotkeys",
    },
    Entry {
        id: "rename",
        title: "Batch rename",
        keywords: "files",
    },
    Entry {
        id: "pantherbar",
        title: "Pantherbar",
        keywords: "selection toolbar",
    },
    Entry {
        id: "launcher",
        title: "Launcher",
        keywords: "search hotkey",
    },
    Entry {
        id: "folder-rules",
        title: "Folder Rules",
        keywords: "downloads sort organize",
    },
    Entry {
        id: "processes",
        title: "Processes",
        keywords: "task manager kill port lock",
    },
    Entry {
        id: "network-explorer",
        title: "Network Explorer",
        keywords: "network processes connections foreground background",
    },
    Entry {
        id: "file-tools",
        title: "File Tools",
        keywords: "pdf image cad convert",
    },
    Entry {
        id: "add-to-desktop",
        title: "Add to Desktop",
        keywords: "shortcut apps applications",
    },
    Entry {
        id: "file-associations",
        title: "File Associations",
        keywords: "open with default program extension",
    },
    Entry {
        id: "terminal",
        title: "Terminal",
        keywords: "cmd powershell console shell command prompt",
    },
    Entry {
        id: "always-on-top",
        title: "Always On Top",
        keywords: "pin window topmost border",
    },
    Entry {
        id: "screen-ruler",
        title: "Screen Ruler",
        keywords: "measure pixels distance",
    },
    Entry {
        id: "color-picker",
        title: "Color Picker",
        keywords: "eyedropper hex rgb colour",
    },
];

const ACTIONS: &[Entry] = &[
    Entry {
        id: "show",
        title: "Open ce-desktop-tools",
        keywords: "main window",
    },
    Entry {
        id: "toggle-expansion",
        title: "Pause or resume text expansion",
        keywords: "expansion",
    },
    Entry {
        id: "lock-vault",
        title: "Lock the vault",
        keywords: "pin snippets",
    },
    Entry {
        id: "restart-explorer",
        title: "Restart Windows Explorer",
        keywords: "taskbar",
    },
    Entry {
        id: "megaclip",
        title: "Run MegaClip",
        keywords: "clipboard script",
    },
    Entry {
        id: "folder-rules-run",
        title: "Run folder rules now",
        keywords: "rules run sort downloads",
    },
    Entry {
        id: "toggle-stay-awake",
        title: "Stay awake on or off",
        keywords: "awake sleep caffeine keep screen on",
    },
    Entry {
        id: "screen-ruler",
        title: "Measure the screen",
        keywords: "ruler pixels",
    },
    Entry {
        id: "color-pick",
        title: "Pick a color",
        keywords: "eyedropper color picker",
    },
    Entry {
        id: "color-editor",
        title: "Open the color editor",
        keywords: "color picker history",
    },
    Entry {
        id: "unpin-all",
        title: "Unpin all windows",
        keywords: "always on top",
    },
];

pub struct Launcher {
    app: AppHandle,
    file: Mutex<LauncherFile>,
}

impl Launcher {
    pub fn start(app: AppHandle) -> Self {
        let mut file = crate::app_settings::load_launcher(&app).unwrap_or_default();
        file.normalize();
        if let Err(error) = ensure_window(&app) {
            eprintln!("ce-desktop-tools launcher window did not open: {error}");
        }
        crate::hotkeys::publish(HotkeyId::Launcher, file.enabled, &file.hotkey);
        apps::start_indexing();
        Self {
            app,
            file: Mutex::new(file),
        }
    }

    pub fn state(&self) -> LauncherState {
        let file = self.file();
        LauncherState {
            enabled: file.enabled,
            hotkey: file.hotkey,
            app_count: apps::snapshot().len(),
        }
    }

    pub fn save(&self, save: LauncherSave) -> Result<LauncherState, String> {
        let hotkey = usable_hotkey(&save.hotkey)
            .ok_or_else(|| "Pick a shortcut that uses Ctrl, Alt, or Win.".to_string())?;
        if save.enabled {
            crate::hotkeys::check_free(HotkeyId::Launcher, &hotkey)?;
        }
        let next = LauncherFile {
            enabled: save.enabled,
            hotkey,
        };
        let saved = crate::app_settings::save_launcher(&self.app, next)?;
        crate::hotkeys::publish(HotkeyId::Launcher, saved.enabled, &saved.hotkey);
        if let Ok(mut slot) = self.file.lock() {
            *slot = saved;
        }
        Ok(self.state())
    }

    /// Settings can move to another folder, which may hold another launcher section.
    pub fn reload(&self) {
        if let Ok(mut file) = crate::app_settings::load_launcher(&self.app) {
            file.normalize();
            crate::hotkeys::publish(HotkeyId::Launcher, file.enabled, &file.hotkey);
            if let Ok(mut slot) = self.file.lock() {
                *slot = file;
            }
        }
    }

    fn file(&self) -> LauncherFile {
        self.file
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    pub fn query(&self, text: &str) -> Vec<LauncherResult> {
        query(text)
    }

    pub fn hide(&self) {
        hide(&self.app);
    }

    pub fn show(&self) {
        show(&self.app);
    }
}

pub fn query(text: &str) -> Vec<LauncherResult> {
    let text = text.trim();
    let mut results = Vec::new();
    if let Some(answer) = calc::evaluate(text) {
        results.push(LauncherResult::new(
            format!("calc:{}", answer.value),
            "calc",
            answer.value.clone(),
            format!("{} · Enter copies the answer", answer.detail),
            "calculator",
        ));
    } else if let Some(conversion) = units::convert(text) {
        results.push(LauncherResult::new(
            format!("unit:{}", conversion.value),
            "unit",
            conversion.title,
            "Enter copies the converted number".into(),
            "ruler",
        ));
    }
    results.extend(process_commands(text));

    let mut ranked: Vec<(i32, LauncherResult)> = Vec::new();
    for entry in TOOLS {
        if let Some(score) = entry_score(text, entry) {
            ranked.push((
                score,
                LauncherResult::new(
                    format!("tool:{}", entry.id),
                    "tool",
                    entry.title.into(),
                    "Open this tool in ce-desktop-tools".into(),
                    "layout-grid",
                ),
            ));
        }
    }
    for entry in ACTIONS {
        if let Some(score) = entry_score(text, entry) {
            ranked.push((
                score,
                LauncherResult::new(
                    format!("action:{}", entry.id),
                    "action",
                    entry.title.into(),
                    "Action".into(),
                    "zap",
                ),
            ));
        }
    }
    if !text.is_empty() {
        for app in apps::snapshot() {
            if let Some(score) = fuzzy::score(text, &app.name) {
                // Apps win a tie with a tool of the same name.
                ranked.push((
                    score + 5,
                    LauncherResult::new(
                        format!("app:{}", app.app_id),
                        "app",
                        app.name,
                        "App".into(),
                        "app-window",
                    ),
                ));
            }
        }
    }
    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    results.extend(ranked.into_iter().map(|(_, result)| result));
    results.truncate(MAX_RESULTS);
    results
}

fn entry_score(text: &str, entry: &Entry) -> Option<i32> {
    let title = fuzzy::score(text, entry.title);
    let keywords = entry
        .keywords
        .split_whitespace()
        .filter_map(|word| fuzzy::score(text, word))
        .max()
        .map(|score| score - 100);
    title.max(keywords)
}

fn process_commands(text: &str) -> Vec<LauncherResult> {
    let lower = text.to_lowercase();
    if let Some(name) = lower.strip_prefix("kill ") {
        return kill_results(name.trim());
    }
    if let Some(port) = lower.strip_prefix("port ") {
        return port_results(port.trim());
    }
    Vec::new()
}

fn kill_results(name: &str) -> Vec<LauncherResult> {
    if name.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<_> = crate::processes::list()
        .unwrap_or_default()
        .into_iter()
        .filter(|process| process.name.to_lowercase().contains(name))
        .collect();
    matches.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    if matches.is_empty() {
        return vec![info(format!("No running process matches \"{name}\""))];
    }
    matches
        .into_iter()
        .take(MAX_PROCESS_RESULTS)
        .map(|process| {
            LauncherResult::new(
                format!("kill:{}", process.pid),
                "process",
                format!("Kill {}", process.name),
                format!(
                    "PID {} · {}",
                    process.pid,
                    crate::processes::format_bytes(process.memory_bytes)
                ),
                "skull",
            )
        })
        .collect()
}

fn port_results(text: &str) -> Vec<LauncherResult> {
    let Ok(pattern) = crate::processes::PortPattern::parse(text) else {
        return vec![info(
            "Type a port, such as port 3000, or a pattern, such as port 30**".into(),
        )];
    };
    let owners = match crate::processes::port_owners(&pattern) {
        Ok(owners) => owners,
        Err(error) => return vec![info(error)],
    };
    if owners.is_empty() {
        return vec![info(format!("Nothing is using {}", pattern.describe()))];
    }
    let mut seen = std::collections::HashSet::new();
    owners
        .into_iter()
        .filter(|owner| seen.insert((owner.pid, owner.local_port)))
        .take(MAX_PROCESS_RESULTS)
        .map(|owner| {
            LauncherResult::new(
                format!("kill:{}", owner.pid),
                "process",
                format!("Kill {} (port {})", owner.name, owner.local_port),
                format!(
                    "{} {} {} · PID {}",
                    owner.protocol, owner.local_address, owner.state, owner.pid
                ),
                "skull",
            )
        })
        .collect()
}

fn info(message: String) -> LauncherResult {
    LauncherResult::new("info:".into(), "info", message, String::new(), "info")
}

pub fn run(app: &AppHandle, id: &str) -> Result<Option<String>, String> {
    let (kind, value) = id.split_once(':').unwrap_or((id, ""));
    match kind {
        "calc" | "unit" => {
            crate::clipboard::set_clipboard_text(value)?;
            hide(app);
            Ok(None)
        }
        "app" => {
            hide(app);
            apps::launch(value)?;
            Ok(None)
        }
        "tool" => {
            hide(app);
            crate::show_main(app);
            let _ = app.emit("launcher-open-tool", value.to_string());
            Ok(None)
        }
        "action" => run_action(app, value),
        "kill" => {
            let pid = value
                .parse::<u32>()
                .map_err(|_| "That process id is not valid.".to_string())?;
            let closed = crate::processes::kill(pid, false)?;
            Ok(Some(format!("Closed {closed}.")))
        }
        "info" => Ok(None),
        _ => Err("That launcher result is no longer available.".into()),
    }
}

fn run_action(app: &AppHandle, id: &str) -> Result<Option<String>, String> {
    match id {
        "show" => {
            hide(app);
            crate::show_main(app);
            Ok(None)
        }
        "toggle-expansion" => {
            let expansion = app
                .try_state::<crate::expansion::Expansion>()
                .ok_or_else(|| "Text expansion is not running.".to_string())?;
            let enabled = expansion.toggle_enabled();
            Ok(Some(if enabled {
                "Text expansion is on.".into()
            } else {
                "Text expansion is paused.".into()
            }))
        }
        "lock-vault" => {
            if let Some(expansion) = app.try_state::<crate::expansion::Expansion>() {
                expansion.lock();
            }
            Ok(Some("The vault is locked.".into()))
        }
        "restart-explorer" => {
            hide(app);
            std::thread::spawn(|| {
                if let Err(error) = crate::explorer_restart::restart() {
                    eprintln!("ce-desktop-tools could not restart Explorer: {error}");
                }
            });
            Ok(None)
        }
        "megaclip" => {
            crate::megaclip_script::start()?;
            hide(app);
            Ok(None)
        }
        "folder-rules-run" => {
            let rules = app
                .try_state::<crate::folder_rules::FolderRules>()
                .ok_or_else(|| "Folder rules are not running.".to_string())?;
            let entries = rules.run(None, false)?;
            let done = entries
                .iter()
                .filter(|entry| entry.outcome == "done")
                .count();
            Ok(Some(match done {
                0 => "No files matched a rule.".into(),
                1 => "Folder rules handled 1 file.".into(),
                count => format!("Folder rules handled {count} files."),
            }))
        }
        "toggle-stay-awake" => {
            let on = crate::stay_awake::toggle(app)?;
            Ok(Some(if on {
                "Stay Awake is on.".into()
            } else {
                "Stay Awake is off.".into()
            }))
        }
        "screen-ruler" => {
            hide(app);
            crate::screen_ruler::open(app);
            Ok(None)
        }
        "color-pick" => {
            hide(app);
            let app = app.clone();
            std::thread::spawn(move || {
                // Let the launcher leave the screen before the capture.
                std::thread::sleep(std::time::Duration::from_millis(150));
                crate::color_picker::hotkey_pick(&app);
            });
            Ok(None)
        }
        "color-editor" => {
            hide(app);
            crate::color_picker::open_editor(app);
            Ok(None)
        }
        "unpin-all" => {
            crate::always_on_top::unpin_all();
            Ok(Some("Every pinned window is unpinned.".into()))
        }
        _ => Err("That action is not available.".into()),
    }
}

fn ensure_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Launcher")
    .inner_size(WIDTH, HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .resizable(false)
    .shadow(false)
    .initialization_script("window.__LAUNCHER__ = true")
    .build()
    .map_err(|error| format!("Couldn't open the launcher window: {error}"))?;
    Ok(())
}

pub fn is_visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
}

pub fn show(app: &AppHandle) {
    if ensure_window(app).is_err() {
        return;
    }
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    place(&window);
    VISIBLE.store(true, Ordering::SeqCst);
    let _ = app.emit_to(WINDOW_LABEL, "launcher-show", ());
    let _ = window.show();
    crate::force_foreground(&window);
    let _ = window.set_focus();
}

pub fn hide(app: &AppHandle) {
    VISIBLE.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

pub fn toggle(app: &AppHandle) {
    if is_visible() {
        hide(app);
    } else {
        show(app);
    }
}

/// Centers the window near the top of the monitor under the mouse pointer.
fn place(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
        };
        use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return;
        }
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let scale = f64::from(dpi_x.max(96)) / 96.0;
        let area = info.rcWork;
        let area_width = (area.right - area.left) as f64;
        let area_height = (area.bottom - area.top) as f64;
        let width = (WIDTH * scale).min(area_width - 32.0).max(320.0);
        let height = (HEIGHT * scale).min(area_height - 32.0).max(200.0);
        let x = area.left as f64 + (area_width - width) / 2.0;
        let y = area.top as f64 + area_height * 0.18;
        let _ = window.set_position(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ));
        let _ = window.set_size(tauri::PhysicalSize::new(
            width.round() as u32,
            height.round() as u32,
        ));
    }
    #[cfg(not(windows))]
    {
        let _ = window.center();
    }
}

#[cfg(test)]
mod tests {
    use super::{LauncherFile, query, usable_hotkey};

    #[test]
    fn a_launcher_hotkey_needs_a_modifier() {
        assert_eq!(usable_hotkey("win+space").as_deref(), Some("Win+Space"));
        assert_eq!(usable_hotkey("alt+space").as_deref(), Some("Alt+Space"));
        assert_eq!(usable_hotkey("Space"), None);
        assert_eq!(usable_hotkey("shift+a"), None);
        assert_eq!(usable_hotkey("nonsense"), None);
    }

    #[test]
    fn a_missing_or_bad_hotkey_falls_back_to_win_space() {
        let mut file: LauncherFile = serde_json::from_str("{}").unwrap();
        file.normalize();
        assert!(file.enabled);
        assert_eq!(file.hotkey, "Win+Space");
        let mut file: LauncherFile =
            serde_json::from_str(r#"{"enabled":false,"hotkey":"q"}"#).unwrap();
        file.normalize();
        assert!(!file.enabled);
        assert_eq!(file.hotkey, "Win+Space");
    }

    #[test]
    fn calculations_and_conversions_come_first() {
        let results = query("2 + 2");
        assert_eq!(results[0].kind, "calc");
        assert_eq!(results[0].title, "4");
        let results = query("5 km to mi");
        assert_eq!(results[0].kind, "unit");
        let results = query("folder");
        assert_eq!(results[0].id, "tool:folder-rules");
    }
}
