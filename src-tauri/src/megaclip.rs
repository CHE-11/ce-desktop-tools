//! Rotating copy stack from the MegaClip AutoHotkey script.
//!
//! Alt+Ctrl+C stores the current selection. Alt+Ctrl+V pastes the current
//! snippet and moves to the next one. Alt+CapsLock previews the next snippet.
//! CapsLock+C clears the stack, does not type C, and leaves Caps Lock where it
//! was before that press. A bare CapsLock tap still toggles the lock. The
//! stack stays in memory.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicU32};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MegaClipState {
    pub clips: Vec<String>,
    pub index: usize,
}

struct Inner {
    app: AppHandle,
    clips: Mutex<Vec<String>>,
    index: Mutex<usize>,
    busy: Mutex<()>,
}

pub struct MegaClip {
    inner: Arc<Inner>,
}

impl MegaClip {
    pub fn start(app: AppHandle) -> Self {
        let inner = Arc::new(Inner {
            app,
            clips: Mutex::new(Vec::new()),
            index: Mutex::new(0),
            busy: Mutex::new(()),
        });
        spawn_tooltip();
        spawn_hotkeys(Arc::clone(&inner));
        Self { inner }
    }

    pub fn state(&self) -> MegaClipState {
        self.inner.view()
    }

    pub fn clear(&self) -> MegaClipState {
        self.inner.clear();
        self.inner.view()
    }

    pub fn copy_clip(&self, index: usize) -> Result<(), String> {
        let text = self
            .inner
            .clips
            .lock()
            .map_err(|_| "MegaClip is unavailable.".to_string())?
            .get(index)
            .cloned()
            .ok_or_else(|| "That snippet is no longer on the stack.".to_string())?;
        wait_paste_hold();
        let _gate = CLIP_GATE.lock().unwrap_or_else(|error| error.into_inner());
        bump_clip_epoch();
        suppress(&self.inner.app, &text);
        crate::clipboard::set_clipboard_text(&text)
    }

    pub fn set_index(&self, index: usize) -> Result<MegaClipState, String> {
        let len = self
            .inner
            .clips
            .lock()
            .map_err(|_| "MegaClip is unavailable.".to_string())?
            .len();
        if index >= len {
            return Err("That snippet is no longer on the stack.".into());
        }
        if let Ok(mut slot) = self.inner.index.lock() {
            *slot = index;
        }
        self.inner.emit();
        Ok(self.inner.view())
    }
}

impl Inner {
    fn view(&self) -> MegaClipState {
        let clips = self
            .clips
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let index = self.index.lock().map(|slot| *slot).unwrap_or(0);
        MegaClipState { clips, index }
    }

    fn emit(&self) {
        let _ = self.app.emit("megaclip-changed", self.view());
    }

    fn copy_selection(&self) {
        wait_paste_hold();
        let after = {
            let _gate = CLIP_GATE.lock().unwrap_or_else(|error| error.into_inner());
            bump_clip_epoch();
            clipboard_sequence()
        };
        send_plain_ctrl(KEY_C);
        let Some(text) = wait_for_copy(after) else {
            return;
        };
        let tip = if let Ok(mut clips) = self.clips.lock() {
            clips.push(text);
            Some(copied_tip(
                clips.last().map(String::as_str).unwrap_or_default(),
                clips.len(),
            ))
        } else {
            None
        };
        if let Some(tip) = tip {
            show_tooltip(&tip);
        }
        self.emit();
    }

    fn paste_current(&self) {
        let clips = match self.clips.lock() {
            Ok(clips) => clips.clone(),
            Err(_) => return,
        };
        if clips.is_empty() {
            show_tooltip("No snippets stored");
            return;
        }
        let mut index = self.index.lock().map(|slot| *slot).unwrap_or(0);
        if index >= clips.len() {
            index = 0;
        }
        let snippet = clips[index].clone();
        let shown = index + 1;
        let backup = crate::clipboard::read_clipboard_text().unwrap_or_default();
        wait_paste_hold();
        let epoch = {
            let _gate = CLIP_GATE.lock().unwrap_or_else(|error| error.into_inner());
            let epoch = bump_clip_epoch();
            suppress(&self.app, &snippet);
            if crate::clipboard::set_clipboard_text(&snippet).is_err() {
                show_tooltip("Couldn't paste");
                return;
            }
            send_plain_ctrl(KEY_V);
            epoch
        };
        arm_paste_hold();
        if let Ok(mut slot) = self.index.lock() {
            *slot = next_index(index, clips.len());
        }
        show_tooltip(&format!("Pasted snippet [{shown}] of [{}]", clips.len()));
        self.emit();
        defer_restore(self.app.clone(), backup, epoch);
    }

    fn show_next(&self) {
        let clips = match self.clips.lock() {
            Ok(clips) => clips.clone(),
            Err(_) => return,
        };
        if clips.is_empty() {
            show_tooltip("No snippets stored");
            return;
        }
        let current = self.index.lock().map(|slot| *slot).unwrap_or(0);
        let next = next_index(current, clips.len());
        if let Ok(mut slot) = self.index.lock() {
            *slot = next;
        }
        show_tooltip(&format!(
            "Copied:\n{}\n\n Clip Location [{}] of [{}]",
            clips[next],
            next + 1,
            clips.len()
        ));
        self.emit();
    }

    fn clear(&self) {
        wait_paste_hold();
        bump_clip_epoch();
        if let Ok(mut clips) = self.clips.lock() {
            clips.clear();
        }
        if let Ok(mut index) = self.index.lock() {
            *index = 0;
        }
        show_tooltip("Snippets cleared");
        self.emit();
    }
}

const PASTE_HOLD: Duration = Duration::from_millis(16);
const COPY_WAIT: Duration = Duration::from_millis(250);
static CLIP_EPOCH: AtomicU64 = AtomicU64::new(0);
static PASTE_UNTIL: AtomicU64 = AtomicU64::new(0);
static CLIP_GATE: Mutex<()> = Mutex::new(());
static CLOCK: OnceLock<Instant> = OnceLock::new();

fn mono_millis() -> u64 {
    CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn bump_clip_epoch() -> u64 {
    CLIP_EPOCH.fetch_add(1, Ordering::Relaxed) + 1
}

fn wait_paste_hold() {
    let until = PASTE_UNTIL.load(Ordering::Relaxed);
    let now = mono_millis();
    if until > now {
        std::thread::sleep(Duration::from_millis(until - now));
    }
}

fn arm_paste_hold() {
    PASTE_UNTIL.store(
        mono_millis().saturating_add(PASTE_HOLD.as_millis() as u64),
        Ordering::Relaxed,
    );
}

fn defer_restore(app: AppHandle, backup: String, epoch: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(PASTE_HOLD);
        let _gate = CLIP_GATE.lock().unwrap_or_else(|error| error.into_inner());
        if CLIP_EPOCH.load(Ordering::Relaxed) != epoch {
            return;
        }
        suppress(&app, &backup);
        let _ = crate::clipboard::set_clipboard_text(&backup);
    });
}

fn next_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { (index + 1) % len }
}

const KEY_C: u16 = 0x43;
const KEY_V: u16 = 0x56;
const KEY_CAPS: u16 = 0x14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chord {
    Copy,
    Paste,
    Next,
    Clear,
}

fn chord_for(vk: u16, caps_held: bool, ctrl: bool, alt: bool, win: bool) -> Option<Chord> {
    if win {
        return None;
    }
    if alt && ctrl && vk == KEY_C {
        return Some(Chord::Copy);
    }
    if alt && ctrl && vk == KEY_V {
        return Some(Chord::Paste);
    }
    if alt && !ctrl && vk == KEY_CAPS {
        return Some(Chord::Next);
    }
    if caps_held && !alt && !ctrl && vk == KEY_C {
        return Some(Chord::Clear);
    }
    None
}

fn replay_caps_on_release(used_chord: bool) -> bool {
    !used_chord
}

fn copy_arrived(sequence: u32, after: u32, text: Option<&str>) -> bool {
    sequence != after && text.is_some_and(|text| !text.trim().is_empty())
}

fn tip_origin(
    cursor_x: i32,
    cursor_y: i32,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> (i32, i32) {
    const GAP: i32 = 16;
    let mut x = cursor_x.saturating_add(GAP);
    let mut y = cursor_y.saturating_add(GAP);
    if x.saturating_add(width) > right {
        x = cursor_x.saturating_sub(GAP + width);
    }
    if y.saturating_add(height) > bottom {
        y = cursor_y.saturating_sub(GAP + height);
    }
    if x < left {
        x = left;
    }
    if y < top {
        y = top;
    }
    let room_x = right.saturating_sub(width);
    let room_y = bottom.saturating_sub(height);
    if x > room_x {
        x = room_x;
    }
    if y > room_y {
        y = room_y;
    }
    if x < left {
        x = left;
    }
    if y < top {
        y = top;
    }
    (x, y)
}

fn copied_tip(text: &str, count: usize) -> String {
    let preview: String = text.chars().take(18).collect();
    let ellipsis = if text.chars().count() > 18 { "..." } else { "" };
    format!("Copied [{count}] {preview}{ellipsis}")
}

fn suppress(app: &AppHandle, text: &str) {
    if let Some(clipboard) = app.try_state::<crate::clipboard::Clipboard>() {
        clipboard.suppress_next(text.to_string());
    }
}

fn wait_for_copy(after: u32) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < COPY_WAIT {
        let sequence = clipboard_sequence();
        if sequence != after {
            let text = crate::clipboard::read_clipboard_text();
            if copy_arrived(sequence, after, text.as_deref()) {
                return text;
            }
        }
        if started.elapsed() < Duration::from_millis(8) {
            std::thread::yield_now();
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    None
}

fn clipboard_sequence() -> u32 {
    #[cfg(windows)]
    {
        unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

fn toggle_caps_lock() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_CAPITAL, keybd_event,
        };
        // Held across the injected press so the hook cannot treat it as another gesture.
        CAPS_REPLAY.store(true, Ordering::SeqCst);
        unsafe {
            keybd_event(VK_CAPITAL.0 as u8, 0x3A, KEYBD_EVENT_FLAGS(0), 0xCE70_0E01);
            keybd_event(VK_CAPITAL.0 as u8, 0x3A, KEYEVENTF_KEYUP, 0xCE70_0E01);
        }
        CAPS_REPLAY.store(false, Ordering::SeqCst);
    }
}

fn send_plain_ctrl(vk: u16) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RMENU, VK_RSHIFT,
        };
        let mut inputs = Vec::with_capacity(8);
        for modifier in [VK_LMENU.0, VK_RMENU.0, VK_LSHIFT.0, VK_RSHIFT.0] {
            if key_down(modifier) {
                inputs.push(key_input(modifier, true));
            }
        }
        inputs.push(key_input(VK_LCONTROL.0, false));
        inputs.push(key_input(vk, false));
        inputs.push(key_input(vk, true));
        inputs.push(key_input(VK_LCONTROL.0, true));
        KEYS_INJECT.store(true, Ordering::SeqCst);
        send_inputs(&inputs);
        KEYS_INJECT.store(false, Ordering::SeqCst);
    }
    #[cfg(not(windows))]
    {
        let _ = vk;
    }
}

#[cfg(windows)]
fn send_inputs(inputs: &[windows::Win32::UI::Input::KeyboardAndMouse::INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::SendInput(
            inputs,
            std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
    }
}

#[cfg(windows)]
fn key_input(vk: u16, up: bool) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, VIRTUAL_KEY,
    };
    let mapped = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX) };
    let mut flags = if up { KEYEVENTF_KEYUP.0 } else { 0 };
    if mapped & 0xFF00 != 0 {
        flags |= KEYEVENTF_EXTENDEDKEY.0;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: (mapped & 0xFF) as u16,
                dwFlags: KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0xCE70_0E01,
            },
        },
    }
}

#[cfg(windows)]
fn key_down(vk: u16) -> bool {
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) as u16 & 0x8000
            != 0
    }
}

fn spawn_hotkeys(inner: Arc<Inner>) {
    #[cfg(windows)]
    {
        std::thread::Builder::new()
            .name("megaclip-hotkeys".into())
            .spawn(move || hotkey_loop(inner))
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = inner;
    }
}

#[cfg(windows)]
fn hotkey_loop(inner: Arc<Inner>) {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
        PM_NOREMOVE, PeekMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    unsafe {
        let mut warmup = MSG::default();
        let _ = PeekMessageW(&mut warmup, None, 0, 0, PM_NOREMOVE);
    }
    HOOK_THREAD
        .set(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
        .ok();
    PENDING.set(Mutex::new(VecDeque::new())).ok();

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let message = wparam.0 as u32;
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if !injected_key(info) {
                let vk = info.vkCode as u16;
                let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
                let up = message == WM_KEYUP || message == WM_SYSKEYUP;
                release_stuck_modifiers();
                if down || up {
                    note_modifier(vk, down);
                }
                if vk == KEY_CAPS && down && !ctrl_held() && !win_held() {
                    if alt_held() {
                        CAPS_NEXT.store(true, Ordering::Relaxed);
                        let first = HELD_VK.swap(u32::from(vk), Ordering::Relaxed) != u32::from(vk);
                        if first {
                            enqueue(Action::Next);
                        }
                        return LRESULT(1);
                    }
                    if !CAPS_HELD.swap(true, Ordering::Relaxed) {
                        CAPS_USED.store(false, Ordering::Relaxed);
                    }
                    return LRESULT(1);
                }
                if vk == KEY_CAPS && up {
                    let held = CAPS_HELD.swap(false, Ordering::Relaxed);
                    let next = CAPS_NEXT.swap(false, Ordering::Relaxed);
                    if held || next {
                        let used = CAPS_USED.swap(false, Ordering::Relaxed);
                        if replay_caps_on_release(used || next) {
                            enqueue(Action::ReplayCaps);
                        }
                        if HELD_VK.load(Ordering::Relaxed) == u32::from(KEY_CAPS) {
                            HELD_VK.store(u32::MAX, Ordering::Relaxed);
                        }
                        return LRESULT(1);
                    }
                }
                if up && HELD_VK.load(Ordering::Relaxed) == u32::from(vk) {
                    HELD_VK.store(u32::MAX, Ordering::Relaxed);
                    return LRESULT(1);
                }
                if down {
                    if CAPS_HELD.load(Ordering::Relaxed)
                        && vk == KEY_C
                        && !ctrl_held()
                        && !alt_held()
                        && !win_held()
                    {
                        CAPS_USED.store(true, Ordering::Relaxed);
                        let first = HELD_VK.swap(u32::from(vk), Ordering::Relaxed) != u32::from(vk);
                        if first {
                            enqueue(Action::Clear);
                        }
                        return LRESULT(1);
                    }
                    if let Some(action) = hotkey_action(vk) {
                        let first = HELD_VK.swap(u32::from(vk), Ordering::Relaxed) != u32::from(vk);
                        if first {
                            enqueue(action);
                        }
                        return LRESULT(1);
                    }
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    unsafe {
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                eprintln!("ce-desktop-tools MegaClip hotkeys did not start: {error}");
                return;
            }
        };
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 != 0 {
            if message.message == WM_APP_ACTION {
                let action = PENDING
                    .get()
                    .and_then(|slot| slot.lock().ok().and_then(|mut pending| pending.pop_front()));
                if let Some(action) = action {
                    if matches!(action, Action::Clear | Action::ReplayCaps) {
                        let worker = Arc::clone(&inner);
                        std::thread::spawn(move || match action {
                            Action::Clear => worker.clear(),
                            Action::ReplayCaps => toggle_caps_lock(),
                            _ => {}
                        });
                        continue;
                    }
                    let worker = Arc::clone(&inner);
                    std::thread::spawn(move || {
                        let Ok(_busy) = worker.busy.try_lock() else {
                            return;
                        };
                        match action {
                            Action::Copy => worker.copy_selection(),
                            Action::Paste => worker.paste_current(),
                            Action::Next => worker.show_next(),
                            Action::Clear | Action::ReplayCaps => {}
                        }
                    });
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
const WM_APP_ACTION: u32 = 0x8000 + 3;

#[cfg(windows)]
const WM_APP_TIP: u32 = 0x8000 + 4;

#[cfg(windows)]
#[derive(Clone, Copy)]
enum Action {
    Copy,
    Paste,
    Next,
    Clear,
    ReplayCaps,
}

#[cfg(windows)]
static PENDING: std::sync::OnceLock<Mutex<VecDeque<Action>>> = std::sync::OnceLock::new();

#[cfg(windows)]
static HELD_VK: AtomicU32 = AtomicU32::new(u32::MAX);

#[cfg(windows)]
static CAPS_HELD: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static CAPS_USED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static CAPS_NEXT: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static CAPS_REPLAY: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static KEYS_INJECT: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static CTRL_L: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static CTRL_R: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static ALT_L: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static ALT_R: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static WIN_L: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static WIN_R: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static TIP_BUSY: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static TIP_AGAIN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
fn injected_key(info: &windows::Win32::UI::WindowsAndMessaging::KBDLLHOOKSTRUCT) -> bool {
    KEYS_INJECT.load(Ordering::SeqCst)
        || CAPS_REPLAY.load(Ordering::SeqCst)
        || info.dwExtraInfo == 0xCE70_0E01
        || info.flags.0 & 0x10 != 0
}

fn note_modifier(vk: u16, down: bool) {
    match vk {
        0xA2 => CTRL_L.store(down, Ordering::Relaxed),
        0xA3 => CTRL_R.store(down, Ordering::Relaxed),
        0x11 => {
            if down {
                CTRL_L.store(true, Ordering::Relaxed);
            } else {
                CTRL_L.store(false, Ordering::Relaxed);
                CTRL_R.store(false, Ordering::Relaxed);
            }
        }
        0xA4 => ALT_L.store(down, Ordering::Relaxed),
        0xA5 => ALT_R.store(down, Ordering::Relaxed),
        0x12 => {
            if down {
                ALT_L.store(true, Ordering::Relaxed);
            } else {
                ALT_L.store(false, Ordering::Relaxed);
                ALT_R.store(false, Ordering::Relaxed);
            }
        }
        0x5B => WIN_L.store(down, Ordering::Relaxed),
        0x5C => WIN_R.store(down, Ordering::Relaxed),
        _ => {}
    }
}

fn release_stuck_modifiers() {
    let pairs = [
        (0xA2, &CTRL_L),
        (0xA3, &CTRL_R),
        (0xA4, &ALT_L),
        (0xA5, &ALT_R),
        (0x5B, &WIN_L),
        (0x5C, &WIN_R),
    ];
    for (vk, flag) in pairs {
        if flag.load(Ordering::Relaxed) && !key_down(vk) {
            flag.store(false, Ordering::Relaxed);
        }
    }
}

fn ctrl_held() -> bool {
    CTRL_L.load(Ordering::Relaxed) || CTRL_R.load(Ordering::Relaxed)
}

fn alt_held() -> bool {
    ALT_L.load(Ordering::Relaxed) || ALT_R.load(Ordering::Relaxed)
}

fn win_held() -> bool {
    WIN_L.load(Ordering::Relaxed) || WIN_R.load(Ordering::Relaxed)
}

#[cfg(windows)]
fn enqueue(action: Action) {
    let Some(thread) = HOOK_THREAD.get().copied() else {
        return;
    };
    let Some(slot) = PENDING.get() else {
        return;
    };
    if slot
        .lock()
        .map(|mut pending| pending.push_back(action))
        .is_err()
    {
        return;
    }
    let posted = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            thread,
            WM_APP_ACTION,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        )
        .is_ok()
    };
    if !posted {
        if let Some(slot) = PENDING.get() {
            if let Ok(mut pending) = slot.lock() {
                pending.pop_back();
            }
        }
    }
}

#[cfg(windows)]
static TIP: std::sync::OnceLock<Mutex<Option<isize>>> = std::sync::OnceLock::new();

#[cfg(windows)]
static TIP_TEXT: std::sync::OnceLock<Mutex<String>> = std::sync::OnceLock::new();

#[cfg(windows)]
static HOOK_THREAD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

#[cfg(windows)]
static TIP_THREAD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

#[cfg(windows)]
fn spawn_tooltip() {
    std::thread::Builder::new()
        .name("megaclip-tip".into())
        .spawn(tooltip_loop)
        .ok();
}

#[cfg(windows)]
fn tooltip_loop() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, TranslateMessage, WM_TIMER,
    };
    unsafe {
        let mut warmup = MSG::default();
        let _ = PeekMessageW(&mut warmup, None, 0, 0, PM_NOREMOVE);
    }
    TIP_THREAD
        .set(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
        .ok();
    TIP.set(Mutex::new(None)).ok();
    TIP_TEXT.set(Mutex::new(String::new())).ok();
    register_tip_class();
    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).0 != 0 {
            if message.message == WM_APP_TIP {
                present_tooltip();
                continue;
            }
            if message.message == WM_TIMER {
                hide_tooltip();
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(windows)]
fn hotkey_action(vk: u16) -> Option<Action> {
    if PENDING.get().is_none() {
        return None;
    }
    let caps = CAPS_HELD.load(Ordering::Relaxed);
    let ctrl = ctrl_held();
    let alt = alt_held();
    let win = win_held();
    match chord_for(vk, caps, ctrl, alt, win) {
        Some(Chord::Copy) => Some(Action::Copy),
        Some(Chord::Paste) => Some(Action::Paste),
        Some(Chord::Next) => Some(Action::Next),
        Some(Chord::Clear) => Some(Action::Clear),
        None => None,
    }
}

#[cfg(windows)]
fn show_tooltip(text: &str) {
    let Some(thread) = TIP_THREAD.get() else {
        return;
    };
    if let Some(slot) = TIP_TEXT.get() {
        if let Ok(mut tip) = slot.lock() {
            *tip = text.to_string();
        }
    }
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            *thread,
            WM_APP_TIP,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

#[cfg(windows)]
fn register_tip_class() {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, COLOR_INFOBK, COLOR_INFOTEXT, CreateSolidBrush, DT_LEFT, DT_NOPREFIX,
        DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawTextW, EndPaint, FillRect,
        GetSysColor, HGDIOBJ, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, DefWindowProcW, RegisterClassW, WM_PAINT, WNDCLASSW,
    };
    use windows::core::w;

    unsafe extern "system" fn tip_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_PAINT {
            unsafe {
                let mut paint = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut paint);
                let mut rect = windows::Win32::Foundation::RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(GetSysColor(
                    COLOR_INFOBK,
                )));
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
                let text = TIP_TEXT
                    .get()
                    .and_then(|slot| slot.lock().ok().map(|tip| tip.clone()))
                    .unwrap_or_default();
                let mut wide: Vec<u16> = text.encode_utf16().collect();
                SetBkMode(hdc, TRANSPARENT);
                let _ = SetTextColor(
                    hdc,
                    windows::Win32::Foundation::COLORREF(GetSysColor(COLOR_INFOTEXT)),
                );
                let font = tip_font(hdc);
                let previous = SelectObject(hdc, font.into());
                rect.left += 12;
                rect.top += 8;
                rect.right -= 12;
                rect.bottom -= 6;
                if !wide.is_empty() && rect.right > rect.left && rect.bottom > rect.top {
                    let format = if text.contains('\n') {
                        DT_LEFT | DT_WORDBREAK | DT_NOPREFIX
                    } else {
                        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX
                    };
                    let _ = DrawTextW(hdc, &mut wide, &mut rect, format);
                }
                if !previous.is_invalid() {
                    let _ = SelectObject(hdc, previous);
                }
                let _ = DeleteObject(font.into());
                let _ = EndPaint(hwnd, &paint);
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    let class = w!("ce-desktop-tools-megaclip-tip");
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(tip_proc),
        hInstance: unsafe {
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .into()
        },
        lpszClassName: class,
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&window_class);
    }
}

#[cfg(windows)]
fn tip_font(hdc: windows::Win32::Graphics::Gdi::HDC) -> windows::Win32::Graphics::Gdi::HFONT {
    use windows::Win32::Graphics::Gdi::{
        CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
        FF_SWISS, FW_NORMAL, GetDeviceCaps, LOGPIXELSY, OUT_DEFAULT_PRECIS,
    };
    use windows::core::w;
    unsafe {
        let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSY).max(96);
        let height = -((9 * dpi) / 72).max(12);
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            u32::from(DEFAULT_PITCH.0) | u32::from(FF_SWISS.0),
            w!("Segoe UI"),
        )
    }
}

#[cfg(windows)]
fn tip_bounds(text: &str) -> (i32, i32) {
    let longest = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1) as i32;
    let lines = text.lines().count().max(1) as i32;
    (
        (longest * 7 + 32).clamp(220, 440),
        (lines * 18 + 22).clamp(52, 240),
    )
}

#[cfg(windows)]
fn work_area(point: windows::Win32::Foundation::POINT) -> (i32, i32, i32, i32) {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !monitor.is_invalid() && GetMonitorInfoW(monitor, &mut info).as_bool() {
            let work = info.rcWork;
            return (work.left, work.top, work.right, work.bottom);
        }
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        (
            left,
            top,
            left + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            top + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

#[cfg(windows)]
fn place_tip(hwnd: windows::Win32::Foundation::HWND, x: i32, y: i32, width: i32, height: i32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE,
        SWP_NOOWNERZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let top = (WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW).0 as isize;
        if style & top != top {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | top);
        }
        let quiet = SWP_NOACTIVATE | SWP_NOOWNERZORDER;
        let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), x, y, width, height, quiet);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            quiet | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(windows)]
fn present_tooltip() {
    if TIP_BUSY.swap(true, Ordering::Relaxed) {
        TIP_AGAIN.store(true, Ordering::Relaxed);
        return;
    }
    loop {
        TIP_AGAIN.store(false, Ordering::Relaxed);
        present_tooltip_once();
        if !TIP_AGAIN.swap(false, Ordering::Relaxed) {
            break;
        }
    }
    TIP_BUSY.store(false, Ordering::Relaxed);
}

#[cfg(windows)]
fn present_tooltip_once() {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, GetCursorPos, SetTimer, WS_BORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };
    use windows::core::w;

    unsafe {
        let text = TIP_TEXT
            .get()
            .and_then(|slot| slot.lock().ok().map(|tip| tip.clone()))
            .unwrap_or_default();
        let mut point = POINT::default();
        if GetCursorPos(&mut point).is_err() {
            return;
        }
        let (width, height) = tip_bounds(&text);
        let (left, top, right, bottom) = work_area(point);
        let (x, y) = tip_origin(point.x, point.y, width, height, left, top, right, bottom);
        let existing = TIP.get().and_then(|slot| {
            slot.lock().ok().and_then(|tip| {
                tip.map(|raw| windows::Win32::Foundation::HWND(raw as *mut std::ffi::c_void))
            })
        });
        let hwnd = if let Some(hwnd) = existing {
            hwnd
        } else {
            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("ce-desktop-tools-megaclip-tip"),
                w!("MegaClip"),
                WS_POPUP | WS_BORDER,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(
                    windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                        .unwrap_or_default()
                        .into(),
                ),
                None,
            ) else {
                return;
            };
            if let Some(slot) = TIP.get() {
                if let Ok(mut tip) = slot.lock() {
                    *tip = Some(hwnd.0 as isize);
                }
            }
            hwnd
        };
        place_tip(hwnd, x, y, width, height);
        let _ = InvalidateRect(Some(hwnd), None, true);
        let _ = SetTimer(Some(hwnd), 1, 1000, None);
    }
}

#[cfg(windows)]
fn hide_tooltip() {
    use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, KillTimer};
    let Some(raw) = TIP
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut tip| tip.take()))
    else {
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(raw as *mut std::ffi::c_void);
    unsafe {
        let _ = KillTimer(Some(hwnd), 1);
        let _ = DestroyWindow(hwnd);
    }
}

#[cfg(not(windows))]
fn show_tooltip(_text: &str) {}

#[cfg(test)]
mod tests {
    use super::{
        Chord, KEY_C, KEY_CAPS, KEY_V, chord_for, copied_tip, copy_arrived, next_index,
        replay_caps_on_release, tip_origin,
    };

    #[test]
    fn the_stack_wraps_to_the_first_snippet() {
        assert_eq!(next_index(0, 3), 1);
        assert_eq!(next_index(2, 3), 0);
        assert_eq!(next_index(0, 0), 0);
    }

    #[test]
    fn a_copy_counts_only_after_the_clipboard_changes() {
        assert!(!copy_arrived(3, 3, Some("same")));
        assert!(!copy_arrived(4, 3, Some("  ")));
        assert!(!copy_arrived(4, 3, None));
        assert!(copy_arrived(4, 3, Some("snippet")));
    }

    #[test]
    fn the_tip_stays_inside_the_work_area() {
        assert_eq!(tip_origin(100, 80, 180, 28, 0, 0, 1920, 1080), (116, 96));
        let (right_x, right_y) = tip_origin(1900, 400, 180, 28, 0, 0, 1920, 1080);
        assert_eq!((right_x, right_y), (1704, 416));
        assert!(right_x + 180 <= 1920);
        let (bottom_x, bottom_y) = tip_origin(40, 1060, 180, 28, 0, 0, 1920, 1080);
        assert_eq!((bottom_x, bottom_y), (56, 1016));
        assert!(bottom_y + 28 <= 1080);
        let (edge_x, edge_y) = tip_origin(-30, 40, 180, 28, -1920, 0, 0, 1080);
        assert!(edge_x >= -1920 && edge_x + 180 <= 0);
        assert_eq!(edge_y, 56);
    }

    #[test]
    fn caps_lock_c_is_recognized_only_while_caps_is_held() {
        assert_eq!(
            chord_for(KEY_C, true, false, false, false),
            Some(Chord::Clear)
        );
        assert_eq!(chord_for(KEY_C, false, false, false, false), None);
        assert_eq!(chord_for(KEY_C, true, true, false, false), None);
        assert_eq!(chord_for(KEY_C, true, false, true, false), None);
        assert_eq!(chord_for(KEY_C, true, false, false, true), None);
        assert_eq!(
            chord_for(KEY_C, false, true, true, false),
            Some(Chord::Copy)
        );
        assert_eq!(
            chord_for(KEY_V, false, true, true, false),
            Some(Chord::Paste)
        );
        assert_eq!(
            chord_for(KEY_CAPS, false, false, true, false),
            Some(Chord::Next)
        );
    }

    #[test]
    fn caps_lock_c_does_not_replay_the_key() {
        assert!(!replay_caps_on_release(true));
        assert!(replay_caps_on_release(false));
    }

    #[cfg(windows)]
    #[test]
    fn the_caps_key_code_matches_windows() {
        assert_eq!(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL.0,
            KEY_CAPS
        );
    }

    #[test]
    fn the_copy_tip_shortens_long_text() {
        assert_eq!(
            copied_tip("abcdefghijklmnopqrstuvwxyz0123456789", 2),
            "Copied [2] abcdefghijklmnopqr..."
        );
        assert_eq!(copied_tip("short", 1), "Copied [1] short");
    }
}
