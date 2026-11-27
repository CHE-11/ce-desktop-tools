//! Always On Top for any app window.
//!
//! The hotkey pins or unpins the window in front. A worker thread owns a
//! click-through layered border for each pinned window and follows it with a
//! WinEvent hook. This app's own main window keeps using `window_pin`, which
//! draws its own purple frame and keeps the toolbar button in step.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::hotkeys::{HotkeyId, usable_hotkey};

pub const DEFAULT_HOTKEY: &str = "Ctrl+Space";
const DEFAULT_BORDER_COLOR: &str = "#a855f7";
const COLOR_MODES: &[&str] = &["windows", "custom"];
const MAX_THICKNESS: u32 = 30;

static SETTINGS: Mutex<Option<AlwaysOnTopFile>> = Mutex::new(None);
static PINNED: Mutex<Option<HashSet<isize>>> = Mutex::new(None);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlwaysOnTopFile {
    pub enabled: bool,
    pub hotkey: String,
    pub skip_in_game_mode: bool,
    pub show_border: bool,
    pub border_color_mode: String,
    pub border_color: String,
    pub border_opacity: u32,
    pub border_thickness: u32,
    pub round_corners: bool,
    pub play_sound: bool,
    pub excluded_apps: Vec<String>,
}

impl Default for AlwaysOnTopFile {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
            skip_in_game_mode: true,
            show_border: true,
            border_color_mode: "windows".into(),
            border_color: DEFAULT_BORDER_COLOR.into(),
            border_opacity: 100,
            border_thickness: 5,
            round_corners: true,
            play_sound: true,
            excluded_apps: Vec::new(),
        }
    }
}

impl AlwaysOnTopFile {
    pub fn normalize(&mut self) {
        self.hotkey = usable_hotkey(&self.hotkey).unwrap_or_else(|| DEFAULT_HOTKEY.into());
        if !COLOR_MODES.contains(&self.border_color_mode.as_str()) {
            self.border_color_mode = "windows".into();
        }
        self.border_color = crate::overlay::parse_hex(&self.border_color)
            .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or_else(|| DEFAULT_BORDER_COLOR.into());
        self.border_opacity = self.border_opacity.clamp(1, 100);
        self.border_thickness = self.border_thickness.clamp(1, MAX_THICKNESS);
        let mut seen = HashSet::new();
        self.excluded_apps = self
            .excluded_apps
            .iter()
            .map(|app| app.trim().to_string())
            .filter(|app| !app.is_empty() && seen.insert(app.to_lowercase()))
            .collect();
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlwaysOnTopState {
    pub settings: AlwaysOnTopFile,
    pub accent_color: String,
    pub pinned_count: usize,
}

pub struct AlwaysOnTop {
    app: AppHandle,
}

impl AlwaysOnTop {
    pub fn start(app: AppHandle) -> Self {
        let this = Self { app };
        this.reload();
        #[cfg(windows)]
        {
            let _ = std::thread::Builder::new()
                .name("always-on-top".into())
                .spawn(worker::run);
        }
        this
    }

    pub fn state(&self) -> AlwaysOnTopState {
        AlwaysOnTopState {
            settings: settings(),
            accent_color: {
                let [r, g, b] = accent_color();
                format!("#{r:02x}{g:02x}{b:02x}")
            },
            pinned_count: pinned().len(),
        }
    }

    pub fn save(&self, mut next: AlwaysOnTopFile) -> Result<AlwaysOnTopState, String> {
        next.hotkey = usable_hotkey(&next.hotkey)
            .ok_or_else(|| "Pick a shortcut that uses Ctrl, Alt, or Win.".to_string())?;
        if next.enabled {
            crate::hotkeys::check_free(HotkeyId::AlwaysOnTop, &next.hotkey)?;
        }
        let saved = crate::app_settings::save_always_on_top(&self.app, next)?;
        apply(saved);
        Ok(self.state())
    }

    pub fn reload(&self) {
        let mut file = crate::app_settings::load_always_on_top(&self.app).unwrap_or_default();
        file.normalize();
        apply(file);
    }
}

fn apply(file: AlwaysOnTopFile) {
    crate::hotkeys::publish(HotkeyId::AlwaysOnTop, file.enabled, &file.hotkey);
    if let Ok(mut slot) = SETTINGS.lock() {
        *slot = Some(file);
    }
    post(worker::REDRAW, 0);
}

fn settings() -> AlwaysOnTopFile {
    SETTINGS
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

fn pinned() -> Vec<isize> {
    PINNED
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|set| set.iter().copied().collect()))
        .unwrap_or_default()
}

fn set_pinned(target: isize, on: bool) {
    if let Ok(mut slot) = PINNED.lock() {
        let set = slot.get_or_insert_with(HashSet::new);
        if on {
            set.insert(target);
        } else {
            set.remove(&target);
        }
    }
}

fn post(message: u32, value: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
        let thread = THREAD_ID.load(Ordering::SeqCst);
        if thread != 0 {
            unsafe {
                let _ = PostThreadMessageW(thread, message, WPARAM(value as usize), LPARAM(0));
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (message, value);
    }
}

/// Unpins every window this module pinned. Called on quit and from the page.
pub fn unpin_all() {
    post(worker::UNPIN_ALL, 0);
    #[cfg(windows)]
    for target in pinned() {
        worker::set_topmost(target, false);
        set_pinned(target, false);
    }
}

pub fn hotkey(app: &AppHandle) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetForegroundWindow};
        let file = settings();
        if !file.enabled {
            return;
        }
        if file.skip_in_game_mode && game_mode() {
            return;
        }
        let target = unsafe { GetAncestor(GetForegroundWindow(), GA_ROOT) };
        if target.0.is_null() {
            return;
        }
        if is_main_window(app, target.0 as isize) {
            let app = app.clone();
            std::thread::spawn(move || {
                let on = !crate::window_pin::current(&app);
                if crate::window_pin::set(&app, on).is_ok() && settings().play_sound {
                    worker::sound(on);
                }
            });
            return;
        }
        if crate::hotkeys::foreground_is_ours() {
            return;
        }
        if let Some(name) = crate::excel::foreground_exe_name() {
            if excluded(&file.excluded_apps, &name) {
                return;
            }
        }
        post(worker::TOGGLE, target.0 as isize);
    }
    #[cfg(not(windows))]
    let _ = app;
}

fn excluded(list: &[String], exe: &str) -> bool {
    let exe = exe.to_lowercase();
    let stem = exe.strip_suffix(".exe").unwrap_or(&exe).to_string();
    list.iter().any(|entry| {
        let entry = entry.trim().to_lowercase();
        let entry = entry.rsplit(['\\', '/']).next().unwrap_or(&entry).to_string();
        entry == exe || entry == stem
    })
}

#[cfg(windows)]
fn is_main_window(app: &AppHandle, hwnd: isize) -> bool {
    use tauri::Manager;
    app.get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|raw| raw.0 as isize == hwnd)
        .unwrap_or(false)
}

#[cfg(windows)]
fn game_mode() -> bool {
    use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
    unsafe { SHQueryUserNotificationState() }
        .map(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
        .unwrap_or(false)
}

fn accent_color() -> [u8; 3] {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
        let mut color = 0u32;
        let mut opaque = windows::core::BOOL(0);
        if unsafe { DwmGetColorizationColor(&mut color, &mut opaque) }.is_ok() {
            return [(color >> 16) as u8, (color >> 8) as u8, color as u8];
        }
    }
    [0x00, 0x78, 0xD4]
}

fn border_rgb(file: &AlwaysOnTopFile) -> [u8; 3] {
    if file.border_color_mode == "custom" {
        crate::overlay::parse_hex(&file.border_color).unwrap_or([0xA8, 0x55, 0xF7])
    } else {
        accent_color()
    }
}

/// Coverage of a pixel center by a ring between two rounded rectangles.
/// `inner` is the window, `thickness` grows it outward.
fn ring_coverage(px: f64, py: f64, inner: (f64, f64, f64, f64), radius: f64, thickness: f64) -> f64 {
    fn rounded_distance(px: f64, py: f64, rect: (f64, f64, f64, f64), radius: f64) -> f64 {
        let (left, top, right, bottom) = rect;
        let cx = (left + right) / 2.0;
        let cy = (top + bottom) / 2.0;
        let hx = (right - left) / 2.0 - radius;
        let hy = (bottom - top) / 2.0 - radius;
        let dx = (px - cx).abs() - hx;
        let dy = (py - cy).abs() - hy;
        let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
        outside + dx.max(dy).min(0.0) - radius
    }
    let (left, top, right, bottom) = inner;
    let outer = (left - thickness, top - thickness, right + thickness, bottom + thickness);
    let outside_outer = rounded_distance(px, py, outer, radius + thickness);
    let outside_inner = rounded_distance(px, py, inner, radius);
    let in_outer = (0.5 - outside_outer).clamp(0.0, 1.0);
    let out_inner = (outside_inner + 0.5).clamp(0.0, 1.0);
    in_outer * out_inner
}

#[cfg(windows)]
mod worker {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;

    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EVENT_OBJECT_DESTROY,
        EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW,
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART,
        GWL_EXSTYLE, GetClassNameW, GetMessageW, GetWindowLongPtrW, GetWindowRect, HTTRANSPARENT,
        HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsWindow, IsWindowVisible, MSG, RegisterClassW,
        SW_HIDE, SW_SHOWNOACTIVATE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetTimer, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow,
        WINEVENT_OUTOFCONTEXT, WM_APP, WM_NCHITTEST, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };
    use windows::core::{PCWSTR, w};

    use crate::screen_capture::Surface;

    pub const TOGGLE: u32 = WM_APP + 51;
    pub const REDRAW: u32 = WM_APP + 52;
    pub const UNPIN_ALL: u32 = WM_APP + 53;
    const CLASS_NAME: PCWSTR = w!("CeDesktopToolsPinBorder");
    const SKIPPED_CLASSES: &[&str] = &[
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "CeDesktopToolsPinBorder",
        "CeDesktopToolsOverlay",
    ];

    struct Border {
        hwnd: HWND,
        surface: Option<Surface>,
        size: (i32, i32),
    }

    thread_local! {
        static BORDERS: RefCell<HashMap<isize, Border>> = RefCell::new(HashMap::new());
    }

    pub fn run() {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class = WNDCLASSW {
                lpfnWndProc: Some(border_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassW(&class);
            super::THREAD_ID.store(
                windows::Win32::System::Threading::GetCurrentThreadId(),
                Ordering::SeqCst,
            );
            let system = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let objects = SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            SetTimer(None, 0, 1000, None);
            let mut message = MSG::default();
            loop {
                let status = GetMessageW(&mut message, None, 0, 0);
                if status.0 == 0 || status.0 == -1 {
                    break;
                }
                match message.message {
                    TOGGLE => toggle(message.wParam.0 as isize),
                    REDRAW => redraw_all(),
                    UNPIN_ALL => remove_all_borders(),
                    WM_TIMER => sweep(),
                    _ => {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
            let _ = UnhookWinEvent(system);
            let _ = UnhookWinEvent(objects);
        }
    }

    unsafe extern "system" fn border_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCHITTEST {
            return LRESULT(HTTRANSPARENT as isize);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        object: i32,
        child: i32,
        _thread: u32,
        _time: u32,
    ) {
        if event == EVENT_SYSTEM_FOREGROUND {
            restack_all();
            return;
        }
        if object != 0 || child != 0 {
            return;
        }
        let key = hwnd.0 as isize;
        let tracked = BORDERS.with(|borders| borders.borrow().contains_key(&key));
        if !tracked {
            if event == EVENT_OBJECT_DESTROY {
                super::set_pinned(key, false);
            }
            return;
        }
        match event {
            EVENT_OBJECT_DESTROY => {
                remove_border(key);
                super::set_pinned(key, false);
            }
            EVENT_SYSTEM_MINIMIZESTART | EVENT_OBJECT_HIDE => hide_border(key),
            EVENT_SYSTEM_MINIMIZEEND | EVENT_OBJECT_SHOW | EVENT_OBJECT_LOCATIONCHANGE => {
                place_border(key, false)
            }
            _ => {}
        }
    }

    fn class_of(hwnd: HWND) -> String {
        let mut buffer = [0u16; 128];
        let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }

    fn skipped(hwnd: HWND) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{GetDesktopWindow, GetShellWindow};
        unsafe {
            if hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
                return true;
            }
        }
        let class = class_of(hwnd);
        SKIPPED_CLASSES.contains(&class.as_str())
    }

    fn is_topmost(hwnd: HWND) -> bool {
        unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & (WS_EX_TOPMOST.0 as isize) != 0 }
    }

    pub fn set_topmost(target: isize, on: bool) {
        unsafe {
            let _ = SetWindowPos(
                HWND(target as *mut _),
                Some(if on { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
        }
    }

    pub fn sound(on: bool) {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
        let alias = if on {
            w!("DeviceConnect")
        } else {
            w!("DeviceDisconnect")
        };
        unsafe {
            let _ = PlaySoundW(alias, None, SND_ALIAS | SND_ASYNC | SND_NODEFAULT);
        }
    }

    fn toggle(target: isize) {
        let hwnd = HWND(target as *mut _);
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() || skipped(hwnd) {
            return;
        }
        let file = super::settings();
        let pinned_here = super::pinned().contains(&target);
        let on = !(pinned_here || is_topmost(hwnd));
        set_topmost(target, on);
        super::set_pinned(target, on);
        if on {
            if file.show_border {
                add_border(target);
            }
        } else {
            remove_border(target);
        }
        if file.play_sound {
            sound(on);
        }
    }

    fn add_border(target: isize) {
        if BORDERS.with(|borders| borders.borrow().contains_key(&target)) {
            place_border(target, true);
            return;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                CLASS_NAME,
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap_or_default().into()),
                None,
            )
        };
        let Ok(hwnd) = hwnd else {
            return;
        };
        BORDERS.with(|borders| {
            borders.borrow_mut().insert(
                target,
                Border {
                    hwnd,
                    surface: None,
                    size: (0, 0),
                },
            );
        });
        place_border(target, true);
    }

    fn remove_border(target: isize) {
        if let Some(border) = BORDERS.with(|borders| borders.borrow_mut().remove(&target)) {
            unsafe {
                let _ = DestroyWindow(border.hwnd);
            }
        }
    }

    fn remove_all_borders() {
        let keys: Vec<isize> = BORDERS.with(|borders| borders.borrow().keys().copied().collect());
        for key in keys {
            remove_border(key);
        }
    }

    fn hide_border(target: isize) {
        BORDERS.with(|borders| {
            if let Some(border) = borders.borrow().get(&target) {
                unsafe {
                    let _ = ShowWindow(border.hwnd, SW_HIDE);
                }
            }
        });
    }

    fn frame_bounds(target: HWND) -> Option<RECT> {
        let mut rect = RECT::default();
        let from_dwm = unsafe {
            DwmGetWindowAttribute(
                target,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut core::ffi::c_void,
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if from_dwm.is_err() && unsafe { GetWindowRect(target, &mut rect) }.is_err() {
            return None;
        }
        (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
    }

    fn place_border(target: isize, force: bool) {
        let target_hwnd = HWND(target as *mut _);
        let file = super::settings();
        BORDERS.with(|borders| {
            let mut borders = borders.borrow_mut();
            let Some(border) = borders.get_mut(&target) else {
                return;
            };
            let visible = unsafe {
                IsWindowVisible(target_hwnd).as_bool() && !IsIconic(target_hwnd).as_bool()
            };
            let Some(bounds) = frame_bounds(target_hwnd).filter(|_| visible) else {
                unsafe {
                    let _ = ShowWindow(border.hwnd, SW_HIDE);
                }
                return;
            };
            let dpi = unsafe { GetDpiForWindow(target_hwnd) }.max(96);
            let scale = f64::from(dpi) / 96.0;
            let thickness = (f64::from(file.border_thickness) * scale).round().max(1.0) as i32;
            let width = bounds.right - bounds.left + thickness * 2;
            let height = bounds.bottom - bounds.top + thickness * 2;
            let left = bounds.left - thickness;
            let top = bounds.top - thickness;
            if force || border.size != (width, height) || border.surface.is_none() {
                let Some(mut surface) = Surface::new(width, height) else {
                    return;
                };
                let radius = if file.round_corners { 8.0 * scale } else { 0.0 };
                paint_ring(&mut surface, thickness, radius, super::border_rgb(&file));
                border.surface = Some(surface);
                border.size = (width, height);
            }
            let Some(surface) = border.surface.as_ref() else {
                return;
            };
            let destination = POINT { x: left, y: top };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: ((file.border_opacity * 255) / 100) as u8,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            unsafe {
                let _ = UpdateLayeredWindow(
                    border.hwnd,
                    None,
                    Some(&destination),
                    Some(&size),
                    Some(surface.dc),
                    Some(&source),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
                let _ = ShowWindow(border.hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    border.hwnd,
                    Some(target_hwnd),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        });
    }

    fn paint_ring(surface: &mut Surface, thickness: i32, radius: f64, rgb: [u8; 3]) {
        let width = surface.width;
        let height = surface.height;
        let t = f64::from(thickness);
        let inner = (t, t, f64::from(width) - t, f64::from(height) - t);
        let reach = thickness + radius.ceil() as i32 + 1;
        let bytes = surface.bytes_mut();
        for y in 0..height {
            let near_edge_row = y < reach || y >= height - reach;
            let mut x = 0;
            while x < width {
                if !near_edge_row && x == reach {
                    x = width - reach;
                    continue;
                }
                let coverage = super::ring_coverage(
                    f64::from(x) + 0.5,
                    f64::from(y) + 0.5,
                    inner,
                    radius,
                    t,
                );
                let alpha = (coverage * 255.0).round() as u32;
                let index = ((y * width + x) * 4) as usize;
                bytes[index] = ((u32::from(rgb[2]) * alpha) / 255) as u8;
                bytes[index + 1] = ((u32::from(rgb[1]) * alpha) / 255) as u8;
                bytes[index + 2] = ((u32::from(rgb[0]) * alpha) / 255) as u8;
                bytes[index + 3] = alpha as u8;
                x += 1;
            }
        }
    }

    fn redraw_all() {
        let file = super::settings();
        let targets = super::pinned();
        for target in targets {
            if file.show_border {
                add_border(target);
            } else {
                remove_border(target);
            }
        }
    }

    fn restack_all() {
        let keys: Vec<isize> = BORDERS.with(|borders| borders.borrow().keys().copied().collect());
        for key in keys {
            BORDERS.with(|borders| {
                if let Some(border) = borders.borrow().get(&key) {
                    unsafe {
                        let _ = SetWindowPos(
                            border.hwnd,
                            Some(HWND(key as *mut _)),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                }
            });
        }
    }

    /// Drops windows that closed or that something else unpinned.
    fn sweep() {
        for target in super::pinned() {
            let hwnd = HWND(target as *mut _);
            let alive = unsafe { IsWindow(Some(hwnd)) }.as_bool();
            if !alive || !is_topmost(hwnd) {
                remove_border(target);
                super::set_pinned(target, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AlwaysOnTopFile, excluded, ring_coverage};

    #[test]
    fn an_old_file_without_the_section_gets_powertoys_defaults() {
        let mut file: AlwaysOnTopFile = serde_json::from_str("{}").unwrap();
        file.normalize();
        assert_eq!(file, AlwaysOnTopFile::default());
        assert_eq!(file.hotkey, "Ctrl+Space");
    }

    #[test]
    fn bad_values_fall_back_and_lists_are_cleaned() {
        let mut file: AlwaysOnTopFile = serde_json::from_str(
            r#"{"borderColorMode":"rainbow","borderColor":"nope","borderOpacity":0,"borderThickness":99,"excludedApps":[" Notepad.exe ","notepad.exe",""]}"#,
        )
        .unwrap();
        file.normalize();
        assert_eq!(file.border_color_mode, "windows");
        assert_eq!(file.border_color, "#a855f7");
        assert_eq!(file.border_opacity, 1);
        assert_eq!(file.border_thickness, 30);
        assert_eq!(file.excluded_apps, vec!["Notepad.exe"]);
    }

    #[test]
    fn excluded_apps_match_with_or_without_exe() {
        let list = vec!["notepad".to_string(), r"C:\Tools\Code.exe".to_string()];
        assert!(excluded(&list, "Notepad.exe"));
        assert!(excluded(&list, "code.exe"));
        assert!(!excluded(&list, "chrome.exe"));
    }

    #[test]
    fn the_ring_covers_the_band_outside_the_window_only() {
        let inner = (10.0, 10.0, 110.0, 60.0);
        assert_eq!(ring_coverage(5.5, 30.5, inner, 0.0, 10.0), 1.0);
        assert_eq!(ring_coverage(50.5, 30.5, inner, 0.0, 10.0), 0.0);
        assert_eq!(ring_coverage(-5.5, 30.5, inner, 0.0, 10.0), 0.0);
        // The outer corner of a rounded ring is cut away.
        assert_eq!(ring_coverage(0.5, 0.5, inner, 8.0, 10.0), 0.0);
    }
}
