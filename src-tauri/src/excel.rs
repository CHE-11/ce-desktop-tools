//! Excel tools. The first one replaces a formula with its displayed text.
//!
//! Ctrl+Shift+T runs only while EXCEL.EXE is the foreground window. It copies
//! the cell, keeps the plain text, and pastes that text back into the same cell.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};

pub struct ExcelTools;

impl ExcelTools {
    pub fn start(app: AppHandle) -> Self {
        spawn_hotkeys(app);
        Self
    }
}

fn spawn_hotkeys(app: AppHandle) {
    #[cfg(windows)]
    {
        std::thread::Builder::new()
            .name("excel-hotkeys".into())
            .spawn(move || hotkey_loop(app))
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = app;
    }
}

pub(crate) fn cell_text(raw: &str) -> String {
    raw.trim_end_matches(['\r', '\n']).to_string()
}

pub(crate) fn is_excel_exe(path: &str) -> bool {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .eq_ignore_ascii_case("EXCEL.EXE")
}

fn paste_displayed_text(app: &AppHandle) {
    if !foreground_is_excel() {
        return;
    }
    let previous = crate::clipboard::read_clipboard_text();
    let _ = clear_clipboard();
    std::thread::sleep(Duration::from_millis(20));
    send_ctrl(0x43);
    let Some(copied) = wait_for_copy(previous.as_deref()) else {
        return;
    };
    let text = cell_text(&copied);
    std::thread::sleep(Duration::from_millis(20));
    if let Some(clipboard) = app.try_state::<crate::clipboard::Clipboard>() {
        clipboard.suppress_next(text.clone());
    }
    if crate::clipboard::set_clipboard_text(&text).is_err() {
        return;
    }
    std::thread::sleep(Duration::from_millis(10));
    if foreground_is_excel() {
        send_ctrl(0x56);
    }
}

fn wait_for_copy(previous: Option<&str>) -> Option<String> {
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(1) {
        std::thread::sleep(Duration::from_millis(20));
        if let Some(text) = crate::clipboard::read_clipboard_text() {
            if previous != Some(text.as_str()) {
                return Some(text);
            }
        }
    }
    None
}

fn clear_clipboard() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
        let _gate = crate::clipboard::clipboard_gate();
        unsafe {
            if OpenClipboard(None).is_err() {
                return Err("Couldn't clear the clipboard.".into());
            }
            let cleared = EmptyClipboard();
            let _ = CloseClipboard();
            cleared.map_err(|error| format!("Couldn't clear the clipboard: {error}"))
        }
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

fn send_ctrl(vk: u16) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
            SendInput, VIRTUAL_KEY, VK_CONTROL,
        };
        unsafe {
            let inputs = [
                key(VK_CONTROL, false),
                key(VIRTUAL_KEY(vk), false),
                key(VIRTUAL_KEY(vk), true),
                key(VK_CONTROL, true),
            ];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }

        fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: if up {
                            KEYEVENTF_KEYUP
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                        time: 0,
                        dwExtraInfo: 0xCE70_0E01,
                    },
                },
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = vk;
    }
}

pub(crate) fn foreground_exe_name() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        use windows::core::PWSTR;

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid = 0u32;
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return None;
            };
            let mut buffer = [0u16; 1024];
            let mut size = buffer.len() as u32;
            let found = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(process);
            if found.is_err() {
                return None;
            }
            let path = String::from_utf16_lossy(&buffer[..size as usize]);
            path.rsplit(['\\', '/'])
                .next()
                .map(str::to_string)
                .filter(|name| !name.is_empty())
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn foreground_is_excel() -> bool {
    foreground_exe_name().is_some_and(|name| is_excel_exe(&name))
}

pub(crate) fn modifier_state() -> (bool, bool, bool, bool) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        };
        (
            key_down(VK_CONTROL.0),
            key_down(VK_SHIFT.0),
            key_down(VK_MENU.0),
            key_down(VK_LWIN.0) || key_down(VK_RWIN.0),
        )
    }
    #[cfg(not(windows))]
    {
        (false, false, false, false)
    }
}

#[cfg(windows)]
fn hotkey_loop(app: AppHandle) {
    use std::sync::Mutex;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
        PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    let app = Arc::new(app);
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let pending_action = Arc::new(Mutex::new(None));
    PENDING_ACTION.set(Arc::clone(&pending_action)).ok();
    HELD.store(false, Ordering::Relaxed);

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let message = wparam.0 as u32;
            if info.flags.0 & 0x10 == 0 {
                let vk = info.vkCode as u16;
                if message == WM_KEYUP || message == WM_SYSKEYUP {
                    if HELD_VK.load(Ordering::Relaxed) == u32::from(vk) {
                        HELD.store(false, Ordering::Relaxed);
                    }
                } else if let (true, Some(action)) = (
                    message == WM_KEYDOWN || message == WM_SYSKEYDOWN,
                    crate::app_shortcuts::matching_action(vk),
                ) {
                    HELD_VK.store(u32::from(vk), Ordering::Relaxed);
                    let first = !HELD.swap(true, Ordering::Relaxed);
                    if first {
                        if let Some(slot) = PENDING_ACTION.get() {
                            let _ = slot.lock().map(|mut pending| *pending = Some(action));
                            unsafe {
                                let _ = PostThreadMessageW(
                                    windows::Win32::System::Threading::GetCurrentThreadId(),
                                    WM_APP_EXCEL,
                                    WPARAM(0),
                                    LPARAM(0),
                                );
                            }
                        }
                    }
                    return LRESULT(1);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    unsafe {
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("ce-desktop-tools Excel hotkeys did not start: {error}");
                return;
            }
        };
        while GetMessageW(&mut message, None, 0, 0).0 != 0 {
            if message.message == WM_APP_EXCEL {
                let action = pending_action.lock().ok().and_then(|mut slot| slot.take());
                let worker = Arc::clone(&app);
                match action {
                    Some(crate::app_shortcuts::ShortcutAction::ExcelFormulaText) => {
                        std::thread::spawn(move || paste_displayed_text(&worker));
                    }
                    Some(crate::app_shortcuts::ShortcutAction::PastePlain) => {
                        std::thread::spawn(move || crate::paste_plain::paste_plain(&worker));
                    }
                    None => {}
                }
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

#[cfg(windows)]
const WM_APP_EXCEL: u32 = 0x8000 + 6;

#[cfg(windows)]
static HELD: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static HELD_VK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(windows)]
static PENDING_ACTION: std::sync::OnceLock<
    Arc<std::sync::Mutex<Option<crate::app_shortcuts::ShortcutAction>>>,
> = std::sync::OnceLock::new();

#[cfg(windows)]
fn key_down(vk: u16) -> bool {
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) as u16 & 0x8000
            != 0
    }
}

#[cfg(test)]
mod tests {
    use super::{cell_text, is_excel_exe};

    #[test]
    fn displayed_text_drops_excels_trailing_line_break() {
        assert_eq!(cell_text("1,234.00\r\n"), "1,234.00");
        assert_eq!(cell_text("line one\nline two\r\n"), "line one\nline two");
        assert_eq!(cell_text("already text"), "already text");
    }

    #[test]
    fn only_excel_owns_the_shortcut() {
        assert!(is_excel_exe(
            r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"
        ));
        assert!(is_excel_exe(
            r"C:\Program Files\Microsoft Office\root\Office16\excel.exe"
        ));
        assert!(!is_excel_exe(r"C:\Windows\explorer.exe"));
        assert!(!is_excel_exe("EXCEL.EXE.bak"));
    }
}
