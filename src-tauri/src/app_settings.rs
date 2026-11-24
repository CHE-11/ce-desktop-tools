use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const POINTER_STORE: &str = "settings.json";
const POINTER_KEY: &str = "settingsFolder";
const SETTINGS_FILE_NAME: &str = "ce-desktop-tools.json";
const DEFAULT_ZOOM: u32 = 100;
const MIN_ZOOM: u32 = 60;
const MAX_ZOOM: u32 = 160;
const ZOOM_STEP: u32 = 10;
const DEFAULT_THEME: &str = "zinc";
const DEFAULT_COLOR_MODE: &str = "dark";
const DEFAULT_DENSITY: &str = "default";
const DENSITIES: &[&str] = &["cozy", "default", "compact"];
const THEMES: &[&str] = &[
    "zinc",
    "nord",
    "catppuccin",
    "solarized",
    "dracula",
    "gruvbox",
    "tokyo",
    "rosepine",
    "github",
    "onedark",
];
const THEME_CSS_FILE_NAME: &str = "ce-desktop-tools.css";
const MAX_THEME_CSS_BYTES: u64 = 256 * 1024;
const THEME_CSS_TEMPLATE: &str = "\
/*
ce-desktop-tools loads this file on top of the selected theme.
Uncomment a block, edit the colors, and save.
This file is read when ce-desktop-tools starts and when its window is focused.

Rules on html[data-theme][data-mode] override the built-in palette.
Shade numbers are roles: 950 is the page, 900 is a raised surface, 800 is hover,
700 is a border, 500 is muted text, and 100 is primary text.
sky is the main accent, violet is a second accent, emerald is success,
amber is a warning, and red is an error. 600 is the solid button color.
*/

/*
html[data-theme][data-mode=\"dark\"] {
  --app-zinc-950: #09090b;
  --app-zinc-900: #18181b;
  --app-zinc-800: #27272a;
  --app-zinc-700: #3f3f46;
  --app-zinc-500: #71717a;
  --app-zinc-300: #d4d4d8;
  --app-zinc-100: #f4f4f5;
  --app-sky-400: #38bdf8;
  --app-sky-500: #0ea5e9;
  --app-sky-600: #0284c7;
  --app-violet-300: #c4b5fd;
  --app-violet-600: #7c3aed;
  --app-emerald-400: #34d399;
  --app-emerald-600: #059669;
  --app-amber-300: #fcd34d;
  --app-red-400: #f87171;
  --app-red-600: #dc2626;
}
*/

/*
html[data-theme][data-mode=\"light\"] {
  --app-zinc-950: #fafafa;
  --app-zinc-900: #f4f4f5;
  --app-zinc-800: #e4e4e7;
  --app-zinc-700: #d4d4d8;
  --app-zinc-500: #71717a;
  --app-zinc-300: #3f3f46;
  --app-zinc-100: #18181b;
  --app-sky-400: #0369a1;
  --app-sky-500: #0284c7;
  --app-sky-600: #0284c7;
  --app-violet-300: #6d28d9;
  --app-violet-600: #7c3aed;
  --app-emerald-400: #059669;
  --app-emerald-600: #059669;
  --app-amber-300: #b45309;
  --app-red-400: #dc2626;
  --app-red-600: #dc2626;
}
*/
";
const COLOR_MODES: &[&str] = &["dark", "light", "system"];

static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub settings_folder: String,
    pub settings_file: String,
    pub shared: bool,
    pub warning: Option<String>,
    pub font_zoom: u32,
    pub density: String,
    pub theme: String,
    pub color_mode: String,
    pub toolbar_labels: bool,
    pub run_at_startup: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettingsPatch {
    pub font_zoom: Option<u32>,
    #[serde(default)]
    pub density: Option<String>,
}

impl Default for AppSettingsPatch {
    fn default() -> Self {
        Self {
            font_zoom: None,
            density: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PortableFile {
    font_zoom: u32,
    #[serde(default)]
    density: String,
    theme: String,
    color_mode: String,
    toolbar_labels: bool,
    run_at_startup: bool,
    #[serde(default)]
    pantherbar: crate::pantherbar::PantherbarFile,
    #[serde(default)]
    launcher: crate::launcher::LauncherFile,
    #[serde(default)]
    terminal: TerminalSettings,
}

const TERMINAL_SHELLS: &[&str] = &["cmd", "powershell", "pwsh"];
const DEFAULT_TERMINAL_FONT: u32 = 14;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalSettings {
    pub default_shell: String,
    pub copy_on_select: bool,
    pub confirm_multiline_paste: bool,
    pub font_size: u32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            default_shell: "powershell".into(),
            copy_on_select: false,
            confirm_multiline_paste: true,
            font_size: DEFAULT_TERMINAL_FONT,
        }
    }
}

impl TerminalSettings {
    fn normalize(&mut self) {
        if !TERMINAL_SHELLS.contains(&self.default_shell.as_str()) {
            self.default_shell = TerminalSettings::default().default_shell;
        }
        if !(8..=32).contains(&self.font_size) {
            self.font_size = DEFAULT_TERMINAL_FONT;
        }
    }
}

impl Default for PortableFile {
    fn default() -> Self {
        Self {
            font_zoom: DEFAULT_ZOOM,
            density: DEFAULT_DENSITY.to_string(),
            theme: DEFAULT_THEME.to_string(),
            color_mode: DEFAULT_COLOR_MODE.to_string(),
            toolbar_labels: false,
            run_at_startup: false,
            pantherbar: crate::pantherbar::PantherbarFile::default(),
            launcher: crate::launcher::LauncherFile::default(),
            terminal: TerminalSettings::default(),
        }
    }
}

struct Loaded {
    folder: String,
    shared: bool,
    warning: Option<String>,
    file: PortableFile,
}

#[derive(Debug, PartialEq, Eq)]
enum FolderPlan {
    Local,
    Adopt,
    Write,
}

pub fn app_settings(app: &AppHandle) -> Result<AppSettings, String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        Ok(view_of(&loaded))
    })
}

pub fn settings_folder(app: &AppHandle) -> Result<String, String> {
    let settings = app_settings(app)?;
    if settings.shared {
        Ok(settings.settings_folder)
    } else {
        Ok(String::new())
    }
}

pub fn preview_app_settings(settings_folder: String) -> Result<AppSettings, String> {
    with_settings_lock(|| preview_folder(&settings_folder))
}

pub fn save_app_settings(
    app: &AppHandle,
    settings_folder: String,
    font_zoom: u32,
    theme: String,
    color_mode: String,
    toolbar_labels: bool,
    run_at_startup: bool,
) -> Result<AppSettings, String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        let next_folder = normalize_folder(&settings_folder);
        if !next_folder.is_empty() {
            validate_folder(&next_folder)?;
        }
        if loaded.warning.is_some()
            && same_folder(&loaded.folder, &next_folder)
            && settings_path(&next_folder).is_file()
        {
            return Err(loaded.warning.clone().unwrap_or_else(|| {
                "Couldn't read the settings file, so it was left unchanged.".into()
            }));
        }

        let zoom = clamp_zoom(font_zoom);
        let theme = normalize_theme(&theme);
        let color_mode = normalize_color_mode(&color_mode);
        let destination_exists = !next_folder.is_empty() && settings_path(&next_folder).is_file();
        let plan = folder_plan(&loaded.folder, &next_folder, destination_exists);

        let saved = match plan {
            FolderPlan::Local => {
                let mut file = loaded.file;
                file.font_zoom = zoom;
                file.theme = theme;
                file.color_mode = color_mode;
                file.toolbar_labels = toolbar_labels;
                file.run_at_startup = run_at_startup;
                write_local(app, &file)?;
                view_of(&Loaded {
                    folder: String::new(),
                    shared: false,
                    warning: None,
                    file,
                })
            }
            FolderPlan::Adopt => {
                let path = settings_path(&next_folder);
                let file = read_portable_file(&path)?.normalize();
                write_pointer(app, &next_folder)?;
                view_of(&Loaded {
                    folder: next_folder,
                    shared: true,
                    warning: None,
                    file,
                })
            }
            FolderPlan::Write => {
                let mut file = loaded.file;
                file.font_zoom = zoom;
                file.theme = theme;
                file.color_mode = color_mode;
                file.toolbar_labels = toolbar_labels;
                file.run_at_startup = run_at_startup;
                file.normalize_in_place();
                let path = settings_path(&next_folder);
                write_portable_file(&path, &file)?;
                write_pointer(app, &next_folder)?;
                view_of(&Loaded {
                    folder: next_folder,
                    shared: true,
                    warning: None,
                    file,
                })
            }
        };
        crate::startup::sync_run_at_startup(saved.run_at_startup)?;
        Ok(saved)
    })
}

pub fn patch_app_settings(app: &AppHandle, patch: AppSettingsPatch) -> Result<(), String> {
    if patch.font_zoom.is_none() && patch.density.is_none() {
        return Ok(());
    }
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        let zoom = patch.font_zoom.map(clamp_zoom);
        let density = patch.density.as_deref().map(normalize_density);
        if !loaded.shared {
            if let Some(zoom) = zoom {
                write_local_value(app, "fontZoom", json!(zoom))?;
            }
            if let Some(density) = density {
                write_local_value(app, "density", json!(density))?;
            }
            return Ok(());
        }
        let path = settings_path(&loaded.folder);
        let mut file = read_portable_file(&path)?.normalize();
        if let Some(zoom) = zoom {
            file.font_zoom = zoom;
        }
        if let Some(density) = &density {
            file.density = density.clone();
        }
        write_portable_file(&path, &file)
    })
}

fn with_settings_lock<T>(body: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = SETTINGS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    body()
}

fn preview_folder(settings_folder: &str) -> Result<AppSettings, String> {
    let folder = normalize_folder(settings_folder);
    if folder.is_empty() {
        return Err("Choose a folder.".into());
    }
    validate_folder(&folder)?;
    let path = settings_path(&folder);
    if !path.is_file() {
        return Ok(view_of(&Loaded {
            folder,
            shared: false,
            warning: None,
            file: PortableFile::default(),
        }));
    }
    match read_portable_file(&path) {
        Ok(file) => Ok(view_of(&Loaded {
            folder,
            shared: true,
            warning: None,
            file: file.normalize(),
        })),
        Err(error) => Ok(view_of(&Loaded {
            folder,
            shared: false,
            warning: Some(error),
            file: PortableFile::default(),
        })),
    }
}

fn load_locked(app: &AppHandle) -> Result<Loaded, String> {
    let folder = read_pointer(app)?;
    if folder.is_empty() {
        return Ok(Loaded {
            folder,
            shared: false,
            warning: None,
            file: read_local(app)?,
        });
    }

    let path = settings_path(&folder);
    if !path.is_file() {
        return Ok(Loaded {
            warning: Some(format!("Settings file not found: {}", path.display())),
            folder,
            shared: false,
            file: read_local(app)?,
        });
    }

    match read_portable_file(&path) {
        Ok(file) => Ok(Loaded {
            folder,
            shared: true,
            warning: None,
            file: file.normalize(),
        }),
        Err(error) => Ok(Loaded {
            folder,
            shared: false,
            warning: Some(error),
            file: read_local(app)?,
        }),
    }
}

fn view_of(loaded: &Loaded) -> AppSettings {
    let file = &loaded.file;
    AppSettings {
        settings_file: if loaded.folder.is_empty() {
            String::new()
        } else {
            settings_path(&loaded.folder).display().to_string()
        },
        settings_folder: loaded.folder.clone(),
        shared: loaded.shared,
        warning: loaded.warning.clone(),
        font_zoom: file.font_zoom,
        density: file.density.clone(),
        theme: file.theme.clone(),
        color_mode: file.color_mode.clone(),
        toolbar_labels: file.toolbar_labels,
        run_at_startup: file.run_at_startup,
    }
}

fn read_local(app: &AppHandle) -> Result<PortableFile, String> {
    let settings = app
        .store(POINTER_STORE)
        .map_err(|error| error.to_string())?;
    let mut file = PortableFile {
        font_zoom: json_u32(settings.get("fontZoom")).unwrap_or(DEFAULT_ZOOM),
        density: json_string(settings.get("density"))
            .unwrap_or_else(|| DEFAULT_DENSITY.to_string()),
        theme: json_string(settings.get("theme")).unwrap_or_else(|| DEFAULT_THEME.to_string()),
        color_mode: json_string(settings.get("colorMode"))
            .unwrap_or_else(|| DEFAULT_COLOR_MODE.to_string()),
        toolbar_labels: json_bool(settings.get("toolbarLabels")).unwrap_or(false),
        run_at_startup: json_bool(settings.get("runAtStartup")).unwrap_or(false),
        pantherbar: settings
            .get("pantherbar")
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
        launcher: settings
            .get("launcher")
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
        terminal: settings
            .get("terminal")
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
    };
    file.normalize_in_place();
    Ok(file)
}

fn write_local(app: &AppHandle, file: &PortableFile) -> Result<(), String> {
    let store = app
        .store(POINTER_STORE)
        .map_err(|error| error.to_string())?;
    store.delete("vaultPath");
    store.delete("alwaysOnTopHotkey");
    store.delete("hibernateHotkey");
    store.set("fontZoom", json!(file.font_zoom));
    store.set("density", json!(file.density));
    store.set("theme", json!(file.theme));
    store.set("colorMode", json!(file.color_mode));
    store.set("toolbarLabels", json!(file.toolbar_labels));
    store.set("runAtStartup", json!(file.run_at_startup));
    store.set("pantherbar", json!(file.pantherbar));
    store.set("launcher", json!(file.launcher));
    store.set("terminal", json!(file.terminal));
    store.set(POINTER_KEY, "");
    store.save().map_err(|error| error.to_string())
}

fn write_local_value(app: &AppHandle, key: &str, value: JsonValue) -> Result<(), String> {
    let store = app
        .store(POINTER_STORE)
        .map_err(|error| error.to_string())?;
    store.set(key, value);
    store.save().map_err(|error| error.to_string())
}

fn write_pointer(app: &AppHandle, folder: &str) -> Result<(), String> {
    write_local_value(app, POINTER_KEY, json!(folder))
}

fn read_pointer(app: &AppHandle) -> Result<String, String> {
    let store = app
        .store(POINTER_STORE)
        .map_err(|error| error.to_string())?;
    Ok(json_string(store.get(POINTER_KEY))
        .map(|value| normalize_folder(&value))
        .unwrap_or_default())
}

fn settings_path(folder: &str) -> PathBuf {
    Path::new(folder).join(SETTINGS_FILE_NAME)
}

pub fn theme_css(app: &AppHandle) -> Result<String, String> {
    with_settings_lock(|| {
        let folder = read_pointer(app)?;
        read_theme_css_file(&folder)
    })
}

fn theme_css_path(folder: &str) -> PathBuf {
    Path::new(folder).join(THEME_CSS_FILE_NAME)
}

fn ensure_theme_css_file(folder: &str) -> Result<(), String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Ok(());
    }
    let path = theme_css_path(folder);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => file
            .write_all(THEME_CSS_TEMPLATE.as_bytes())
            .map_err(|error| {
                let _ = fs::remove_file(&path);
                format!("Couldn't create {}: {error}", path.display())
            }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!("Couldn't create {}: {error}", path.display())),
    }
}

fn read_theme_css_file(folder: &str) -> Result<String, String> {
    if folder.is_empty() {
        return Ok(String::new());
    }
    ensure_theme_css_file(folder)?;
    let path = theme_css_path(folder);
    if !path.is_file() {
        return Ok(String::new());
    }
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    if metadata.len() > MAX_THEME_CSS_BYTES {
        return Err(format!(
            "{} is larger than 256 KB, so it was not loaded.",
            path.display()
        ));
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    if bytes.contains(&0) {
        return Err(format!("{} is not a text stylesheet.", path.display()));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8.", path.display()))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

fn read_portable_file(path: &Path) -> Result<PortableFile, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{} is not valid ce-desktop-tools settings: {error}",
            path.display()
        )
    })
}

fn write_portable_file(path: &Path, file: &PortableFile) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(file)
        .map_err(|error| format!("Couldn't save settings: {error}"))?;
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

fn validate_folder(folder: &str) -> Result<(), String> {
    let path = Path::new(folder);
    if path.is_dir() {
        return Ok(());
    }
    if path.exists() {
        return Err(format!("{folder} is not a folder."));
    }
    Err(format!("Folder does not exist: {folder}"))
}

fn json_string(value: Option<JsonValue>) -> Option<String> {
    let text = value?.as_str()?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn json_u32(value: Option<JsonValue>) -> Option<u32> {
    let number = value?.as_u64()?;
    u32::try_from(number).ok()
}

fn json_bool(value: Option<JsonValue>) -> Option<bool> {
    value?.as_bool()
}

impl PortableFile {
    fn normalize(mut self) -> Self {
        self.normalize_in_place();
        self
    }

    fn normalize_in_place(&mut self) {
        self.font_zoom = clamp_zoom(self.font_zoom);
        self.density = normalize_density(&self.density);
        self.theme = normalize_theme(&self.theme);
        self.color_mode = normalize_color_mode(&self.color_mode);
        self.pantherbar.normalize();
        self.launcher.normalize();
        self.terminal.normalize();
    }
}

pub fn load_terminal(app: &AppHandle) -> Result<TerminalSettings, String> {
    with_settings_lock(|| {
        let mut settings = load_locked(app)?.file.terminal;
        settings.normalize();
        Ok(settings)
    })
}

pub fn save_terminal(
    app: &AppHandle,
    mut next: TerminalSettings,
) -> Result<TerminalSettings, String> {
    next.normalize();
    let saved = next.clone();
    save_section(app, move |file| file.terminal = next)?;
    Ok(saved)
}

pub fn load_launcher(app: &AppHandle) -> Result<crate::launcher::LauncherFile, String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        let mut file = loaded.file.launcher;
        file.normalize();
        Ok(file)
    })
}

pub fn save_launcher(
    app: &AppHandle,
    mut next: crate::launcher::LauncherFile,
) -> Result<crate::launcher::LauncherFile, String> {
    next.normalize();
    let saved = next.clone();
    save_section(app, move |file| file.launcher = next)?;
    Ok(saved)
}

/// Writes one section of the settings to wherever settings live now, and
/// keeps every other value already in the file.
fn save_section(app: &AppHandle, apply: impl FnOnce(&mut PortableFile)) -> Result<(), String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        if loaded.warning.is_some()
            && !loaded.folder.is_empty()
            && settings_path(&loaded.folder).is_file()
        {
            return Err(loaded.warning.unwrap_or_else(|| {
                "Couldn't read the settings file, so it was left unchanged.".into()
            }));
        }
        if loaded.shared {
            let path = settings_path(&loaded.folder);
            let mut file = read_portable_file(&path)?.normalize();
            apply(&mut file);
            file.normalize_in_place();
            return write_portable_file(&path, &file);
        }
        let mut file = loaded.file;
        apply(&mut file);
        file.normalize_in_place();
        if loaded.folder.is_empty() {
            return write_local(app, &file);
        }
        write_portable_file(&settings_path(&loaded.folder), &file)
    })
}

pub fn load_pantherbar(app: &AppHandle) -> Result<crate::pantherbar::PantherbarFile, String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        let mut file = loaded.file.pantherbar;
        file.normalize();
        Ok(file)
    })
}

pub fn save_pantherbar(
    app: &AppHandle,
    next: crate::pantherbar::PantherbarFile,
) -> Result<crate::pantherbar::PantherbarFile, String> {
    with_settings_lock(|| {
        let loaded = load_locked(app)?;
        if loaded.warning.is_some()
            && !loaded.folder.is_empty()
            && settings_path(&loaded.folder).is_file()
        {
            return Err(loaded.warning.unwrap_or_else(|| {
                "Couldn't read the settings file, so it was left unchanged.".into()
            }));
        }
        let next = next.prepare()?;
        if loaded.shared {
            let path = settings_path(&loaded.folder);
            let mut file = read_portable_file(&path)?.normalize();
            file.pantherbar = next.clone();
            write_portable_file(&path, &file)?;
            return Ok(next);
        }
        if loaded.folder.is_empty() {
            let mut file = loaded.file;
            file.pantherbar = next.clone();
            write_local(app, &file)?;
            return Ok(next);
        }
        let mut file = loaded.file;
        file.pantherbar = next.clone();
        file.normalize_in_place();
        write_portable_file(&settings_path(&loaded.folder), &file)?;
        Ok(next)
    })
}

fn normalize_theme(value: &str) -> String {
    let value = value.trim();
    if THEMES.contains(&value) {
        value.to_string()
    } else {
        DEFAULT_THEME.to_string()
    }
}

fn normalize_density(value: &str) -> String {
    let value = value.trim();
    if DENSITIES.contains(&value) {
        value.to_string()
    } else {
        DEFAULT_DENSITY.to_string()
    }
}

fn normalize_color_mode(value: &str) -> String {
    let value = value.trim();
    if COLOR_MODES.contains(&value) {
        value.to_string()
    } else {
        DEFAULT_COLOR_MODE.to_string()
    }
}

fn clamp_zoom(value: u32) -> u32 {
    if value == 0 {
        return DEFAULT_ZOOM;
    }
    let stepped = value.saturating_add(ZOOM_STEP / 2) / ZOOM_STEP * ZOOM_STEP;
    stepped.clamp(MIN_ZOOM, MAX_ZOOM)
}

fn normalize_folder(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let stripped = trimmed.trim_end_matches(['/', '\\']);
    if stripped.is_empty() {
        return trimmed.to_string();
    }
    let bytes = stripped.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return format!("{stripped}\\");
    }
    stripped.to_string()
}

fn same_folder(left: &str, right: &str) -> bool {
    normalize_folder(left).eq_ignore_ascii_case(&normalize_folder(right))
}

fn folder_plan(previous: &str, next: &str, destination_exists: bool) -> FolderPlan {
    if next.is_empty() {
        return FolderPlan::Local;
    }
    if !same_folder(previous, next) && destination_exists {
        return FolderPlan::Adopt;
    }
    FolderPlan::Write
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_ZOOM, FolderPlan, PortableFile, clamp_zoom, folder_plan, normalize_folder,
        same_folder, write_portable_file,
    };

    #[test]
    fn zoom_stays_on_the_ten_percent_steps() {
        assert_eq!(clamp_zoom(0), DEFAULT_ZOOM);
        assert_eq!(clamp_zoom(50), 60);
        assert_eq!(clamp_zoom(60), 60);
        assert_eq!(clamp_zoom(80), 80);
        assert_eq!(clamp_zoom(84), 80);
        assert_eq!(clamp_zoom(85), 90);
        assert_eq!(clamp_zoom(100), 100);
        assert_eq!(clamp_zoom(160), 160);
        assert_eq!(clamp_zoom(1000), 160);
    }

    #[test]
    fn folder_paths_ignore_case_and_a_trailing_slash() {
        assert!(same_folder(r"C:\Settings", r"c:\settings\"));
        assert!(same_folder(r"\\server\share\app", r"\\server\share\app\"));
        assert!(same_folder(r"C:\", r"C:"));
        assert!(!same_folder(r"C:\Settings", r"C:\Settings2"));
        assert_eq!(normalize_folder("  "), "");
    }

    #[test]
    fn preview_reads_an_existing_settings_file() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-preview-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("ce-desktop-tools.json");
        std::fs::write(
            &path,
            br#"{"rootPaths":["C:\\Repos"],"fontZoom":130,"theme":"nord","colorMode":"light"}"#,
        )
        .unwrap();

        let preview = super::preview_folder(&directory.to_string_lossy()).unwrap();
        assert!(preview.shared);
        assert_eq!(preview.font_zoom, 130);
        assert_eq!(preview.theme, "nord");
        assert_eq!(preview.color_mode, "light");
        assert!(preview.settings_file.ends_with("ce-desktop-tools.json"));
        assert_eq!(
            preview.settings_folder,
            super::normalize_folder(&directory.to_string_lossy())
        );

        let missing = directory.join("missing-folder");
        assert!(super::preview_folder(&missing.to_string_lossy()).is_err());

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn an_existing_file_in_a_new_folder_is_adopted() {
        assert_eq!(folder_plan("", "", false), FolderPlan::Local);
        assert_eq!(folder_plan(r"C:\Old", "", true), FolderPlan::Local);
        assert_eq!(
            folder_plan(r"C:\Settings\", r"C:\Settings", true),
            FolderPlan::Write
        );
        assert_eq!(
            folder_plan(r"C:\Old", r"D:\Shared", true),
            FolderPlan::Adopt
        );
        assert_eq!(
            folder_plan(r"C:\Old", r"D:\Shared", false),
            FolderPlan::Write
        );
    }

    #[test]
    fn an_old_git_settings_file_loads_and_saves_the_current_shape() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-settings-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("ce-desktop-tools.json");
        std::fs::write(
            &path,
            br#"{"rootPaths":["\\\\server\\share\\repos"],"vaultPath":"D:\\OldVault","fontZoom":130,"commitVerbosity":"descriptive","model":"gpt-test","alwaysOnTopHotkey":"Ctrl+Space","hibernateHotkey":"Shift+F1"}"#,
        )
        .unwrap();

        let loaded: PortableFile = super::read_portable_file(&path).unwrap().normalize();
        assert_eq!(loaded.font_zoom, 130);
        assert_eq!(loaded.density, "default");
        assert_eq!(loaded.theme, "zinc");
        assert_eq!(loaded.color_mode, "dark");
        assert!(!loaded.toolbar_labels);
        assert!(!loaded.run_at_startup);
        assert!(!loaded.pantherbar.enabled);
        assert!(loaded.pantherbar.installed.is_none());
        assert!(loaded.pantherbar.custom.is_empty());
        assert!(loaded.pantherbar.excluded_apps.is_empty());
        assert!(loaded.launcher.enabled);
        assert_eq!(loaded.launcher.hotkey, "Win+Space");

        let mut next = loaded;
        next.theme = "nord".into();
        next.color_mode = "light".into();
        next.toolbar_labels = true;
        next.run_at_startup = true;
        next.normalize_in_place();
        write_portable_file(&path, &next).unwrap();
        let saved: PortableFile = super::read_portable_file(&path).unwrap().normalize();
        assert_eq!(saved.font_zoom, 130);
        assert_eq!(saved.density, "default");
        assert_eq!(saved.theme, "nord");
        assert_eq!(saved.color_mode, "light");
        assert!(saved.toolbar_labels);
        assert!(saved.run_at_startup);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("vaultPath"));
        assert!(!raw.contains("rootPaths"));
        assert!(!raw.contains("commitVerbosity"));
        assert!(raw.contains("density"));
        assert!(raw.contains("runAtStartup"));
        assert!(raw.contains("\"launcher\""));
        assert_eq!(saved.launcher.hotkey, "Win+Space");
        assert!(!raw.contains("alwaysOnTopHotkey"));
        assert!(!raw.contains("hibernateHotkey"));

        let mut kept = saved.clone();
        kept.density = "compact".into();
        assert_eq!(kept.normalize().density, "compact");

        let mut invalid = saved;
        invalid.theme = "nope".into();
        invalid.color_mode = "sepia".into();
        invalid.density = "roomy".into();
        let normalized = invalid.normalize();
        assert_eq!(normalized.theme, "zinc");
        assert_eq!(normalized.color_mode, "dark");
        assert_eq!(normalized.density, "default");

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn known_themes_are_kept_and_unknown_themes_fall_back() {
        for theme in [
            "zinc",
            "nord",
            "catppuccin",
            "solarized",
            "dracula",
            "gruvbox",
            "tokyo",
            "rosepine",
            "github",
            "onedark",
        ] {
            let mut file = PortableFile::default();
            file.theme = theme.into();
            assert_eq!(file.normalize().theme, theme);
        }
    }

    #[test]
    fn theme_css_reads_a_stylesheet_from_the_settings_folder() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-theme-css-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let folder = directory.to_string_lossy().to_string();

        assert_eq!(super::read_theme_css_file("").unwrap(), "");
        let missing = directory.join("missing");
        assert_eq!(
            super::read_theme_css_file(&missing.to_string_lossy()).unwrap(),
            ""
        );
        assert!(!missing.join("ce-desktop-tools.css").exists());

        let created = super::read_theme_css_file(&folder).unwrap();
        assert_eq!(created, super::THEME_CSS_TEMPLATE);
        assert_eq!(super::read_theme_css_file(&folder).unwrap(), created);

        let path = directory.join("ce-desktop-tools.css");
        std::fs::write(&path, "\u{feff}html { color: red; }").unwrap();
        assert_eq!(
            super::read_theme_css_file(&folder).unwrap(),
            "html { color: red; }"
        );

        std::fs::write(&path, b"html { color: red; }\0").unwrap();
        assert!(
            super::read_theme_css_file(&folder)
                .unwrap_err()
                .contains("not a text stylesheet")
        );

        std::fs::write(&path, vec![b'a'; (256 * 1024) + 1]).unwrap();
        assert!(
            super::read_theme_css_file(&folder)
                .unwrap_err()
                .contains("256 KB")
        );

        let _ = std::fs::remove_dir_all(&directory);
    }
}
