//! One low-level keyboard hook for every global hotkey this app owns.
//!
//! `RegisterHotKey` cannot claim Win+Space or Ctrl+Space reliably, so each
//! chord is swallowed here. The callback only posts a message. The hook thread
//! then taps a masked key, so releasing Win does not open Start (or releasing
//! Alt does not open the menu bar), and hands the press to its module.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use crate::pantherbar::Chord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyId {
    Launcher = 0,
    AlwaysOnTop = 1,
    ScreenRuler = 2,
    ColorPicker = 3,
    Record = 4,
    Present = 5,
}

const COUNT: usize = 6;
const ALL: [HotkeyId; COUNT] = [
    HotkeyId::Launcher,
    HotkeyId::AlwaysOnTop,
    HotkeyId::ScreenRuler,
    HotkeyId::ColorPicker,
    HotkeyId::Record,
    HotkeyId::Present,
];

impl HotkeyId {
    fn from_index(index: usize) -> Option<Self> {
        ALL.get(index).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            HotkeyId::Launcher => "Launcher",
            HotkeyId::AlwaysOnTop => "Always On Top",
            HotkeyId::ScreenRuler => "Screen Ruler",
            HotkeyId::ColorPicker => "Color Picker",
            HotkeyId::Record => "Record",
            HotkeyId::Present => "Present",
        }
    }
}

#[derive(Clone, Copy)]
struct Slot {
    enabled: bool,
    chord: Option<Chord>,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static SLOTS: Mutex<[Slot; COUNT]> = Mutex::new(
    [Slot {
        enabled: false,
        chord: None,
    }; COUNT],
);
static RECORDING: AtomicBool = AtomicBool::new(false);
static HELD_KEY: AtomicU16 = AtomicU16::new(0);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[cfg(windows)]
const INJECT_MARK: usize = 0xCE70_0E02;
/// Unassigned virtual key. Tapping it breaks the "Win pressed alone" gesture.
#[cfg(windows)]
const MASK_KEY: u16 = 0xE8;
#[cfg(windows)]
const FIRE_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 21;
/// Escape or Delete that an open Record or Present overlay claimed. `wParam` is a `SpecialKey`.
#[cfg(windows)]
const KEY_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 22;

#[cfg(windows)]
#[derive(Clone, Copy)]
enum SpecialKey {
    RecordEscape = 0,
    PresentEscape = 1,
    PresentDelete = 2,
}

/// A global hotkey needs a modifier. A bare letter would fire while typing.
pub fn usable_hotkey(value: &str) -> Option<String> {
    let chord = crate::pantherbar::parse_hotkey(value)?;
    (chord.ctrl || chord.alt || chord.win).then(|| chord.label())
}

pub fn publish(id: HotkeyId, enabled: bool, hotkey: &str) {
    let chord = crate::pantherbar::parse_hotkey(hotkey);
    if let Ok(mut slots) = SLOTS.lock() {
        slots[id as usize] = Slot {
            enabled: enabled && chord.is_some(),
            chord,
        };
    }
}

/// Fails when another enabled module already owns the same chord.
pub fn check_free(id: HotkeyId, hotkey: &str) -> Result<(), String> {
    let Some(wanted) = crate::pantherbar::parse_hotkey(hotkey) else {
        return Ok(());
    };
    let label = wanted.label();
    let Ok(slots) = SLOTS.lock() else {
        return Ok(());
    };
    for other in ALL {
        if other == id {
            continue;
        }
        let slot = slots[other as usize];
        if slot.enabled && slot.chord.map(|chord| chord.label()) == Some(label.clone()) {
            return Err(format!(
                "{label} is already the {} shortcut.",
                other.title()
            ));
        }
    }
    Ok(())
}

/// A shortcut field in this app is waiting for a chord, so let every chord through.
pub fn set_recording(on: bool) {
    RECORDING.store(on, Ordering::SeqCst);
}

pub fn spawn(app: AppHandle) {
    if APP.set(app).is_err() {
        return;
    }
    #[cfg(windows)]
    {
        let _ = std::thread::Builder::new()
            .name("global-hotkeys".into())
            .spawn(run);
    }
}

fn dispatch(app: &AppHandle, id: HotkeyId) {
    match id {
        HotkeyId::Launcher => crate::launcher::toggle(app),
        HotkeyId::AlwaysOnTop => crate::always_on_top::hotkey(app),
        HotkeyId::ScreenRuler => crate::screen_ruler::hotkey(app),
        HotkeyId::ColorPicker => crate::color_picker::hotkey(app),
        HotkeyId::Record => crate::record::hotkey(app),
        HotkeyId::Present => crate::present::hotkey(app),
    }
}

/// Runs on its own thread, because Present can wait on the main thread while it closes.
#[cfg(windows)]
fn dispatch_key(app: &AppHandle, key: usize) {
    let app = app.clone();
    std::thread::spawn(move || match key {
        0 => crate::record::on_escape(&app),
        1 => crate::present::on_escape(&app),
        2 => crate::present::on_delete(&app),
        _ => {}
    });
}

#[cfg(windows)]
fn run() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, WH_KEYBOARD_LL,
    };
    unsafe {
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("ce-desktop-tools global hotkeys did not start: {error}");
                return;
            }
        };
        THREAD_ID.store(
            windows::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::SeqCst,
        );
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 || status.0 == -1 {
                break;
            }
            if message.message == FIRE_MESSAGE {
                if message.lParam.0 != 0 {
                    tap_mask_key();
                }
                if let (Some(app), Some(id)) = (APP.get(), HotkeyId::from_index(message.wParam.0)) {
                    dispatch(app, id);
                }
                continue;
            }
            if message.message == KEY_MESSAGE {
                if let Some(app) = APP.get() {
                    dispatch_key(app, message.wParam.0);
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
unsafe extern "system" fn key_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let message = wparam.0 as u32;
    let vk = info.vkCode as u16;
    if message == WM_KEYUP || message == WM_SYSKEYUP {
        if HELD_KEY.load(Ordering::SeqCst) == vk {
            HELD_KEY.store(0, Ordering::SeqCst);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let typed = (message == WM_KEYDOWN || message == WM_SYSKEYDOWN)
        && info.dwExtraInfo != INJECT_MARK
        && !info.flags.contains(LLKHF_INJECTED);
    if typed && (pressed(vk) || special(vk)) {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Escape leaves Record's drawing or steps Present back, and Delete removes a Present lasso
/// selection. Other keys, and these keys with Ctrl, Alt, or Win, reach the app in front.
#[cfg(windows)]
fn special(vk: u16) -> bool {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_DELETE, VK_ESCAPE};
    use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

    let key = if vk == VK_ESCAPE.0 {
        if crate::record::escape_is_useful() {
            SpecialKey::RecordEscape
        } else if crate::present::escape_is_useful() {
            SpecialKey::PresentEscape
        } else {
            return false;
        }
    } else if (vk == VK_DELETE.0 || vk == VK_BACK.0) && crate::present::delete_is_useful() {
        SpecialKey::PresentDelete
    } else {
        return false;
    };
    if down(&[0x11, 0xA2, 0xA3]) || down(&[0x12, 0xA4, 0xA5]) || down(&[0x5B, 0x5C]) {
        return false;
    }
    let thread = THREAD_ID.load(Ordering::SeqCst);
    if thread == 0 {
        return false;
    }
    unsafe {
        let _ = PostThreadMessageW(thread, KEY_MESSAGE, WPARAM(key as usize), LPARAM(0));
    }
    true
}

#[cfg(windows)]
fn pressed(vk: u16) -> bool {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

    let Some((id, chord)) = matching(vk) else {
        return false;
    };
    if foreground_is_ours() {
        if RECORDING.load(Ordering::SeqCst) {
            return false;
        }
        // In this app's own windows the launcher chord only closes an open launcher.
        if id == HotkeyId::Launcher && !crate::launcher::is_visible() {
            return false;
        }
    }
    if HELD_KEY.swap(vk, Ordering::SeqCst) == vk {
        return true;
    }
    let thread = THREAD_ID.load(Ordering::SeqCst);
    if thread == 0 {
        return false;
    }
    let mask = isize::from(chord.win || chord.alt);
    unsafe {
        let _ = PostThreadMessageW(thread, FIRE_MESSAGE, WPARAM(id as usize), LPARAM(mask));
    }
    true
}

#[cfg(windows)]
fn matching(vk: u16) -> Option<(HotkeyId, Chord)> {
    let slots = SLOTS.lock().ok()?;
    let mut held = None;
    for id in ALL {
        let slot = slots[id as usize];
        let Some(chord) = slot.chord else {
            continue;
        };
        if !slot.enabled || chord.vk != vk {
            continue;
        }
        let modifiers = *held.get_or_insert_with(|| {
            (
                down(&[0x11, 0xA2, 0xA3]),
                down(&[0x12, 0xA4, 0xA5]),
                down(&[0x10, 0xA0, 0xA1]),
                down(&[0x5B, 0x5C]),
            )
        });
        if modifiers == (chord.ctrl, chord.alt, chord.shift, chord.win) {
            return Some((id, chord));
        }
    }
    None
}

#[cfg(windows)]
fn down(keys: &[u16]) -> bool {
    keys.iter().any(|vk| unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(*vk)) as u16
            & 0x8000
            != 0
    })
}

#[cfg(windows)]
pub(crate) fn foreground_is_ours() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == std::process::id()
    }
}

#[cfg(windows)]
fn tap_mask_key() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY,
    };
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(MASK_KEY),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECT_MARK,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::{HotkeyId, check_free, publish, usable_hotkey};

    #[test]
    fn a_global_hotkey_needs_a_modifier() {
        assert_eq!(usable_hotkey("win+space").as_deref(), Some("Win+Space"));
        assert_eq!(
            usable_hotkey("Win+Ctrl+Shift+M").as_deref(),
            Some("Ctrl+Shift+Win+M")
        );
        assert_eq!(usable_hotkey("Space"), None);
        assert_eq!(usable_hotkey("shift+a"), None);
    }

    #[test]
    fn two_modules_cannot_share_a_chord() {
        publish(HotkeyId::Launcher, true, "Alt+F9");
        assert!(check_free(HotkeyId::ColorPicker, "alt+f9").is_err());
        assert!(check_free(HotkeyId::Launcher, "Alt+F9").is_ok());
        publish(HotkeyId::Launcher, false, "Alt+F9");
        assert!(check_free(HotkeyId::ColorPicker, "Alt+F9").is_ok());
    }
}
