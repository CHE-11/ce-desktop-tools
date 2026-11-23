//! Windows low-level keyboard hook.
//!
//! Typed keys stay in the matcher buffer and are never written down. The
//! completing key is consumed here. Another thread deletes the trigger and
//! types the replacement.
//!
//! `SendInput` must not run on this thread. Windows delivers hook calls by
//! waiting on the thread that installed the hook, and it deletes the hook,
//! with no notice, when that wait exceeds the low-level hook timeout. Typing
//! a replacement from this thread used to trip that timeout, and expansion
//! stayed dead until the process was started again. Sleep and unlocking the
//! workstation drop the hook the same way, so it is installed again there.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::Emitter;

use crate::expansion::matcher::{ExpansionHit, Matcher, Snippet};

struct InjectJob {
    backspaces: usize,
    text: String,
    ending: Option<char>,
    timed: bool,
    /// Delete the trigger, then open the window so the vault can be unlocked.
    unlock: bool,
}

pub struct HookState {
    pub snippets: std::sync::RwLock<Arc<Vec<Snippet>>>,
    pub enabled: AtomicBool,
    pub locked: AtomicBool,
    pub matcher: Mutex<Matcher>,
    pub app: Mutex<Option<tauri::AppHandle>>,
    pub last_notice: AtomicU64,
    pub our_pid: u32,
    pub thread_id: AtomicU32,
    /// Bumped while the hook thread is pumping and while the hook is called.
    pub beat: AtomicU64,
    /// Hook threads currently inside `run`, including one stuck in a callback.
    pub threads: AtomicU32,
    pub stopped: AtomicBool,
    /// Set while the hook handle is being replaced, so a key is not expanded twice.
    swapping: AtomicBool,
    /// Jobs the callback queued. The hook thread hands them to the worker only
    /// after the callback has returned, so `SendInput` is never nested inside it.
    queued: Mutex<Vec<InjectJob>>,
    inject_tx: Mutex<Option<Sender<InjectJob>>>,
}

impl HookState {
    pub fn new() -> Self {
        Self {
            snippets: std::sync::RwLock::new(Arc::new(Vec::new())),
            enabled: AtomicBool::new(true),
            locked: AtomicBool::new(true),
            matcher: Mutex::new(Matcher::default()),
            app: Mutex::new(None),
            last_notice: AtomicU64::new(0),
            our_pid: std::process::id(),
            thread_id: AtomicU32::new(0),
            beat: AtomicU64::new(0),
            threads: AtomicU32::new(0),
            stopped: AtomicBool::new(false),
            swapping: AtomicBool::new(false),
            queued: Mutex::new(Vec::new()),
            inject_tx: Mutex::new(None),
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        let previous = self.last_notice.load(Ordering::Relaxed);
        if now.saturating_sub(previous) < 1500 {
            return;
        }
        self.last_notice.store(now, Ordering::Relaxed);
        // Show first so the pin field is focused in a window that is already open,
        // including one that was only sitting in the tray.
        crate::show_main(&app);
        let _ = app.emit("expansion-unlock-request", ());
    }

    pub fn replace_snippets(&self, snippets: Vec<Snippet>) {
        match self.matcher.lock() {
            Ok(mut matcher) => {
                matcher.set_cap_from(&snippets);
                matcher.clear();
            }
            Err(poisoned) => {
                let mut matcher = poisoned.into_inner();
                matcher.set_cap_from(&snippets);
                matcher.clear();
            }
        }
        let shared = Arc::new(snippets);
        match self.snippets.write() {
            Ok(mut slot) => *slot = shared,
            Err(poisoned) => *poisoned.into_inner() = shared,
        }
    }
}

#[cfg(windows)]
pub fn spawn(state: Arc<HookState>) {
    let (tx, rx) = mpsc::channel();
    let injector = Arc::clone(&state);
    let worker = std::thread::Builder::new()
        .name("expansion-inject".into())
        .spawn(move || inject_loop(injector, rx));
    if worker.is_ok() {
        match state.inject_tx.lock() {
            Ok(mut slot) => *slot = Some(tx),
            Err(poisoned) => *poisoned.into_inner() = Some(tx),
        }
    }
    let hook = Arc::clone(&state);
    let _ = std::thread::Builder::new()
        .name("expansion-hook".into())
        .spawn(move || run(hook));
    let _ = std::thread::Builder::new()
        .name("expansion-hook-watch".into())
        .spawn(move || watch(state));
}

#[cfg(not(windows))]
pub fn spawn(_state: Arc<HookState>) {}

#[cfg(windows)]
pub fn stop(state: &HookState) {
    state.stopped.store(true, Ordering::SeqCst);
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
const INJECT_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
#[cfg(windows)]
const REHOOK_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;
#[cfg(windows)]
const REHOOK_EVERY: u32 = 10;
#[cfg(windows)]
const TIMER_ID: usize = 1;

#[cfg(windows)]
struct ThreadCount<'a>(&'a HookState);

#[cfg(windows)]
impl Drop for ThreadCount<'_> {
    fn drop(&mut self) {
        self.0.threads.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn run(state: Arc<HookState>) {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, KillTimer, MSG, TranslateMessage, UnhookWindowsHookEx,
    };

    if state.stopped.load(Ordering::SeqCst) {
        return;
    }
    let my_id = unsafe { GetCurrentThreadId() };
    state.thread_id.store(my_id, Ordering::SeqCst);
    state.threads.fetch_add(1, Ordering::SeqCst);
    let _alive = ThreadCount(&state);
    STATE.set(Arc::clone(&state)).ok();
    state.beat.fetch_add(1, Ordering::Relaxed);
    ensure_power_notify();

    let mut hook = match install_hook() {
        Ok(hook) => hook,
        Err(error) => {
            eprintln!("ce-desktop-tools text expansion did not start: {error}");
            windows::Win32::UI::WindowsAndMessaging::HHOOK(std::ptr::null_mut())
        }
    };
    let window = open_session_window();
    let thread_timer = window.is_none() && arm_thread_timer();
    let mut ticks = 0u32;
    let mut message = MSG::default();
    unsafe {
        loop {
            let status = GetMessageW(&mut message, None, 0, 0);
            state.beat.fetch_add(1, Ordering::Relaxed);
            if state.stopped.load(Ordering::SeqCst) || status.0 == 0 {
                break;
            }
            if status.0 == -1 {
                flush_jobs(&state);
                continue;
            }
            if state.thread_id.load(Ordering::SeqCst) != my_id {
                break;
            }
            if hook_needs_reinstall(
                message.message,
                message.wParam.0,
                !hook.is_invalid(),
                &mut ticks,
            ) {
                reinstall(&state, &mut hook);
            }
            if message.message != INJECT_MESSAGE {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // Hand replacements to the worker only once this thread is about to
            // wait again. SendInput from inside the callback, or before this
            // wait, makes Windows drop the hook.
            flush_jobs(&state);
        }
        state.swapping.store(true, Ordering::SeqCst);
        if !hook.is_invalid() {
            let _ = UnhookWindowsHookEx(hook);
        }
        state.swapping.store(false, Ordering::SeqCst);
        if let Some(hwnd) = window {
            let _ = KillTimer(Some(hwnd), TIMER_ID);
            let _ = windows::Win32::System::RemoteDesktop::WTSUnRegisterSessionNotification(hwnd);
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
        } else if thread_timer {
            let _ = KillTimer(None, TIMER_ID);
        }
    }
}

#[cfg(windows)]
fn watch(state: Arc<HookState>) {
    let mut seen = state.beat.load(Ordering::Relaxed);
    let mut still = 0u32;
    loop {
        std::thread::sleep(Duration::from_secs(2));
        if state.stopped.load(Ordering::SeqCst) {
            return;
        }
        let beat = state.beat.load(Ordering::Relaxed);
        if beat != seen {
            seen = beat;
            still = 0;
            continue;
        }
        still += 1;
        // A stuck callback never comes back to this loop. Retry a few times,
        // then stop so those threads do not pile up.
        if still < 3 || state.threads.load(Ordering::SeqCst) >= 8 {
            continue;
        }
        still = 0;
        eprintln!("ce-desktop-tools text expansion hook stalled; starting it again");
        let next = Arc::clone(&state);
        let _ = std::thread::Builder::new()
            .name("expansion-hook".into())
            .spawn(move || run(next));
    }
}

#[cfg(windows)]
fn inject_loop(state: Arc<HookState>, jobs: mpsc::Receiver<InjectJob>) {
    while let Ok(job) = jobs.recv() {
        let mut text = if job.timed {
            crate::date_expand::render_now(&job.text)
        } else {
            crate::date_expand::expand_variables(&job.text)
        };
        if let Some(ch) = job.ending {
            text.push(ch);
        }
        inject(job.backspaces, &text);
        if job.unlock {
            // The backspaces are still in the other app's queue. Opening now would
            // take focus first and delete the trigger in this window instead.
            std::thread::sleep(Duration::from_millis(60));
            state.prompt_unlock();
        }
    }
}

/// A live hook is replaced on a slow timer. A missing hook is replaced on the
/// next tick. Unlock and resume ask for a replacement immediately.
#[cfg(windows)]
fn hook_needs_reinstall(message: u32, wparam: usize, hook_up: bool, ticks: &mut u32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_TIMER, WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT, WTS_REMOTE_CONNECT, WTS_SESSION_UNLOCK,
    };
    if message == REHOOK_MESSAGE {
        return true;
    }
    if message == WM_WTSSESSION_CHANGE {
        let event = wparam as u32;
        return event == WTS_SESSION_UNLOCK
            || event == WTS_CONSOLE_CONNECT
            || event == WTS_REMOTE_CONNECT;
    }
    if message != WM_TIMER {
        return false;
    }
    *ticks = ticks.wrapping_add(1);
    if !hook_up {
        return true;
    }
    *ticks % REHOOK_EVERY == 0
}

#[cfg(windows)]
fn reinstall(state: &HookState, hook: &mut windows::Win32::UI::WindowsAndMessaging::HHOOK) {
    use windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx;
    state.swapping.store(true, Ordering::SeqCst);
    unsafe {
        if !hook.is_invalid() {
            let _ = UnhookWindowsHookEx(*hook);
            *hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(std::ptr::null_mut());
        }
        match install_hook() {
            Ok(next) => *hook = next,
            Err(error) => {
                eprintln!("ce-desktop-tools text expansion hook dropped: {error}");
            }
        }
    }
    state.swapping.store(false, Ordering::SeqCst);
}

#[cfg(windows)]
fn ensure_power_notify() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Power::{
            DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, RegisterSuspendResumeNotification,
        };
        use windows::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_CALLBACK;

        let params = Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(on_resume),
            Context: std::ptr::null_mut(),
        }));
        let recipient = HANDLE(params as *mut _ as *mut core::ffi::c_void);
        if RegisterSuspendResumeNotification(recipient, DEVICE_NOTIFY_CALLBACK).is_err() {
            eprintln!("ce-desktop-tools could not watch resume for text expansion");
        }
    });
}

#[cfg(windows)]
unsafe extern "system" fn on_resume(
    _context: *const core::ffi::c_void,
    kind: u32,
    _setting: *const core::ffi::c_void,
) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND};
    if kind == PBT_APMRESUMEAUTOMATIC || kind == PBT_APMRESUMESUSPEND {
        post_rehook();
    }
    0
}

#[cfg(windows)]
fn post_rehook() {
    let Some(state) = STATE.get() else {
        return;
    };
    let thread_id = state.thread_id.load(Ordering::SeqCst);
    if thread_id == 0 {
        return;
    }
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
        let _ = PostThreadMessageW(thread_id, REHOOK_MESSAGE, WPARAM(0), LPARAM(0));
    }
}

#[cfg(windows)]
fn open_session_window() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::RemoteDesktop::{
        NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CreateWindowExW, DefWindowProcW, HWND_MESSAGE, RegisterClassW, SetTimer,
        WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPED,
    };
    use windows::core::w;

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let class = w!("ce-desktop-tools-expansion");
        let instance = GetModuleHandleW(None).unwrap_or_default().into();
        let window_class = WNDCLASSW {
            style: CS_HREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            let error = windows::Win32::Foundation::GetLastError();
            if error.0 != 1410 {
                eprintln!("ce-desktop-tools could not watch session unlock for text expansion");
            }
        }
    });

    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("ce-desktop-tools-expansion"),
            w!("expansion"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
        .ok()?;
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
            eprintln!("ce-desktop-tools could not watch session unlock for text expansion");
        }
        if SetTimer(Some(hwnd), TIMER_ID, 1000, None) == 0 {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            return None;
        }
        Some(hwnd)
    }
}

#[cfg(windows)]
fn arm_thread_timer() -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetTimer;

    unsafe extern "system" fn thread_timer(_: HWND, _: u32, _: usize, _: u32) {}

    unsafe { SetTimer(None, TIMER_ID, 1000, Some(thread_timer)) != 0 }
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
    let Some(state) = STATE.get() else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };
    let current = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    if current != state.thread_id.load(Ordering::SeqCst) || state.swapping.load(Ordering::SeqCst) {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    state.beat.fetch_add(1, Ordering::Relaxed);
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
    if !state.enabled.load(Ordering::Relaxed) || foreground_is_us(state.our_pid) || menu_is_open() {
        clear_matcher(state);
        return false;
    }
    let vk = info.vkCode as u16;
    if shortcut_clears_buffer(vk) {
        clear_matcher(state);
        return false;
    }
    if vk == windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK.0 {
        lock_matcher(state).backspace();
        return false;
    }
    let Some(ch) = key_char(vk, info.scanCode) else {
        if !is_modifier(vk) {
            clear_matcher(state);
        }
        return false;
    };
    // Clone the list pointer only. Copying every replacement here made the
    // callback slow enough for Windows to remove the hook.
    let snippets = match state.snippets.read() {
        Ok(slot) => Arc::clone(&slot),
        Err(poisoned) => Arc::clone(&poisoned.into_inner()),
    };
    let mut matcher = lock_matcher(state);
    let Some(hit) = matcher.push(ch, &snippets) else {
        return false;
    };
    if hit.secret {
        if !schedule_unlock(state, &hit) {
            return false;
        }
        matcher.clear();
        return true;
    }
    if !schedule(state, &hit) {
        return false;
    }
    matcher.clear();
    true
}

#[cfg(windows)]
fn lock_matcher(state: &HookState) -> std::sync::MutexGuard<'_, Matcher> {
    match state.matcher.lock() {
        Ok(matcher) => matcher,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(windows)]
fn clear_matcher(state: &HookState) {
    lock_matcher(state).clear();
}

#[cfg(windows)]
fn schedule_unlock(state: &HookState, hit: &ExpansionHit) -> bool {
    enqueue(
        state,
        InjectJob {
            backspaces: clear_count(&hit.trigger, hit.ending.is_some()),
            text: String::new(),
            ending: None,
            timed: false,
            unlock: true,
        },
    )
}

#[cfg(windows)]
fn schedule(state: &HookState, hit: &ExpansionHit) -> bool {
    enqueue(
        state,
        InjectJob {
            backspaces: clear_count(&hit.trigger, hit.ending.is_some()),
            text: hit.text.clone(),
            ending: hit.ending,
            timed: hit.timed,
            unlock: false,
        },
    )
}

#[cfg(windows)]
fn enqueue(state: &HookState, job: InjectJob) -> bool {
    let ready = match state.inject_tx.lock() {
        Ok(slot) => slot.is_some(),
        Err(poisoned) => poisoned.into_inner().is_some(),
    };
    if !ready {
        return false;
    }
    match state.queued.lock() {
        Ok(mut slot) => slot.push(job),
        Err(poisoned) => poisoned.into_inner().push(job),
    }
    let thread_id = state.thread_id.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
            let _ = PostThreadMessageW(thread_id, INJECT_MESSAGE, WPARAM(0), LPARAM(0));
        }
    }
    true
}

#[cfg(windows)]
fn flush_jobs(state: &HookState) {
    let jobs = match state.queued.lock() {
        Ok(mut slot) => std::mem::take(&mut *slot),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    if jobs.is_empty() {
        return;
    }
    let tx = match state.inject_tx.lock() {
        Ok(slot) => slot.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let Some(tx) = tx else {
        return;
    };
    for job in jobs {
        if tx.send(job).is_err() {
            break;
        }
    }
}

/// Characters of a locked trigger already on screen. The completing key is
/// swallowed, so it is not counted unless it was only an ending character.
#[cfg(windows)]
fn clear_count(trigger: &str, ended: bool) -> usize {
    let count = trigger.chars().count();
    if ended {
        count
    } else {
        count.saturating_sub(1)
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
        VIRTUAL_KEY, VK_BACK, VK_LEFT, VK_RETURN, VK_SPACE, VK_TAB,
    };

    let mut inputs = Vec::new();
    for _ in 0..backspaces {
        push_vk(&mut inputs, VK_BACK, false);
        push_vk(&mut inputs, VK_BACK, true);
    }
    let mut after_cursor: Option<usize> = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == crate::date_expand::CURSOR {
            if after_cursor.is_none() {
                after_cursor = Some(0);
            }
            continue;
        }
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
            if let Some(left) = after_cursor.as_mut() {
                *left += count.saturating_sub(1);
            }
        }
        if let Some(left) = after_cursor.as_mut() {
            *left += 1;
        }
    }
    if let Some(left) = after_cursor {
        for _ in 0..left {
            push_vk(&mut inputs, VK_LEFT, false);
            push_vk(&mut inputs, VK_LEFT, true);
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
    use super::{ascii_from_vk, boundary_char, clear_count};

    #[test]
    fn a_locked_trigger_is_cleared_before_the_window_opens() {
        assert_eq!(clear_count(":lock", false), 4);
        assert_eq!(clear_count(":lock", true), 5);
        assert_eq!(clear_count("a", false), 0);
    }

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

    #[test]
    fn the_hook_is_replaced_after_unlock_and_when_it_is_gone() {
        use windows::Win32::UI::WindowsAndMessaging::{
            WM_KEYDOWN, WM_TIMER, WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT, WTS_SESSION_LOCK,
            WTS_SESSION_UNLOCK,
        };
        let mut ticks = 0u32;
        for _ in 0..9 {
            assert!(!super::hook_needs_reinstall(WM_TIMER, 1, true, &mut ticks));
        }
        assert!(super::hook_needs_reinstall(WM_TIMER, 1, true, &mut ticks));
        assert!(super::hook_needs_reinstall(WM_TIMER, 1, false, &mut ticks));
        assert!(super::hook_needs_reinstall(
            WM_WTSSESSION_CHANGE,
            WTS_SESSION_UNLOCK as usize,
            true,
            &mut ticks
        ));
        assert!(super::hook_needs_reinstall(
            WM_WTSSESSION_CHANGE,
            WTS_CONSOLE_CONNECT as usize,
            true,
            &mut ticks
        ));
        assert!(!super::hook_needs_reinstall(
            WM_WTSSESSION_CHANGE,
            WTS_SESSION_LOCK as usize,
            true,
            &mut ticks
        ));
        assert!(super::hook_needs_reinstall(
            super::REHOOK_MESSAGE,
            0,
            true,
            &mut ticks
        ));
        assert!(!super::hook_needs_reinstall(
            WM_KEYDOWN, 0, false, &mut ticks
        ));
    }
}
