//! Stage windows, one per monitor, plus the palette. A pump thread posts a sync to the main
//! thread every few milliseconds: it sends each stage the pointer in that monitor's CSS
//! pixels, moves the lens, hides the system cursor, and keeps the stages click-through.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GWL_EXSTYLE, GetCursorPos, MONITORINFOF_PRIMARY, GetWindowLongPtrW, GetWindowRect, HTTRANSPARENT,
    HWND_TOPMOST, IsWindow, IsWindowVisible, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos,
    WDA_EXCLUDEFROMCAPTURE, WM_NCHITTEST, WS_EX_TRANSPARENT,
};
use windows::core::BOOL;

use super::lens::{self, ScreenBox};
use super::{PresentPointer, PresentRect};

pub const PALETTE: &str = "present-palette";
const STAGE_PREFIX: &str = "present-stage-";
const STAGE_PASS: usize = 0xce_0e51;
/// More monitors than this are not given a stage.
const MAX_STAGES: usize = 12;
/// Logical px. The stage draws the same box; the lens window is this size times the scale.
const LENS_W: f64 = 280.0;
const LENS_H: f64 = 168.0;
const ZOOM: f32 = 2.0;
const LENS_INSET: f64 = 3.0;
const PALETTE_W: f64 = 780.0;
const PALETTE_H: f64 = 84.0;
const PALETTE_GAP: f64 = 28.0;

static CAPTURE: AtomicBool = AtomicBool::new(false);
static PLACEABLE: AtomicBool = AtomicBool::new(false);
static FAST: AtomicBool = AtomicBool::new(false);
static BLANK: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static POSTED: AtomicBool = AtomicBool::new(false);
static PALETTE_HWND: AtomicIsize = AtomicIsize::new(0);
static COUNT: AtomicUsize = AtomicUsize::new(0);
/// Last click-through state pushed to the stages: 0 unknown, 1 through, 2 taking clicks.
static THROUGH: AtomicUsize = AtomicUsize::new(0);
static TICK: AtomicU64 = AtomicU64::new(0);
static SCREENS: Mutex<Vec<Screen>> = Mutex::new(Vec::new());
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub bounds: ScreenBox,
    pub work: ScreenBox,
    pub scale: f64,
}

pub fn label(index: usize) -> String {
    format!("{STAGE_PREFIX}{index}")
}

pub fn set_flags(capture: bool, placeable: bool, fast: bool, blank: bool) {
    CAPTURE.store(capture, Ordering::Relaxed);
    PLACEABLE.store(placeable, Ordering::Relaxed);
    FAST.store(fast, Ordering::Relaxed);
    BLANK.store(blank, Ordering::Relaxed);
    THROUGH.store(0, Ordering::Relaxed);
}

pub fn monitor_count() -> usize {
    COUNT.load(Ordering::Relaxed)
}

/// Monitors with the primary first, then left to right.
pub fn screens() -> Vec<Screen> {
    let mut found: Vec<(bool, Screen)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut found as *mut Vec<(bool, Screen)> as isize),
        );
    }
    found.sort_by_key(|(primary, screen)| (!*primary, screen.bounds.x, screen.bounds.y));
    found.into_iter().map(|(_, screen)| screen).take(MAX_STAGES).collect()
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    param: LPARAM,
) -> BOOL {
    let found = unsafe { &mut *(param.0 as *mut Vec<(bool, Screen)>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return BOOL(1);
    }
    let mut dpi_x = 96;
    let mut dpi_y = 96;
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let boxed = |rect: RECT| ScreenBox {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    };
    found.push((
        info.dwFlags & MONITORINFOF_PRIMARY != 0,
        Screen {
            bounds: boxed(info.rcMonitor),
            work: boxed(info.rcWork),
            scale: (f64::from(dpi_x.max(48)) / 96.0).max(0.5),
        },
    ));
    BOOL(1)
}

/// Opens a stage on every monitor and the palette on the one under the pointer. Call it off
/// the main thread: window creation waits for the main thread.
pub fn open(app: &AppHandle) -> Result<(), String> {
    remember_main(app)?;
    let list = screens();
    if list.is_empty() {
        return Err("No monitors were found.".into());
    }
    fit_stages(app, &list)?;
    open_palette(app, &list)?;
    on_main(app, |app| {
        if let Err(error) = lens::ensure() {
            eprintln!("ce-desktop-tools present lens: {error}");
        }
        prepare_windows(app);
    })?;
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    start_pump(app.clone(), generation);
    Ok(())
}

pub fn close(app: &AppHandle) {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    set_flags(false, false, false, false);
    PALETTE_HWND.store(0, Ordering::Relaxed);
    let _ = on_main(app, |_| lens::destroy());
    for index in 0..MAX_STAGES {
        if let Some(window) = app.get_webview_window(&label(index)) {
            let _ = window.destroy();
        }
    }
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.destroy();
    }
    if let Ok(mut slot) = SCREENS.lock() {
        slot.clear();
    }
    COUNT.store(0, Ordering::Relaxed);
}

/// For app exit, which already runs on the main thread.
pub fn shutdown() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    lens::destroy();
}

fn remember_main(app: &AppHandle) -> Result<(), String> {
    if MAIN_THREAD.get().is_some() {
        return Ok(());
    }
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(thread::current().id());
    })
    .map_err(|error| error.to_string())?;
    let id = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    let _ = MAIN_THREAD.set(id);
    Ok(())
}

fn on_main<R: Send + 'static>(
    app: &AppHandle,
    task: impl FnOnce(&AppHandle) -> R + Send + 'static,
) -> Result<R, String> {
    if MAIN_THREAD.get() == Some(&thread::current().id()) {
        return Ok(task(app));
    }
    let (tx, rx) = mpsc::channel();
    let inside = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(task(&inside));
    })
    .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|error| error.to_string())
}

/// One stage per monitor, each sized to its monitor in physical pixels.
fn fit_stages(app: &AppHandle, list: &[Screen]) -> Result<(), String> {
    for (index, screen) in list.iter().enumerate() {
        let window = match app.get_webview_window(&label(index)) {
            Some(window) => window,
            None => build(app, &label(index), &format!("window.__PRESENT_STAGE__ = {index}"))?,
        };
        place(&window, screen.bounds);
    }
    for index in list.len()..MAX_STAGES {
        if let Some(window) = app.get_webview_window(&label(index)) {
            let _ = window.destroy();
        }
    }
    if let Ok(mut slot) = SCREENS.lock() {
        *slot = list.to_vec();
    }
    COUNT.store(list.len(), Ordering::Relaxed);
    THROUGH.store(0, Ordering::Relaxed);
    Ok(())
}

fn open_palette(app: &AppHandle, list: &[Screen]) -> Result<(), String> {
    let window = match app.get_webview_window(PALETTE) {
        Some(window) => window,
        None => build(app, PALETTE, "window.__PRESENT_PALETTE__ = true")?,
    };
    let (x, y) = cursor();
    let screen = list
        .iter()
        .find(|screen| contains(screen.bounds, x, y))
        .unwrap_or(&list[0]);
    let width = (PALETTE_W * screen.scale).round() as i32;
    let height = (PALETTE_H * screen.scale).round() as i32;
    let gap = (PALETTE_GAP * screen.scale).round() as i32;
    let work = screen.work;
    let width = width.min(work.width);
    place(
        &window,
        ScreenBox {
            x: work.x + (work.width - width) / 2,
            y: work.y + work.height - height - gap,
            width,
            height,
        },
    );
    if let Some(hwnd) = hwnd_of(&window) {
        PALETTE_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        unsafe {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
    }
    Ok(())
}

fn build(app: &AppHandle, label: &str, script: &str) -> Result<WebviewWindow, String> {
    tauri::WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Present")
        .inner_size(320.0, 120.0)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .always_on_top(true)
        .transparent(true)
        .disable_drag_drop_handler()
        .initialization_script(script)
        .build()
        .map_err(|error| error.to_string())
}

fn place(window: &WebviewWindow, rect: ScreenBox) {
    // Moving to a monitor with another scale resizes the window once it lands, so size it again.
    let size = PhysicalSize::new(rect.width.max(1) as u32, rect.height.max(1) as u32);
    let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    let _ = window.set_size(size);
    let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
    let _ = window.set_size(size);
}

/// Main thread: shows the windows and hooks the stage hit tests. A raw `ShowWindow` would leave
/// the webview inside hidden, so this goes through Tauri.
fn prepare_windows(app: &AppHandle) {
    for index in 0..monitor_count() {
        if let Some(window) = app.get_webview_window(&label(index)) {
            let _ = window.show();
            pass_clicks(&window);
        }
    }
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.show();
    }
    THROUGH.store(0, Ordering::Relaxed);
    restack(app, None);
}

fn start_pump(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        let mut checked = Instant::now();
        while super::is_on() && GENERATION.load(Ordering::Relaxed) == generation {
            if checked.elapsed() > Duration::from_millis(1500) {
                checked = Instant::now();
                refit(&app);
            }
            if !POSTED.swap(true, Ordering::AcqRel) {
                let inside = app.clone();
                let posted = app.run_on_main_thread(move || {
                    POSTED.store(false, Ordering::Release);
                    if super::is_on() && GENERATION.load(Ordering::Relaxed) == generation {
                        sync(&inside);
                    }
                });
                if posted.is_err() {
                    POSTED.store(false, Ordering::Release);
                }
            }
            let wait = if FAST.load(Ordering::Relaxed) { 8 } else { 32 };
            thread::sleep(Duration::from_millis(wait));
        }
    });
}

/// Follows monitors being plugged in, removed, or rearranged.
fn refit(app: &AppHandle) {
    let list = screens();
    let same = SCREENS.lock().map(|slot| *slot == list).unwrap_or(true);
    if same || list.is_empty() {
        return;
    }
    if fit_stages(app, &list).is_ok() {
        let _ = on_main(app, prepare_windows);
        let _ = app.emit("present-changed", super::state());
    }
}

fn sync(app: &AppHandle) {
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let list = SCREENS.lock().map(|slot| slot.clone()).unwrap_or_default();
    if list.is_empty() {
        return;
    }
    let Some(live) = super::live() else {
        return;
    };
    let (x, y) = cursor();
    let current = list.iter().position(|screen| contains(screen.bounds, x, y));
    let stages: Vec<Option<HWND>> = (0..list.len())
        .map(|index| {
            app.get_webview_window(&label(index))
                .and_then(|window| hwnd_of(&window))
        })
        .collect();
    let lens_on = if live.tool == "zoom" {
        place_lens(&list, &stages, &live, current, x, y)
    } else {
        lens::hide();
        None
    };
    for (index, screen) in list.iter().enumerate() {
        let pointer = PresentPointer {
            x: f64::from(x - screen.bounds.x) / screen.scale,
            y: f64::from(y - screen.bounds.y) / screen.scale,
            inside: current == Some(index),
            lens: lens_on.and_then(|(monitor, rect)| (monitor == index).then_some(rect)),
            monitor: index,
        };
        let _ = app.emit_to(label(index), "present-pointer", pointer);
    }
    let over_palette = over_palette(POINT { x, y });
    lens::hide_cursor(BLANK.load(Ordering::Relaxed) && current.is_some() && !over_palette);
    apply_click_through(app, list.len());
    if tick % 20 == 0 {
        restack(app, lens_on.map(|(monitor, _)| monitor));
    }
}

/// Places the magnifier on the locked zoom's monitor, or around the pointer. Returns the glass
/// as fractions of its monitor.
fn place_lens(
    list: &[Screen],
    stages: &[Option<HWND>],
    live: &super::Live,
    current: Option<usize>,
    x: i32,
    y: i32,
) -> Option<(usize, PresentRect)> {
    let locked = live
        .zoom
        .zip(live.zoom_monitor)
        .filter(|(_, monitor)| *monitor < list.len());
    let (monitor, glass, zoom, source) = match locked {
        Some((rect, monitor)) => {
            let screen = list[monitor];
            let inset = (LENS_INSET * screen.scale).round().max(1.0) as i32;
            let lens = super::lock_lens(rect, screen.bounds.width, screen.bounds.height, inset)?;
            (
                monitor,
                fraction_box(&lens.glass, screen.bounds),
                lens.zoom,
                fraction_box(&lens.source, screen.bounds),
            )
        }
        None => {
            let monitor = current?;
            let screen = list[monitor];
            let bounds = screen.bounds;
            let width = (LENS_W * screen.scale).round().max(1.0) as i32;
            let height = (LENS_H * screen.scale).round().max(1.0) as i32;
            let glass = ScreenBox {
                x: (x - width / 2).clamp(bounds.x, (bounds.x + bounds.width - width).max(bounds.x)),
                y: (y - height / 2)
                    .clamp(bounds.y, (bounds.y + bounds.height - height).max(bounds.y)),
                width: width.min(bounds.width).max(1),
                height: height.min(bounds.height).max(1),
            };
            let (sx, sy, sw, sh) = super::zoom_source(
                glass.x,
                glass.y,
                glass.width,
                glass.height,
                x,
                y,
                ZOOM,
                (bounds.x, bounds.y, bounds.width, bounds.height),
            );
            (
                monitor,
                glass,
                ZOOM,
                ScreenBox {
                    x: sx,
                    y: sy,
                    width: sw,
                    height: sh,
                },
            )
        }
    };
    let screen = list[monitor];
    let inset = (LENS_INSET * screen.scale).round().max(1.0) as i32;
    let mut exclude: Vec<HWND> = stages.iter().flatten().copied().collect();
    let palette = PALETTE_HWND.load(Ordering::Relaxed);
    if palette != 0 {
        exclude.push(HWND(palette as *mut core::ffi::c_void));
    }
    if !lens::show(glass, zoom, source, inset, &exclude) {
        return None;
    }
    if let Some(Some(stage)) = stages.get(monitor) {
        lens::tuck_under(*stage);
    }
    Some((monitor, monitor_fraction(&glass, screen.bounds)))
}

fn fraction_box(rect: &PresentRect, bounds: ScreenBox) -> ScreenBox {
    ScreenBox {
        x: bounds.x + (rect.x * bounds.width as f32).round() as i32,
        y: bounds.y + (rect.y * bounds.height as f32).round() as i32,
        width: (rect.width * bounds.width as f32).round().max(1.0) as i32,
        height: (rect.height * bounds.height as f32).round().max(1.0) as i32,
    }
}

fn monitor_fraction(glass: &ScreenBox, bounds: ScreenBox) -> PresentRect {
    let width = bounds.width.max(1) as f32;
    let height = bounds.height.max(1) as f32;
    PresentRect {
        x: (glass.x - bounds.x) as f32 / width,
        y: (glass.y - bounds.y) as f32 / height,
        width: glass.width as f32 / width,
        height: glass.height as f32 / height,
    }
}

/// Stages stay topmost, the lens sits just under the stage it is on, and the palette is on top.
fn restack(app: &AppHandle, lens_monitor: Option<usize>) {
    for index in 0..monitor_count() {
        if let Some(window) = app.get_webview_window(&label(index)) {
            above_top(&window);
            if tick_pass_due() {
                pass_clicks(&window);
            }
        }
    }
    if let Some(monitor) = lens_monitor {
        if let Some(hwnd) = app
            .get_webview_window(&label(monitor))
            .and_then(|window| hwnd_of(&window))
        {
            lens::tuck_under(hwnd);
        }
    }
    if let Some(palette) = app.get_webview_window(PALETTE) {
        above_top(&palette);
    }
}

/// WebView2 can add child windows after the first hook-up, so hit tests are hooked again now
/// and then.
fn tick_pass_due() -> bool {
    TICK.load(Ordering::Relaxed) % 200 < 20
}

fn hwnd_of(window: &WebviewWindow) -> Option<HWND> {
    window.hwnd().ok().map(|raw| HWND(raw.0))
}

fn above_top(window: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

fn takes_clicks() -> bool {
    CAPTURE.load(Ordering::Relaxed) || (PLACEABLE.load(Ordering::Relaxed) && alt_down())
}

/// Spotlight and cursor stay click-through. A drawing tool, or Alt while locking, takes the click.
fn apply_click_through(app: &AppHandle, count: usize) {
    let through = !takes_clicks();
    let code = if through { 1 } else { 2 };
    let changed = THROUGH.swap(code, Ordering::Relaxed) != code;
    for index in 0..count {
        let Some(stage) = app.get_webview_window(&label(index)) else {
            continue;
        };
        if changed {
            let _ = stage.set_ignore_cursor_events(through);
        }
        let Some(hwnd) = hwnd_of(&stage) else {
            continue;
        };
        set_click_through(hwnd, through);
        unsafe {
            let _ = EnumChildWindows(
                Some(hwnd),
                Some(through_child),
                LPARAM(isize::from(through)),
            );
        }
    }
}

unsafe extern "system" fn through_child(child: HWND, param: LPARAM) -> BOOL {
    set_click_through(child, param.0 != 0);
    BOOL(1)
}

fn set_click_through(hwnd: HWND, through: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let flag = WS_EX_TRANSPARENT.0;
        let next = if through { style | flag } else { style & !flag };
        if next == style {
            return;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next as isize);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

fn pass_clicks(window: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(stage_hit_proc), STAGE_PASS, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(pass_child), LPARAM(0));
    }
}

unsafe extern "system" fn pass_child(child: HWND, _: LPARAM) -> BOOL {
    unsafe {
        let _ = SetWindowSubclass(child, Some(stage_hit_proc), STAGE_PASS, 0);
    }
    BOOL(1)
}

unsafe extern "system" fn stage_hit_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCHITTEST && (over_palette(hit_point(lparam)) || !takes_clicks()) {
        return LRESULT(HTTRANSPARENT as isize);
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn hit_point(lparam: LPARAM) -> POINT {
    let packed = lparam.0 as u32;
    POINT {
        x: packed as i16 as i32,
        y: (packed >> 16) as i16 as i32,
    }
}

fn alt_down() -> bool {
    unsafe { GetAsyncKeyState(VK_MENU.0 as i32) < 0 }
}

fn over_palette(point: POINT) -> bool {
    let raw = PALETTE_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}

fn cursor() -> (i32, i32) {
    let mut point = POINT::default();
    let _ = unsafe { GetCursorPos(&mut point) };
    (point.x, point.y)
}

fn contains(rect: ScreenBox, x: i32, y: i32) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}
