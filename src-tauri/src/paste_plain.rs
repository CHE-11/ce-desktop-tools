//! Paste as plain text.
//!
//! The shortcut keeps a copy of every clipboard format, pastes the text only,
//! and puts the original clipboard back a second later.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

const CF_TEXT: u32 = 1;
const CF_UNICODETEXT: u32 = 13;
const MAX_FORMAT_BYTES: usize = 32 * 1024 * 1024;

struct FormatData {
    format: u32,
    bytes: Vec<u8>,
}

pub(crate) struct ClipboardSnapshot {
    items: Vec<FormatData>,
}

pub(crate) fn paste_plain(app: &AppHandle) {
    let Ok(_busy) = BUSY.lock() else {
        return;
    };
    let Ok(snapshot) = capture_clipboard() else {
        return;
    };
    let Some(text) = text_of(&snapshot.items) else {
        return;
    };
    suppress(app, &text);
    if crate::clipboard::set_clipboard_text(&text).is_err() {
        restore_clipboard(&snapshot);
        return;
    }
    send_ctrl_v();
    std::thread::sleep(Duration::from_millis(1000));
    suppress(app, &text);
    restore_clipboard(&snapshot);
}

fn suppress(app: &AppHandle, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(clipboard) = app.try_state::<crate::clipboard::Clipboard>() {
        clipboard.suppress_next(trimmed.to_string());
    }
}

fn text_of(formats: &[FormatData]) -> Option<String> {
    if let Some(item) = formats.iter().find(|item| item.format == CF_UNICODETEXT) {
        if let Some(text) = unicode_text(&item.bytes) {
            return Some(text);
        }
    }
    formats
        .iter()
        .find(|item| item.format == CF_TEXT)
        .and_then(|item| ansi_text(&item.bytes))
}

fn unicode_text(bytes: &[u8]) -> Option<String> {
    let mut units = Vec::new();
    for pair in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    if units.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&units))
    }
}

fn ansi_text(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    if end == 0 {
        return None;
    }
    Some(bytes[..end].iter().map(|byte| char::from(*byte)).collect())
}

pub(crate) fn with_clipboard<T>(body: impl FnOnce() -> T) -> Option<T> {
    let Ok(_busy) = BUSY.lock() else {
        return None;
    };
    Some(body())
}

pub(crate) fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{EnumClipboardFormats, GetClipboardData};
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        let _gate = crate::clipboard::clipboard_gate();
        unsafe {
            open_clipboard()?;
            let mut items = Vec::new();
            let mut format = 0u32;
            for _ in 0..64 {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                let Ok(handle) = GetClipboardData(format) else {
                    continue;
                };
                let global = HGLOBAL(handle.0);
                let size = GlobalSize(global);
                if size == 0 || size > MAX_FORMAT_BYTES {
                    continue;
                }
                let pointer = GlobalLock(global);
                if pointer.is_null() {
                    continue;
                }
                let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
                let _ = GlobalUnlock(global);
                items.push(FormatData { format, bytes });
            }
            close_clipboard();
            Ok(ClipboardSnapshot { items })
        }
    }
    #[cfg(not(windows))]
    {
        Ok(ClipboardSnapshot { items: Vec::new() })
    }
}

pub(crate) fn restore_clipboard(snapshot: &ClipboardSnapshot) {
    restore_items(&snapshot.items);
}

fn restore_items(items: &[FormatData]) {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
        use windows::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
        use windows::Win32::System::Memory::{
            GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
        };

        let _gate = crate::clipboard::clipboard_gate();
        unsafe {
            if open_clipboard().is_err() {
                return;
            }
            if EmptyClipboard().is_err() {
                close_clipboard();
                return;
            }
            for item in items {
                let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, item.bytes.len()) else {
                    continue;
                };
                let pointer = GlobalLock(global);
                if pointer.is_null() {
                    let _ = GlobalFree(Some(global));
                    continue;
                }
                std::ptr::copy_nonoverlapping(
                    item.bytes.as_ptr().cast::<c_void>(),
                    pointer,
                    item.bytes.len(),
                );
                let _ = GlobalUnlock(global);
                if SetClipboardData(item.format, Some(HANDLE(global.0))).is_err() {
                    let _ = GlobalFree(Some(global));
                }
            }
            close_clipboard();
            let _ = HGLOBAL(std::ptr::null_mut());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = items;
    }
}

fn open_clipboard() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::DataExchange::OpenClipboard;
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err("Couldn't read the clipboard.".into())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

fn close_clipboard() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::DataExchange::CloseClipboard();
    }
}

fn send_ctrl_v() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_V;
        send_ctrl(VK_V.0);
    }
}

pub(crate) fn send_ctrl(key: u16) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
            SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_RCONTROL, VK_RSHIFT,
        };

        let mut inputs = Vec::new();
        let mut restore = Vec::new();
        for vk in [VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_CONTROL] {
            if key_down(vk.0) {
                inputs.push(key_input(vk, true));
                restore.push(vk);
            }
        }
        inputs.push(key_input(VK_CONTROL, false));
        inputs.push(key_input(VIRTUAL_KEY(key), false));
        inputs.push(key_input(VIRTUAL_KEY(key), true));
        inputs.push(key_input(VK_CONTROL, true));
        for vk in restore {
            inputs.push(key_input(vk, false));
        }
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }

        fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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
        let _ = key;
    }
}

#[cfg(windows)]
fn key_down(vk: u16) -> bool {
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) as u16 & 0x8000
            != 0
    }
}

static BUSY: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::{CF_TEXT, CF_UNICODETEXT, FormatData, text_of, unicode_text};

    #[test]
    fn unicode_clipboard_text_stops_at_the_terminator() {
        let bytes = [b'H', 0, b'i', 0, 0, 0, b'X', 0];
        assert_eq!(unicode_text(&bytes).as_deref(), Some("Hi"));
        assert_eq!(unicode_text(&[0, 0]), None);
    }

    #[test]
    fn plain_text_prefers_unicode_over_ansi() {
        let formats = vec![
            FormatData {
                format: CF_TEXT,
                bytes: b"Ansi\0".to_vec(),
            },
            FormatData {
                format: CF_UNICODETEXT,
                bytes: [b'O', 0, b'k', 0, 0, 0].to_vec(),
            },
        ];
        assert_eq!(text_of(&formats).as_deref(), Some("Ok"));
        assert_eq!(
            text_of(&[FormatData {
                format: CF_TEXT,
                bytes: b"Ansi\0".to_vec()
            }])
            .as_deref(),
            Some("Ansi")
        );
    }
}
