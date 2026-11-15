//! Windows low-level keyboard hook.
//!
//! Typed keys stay in the matcher buffer and are never written down. The
//! completing key is consumed here, then the hook thread deletes the trigger
//! characters already on screen and types the replacement. `SendInput` is not
//! called from the hook callback.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager};

use crate::expansion::matcher::{ExpansionHit, Matcher, Snippet};

pub struct Pending {
    pub backspaces: usize,
    pub text: String,
}

pub struct HookState {
    pub snippets: std::sync::RwLock<Vec<Snippet>>,
    pub enabled: AtomicBool,
    pub locked: AtomicBool,
    pub matcher: Mutex<Matcher>,
    pub pending: Mutex<Option<Pending>>,
    pub app: Mutex<Option<tauri::AppHandle>>,
    pub last_notice: AtomicU64,
    pub our_pid: u32,
    pub thread_id: AtomicU32,
}

impl HookState {
    pub fn new() -> Self {
        Self {
            snippets: std::sync::RwLock::new(Vec::new()),
            enabled: AtomicBool::new(true),
            locked: AtomicBool::new(true),
            matcher: Mutex::new(Matcher::default()),
            pending: Mutex::new(None),
            app: Mutex::new(None),
            last_notice: AtomicU64::new(0),
            our_pid: std::process::id(),
            thread_id: AtomicU32::new(0),
        }
    }

    pub fn set_app(&self, app: tauri::AppHandle) {
        if let Ok(mut slot) = self.app.lock() {
            *slot = Some(app);
        }
    }

    pub fn prompt_unlock(&self) {
        let Some(app) = self.app.lock().ok().and_then(|slot| slot.clone()) else {
            return;
        };
        let _ = app.emit("expansion-unlock-request", ());
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    pub fn replace_snippets(&self, snippets: Vec<Snippet>) {
        if let Ok(mut matcher) = self.matcher.lock() {
            matcher.set_cap_from(&snippets);
            matcher.clear();
        }
        if let Ok(mut slot) = self.snippets.write() {
            *slot = snippets;
        }
    }
}

#[cfg(windows)]
pub fn spawn(state: Arc<HookState>) {
    std::thread::Builder::new()
        .name("expansion-hook".into())
        .spawn(move || run(state))
        .ok();
}

#[cfg(not(windows))]
pub fn spawn(_state: Arc<HookState>) {}

#[cfg(windows)]
pub fn stop(state: &HookState) {
    let thread_id = state.thread_id.load(Ordering::SeqCst);
    if thread_id == 0 {
        return;
    }
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            thread_id,
            windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

#[cfg(not(windows))]
pub fn stop(_state: &HookState) {}

#[cfg(windows)]
fn run(state: Arc<HookState>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage, UnhookWindowsHookEx, WM_APP,
    };

    STATE.set(Arc::clone(&state)).ok();
    unsafe {
        let hook = match install_hook() {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("ce-desktop-tools text expansion did not start: {error}");
                return;
            }
        };
        state.thread_id.store(
            windows::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::SeqCst,
        );
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 || status.0 == -1 {
                break;
            }
            if message.message == WM_APP + 1 {
                perform_pending(&state);
                continue;
            }
            if message.message == WM_APP + 2 {
                state.prompt_unlock();
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

#[cfg(windows)]
fn perform_pending(state: &HookState) {
    let pending = state.pending.lock().ok().and_then(|mut slot| slot.take());
    let Some(pending) = pending else {
        return;
    };
    inject(pending.backspaces, &pending.text);
}

#[cfg(windows)]
static STATE: std::sync::OnceLock<Arc<HookState>> = std::sync::OnceLock::new();

#[cfg(windows)]
fn install_hook() -> Result<windows::Win32::UI::WindowsAndMessaging::HHOOK, String> {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_KEYBOARD_LL};
    unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)
            .map_err(|error| error.to_string())
    }
}

#[cfg(windows)]
unsafe extern "system" fn hook_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, WM_KEYDOWN, WM_SYSKEYDOWN};

    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let message = wparam.0 as u32;
    if message != WM_KEYDOWN && message != WM_SYSKEYDOWN {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info =
        unsafe { &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::KBDLLHOOKSTRUCT) };
    if is_ours(info) {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let swallow = on_key(info);
    if swallow {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(windows)]
fn is_ours(info: &windows::Win32::UI::WindowsAndMessaging::KBDLLHOOKSTRUCT) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::LLKHF_INJECTED;
    info.dwExtraInfo == INJECT_MARK || info.flags.contains(LLKHF_INJECTED)
}

#[cfg(windows)]
fn on_key(info: &windows::Win32::UI::WindowsAndMessaging::KBDLLHOOKSTRUCT) -> bool {
    let Some(state) = STATE.get() else {
        return false;
    };
    let mut matcher = match state.matcher.lock() {
        Ok(matcher) => matcher,
        Err(poisoned) => poisoned.into_inner(),
    };
    if !state.enabled.load(Ordering::Relaxed) || foreground_is_us(state.our_pid) || menu_is_open() {
        matcher.clear();
        return false;
    }
    let vk = info.vkCode as u16;
    if shortcut_clears_buffer(vk) {
        matcher.clear();
        return false;
    }
    if vk == windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK.0 {
        matcher.backspace();
        return false;
    }
    let Some(ch) = key_char(vk, info.scanCode) else {
        if !is_modifier(vk) {
            matcher.clear();
        }
        return false;
    };
    let snippets = state
        .snippets
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let Some(hit) = matcher.push(ch, &snippets) else {
        return false;
    };
    if hit.secret {
        matcher.clear();
        request_unlock(state);
        return false;
    }
    schedule(state, &hit);
    matcher.clear();
    true
}

#[cfg(windows)]
fn request_unlock(state: &HookState) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let previous = state.last_notice.load(Ordering::Relaxed);
    if now.saturating_sub(previous) < 1500 {
        return;
    }
    state.last_notice.store(now, Ordering::Relaxed);
    unsafe {
        let _ = PostThreadMessageW(GetCurrentThreadId(), WM_APP + 2, WPARAM(0), LPARAM(0));
    }
}

#[cfg(windows)]
fn schedule(state: &HookState, hit: &ExpansionHit) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

    let backspaces = match hit.ending {
        Some(_) => hit.trigger.chars().count(),
        None => hit.trigger.chars().count().saturating_sub(1),
    };
    let mut text = if hit.timed {
        crate::date_expand::render_now(&hit.text)
    } else {
        hit.text.clone()
    };
    if let Some(ch) = hit.ending {
        text.push(ch);
    }
    if let Ok(mut pending) = state.pending.lock() {
        *pending = Some(Pending { backspaces, text });
    }
    unsafe {
        let _ = PostThreadMessageW(GetCurrentThreadId(), WM_APP + 1, WPARAM(0), LPARAM(0));
    }
}

#[cfg(windows)]
fn shortcut_clears_buffer(_vk: u16) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN};
    if is_down(VK_LWIN.0) || is_down(VK_RWIN.0) {
        return true;
    }
    let ctrl = is_down(VK_CONTROL.0);
    let alt = is_down(VK_MENU.0);
    // AltGr reports Ctrl+Alt and still produces a character.
    if ctrl && alt {
        return false;
    }
    ctrl || alt
}

#[cfg(windows)]
fn is_down(vk: u16) -> bool {
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) as u16 & 0x8000
            != 0
    }
}

#[cfg(windows)]
fn is_modifier(vk: u16) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT,
    };
    matches!(
        vk,
        x if x == VK_SHIFT.0
            || x == VK_LSHIFT.0
            || x == VK_RSHIFT.0
            || x == VK_CONTROL.0
            || x == VK_LCONTROL.0
            || x == VK_RCONTROL.0
            || x == VK_MENU.0
            || x == VK_LMENU.0
            || x == VK_RMENU.0
            || x == VK_LWIN.0
            || x == VK_RWIN.0
            || x == VK_CAPITAL.0
            || x == VK_NUMLOCK.0
            || x == VK_SCROLL.0
    )
}

#[cfg(windows)]
fn foreground_is_us(our_pid: u32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(foreground, Some(&mut pid));
        pid != 0 && pid == our_pid
    }
}

#[cfg(windows)]
fn menu_is_open() -> bool {
    use std::mem::{size_of, zeroed};
    use windows::Win32::UI::WindowsAndMessaging::{
        GUI_INMENUMODE, GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo,
        GetWindowThreadProcessId,
    };
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return false;
        }
        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info: GUITHREADINFO = zeroed();
        info.cbSize = size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread, &mut info).is_err() {
            return false;
        }
        info.flags.contains(GUI_INMENUMODE)
    }
}

/// Space, Enter, and Tab end a word on every layout. The hook thread's keyboard
/// state often does not list the key that is going down, and `ToUnicode` then
/// returns nothing for Space. Treating that as "not a character" used to drop
/// the typed word, so `dont` never became `don't`.
#[cfg(windows)]
fn boundary_char(vk: u16) -> Option<char> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_RETURN, VK_SPACE, VK_TAB};
    if vk == VK_SPACE.0 {
        Some(' ')
    } else if vk == VK_RETURN.0 {
        Some('\n')
    } else if vk == VK_TAB.0 {
        Some('\t')
    } else {
        None
    }
}

#[cfg(windows)]
fn key_char(vk: u16, scan: u32) -> Option<char> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_PACKET,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    if let Some(ch) = boundary_char(vk) {
        return Some(ch);
    }
    if vk == VK_PACKET.0 {
        return char::from_u32(scan).filter(|ch| !ch.is_control());
    }
    let mut state = [0u8; 256];
    unsafe {
        // The hook thread often has no keyboard state of its own. A failure
        // here used to discard the key and forget the word typed so far.
        let _ = GetKeyboardState(&mut state);
        overlay_modifiers(&mut state);
        if (vk as usize) < state.len() {
            state[vk as usize] |= 0x80;
        }
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let layout = GetKeyboardLayout(thread);
        let mut buffer = [0u16; 8];
        let written = ToUnicodeEx(vk as u32, scan, &state, &mut buffer, 0, Some(layout));
        if written > 0 {
            return String::from_utf16_lossy(&buffer[..written as usize])
                .chars()
                .next();
        }
        if written < 0 {
            return None;
        }
        ascii_from_vk(
            vk,
            shift_down(&state),
            state[VK_CAPITAL.0 as usize] & 0x01 != 0 || caps_on(),
        )
    }
}

#[cfg(windows)]
fn shift_down(state: &[u8; 256]) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT;
    state[VK_SHIFT.0 as usize] & 0x80 != 0
}

#[cfg(windows)]
fn caps_on() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL;
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0
    }
}

/// Used when `ToUnicode` cannot name the key. Letters still have to land in the
/// matcher, or a later space has nothing to correct.
#[cfg(windows)]
fn ascii_from_vk(vk: u16, shift: bool, caps: bool) -> Option<char> {
    if (u16::from(b'A')..=u16::from(b'Z')).contains(&vk) {
        let upper = shift ^ caps;
        let ch = char::from(vk as u8);
        return Some(if upper { ch } else { ch.to_ascii_lowercase() });
    }
    if !shift && (u16::from(b'0')..=u16::from(b'9')).contains(&vk) {
        return Some(char::from(vk as u8));
    }
    None
}

#[cfg(windows)]
fn overlay_modifiers(state: &mut [u8; 256]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
        VK_SHIFT,
    };
    for vk in [
        VK_SHIFT.0,
        VK_LSHIFT.0,
        VK_RSHIFT.0,
        VK_CONTROL.0,
        VK_LCONTROL.0,
        VK_RCONTROL.0,
        VK_MENU.0,
        VK_LMENU.0,
        VK_RMENU.0,
    ] {
        state[vk as usize] = if is_down(vk) { 0x80 } else { 0 };
    }
}

#[cfg(windows)]
const INJECT_MARK: usize = 0xCE70_0E01;

#[cfg(windows)]
fn inject(backspaces: usize, text: &str) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
        VIRTUAL_KEY, VK_BACK, VK_RETURN, VK_SPACE, VK_TAB,
    };

    let mut inputs = Vec::new();
    for _ in 0..backspaces {
        push_vk(&mut inputs, VK_BACK, false);
        push_vk(&mut inputs, VK_BACK, true);
    }
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            push_vk(&mut inputs, VK_RETURN, false);
            push_vk(&mut inputs, VK_RETURN, true);
        } else if ch == '\n' {
            push_vk(&mut inputs, VK_RETURN, false);
            push_vk(&mut inputs, VK_RETURN, true);
        } else if ch == '\t' {
            push_vk(&mut inputs, VK_TAB, false);
            push_vk(&mut inputs, VK_TAB, true);
        } else if ch == ' ' {
            push_vk(&mut inputs, VK_SPACE, false);
            push_vk(&mut inputs, VK_SPACE, true);
        } else {
            let mut units = [0u16; 2];
            let count = ch.encode_utf16(&mut units).len();
            for unit in units.into_iter().take(count) {
                push_unicode(&mut inputs, unit, false);
                push_unicode(&mut inputs, unit, true);
            }
        }
    }
    if inputs.is_empty() {
        return;
    }
    for chunk in inputs.chunks(32) {
        unsafe {
            SendInput(chunk, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn push_vk(inputs: &mut Vec<INPUT>, vk: VIRTUAL_KEY, up: bool) {
        use windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS;
        inputs.push(INPUT {
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
                    dwExtraInfo: INJECT_MARK,
                },
            },
        });
    }

    fn push_unicode(inputs: &mut Vec<INPUT>, unit: u16, up: bool) {
        let flags = if up {
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
        } else {
            KEYEVENTF_UNICODE
        };
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECT_MARK,
                },
            },
        });
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{ascii_from_vk, boundary_char};

    #[test]
    fn space_ends_a_word_without_asking_the_layout() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_RETURN, VK_SPACE, VK_TAB};
        assert_eq!(boundary_char(VK_SPACE.0), Some(' '));
        assert_eq!(boundary_char(VK_RETURN.0), Some('\n'));
        assert_eq!(boundary_char(VK_TAB.0), Some('\t'));
        assert_eq!(boundary_char(u16::from(b'D')), None);
    }

    #[test]
    fn a_letter_still_counts_when_the_layout_call_fails() {
        assert_eq!(ascii_from_vk(u16::from(b'D'), false, false), Some('d'));
        assert_eq!(ascii_from_vk(u16::from(b'D'), true, false), Some('D'));
        assert_eq!(ascii_from_vk(u16::from(b'D'), false, true), Some('D'));
    }
}
