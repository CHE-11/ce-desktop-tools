//! Keeps the computer and the display awake until it is turned off.
//!
//! `SetThreadExecutionState` belongs to the thread that calls it, so one
//! thread holds the request and clears it when its channel closes. Nothing is
//! saved: a synced settings folder must not keep a second machine awake.

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};

use tauri::{AppHandle, Emitter};

static HOLDER: Mutex<Option<Sender<()>>> = Mutex::new(None);
static TRAY_ITEM: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>> = Mutex::new(None);

pub fn is_on() -> bool {
    HOLDER.lock().map(|slot| slot.is_some()).unwrap_or(false)
}

pub fn set(app: &AppHandle, on: bool) -> Result<bool, String> {
    {
        let mut slot = HOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if on && slot.is_none() {
            *slot = Some(start()?);
        } else if !on {
            // Dropping the sender ends the holder thread, which clears the request.
            slot.take();
        }
    }
    let now = is_on();
    sync_tray_label(now);
    let _ = app.emit("stay-awake", now);
    Ok(now)
}

pub fn toggle(app: &AppHandle) -> Result<bool, String> {
    set(app, !is_on())
}

pub fn set_tray_item(item: tauri::menu::MenuItem<tauri::Wry>) {
    if let Ok(mut slot) = TRAY_ITEM.lock() {
        *slot = Some(item);
    }
    sync_tray_label(is_on());
}

fn sync_tray_label(on: bool) {
    if let Ok(slot) = TRAY_ITEM.lock() {
        if let Some(item) = slot.as_ref() {
            let _ = item.set_text(if on {
                "Stay awake: on"
            } else {
                "Stay awake: off"
            });
        }
    }
}

fn start() -> Result<Sender<()>, String> {
    let (sender, receiver) = channel::<()>();
    let (ready_sender, ready) = channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("stay-awake".into())
        .spawn(move || {
            let _ = ready_sender.send(request(true));
            while receiver.recv().is_ok() {}
            let _ = request(false);
        })
        .map_err(|error| format!("Couldn't start Stay Awake: {error}"))?;
    ready
        .recv()
        .map_err(|_| "Stay Awake stopped before it started.".to_string())??;
    Ok(sender)
}

#[cfg(windows)]
fn request(on: bool) -> Result<(), String> {
    use windows::Win32::System::Power::{
        ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };
    let flags = if on {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    let previous = unsafe { SetThreadExecutionState(flags) };
    if previous.0 == 0 {
        return Err("Windows refused to keep the computer awake.".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn request(_on: bool) -> Result<(), String> {
    Ok(())
}
