//! Shortcuts that run only while a chosen application is in front.
//!
//! They live in app-shortcuts.json beside the settings file. A missing file is seeded
//! with Excel's shortcut that replaces a formula with its displayed text.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

pub const FILE_NAME: &str = "app-shortcuts.json";
pub const EXCEL_ACTION: &str = "excel-formula-text";
pub const PASTE_ACTION: &str = "paste-plain";
const REVISION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    ExcelFormulaText,
    PastePlain,
}

fn enabled_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shortcut {
    pub id: String,
    pub title: String,
    pub hotkey: String,
    pub detail: String,
    pub action: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

impl Default for Shortcut {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            hotkey: String::new(),
            detail: String::new(),
            action: String::new(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutApp {
    pub id: String,
    pub name: String,
    pub exe: String,
    pub shortcuts: Vec<Shortcut>,
}

impl Default for ShortcutApp {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            exe: String::new(),
            shortcuts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ShortcutFile {
    apps: Vec<ShortcutApp>,
    revision: u32,
}

impl Default for ShortcutFile {
    fn default() -> Self {
        Self {
            apps: Vec::new(),
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppShortcutState {
    pub settings_folder: String,
    pub apps: Vec<ShortcutApp>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
struct Armed {
    exe: String,
    vk: u16,
    ctrl: bool,
    shift: bool,
    alt: bool,
    win: bool,
    action: ShortcutAction,
}

static ARMED: Mutex<Vec<Armed>> = Mutex::new(Vec::new());

pub struct AppShortcuts;

impl AppShortcuts {
    pub fn start(app: &AppHandle) -> Self {
        let folder = folder_of(app);
        match load(&folder) {
            Ok(apps) => publish(&apps),
            Err(_) => publish(&seed()),
        }
        Self
    }

    pub fn reload(app: &AppHandle) {
        let folder = folder_of(app);
        if let Ok(apps) = load(&folder) {
            publish(&apps);
        }
    }
}

pub fn state(app: &AppHandle) -> AppShortcutState {
    let folder = folder_of(app);
    match load(&folder) {
        Ok(apps) => AppShortcutState {
            settings_folder: folder,
            apps,
            error: None,
        },
        Err(error) => AppShortcutState {
            settings_folder: folder,
            apps: seed(),
            error: Some(error),
        },
    }
}

fn folder_of(app: &AppHandle) -> String {
    crate::app_settings::settings_folder(app).unwrap_or_default()
}

pub fn save(app: &AppHandle, apps: Vec<ShortcutApp>) -> Result<AppShortcutState, String> {
    let folder = folder_of(app);
    if folder.is_empty() || !Path::new(&folder).is_dir() {
        return Err(
            "Choose a settings folder. app-shortcuts.json is saved there with the settings file."
                .into(),
        );
    }
    let apps = normalize(apps)?;
    write_file(&file_path(&folder), &apps)?;
    publish(&apps);
    Ok(AppShortcutState {
        settings_folder: folder,
        apps,
        error: None,
    })
}

pub fn seed() -> Vec<ShortcutApp> {
    vec![system_app(), excel_app()]
}

fn system_app() -> ShortcutApp {
    ShortcutApp {
        id: "system".into(),
        name: "System".into(),
        exe: String::new(),
        shortcuts: vec![Shortcut {
            id: "paste-plain".into(),
            title: "Paste as plain text".into(),
            hotkey: "Ctrl+Shift+V".into(),
            detail: "Pastes the clipboard as plain text into the active window. Formatting is removed for the paste, and the original clipboard returns a second later.".into(),
            action: PASTE_ACTION.into(),
            enabled: true,
        }],
    }
}

fn excel_app() -> ShortcutApp {
    ShortcutApp {
        id: "excel".into(),
        name: "Excel".into(),
        exe: "EXCEL.EXE".into(),
        shortcuts: vec![Shortcut {
            id: "formula-text".into(),
            title: "Replace a formula with its text".into(),
            hotkey: "Ctrl+Shift+T".into(),
            detail: "Copies the displayed text from the selected cell, removes the formula, and pastes that text back into the same cell. This runs only while Excel is the active window.".into(),
            action: EXCEL_ACTION.into(),
            enabled: true,
        }],
    }
}

pub fn matching_action(vk: u16) -> Option<ShortcutAction> {
    let exe = crate::excel::foreground_exe_name().unwrap_or_default();
    let (ctrl, shift, alt, win) = crate::excel::modifier_state();
    action_for(vk, ctrl, shift, alt, win, &exe)
}

pub fn action_for(
    vk: u16,
    ctrl: bool,
    shift: bool,
    alt: bool,
    win: bool,
    exe: &str,
) -> Option<ShortcutAction> {
    let armed = ARMED.lock().ok()?;
    let matches = |shortcut: &&Armed| {
        shortcut.vk == vk
            && shortcut.ctrl == ctrl
            && shortcut.shift == shift
            && shortcut.alt == alt
            && shortcut.win == win
    };
    armed
        .iter()
        .find(|shortcut| {
            matches(shortcut) && !shortcut.exe.is_empty() && shortcut.exe.eq_ignore_ascii_case(exe)
        })
        .or_else(|| {
            armed
                .iter()
                .find(|shortcut| matches(shortcut) && shortcut.exe.is_empty())
        })
        .map(|shortcut| shortcut.action)
}

fn publish(apps: &[ShortcutApp]) {
    let mut armed = Vec::new();
    for app in apps {
        let exe = exe_name(&app.exe);
        for shortcut in &app.shortcuts {
            if !shortcut.enabled {
                continue;
            }
            let Some(action) = action_of(&shortcut.action) else {
                continue;
            };
            let Some(chord) = parse_hotkey(&shortcut.hotkey) else {
                continue;
            };
            armed.push(Armed {
                exe: exe.clone(),
                vk: chord.vk,
                ctrl: chord.ctrl,
                shift: chord.shift,
                alt: chord.alt,
                win: chord.win,
                action,
            });
        }
    }
    if let Ok(mut slot) = ARMED.lock() {
        *slot = armed;
    }
}

fn action_of(value: &str) -> Option<ShortcutAction> {
    if value == EXCEL_ACTION {
        Some(ShortcutAction::ExcelFormulaText)
    } else if value == PASTE_ACTION {
        Some(ShortcutAction::PastePlain)
    } else {
        None
    }
}

pub fn load(folder: &str) -> Result<Vec<ShortcutApp>, String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Ok(seed());
    }
    let path = file_path(folder);
    if !path.is_file() {
        let apps = seed();
        write_file(&path, &apps)?;
        return Ok(apps);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let file: ShortcutFile = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{} is not valid shortcut data, so it was left unchanged: {error}",
            path.display()
        )
    })?;
    Ok(migrate(sanitize(file.apps), file.revision))
}

fn migrate(mut apps: Vec<ShortcutApp>, revision: u32) -> Vec<ShortcutApp> {
    if revision >= REVISION {
        return apps;
    }
    let has_paste = apps.iter().any(|app| {
        app.shortcuts
            .iter()
            .any(|shortcut| shortcut.id == "paste-plain" || shortcut.action == PASTE_ACTION)
    });
    if !has_paste {
        apps.insert(0, system_app());
    }
    apps
}

fn file_path(folder: &str) -> PathBuf {
    Path::new(folder).join(FILE_NAME)
}

fn sanitize(apps: Vec<ShortcutApp>) -> Vec<ShortcutApp> {
    let mut next = Vec::new();
    for app in apps {
        let name = app.name.trim().to_string();
        let exe = exe_name(&app.exe);
        if name.is_empty() {
            continue;
        }
        let mut shortcuts = Vec::new();
        for shortcut in app.shortcuts {
            let hotkey = shortcut.hotkey.trim().to_string();
            let title = shortcut.title.trim().to_string();
            if title.is_empty() && hotkey.is_empty() {
                continue;
            }
            let hotkey = parse_hotkey(&hotkey)
                .map(|chord| chord.label)
                .unwrap_or(hotkey);
            shortcuts.push(Shortcut {
                id: filled_id(&shortcut.id),
                title,
                hotkey,
                detail: shortcut.detail.replace("\r\n", "\n").replace('\r', "\n"),
                action: shortcut.action.trim().to_string(),
                enabled: shortcut.enabled,
            });
        }
        next.push(ShortcutApp {
            id: filled_id(&app.id),
            name,
            exe,
            shortcuts,
        });
    }
    next
}

fn normalize(apps: Vec<ShortcutApp>) -> Result<Vec<ShortcutApp>, String> {
    let mut next = Vec::new();
    for app in apps {
        let name = app.name.trim().to_string();
        if name.is_empty() {
            return Err("An application name is required.".into());
        }
        let exe = exe_name(&app.exe);
        let mut seen = std::collections::BTreeSet::new();
        let mut shortcuts = Vec::new();
        for shortcut in app.shortcuts {
            let title = shortcut.title.trim().to_string();
            let hotkey = shortcut.hotkey.trim().to_string();
            if title.is_empty() && hotkey.is_empty() && shortcut.detail.trim().is_empty() {
                continue;
            }
            if title.is_empty() {
                return Err("A shortcut name is required.".into());
            }
            let hotkey = if hotkey.is_empty() {
                String::new()
            } else {
                let Some(chord) = parse_hotkey(&hotkey) else {
                    return Err(format!(
                        "\"{hotkey}\" is not a shortcut. Use a form like Ctrl+Shift+T."
                    ));
                };
                if !seen.insert(chord.label.clone()) {
                    return Err(format!("{name} already uses {}.", chord.label));
                }
                chord.label
            };
            shortcuts.push(Shortcut {
                id: filled_id(&shortcut.id),
                title,
                hotkey,
                detail: shortcut.detail.replace("\r\n", "\n").replace('\r', "\n"),
                action: shortcut.action.trim().to_string(),
                enabled: shortcut.enabled,
            });
        }
        next.push(ShortcutApp {
            id: filled_id(&app.id),
            name,
            exe,
            shortcuts,
        });
    }
    Ok(next)
}

fn exe_name(value: &str) -> String {
    value
        .trim()
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn filled_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        new_id()
    } else {
        trimmed.to_string()
    }
}

fn new_id() -> String {
    static IDS: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tick = IDS.fetch_add(1, Ordering::Relaxed);
    format!("s{nanos:x}{tick:x}")
}

struct Chord {
    vk: u16,
    ctrl: bool,
    shift: bool,
    alt: bool,
    win: bool,
    label: String,
}

fn parse_hotkey(text: &str) -> Option<Chord> {
    let parts: Vec<&str> = text
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let (key, modifiers) = parts.split_last()?;
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "win" | "meta" | "windows" => win = true,
            _ => return None,
        }
    }
    let (vk, key_label) = key_vk(key)?;
    let mut label = Vec::new();
    if ctrl {
        label.push("Ctrl");
    }
    if shift {
        label.push("Shift");
    }
    if alt {
        label.push("Alt");
    }
    if win {
        label.push("Win");
    }
    label.push(key_label);
    Some(Chord {
        vk,
        ctrl,
        shift,
        alt,
        win,
        label: label.join("+"),
    })
}

fn key_vk(key: &str) -> Option<(u16, &'static str)> {
    let mut chars = key.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        let upper = first.to_ascii_uppercase();
        if upper.is_ascii_alphanumeric() {
            return Some((
                upper as u16,
                match upper {
                    'A'..='Z' | '0'..='9' => letter_label(upper),
                    _ => return None,
                },
            ));
        }
    }
    match key.to_ascii_lowercase().as_str() {
        "space" => Some((0x20, "Space")),
        "tab" => Some((0x09, "Tab")),
        "enter" | "return" => Some((0x0D, "Enter")),
        "esc" | "escape" => Some((0x1B, "Esc")),
        "left" => Some((0x25, "Left")),
        "up" => Some((0x26, "Up")),
        "right" => Some((0x27, "Right")),
        "down" => Some((0x28, "Down")),
        "f1" => Some((0x70, "F1")),
        "f2" => Some((0x71, "F2")),
        "f3" => Some((0x72, "F3")),
        "f4" => Some((0x73, "F4")),
        "f5" => Some((0x74, "F5")),
        "f6" => Some((0x75, "F6")),
        "f7" => Some((0x76, "F7")),
        "f8" => Some((0x77, "F8")),
        "f9" => Some((0x78, "F9")),
        "f10" => Some((0x79, "F10")),
        "f11" => Some((0x7A, "F11")),
        "f12" => Some((0x7B, "F12")),
        _ => None,
    }
}

fn letter_label(upper: char) -> &'static str {
    match upper {
        'A' => "A",
        'B' => "B",
        'C' => "C",
        'D' => "D",
        'E' => "E",
        'F' => "F",
        'G' => "G",
        'H' => "H",
        'I' => "I",
        'J' => "J",
        'K' => "K",
        'L' => "L",
        'M' => "M",
        'N' => "N",
        'O' => "O",
        'P' => "P",
        'Q' => "Q",
        'R' => "R",
        'S' => "S",
        'T' => "T",
        'U' => "U",
        'V' => "V",
        'W' => "W",
        'X' => "X",
        'Y' => "Y",
        'Z' => "Z",
        '0' => "0",
        '1' => "1",
        '2' => "2",
        '3' => "3",
        '4' => "4",
        '5' => "5",
        '6' => "6",
        '7' => "7",
        '8' => "8",
        '9' => "9",
        _ => "",
    }
}

fn write_file(path: &Path, apps: &[ShortcutApp]) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(&ShortcutFile {
        apps: apps.to_vec(),
        revision: REVISION,
    })
    .map_err(|error| format!("Couldn't save shortcuts: {error}"))?;
    payload.push(b'\n');
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &payload)
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if fs::rename(&temporary, path).is_ok() {
        return Ok(());
    }
    let wrote = fs::write(path, &payload);
    let _ = fs::remove_file(&temporary);
    wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{EXCEL_ACTION, action_for, load, parse_hotkey, seed};

    #[test]
    fn the_excel_shortcut_parses_to_ctrl_shift_t() {
        let chord = parse_hotkey("Ctrl+Shift+T").unwrap();
        assert_eq!(chord.vk, 0x54);
        assert!(chord.ctrl && chord.shift && !chord.alt && !chord.win);
        assert_eq!(chord.label, "Ctrl+Shift+T");
        assert!(parse_hotkey("not a shortcut").is_none());
    }

    #[test]
    fn the_seed_keeps_excel_in_excel_and_paste_everywhere() {
        let apps = seed();
        assert_eq!(apps[0].id, "system");
        assert!(apps[0].exe.is_empty());
        assert_eq!(apps[0].shortcuts[0].hotkey, "Ctrl+Shift+V");
        assert_eq!(apps[1].exe, "EXCEL.EXE");
        assert_eq!(apps[1].shortcuts[0].action, EXCEL_ACTION);
        super::publish(&apps);
        assert_eq!(
            action_for(0x54, true, true, false, false, "EXCEL.EXE"),
            Some(super::ShortcutAction::ExcelFormulaText)
        );
        assert_eq!(
            action_for(0x54, true, true, false, false, "NOTEPAD.EXE"),
            None
        );
        assert_eq!(
            action_for(0x56, true, true, false, false, "NOTEPAD.EXE"),
            Some(super::ShortcutAction::PastePlain)
        );
        assert_eq!(
            action_for(0x56, true, true, false, false, ""),
            Some(super::ShortcutAction::PastePlain)
        );
    }

    #[test]
    fn a_missing_file_is_seeded_and_a_current_empty_file_stays_empty() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-shortcuts-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let folder = directory.to_string_lossy().to_string();
        let loaded = load(&folder).unwrap();
        assert_eq!(loaded[0].id, "system");
        assert_eq!(loaded[1].id, "excel");
        assert!(directory.join("app-shortcuts.json").is_file());

        std::fs::write(
            directory.join("app-shortcuts.json"),
            br#"{"apps":[{"id":"excel","name":"Excel","exe":"EXCEL.EXE","shortcuts":[]}]}"#,
        )
        .unwrap();
        let migrated = load(&folder).unwrap();
        assert_eq!(migrated[0].id, "system");
        assert!(migrated.iter().any(|app| app.id == "excel"));

        std::fs::write(
            directory.join("app-shortcuts.json"),
            br#"{"revision":1,"apps":[]}"#,
        )
        .unwrap();
        assert!(load(&folder).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&directory);
    }
}
