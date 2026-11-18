//! Low-level keyboard hook for the launcher hotkey.
//!
//! `RegisterHotKey` cannot claim Win+Space, so the chord is swallowed here.
//! The callback only posts a message. The hook thread then taps a masked key,
//! so releasing Win does not open Start (or releasing Alt does not open the
//! menu bar), and shows the window.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use crate::pantherbar::Chord;

static APP: OnceLock<AppHandle> = OnceLock::new();
static CHORD: Mutex<Option<Chord>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);
static HELD_KEY: AtomicU16 = AtomicU16::new(0);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[cfg(windows)]
const INJECT_MARK: usize = 0xCE70_0E02;
/// Unassigned virtual key. Tapping it breaks the "Win pressed alone" gesture.
#[cfg(windows)]
const MASK_KEY: u16 = 0xE8;
#[cfg(windows)]
const OPEN_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 21;

pub fn publish(enabled: bool, hotkey: &str) {
    let chord = crate::pantherbar::parse_hotkey(hotkey);
    if let Ok(mut slot) = CHORD.lock() {
        *slot = chord;
    }
    ENABLED.store(enabled && chord.is_some(), Ordering::SeqCst);
}

pub fn spawn(app: AppHandle) {
    if APP.set(app).is_err() {
        return;
    }
    #[cfg(windows)]
    {
        let _ = std::thread::Builder::new()
            .name("launcher-hook".into())
            .spawn(run);
    }
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
                eprintln!("ce-desktop-tools launcher hotkey did not start: {error}");
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
            if message.message == OPEN_MESSAGE {
                if message.wParam.0 != 0 {
                    tap_mask_key();
                }
                if let Some(app) = APP.get() {
                    super::toggle(app);
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
    if (message == WM_KEYDOWN || message == WM_SYSKEYDOWN)
        && info.dwExtraInfo != INJECT_MARK
        && !info.flags.contains(LLKHF_INJECTED)
        && pressed(vk)
    {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(windows)]
fn pressed(vk: u16) -> bool {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

    if !ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(chord) = CHORD.lock().ok().and_then(|slot| *slot) else {
        return false;
    };
    if !matches(chord, vk) {
        return false;
    }
    // In this app's own windows the chord only closes an open launcher, so a
    // shortcut field can still record it.
    if foreground_is_ours() && !super::is_visible() {
        return false;
    }
    if HELD_KEY.swap(vk, Ordering::SeqCst) == vk {
        return true;
    }
    let thread = THREAD_ID.load(Ordering::SeqCst);
    if thread == 0 {
        return false;
    }
    let mask = usize::from(chord.win || chord.alt);
    unsafe {
        let _ = PostThreadMessageW(thread, OPEN_MESSAGE, WPARAM(mask), LPARAM(0));
    }
    true
}

#[cfg(windows)]
fn matches(chord: Chord, vk: u16) -> bool {
    vk == chord.vk
        && down(&[0x11, 0xA2, 0xA3]) == chord.ctrl
        && down(&[0x12, 0xA4, 0xA5]) == chord.alt
        && down(&[0x10, 0xA0, 0xA1]) == chord.shift
        && down(&[0x5B, 0x5C]) == chord.win
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
fn foreground_is_ours() -> bool {
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
