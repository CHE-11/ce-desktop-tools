//! Always-on-top for the main window, and a purple frame while it is pinned.
//!
//! Windows hotkeys such as Ctrl+Space pin whichever window is in front by
//! changing its z-order. A subclass watches that change so the frame and the
//! toolbar stay in step with the hotkey.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter, Manager};

/// `#a855f7` stored as a COLORREF (0x00BBGGRR).
const PIN_BORDER_BGR: u32 = 0x00F7_55A8;
/// `DWMWA_COLOR_NONE` — hand the frame color back to Windows.
const FRAME_COLOR_NONE: u32 = 0xFFFF_FFFE;
const SUBCLASS_ID: usize = 0xCE70_0911;

static APP: OnceLock<AppHandle> = OnceLock::new();
static PINNED: AtomicBool = AtomicBool::new(false);

pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(windows)]
    {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let Some(hwnd) = hwnd_of(&window) else {
            return;
        };
        unsafe {
            let _ = windows::Win32::UI::Shell::SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, 0);
        }
        apply(hwnd, topmost(hwnd));
    }
}

pub fn current(app: &AppHandle) -> bool {
    #[cfg(windows)]
    {
        if let Some(window) = app.get_webview_window("main") {
            if let Some(hwnd) = hwnd_of(&window) {
                return topmost(hwnd);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
    }
    PINNED.load(Ordering::SeqCst)
}

pub fn set(app: &AppHandle, on: bool) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The main window is not open.".to_string())?;
    window
        .set_always_on_top(on)
        .map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        if let Some(hwnd) = hwnd_of(&window) {
            place(hwnd, on);
            let actual = topmost(hwnd);
            apply(hwnd, actual);
            return Ok(actual);
        }
    }
    #[cfg(not(windows))]
    {
        PINNED.store(on, Ordering::SeqCst);
        let _ = app.emit("always-on-top", on);
    }
    Ok(on)
}

#[cfg(windows)]
fn apply(hwnd: windows::Win32::Foundation::HWND, on: bool) {
    if PINNED.swap(on, Ordering::SeqCst) == on {
        return;
    }
    paint(hwnd, on);
    let Some(app) = APP.get().cloned() else {
        return;
    };
    std::thread::spawn(move || {
        let _ = app.emit("always-on-top", on);
    });
}

#[cfg(windows)]
fn hwnd_of(window: &tauri::WebviewWindow) -> Option<windows::Win32::Foundation::HWND> {
    let hwnd = window.hwnd().ok()?;
    Some(windows::Win32::Foundation::HWND(hwnd.0))
}

#[cfg(windows)]
fn topmost(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOPMOST};
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & (WS_EX_TOPMOST.0 as isize) != 0 }
}

#[cfg(windows)]
fn place(hwnd: windows::Win32::Foundation::HWND, on: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(if on { HWND_TOPMOST } else { HWND_NOTOPMOST }),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(windows)]
fn paint(hwnd: windows::Win32::Foundation::HWND, on: bool) {
    use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmSetWindowAttribute};
    let color = if on { PIN_BORDER_BGR } else { FRAME_COLOR_NONE };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const u32 as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(windows)]
unsafe extern "system" fn subclass(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{WM_NCDESTROY, WM_WINDOWPOSCHANGED};

    if msg == WM_WINDOWPOSCHANGED {
        apply(hwnd, topmost(hwnd));
    } else if msg == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID);
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::PIN_BORDER_BGR;

    #[test]
    fn the_pin_frame_is_purple() {
        let red = PIN_BORDER_BGR & 0xFF;
        let green = (PIN_BORDER_BGR >> 8) & 0xFF;
        let blue = (PIN_BORDER_BGR >> 16) & 0xFF;
        assert_eq!((red, green, blue), (0xA8, 0x55, 0xF7));
    }
}
