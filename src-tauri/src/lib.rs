mod always_on_top;
mod app_calls;
mod app_settings;
mod app_shortcuts;
mod ascii;
mod clipboard;
mod color_picker;
mod date_expand;
mod desktop_app_details;
mod desktop_apps;
mod dev_feedback;
mod excel;
mod expansion;
mod explorer_restart;
mod file_associations;
mod folder_rules;
mod hotkeys;
mod launcher;
mod megaclip_script;
mod misspell;
mod overlay;
mod pantherbar;
mod paste_plain;
mod processes;
mod rename;
mod screen_capture;
mod screen_ruler;
mod startup;
mod stay_awake;
mod taskbar;
mod terminal;
mod update;
mod vault_crypto;
mod window_pin;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use dev_feedback::{dev_feedback_probe, dev_feedback_push};
use expansion::{Expansion, ExpansionState, Snippet};

#[tauri::command]
fn app_settings(app: tauri::AppHandle) -> Result<app_settings::AppSettings, String> {
    app_settings::app_settings(&app)
}

#[tauri::command]
fn save_app_settings(
    app: tauri::AppHandle,
    settings_folder: String,
    font_zoom: u32,
    density: String,
    theme: String,
    color_mode: String,
    toolbar_labels: bool,
    run_at_startup: bool,
) -> Result<app_settings::AppSettings, String> {
    let saved = app_settings::save_app_settings(
        &app,
        settings_folder,
        font_zoom,
        density,
        theme,
        color_mode,
        toolbar_labels,
        run_at_startup,
    )?;
    if let Some(expansion) = app.try_state::<Expansion>() {
        expansion.load_from_settings();
    }
    app_shortcuts::AppShortcuts::reload(&app);
    if let Some(launcher) = app.try_state::<launcher::Launcher>() {
        launcher.reload();
    }
    if let Some(rules) = app.try_state::<folder_rules::FolderRules>() {
        rules.reload();
    }
    if let Some(pin) = app.try_state::<always_on_top::AlwaysOnTop>() {
        pin.reload();
    }
    if let Some(ruler) = app.try_state::<screen_ruler::ScreenRuler>() {
        ruler.reload();
    }
    if let Some(picker) = app.try_state::<color_picker::ColorPicker>() {
        picker.reload();
    }
    Ok(saved)
}

#[tauri::command]
fn hotkeys_recording(on: bool) {
    hotkeys::set_recording(on);
}

#[tauri::command]
fn stay_awake_state() -> bool {
    stay_awake::is_on()
}

#[tauri::command]
fn set_stay_awake(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    stay_awake::set(&app, on)
}

#[tauri::command]
fn always_on_top_state(
    pin: tauri::State<'_, always_on_top::AlwaysOnTop>,
) -> always_on_top::AlwaysOnTopState {
    pin.state()
}

#[tauri::command]
fn always_on_top_save(
    pin: tauri::State<'_, always_on_top::AlwaysOnTop>,
    settings: always_on_top::AlwaysOnTopFile,
) -> Result<always_on_top::AlwaysOnTopState, String> {
    pin.save(settings)
}

#[tauri::command]
fn always_on_top_unpin_all(
    pin: tauri::State<'_, always_on_top::AlwaysOnTop>,
) -> always_on_top::AlwaysOnTopState {
    always_on_top::unpin_all();
    pin.state()
}

#[tauri::command]
fn screen_ruler_state(
    ruler: tauri::State<'_, screen_ruler::ScreenRuler>,
) -> screen_ruler::ScreenRulerFile {
    ruler.state()
}

#[tauri::command]
fn screen_ruler_save(
    ruler: tauri::State<'_, screen_ruler::ScreenRuler>,
    settings: screen_ruler::ScreenRulerFile,
) -> Result<screen_ruler::ScreenRulerFile, String> {
    ruler.save(settings)
}

#[tauri::command]
fn screen_ruler_style() -> String {
    screen_ruler::style()
}

#[tauri::command]
fn screen_ruler_measure(app: tauri::AppHandle, style: String) {
    screen_ruler::measure(&app, &style);
}

#[tauri::command]
async fn screen_ruler_open(app: tauri::AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || screen_ruler::open(&app)).await;
}

#[tauri::command]
fn screen_ruler_close(app: tauri::AppHandle) {
    screen_ruler::close(&app);
}

#[tauri::command]
fn color_picker_state(
    picker: tauri::State<'_, color_picker::ColorPicker>,
) -> color_picker::ColorPickerState {
    picker.state()
}

#[tauri::command]
fn color_picker_save(
    picker: tauri::State<'_, color_picker::ColorPicker>,
    settings: color_picker::ColorPickerFile,
) -> Result<color_picker::ColorPickerState, String> {
    picker.save(settings)
}

#[tauri::command]
fn color_picker_describe(color: String) -> Result<color_picker::ColorDescription, String> {
    color_picker::describe(&color)
}

#[tauri::command]
fn color_picker_render(template: String, color: String) -> Result<String, String> {
    color_picker::render(&template, &color)
}

#[tauri::command]
fn color_picker_set_history(
    app: tauri::AppHandle,
    history: Vec<String>,
) -> Result<Vec<String>, String> {
    color_picker::set_history(&app, history)
}

#[tauri::command]
fn color_picker_pick(app: tauri::AppHandle) {
    color_picker::pick_from_editor(&app);
}

#[tauri::command]
async fn color_picker_open_editor(app: tauri::AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || color_picker::open_editor(&app)).await;
}

#[tauri::command]
fn preview_app_settings(settings_folder: String) -> Result<app_settings::AppSettings, String> {
    app_settings::preview_app_settings(settings_folder)
}

#[tauri::command]
fn patch_app_settings(
    app: tauri::AppHandle,
    patch: app_settings::AppSettingsPatch,
) -> Result<(), String> {
    app_settings::patch_app_settings(&app, patch)
}

#[tauri::command]
fn theme_css(app: tauri::AppHandle) -> Result<String, String> {
    app_settings::theme_css(&app)
}

#[tauri::command]
fn always_on_top(app: tauri::AppHandle) -> bool {
    window_pin::current(&app)
}

#[tauri::command]
fn set_always_on_top(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    window_pin::set(&app, on)
}

#[tauri::command]
async fn app_release() -> Result<update::AppRelease, String> {
    update::app_release().await
}

#[tauri::command]
fn app_executable() -> Result<String, String> {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .map_err(|error| format!("Couldn't find this application: {error}"))
}

#[tauri::command]
fn megaclip_running() -> bool {
    megaclip_script::running()
}

#[tauri::command]
fn run_megaclip() -> Result<(), String> {
    megaclip_script::start()
}

#[tauri::command]
fn stop_megaclip() -> Result<(), String> {
    megaclip_script::stop()
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("That address needs to start with http:// or https://.".into());
    }
    // explorer.exe treats a search URL as a folder path. ShellExecute opens the browser.
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{PCWSTR, w};

        let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let code = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = code.0 as isize;
        if code > 32 {
            return Ok(());
        }
        return Err(format!("Couldn't open that link (Windows error {code})."));
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Opening a link only works on Windows.".into())
    }
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("There is no file to show.".into());
    }
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Couldn't open File Explorer: {error}"))
}

#[tauri::command]
fn expansion_state(expansion: tauri::State<'_, Expansion>) -> ExpansionState {
    expansion.state()
}

#[tauri::command]
fn save_expansions(
    expansion: tauri::State<'_, Expansion>,
    snippets: Vec<Snippet>,
) -> Result<ExpansionState, String> {
    expansion.save(snippets)
}

#[tauri::command]
async fn create_vault_pin(
    expansion: tauri::State<'_, Expansion>,
    pin: String,
) -> Result<ExpansionState, String> {
    let expansion = expansion.inner().clone();
    let state = tauri::async_runtime::spawn_blocking(move || expansion.create_pin(pin))
        .await
        .map_err(|error| error.to_string())??;
    Ok(state)
}

#[tauri::command]
async fn change_vault_pin(
    expansion: tauri::State<'_, Expansion>,
    current: String,
    next: String,
) -> Result<ExpansionState, String> {
    let expansion = expansion.inner().clone();
    let state = tauri::async_runtime::spawn_blocking(move || expansion.change_pin(current, next))
        .await
        .map_err(|error| error.to_string())??;
    Ok(state)
}

#[tauri::command]
async fn unlock_vault(
    expansion: tauri::State<'_, Expansion>,
    pin: String,
) -> Result<ExpansionState, String> {
    let expansion = expansion.inner().clone();
    let state = tauri::async_runtime::spawn_blocking(move || expansion.unlock(pin))
        .await
        .map_err(|error| error.to_string())??;
    Ok(state)
}

#[tauri::command]
fn lock_vault(expansion: tauri::State<'_, Expansion>) -> ExpansionState {
    expansion.lock()
}

#[tauri::command]
fn clipboard_state(clipboard: tauri::State<'_, clipboard::Clipboard>) -> clipboard::ClipboardState {
    clipboard.state()
}

#[tauri::command]
fn clipboard_set_pinned(
    clipboard: tauri::State<'_, clipboard::Clipboard>,
    id: String,
    pinned: bool,
) -> Result<clipboard::ClipboardState, String> {
    clipboard.set_pinned(id, pinned)
}

#[tauri::command]
fn clipboard_delete(
    clipboard: tauri::State<'_, clipboard::Clipboard>,
    id: String,
) -> Result<clipboard::ClipboardState, String> {
    clipboard.delete(id)
}

#[tauri::command]
fn clipboard_copy(
    clipboard: tauri::State<'_, clipboard::Clipboard>,
    id: String,
) -> Result<(), String> {
    clipboard.copy(id)
}

#[tauri::command]
fn clipboard_clear(
    clipboard: tauri::State<'_, clipboard::Clipboard>,
) -> Result<clipboard::ClipboardState, String> {
    clipboard.clear()
}

#[tauri::command]
fn clipboard_clear_windows() -> Result<(), String> {
    clipboard::clear_clipboard()
}

#[tauri::command]
fn app_shortcuts_state(app: tauri::AppHandle) -> app_shortcuts::AppShortcutState {
    app_shortcuts::state(&app)
}

#[tauri::command]
fn app_shortcuts_save(
    app: tauri::AppHandle,
    apps: Vec<app_shortcuts::ShortcutApp>,
) -> Result<app_shortcuts::AppShortcutState, String> {
    app_shortcuts::save(&app, apps)
}

#[tauri::command]
fn misspell_state(expansion: tauri::State<'_, Expansion>) -> misspell::MisspellState {
    expansion.misspell_state()
}

#[tauri::command]
fn misspell_save(
    expansion: tauri::State<'_, Expansion>,
    entries: Vec<misspell::Entry>,
) -> Result<misspell::MisspellState, String> {
    expansion.save_misspells(entries)
}

#[tauri::command]
fn date_state(expansion: tauri::State<'_, Expansion>) -> date_expand::DateState {
    expansion.date_state()
}

#[tauri::command]
fn date_save(
    expansion: tauri::State<'_, Expansion>,
    entries: Vec<date_expand::Entry>,
    prefix: String,
) -> Result<date_expand::DateState, String> {
    expansion.save_dates(entries, prefix)
}

#[tauri::command]
fn ascii_state(expansion: tauri::State<'_, Expansion>) -> ascii::AsciiState {
    expansion.ascii_state()
}

#[tauri::command]
fn ascii_save(
    expansion: tauri::State<'_, Expansion>,
    entries: Vec<ascii::Entry>,
    prefix: String,
) -> Result<ascii::AsciiState, String> {
    expansion.save_ascii(entries, prefix)
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    clipboard::set_clipboard_text(&text)
}

#[tauri::command]
async fn restart_explorer() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(explorer_restart::restart)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn preview_rename(plan: rename::RenamePlan) -> Vec<rename::RenameRow> {
    rename::preview(plan)
}

#[tauri::command]
fn apply_rename(rows: Vec<rename::RenameRow>) -> Result<Vec<rename::RenameRow>, String> {
    rename::apply(rows)
}

#[tauri::command]
fn set_expansion_enabled(expansion: tauri::State<'_, Expansion>, enabled: bool) -> bool {
    expansion.set_enabled(enabled)
}

#[tauri::command]
fn pantherbar_state(
    pantherbar: tauri::State<'_, pantherbar::Pantherbar>,
) -> pantherbar::PantherbarState {
    pantherbar.state()
}

#[tauri::command]
fn pantherbar_save(
    pantherbar: tauri::State<'_, pantherbar::Pantherbar>,
    settings: pantherbar::PantherbarSave,
) -> Result<pantherbar::PantherbarState, String> {
    pantherbar.save(settings)
}

#[tauri::command]
fn pantherbar_run(
    pantherbar: tauri::State<'_, pantherbar::Pantherbar>,
    id: String,
) -> Result<Option<String>, String> {
    pantherbar.run(id)
}

#[tauri::command]
fn pantherbar_fit(pantherbar: tauri::State<'_, pantherbar::Pantherbar>, width: f64, height: f64) {
    pantherbar.fit(width, height);
}

#[tauri::command]
fn launcher_state(launcher: tauri::State<'_, launcher::Launcher>) -> launcher::LauncherState {
    launcher.state()
}

#[tauri::command]
fn launcher_save(
    launcher: tauri::State<'_, launcher::Launcher>,
    settings: launcher::LauncherSave,
) -> Result<launcher::LauncherState, String> {
    launcher.save(settings)
}

#[tauri::command]
fn calculator_eval(
    expression: String,
    previous: Vec<launcher::calculator::CalcRef>,
) -> launcher::calculator::CalcResult {
    launcher::calculator::evaluate(&expression, &previous)
}

#[tauri::command]
fn units_query(text: String) -> launcher::units::UnitsAnswer {
    launcher::units::interpret(&text)
}

#[tauri::command]
fn units_catalog() -> launcher::units::UnitsCatalog {
    launcher::units::catalog()
}

#[tauri::command]
async fn launcher_query(
    app: tauri::AppHandle,
    text: String,
) -> Result<Vec<launcher::LauncherResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.try_state::<launcher::Launcher>()
            .map(|launcher| launcher.query(&text))
            .unwrap_or_default()
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn launcher_run(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || launcher::run(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn launcher_hide(launcher: tauri::State<'_, launcher::Launcher>) {
    launcher.hide();
}

#[tauri::command]
fn launcher_show(launcher: tauri::State<'_, launcher::Launcher>) {
    launcher.show();
}

#[tauri::command]
fn launcher_refresh_apps() {
    std::thread::spawn(launcher::apps::refresh);
}

#[tauri::command]
async fn desktop_apps() -> Result<Vec<desktop_apps::DesktopApp>, String> {
    tauri::async_runtime::spawn_blocking(desktop_apps::list)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn desktop_app_add(name: String, app_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || desktop_apps::add(&name, &app_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn folder_rules_state(
    rules: tauri::State<'_, folder_rules::FolderRules>,
) -> folder_rules::FolderRulesState {
    rules.state()
}

#[tauri::command]
async fn folder_rules_save(
    rules: tauri::State<'_, folder_rules::FolderRules>,
    enabled: bool,
    folders: Vec<folder_rules::WatchedFolder>,
) -> Result<folder_rules::FolderRulesState, String> {
    let rules = rules.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rules.save(enabled, folders))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn folder_rules_run(
    rules: tauri::State<'_, folder_rules::FolderRules>,
    folder_id: Option<String>,
    dry_run: bool,
) -> Result<Vec<folder_rules::ActivityEntry>, String> {
    let rules = rules.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rules.run(folder_id, dry_run))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn folder_rules_activity(
    rules: tauri::State<'_, folder_rules::FolderRules>,
) -> Vec<folder_rules::ActivityEntry> {
    rules.activity()
}

#[tauri::command]
async fn file_assoc_list(
    app: tauri::AppHandle,
) -> Result<file_associations::FileAssociationsState, String> {
    tauri::async_runtime::spawn_blocking(move || file_associations::state(&app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn file_assoc_apply(
    app: tauri::AppHandle,
    association: file_associations::FileAssociation,
) -> Result<file_associations::FileAssociationsState, String> {
    tauri::async_runtime::spawn_blocking(move || file_associations::apply(&app, association))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn file_assoc_remove(
    app: tauri::AppHandle,
    id: String,
) -> Result<file_associations::FileAssociationsState, String> {
    tauri::async_runtime::spawn_blocking(move || file_associations::remove(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn file_assoc_lookup(
    extension: String,
) -> Result<file_associations::ExtensionHandler, String> {
    tauri::async_runtime::spawn_blocking(move || file_associations::lookup(&extension))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn file_assoc_preview(program_path: String, arguments: String, sample: String) -> String {
    file_associations::preview(&program_path, &arguments, &sample)
}

#[tauri::command]
fn file_assoc_open_default_apps() -> Result<(), String> {
    file_associations::open_default_apps()
}

#[tauri::command]
fn terminal_shells() -> Vec<terminal::ShellInfo> {
    terminal::shells()
}

#[tauri::command]
async fn terminal_spawn(
    app: tauri::AppHandle,
    terminals: tauri::State<'_, terminal::Terminals>,
    shell: String,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<terminal::TerminalSession, String> {
    let terminals = terminals.inner().clone();
    tauri::async_runtime::spawn_blocking(move || terminals.spawn(app, &shell, cwd, cols, rows))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn terminal_write(
    terminals: tauri::State<'_, terminal::Terminals>,
    id: String,
    data: String,
) -> Result<(), String> {
    terminals.write(&id, &data)
}

#[tauri::command]
fn terminal_resize(
    terminals: tauri::State<'_, terminal::Terminals>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    terminals.resize(&id, cols, rows)
}

#[tauri::command]
fn terminal_kill(terminals: tauri::State<'_, terminal::Terminals>, id: String) {
    terminals.kill(&id);
}

#[tauri::command]
fn terminal_list(terminals: tauri::State<'_, terminal::Terminals>) -> Vec<terminal::TerminalSession> {
    terminals.list()
}

#[tauri::command]
fn terminal_is_folder(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

#[tauri::command]
fn terminal_clipboard_text() -> String {
    #[cfg(windows)]
    {
        clipboard_win::get_clipboard_string().unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        String::new()
    }
}

#[tauri::command]
fn terminal_settings(app: tauri::AppHandle) -> Result<app_settings::TerminalSettings, String> {
    app_settings::load_terminal(&app)
}

#[tauri::command]
async fn terminal_settings_save(
    app: tauri::AppHandle,
    settings: app_settings::TerminalSettings,
) -> Result<app_settings::TerminalSettings, String> {
    tauri::async_runtime::spawn_blocking(move || app_settings::save_terminal(&app, settings))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn process_list() -> Result<Vec<processes::ProcessInfo>, String> {
    tauri::async_runtime::spawn_blocking(processes::list)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn process_icons(paths: Vec<String>) -> Result<Vec<processes::ProcessIcon>, String> {
    tauri::async_runtime::spawn_blocking(move || processes::icons(&paths))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn process_kill(pid: u32, tree: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || processes::kill(pid, tree))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn port_owners(port: String) -> Result<Vec<processes::PortOwner>, String> {
    let pattern = processes::PortPattern::parse(&port)?;
    tauri::async_runtime::spawn_blocking(move || processes::port_owners(&pattern))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn file_lockers(path: String) -> Result<Vec<processes::FileLocker>, String> {
    tauri::async_runtime::spawn_blocking(move || processes::file_lockers(&path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn app_calls() -> Result<app_calls::AppCallsSnapshot, String> {
    tauri::async_runtime::spawn_blocking(app_calls::snapshot)
        .await
        .map_err(|error| error.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    taskbar::set_process_app_id();
    // Close hides the window and leaves this process in the tray. This has to
    // be first so a later launch from the taskbar exits before it can add
    // another tray icon.
    tauri::Builder::default()
        // Tao registers for raw input while a window is focused, and that
        // registration stops Windows low-level hotkeys from receiving keys in
        // this window. Filtering device events removes that registration.
        .device_event_filter(tauri::DeviceEventFilter::Always)
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|arg| arg == "--background") {
                return;
            }
            show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if expansion::is_quitting() {
                    return;
                }
                api.prevent_close();
                // The vault stays unlocked in the tray. It locks on quit, a manual lock, or after 12 hours.
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let expansion = Expansion::new(app.handle().clone());
            let clipboard = clipboard::Clipboard::start(app.handle().clone());
            app.manage(clipboard);
            app.manage(app_shortcuts::AppShortcuts::start(app.handle()));
            let excel = excel::ExcelTools::start(app.handle().clone());
            app.manage(excel);
            expansion.load_from_settings();
            app.manage(expansion);

            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(icon)?;
            }

            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause expansion", true, None::<&str>)?;
            let awake =
                MenuItem::with_id(app, "stay-awake", "Stay awake: off", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &awake, &quit])?;
            if let Some(expansion) = app.try_state::<Expansion>() {
                expansion.set_pause_item(pause);
            }
            stay_awake::set_tray_item(awake);
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            TrayIconBuilder::with_id("tray")
                .icon(tray_icon)
                .menu(&menu)
                .tooltip("ce-desktop-tools")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "pause" => {
                        if let Some(expansion) = app.try_state::<Expansion>() {
                            expansion.toggle_enabled();
                        }
                    }
                    "stay-awake" => {
                        if let Err(error) = stay_awake::toggle(app) {
                            eprintln!("ce-desktop-tools Stay Awake did not change: {error}");
                        }
                    }
                    "quit" => {
                        expansion::request_quit();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            #[cfg(debug_assertions)]
            {
                let feedback = Arc::new(dev_feedback::DevFeedback::enabled());
                if let Err(error) = dev_feedback::start(Arc::clone(&feedback)) {
                    eprintln!("ce-desktop-tools dev feedback bridge did not start: {error}");
                }
                app.manage(feedback);
            }
            #[cfg(not(debug_assertions))]
            {
                app.manage(Arc::new(dev_feedback::DevFeedback::disabled()));
            }
            let pantherbar = pantherbar::Pantherbar::start(app.handle().clone())
                .map_err(std::io::Error::other)?;
            app.manage(pantherbar);
            app.manage(launcher::Launcher::start(app.handle().clone()));
            app.manage(always_on_top::AlwaysOnTop::start(app.handle().clone()));
            app.manage(screen_ruler::ScreenRuler::start(app.handle().clone()));
            app.manage(color_picker::ColorPicker::start(app.handle().clone()));
            hotkeys::spawn(app.handle().clone());
            app.manage(folder_rules::FolderRules::start(app.handle().clone()));
            app.manage(terminal::Terminals::default());
            if let Ok(settings) = app_settings::app_settings(app.handle()) {
                if let Err(error) = startup::sync_run_at_startup(settings.run_at_startup) {
                    eprintln!("ce-desktop-tools startup setting was not applied: {error}");
                }
            }
            window_pin::install(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                if !startup::started_in_background() {
                    let _ = window.show();
                }
            }
            // The script looks for its own process, so this does not open a second copy.
            let _ = std::thread::Builder::new()
                .name("megaclip-script".into())
                .spawn(|| {
                    if let Err(error) = megaclip_script::start() {
                        eprintln!("ce-desktop-tools could not start MegaClip: {error}");
                    }
                });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_settings,
            save_app_settings,
            preview_app_settings,
            patch_app_settings,
            theme_css,
            always_on_top,
            set_always_on_top,
            app_release,
            app_executable,
            reveal_path,
            open_url,
            megaclip_running,
            run_megaclip,
            stop_megaclip,
            expansion_state,
            save_expansions,
            create_vault_pin,
            change_vault_pin,
            unlock_vault,
            lock_vault,
            clipboard_state,
            clipboard_set_pinned,
            clipboard_delete,
            clipboard_copy,
            clipboard_clear,
            clipboard_clear_windows,
            app_shortcuts_state,
            app_shortcuts_save,
            misspell_state,
            misspell_save,
            date_state,
            date_save,
            ascii_state,
            ascii_save,
            copy_text,
            restart_explorer,
            preview_rename,
            apply_rename,
            set_expansion_enabled,
            pantherbar_state,
            pantherbar_save,
            pantherbar_run,
            pantherbar_fit,
            launcher_state,
            launcher_save,
            calculator_eval,
            units_query,
            units_catalog,
            launcher_query,
            launcher_run,
            launcher_hide,
            launcher_show,
            launcher_refresh_apps,
            hotkeys_recording,
            stay_awake_state,
            set_stay_awake,
            always_on_top_state,
            always_on_top_save,
            always_on_top_unpin_all,
            screen_ruler_state,
            screen_ruler_save,
            screen_ruler_style,
            screen_ruler_measure,
            screen_ruler_open,
            screen_ruler_close,
            color_picker_state,
            color_picker_save,
            color_picker_describe,
            color_picker_render,
            color_picker_set_history,
            color_picker_pick,
            color_picker_open_editor,
            desktop_apps,
            desktop_app_add,
            folder_rules_state,
            folder_rules_save,
            folder_rules_run,
            folder_rules_activity,
            file_assoc_list,
            file_assoc_apply,
            file_assoc_remove,
            file_assoc_lookup,
            file_assoc_preview,
            file_assoc_open_default_apps,
            terminal_shells,
            terminal_spawn,
            terminal_write,
            terminal_resize,
            terminal_kill,
            terminal_list,
            terminal_clipboard_text,
            terminal_is_folder,
            terminal_settings,
            terminal_settings_save,
            process_list,
            process_icons,
            process_kill,
            port_owners,
            file_lockers,
            app_calls,
            dev_feedback_probe,
            dev_feedback_push
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(expansion) = app.try_state::<Expansion>() {
                    expansion.shutdown();
                }
                if let Some(terminals) = app.try_state::<terminal::Terminals>() {
                    terminals.kill_all();
                }
                always_on_top::unpin_all();
                let _ = stay_awake::set(app, false);
                #[cfg(debug_assertions)]
                dev_feedback::clear_port_file();
            }
        });
}

pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        // A key typed in another app does not give this process the right to
        // take focus. Attaching to the foreground thread is what lets a
        // tray-hidden window actually open.
        force_foreground(&window);
        let _ = window.set_focus();
    }
}

/// Windows refuses focus to a background process unless it shares input
/// with the window in front, so attach to that thread for the switch.
pub(crate) fn force_foreground(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SW_SHOW,
            SetForegroundWindow, ShowWindow,
        };
        let Ok(raw) = window.hwnd() else {
            return;
        };
        let target = HWND(raw.0);
        let _ = ShowWindow(target, SW_SHOW);
        let foreground = GetForegroundWindow();
        let current = GetCurrentThreadId();
        let other = if foreground.0.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, None)
        };
        let attached =
            other != 0 && other != current && AttachThreadInput(current, other, true).as_bool();
        let _ = SetForegroundWindow(target);
        let _ = BringWindowToTop(target);
        if attached {
            let _ = AttachThreadInput(current, other, false);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}
