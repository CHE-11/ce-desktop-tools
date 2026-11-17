mod app_settings;
mod app_shortcuts;
mod ascii;
mod clipboard;
mod date_expand;
mod dev_feedback;
mod excel;
mod expansion;
mod explorer_restart;
mod megaclip;
mod misspell;
mod pantherbar;
mod paste_plain;
mod rename;
mod startup;
mod taskbar;
mod update;
mod vault_crypto;

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
    theme: String,
    color_mode: String,
    toolbar_labels: bool,
    run_at_startup: bool,
) -> Result<app_settings::AppSettings, String> {
    let saved = app_settings::save_app_settings(
        &app,
        settings_folder,
        font_zoom,
        theme,
        color_mode,
        toolbar_labels,
        run_at_startup,
    )?;
    if let Some(expansion) = app.try_state::<Expansion>() {
        expansion.load_from_settings();
    }
    app_shortcuts::AppShortcuts::reload(&app);
    Ok(saved)
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
fn megaclip_state(megaclip: tauri::State<'_, megaclip::MegaClip>) -> megaclip::MegaClipState {
    megaclip.state()
}

#[tauri::command]
fn megaclip_set_enabled(
    megaclip: tauri::State<'_, megaclip::MegaClip>,
    enabled: bool,
) -> megaclip::MegaClipState {
    megaclip.set_enabled(enabled)
}

#[tauri::command]
fn megaclip_clear(megaclip: tauri::State<'_, megaclip::MegaClip>) -> megaclip::MegaClipState {
    megaclip.clear()
}

#[tauri::command]
fn megaclip_copy(
    megaclip: tauri::State<'_, megaclip::MegaClip>,
    index: usize,
) -> Result<(), String> {
    megaclip.copy_clip(index)
}

#[tauri::command]
fn megaclip_set_index(
    megaclip: tauri::State<'_, megaclip::MegaClip>,
    index: usize,
) -> Result<megaclip::MegaClipState, String> {
    megaclip.set_index(index)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    taskbar::set_process_app_id();
    // Close hides the window and leaves this process in the tray. This has to
    // be first so a later launch from the taskbar exits before it can add
    // another tray icon.
    tauri::Builder::default()
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
            let megaclip = megaclip::MegaClip::start(app.handle().clone());
            app.manage(megaclip);
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
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &quit])?;
            if let Some(expansion) = app.try_state::<Expansion>() {
                expansion.set_pause_item(pause);
            }
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
            if let Ok(settings) = app_settings::app_settings(app.handle()) {
                if let Err(error) = startup::sync_run_at_startup(settings.run_at_startup) {
                    eprintln!("ce-desktop-tools startup setting was not applied: {error}");
                }
            }
            if let Some(window) = app.get_webview_window("main") {
                if !startup::started_in_background() {
                    let _ = window.show();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_settings,
            save_app_settings,
            preview_app_settings,
            patch_app_settings,
            theme_css,
            app_release,
            app_executable,
            reveal_path,
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
            megaclip_state,
            megaclip_set_enabled,
            megaclip_clear,
            megaclip_copy,
            megaclip_set_index,
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
                #[cfg(debug_assertions)]
                dev_feedback::clear_port_file();
            }
        });
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
