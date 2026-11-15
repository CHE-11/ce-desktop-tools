mod app_settings;
mod dev_feedback;
mod expansion;
mod startup;
mod update;

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
    Ok(saved)
}

#[tauri::command]
fn preview_app_settings(settings_folder: String) -> Result<app_settings::AppSettings, String> {
    app_settings::preview_app_settings(settings_folder)
}

#[tauri::command]
fn patch_app_settings(app: tauri::AppHandle, patch: app_settings::AppSettingsPatch) -> Result<(), String> {
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
fn save_expansions(expansion: tauri::State<'_, Expansion>, snippets: Vec<Snippet>) -> Result<ExpansionState, String> {
    expansion.save(snippets)
}

#[tauri::command]
async fn create_vault_pin(expansion: tauri::State<'_, Expansion>, pin: String) -> Result<ExpansionState, String> {
    let expansion = expansion.inner().clone();
    let state = tauri::async_runtime::spawn_blocking(move || expansion.create_pin(pin))
        .await
        .map_err(|error| error.to_string())??;
    Ok(state)
}

#[tauri::command]
async fn unlock_vault(expansion: tauri::State<'_, Expansion>, pin: String) -> Result<ExpansionState, String> {
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
fn set_expansion_enabled(expansion: tauri::State<'_, Expansion>, enabled: bool) -> bool {
    expansion.set_enabled(enabled)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let expansion = Expansion::new(app.handle().clone());
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
            if let Ok(settings) = app_settings::app_settings(app.handle()) {
                if let Err(error) = startup::sync_run_at_startup(settings.run_at_startup) {
                    eprintln!("ce-desktop-tools startup setting was not applied: {error}");
                }
            }
            if !startup::started_in_background() {
                if let Some(window) = app.get_webview_window("main") {
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
            unlock_vault,
            lock_vault,
            set_expansion_enabled,
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
