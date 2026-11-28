//! The control bar, camera bubble, and draw layer. Each is its own always-on-top window, and
//! each is left out of screen capture so a recording of the desktop does not film the controls.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetWindowRect, HTTRANSPARENT, HWND_TOPMOST, IsWindowVisible,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowDisplayAffinity, SetWindowPos,
    WDA_EXCLUDEFROMCAPTURE, WM_NCHITTEST,
};
use windows::core::BOOL;

use super::targets::{self, ScreenRect, Target};

pub const CONTROLS: &str = "record-controls";
pub const CAMERA: &str = "record-camera";
pub const DRAW: &str = "record-draw";
const DRAW_PASS: usize = 0xce_d4a9;
const PILL_W: i32 = 440;
const PILL_H: i32 = 44;

/// Screen rectangles the draw layer must not steal. Clicks there belong to the controls and camera.
static CHROME: [AtomicIsize; 2] = [AtomicIsize::new(0), AtomicIsize::new(0)];
/// Escape leaves drawing while the draw layer is up. The global hotkey hook reads this.
static DRAWING: AtomicBool = AtomicBool::new(false);

pub fn drawing() -> bool {
    DRAWING.load(Ordering::Relaxed)
}

pub fn open(app: &AppHandle, target: &Target, camera: bool) -> Result<(), String> {
    close(app);
    let rect = targets::screen_rect(target);
    open_page(app, CONTROLS, "__RECORD_CONTROLS__")?;
    open_page(app, DRAW, "__RECORD_DRAW__")?;
    if camera {
        open_page(app, CAMERA, "__RECORD_CAMERA__")?;
    }
    place(app, target, rect.as_ref(), camera);
    hide(app, DRAW);
    Ok(())
}

/// Grows or shrinks the pill window. The bottom edge stays put so the settings open upward
/// instead of off the screen.
pub fn set_controls_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(CONTROLS) else {
        return Ok(());
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (logical_w, logical_h) = match mode {
        "hidden" => (168.0, 48.0),
        "settings" => (460.0, 168.0),
        "tools" => (460.0, 104.0),
        _ => (460.0, 56.0),
    };
    let width = (logical_w * scale).round().max(1.0) as u32;
    let height = (logical_h * scale).round().max(1.0) as u32;
    let outer = window.outer_position().map_err(|error| error.to_string())?;
    let outer_size = window.outer_size().map_err(|error| error.to_string())?;
    let y = outer.y + outer_size.height as i32 - height as i32;
    let _ = window.set_position(PhysicalPosition::new(outer.x, y));
    let _ = window.set_size(PhysicalSize::new(width, height));
    remember(&window, 0);
    restack(app);
    Ok(())
}

pub fn close(app: &AppHandle) {
    DRAWING.store(false, Ordering::Relaxed);
    for slot in &CHROME {
        slot.store(0, Ordering::Relaxed);
    }
    for label in [CONTROLS, CAMERA, DRAW] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.destroy();
        }
    }
}

pub fn show_camera(app: &AppHandle, target: &Target) -> Result<(), String> {
    move_camera(app, target, "bottom-left")
}

pub fn move_camera(app: &AppHandle, target: &Target, corner: &str) -> Result<(), String> {
    if app.get_webview_window(CAMERA).is_none() {
        open_page(app, CAMERA, "__RECORD_CAMERA__")?;
    }
    let Some(rect) = targets::screen_rect(target) else {
        return Ok(());
    };
    let Some(window) = app.get_webview_window(CAMERA) else {
        return Ok(());
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = ((160.0 * scale).round() as i32).min(rect.width / 2).min(rect.height / 2).max(64);
    let margin = (20.0 * scale).round() as i32;
    let (x, y) = match corner {
        "bottom-right" => (
            rect.x + rect.width - size - margin,
            rect.y + rect.height - size - margin,
        ),
        "top-left" => (rect.x + margin, rect.y + margin),
        "top-right" => (rect.x + rect.width - size - margin, rect.y + margin),
        _ => (rect.x + margin, rect.y + rect.height - size - margin),
    };
    place_window(&window, x, y, size as u32, size as u32);
    let _ = window.show();
    exclude(&window);
    remember(&window, 1);
    restack(app);
    Ok(())
}

pub fn hide_camera(app: &AppHandle) {
    hide(app, CAMERA);
}

pub fn show_draw(app: &AppHandle, target: &Target) {
    let Some(rect) = targets::screen_rect(target) else {
        return;
    };
    let app = app.clone();
    let _ = app
        .clone()
        .run_on_main_thread(move || show_draw_at(&app, &rect));
}

fn show_draw_at(app: &AppHandle, rect: &ScreenRect) {
    let Some(window) = app.get_webview_window(DRAW) else {
        return;
    };
    place_window(
        &window,
        rect.x,
        rect.y,
        rect.width.max(1) as u32,
        rect.height.max(1) as u32,
    );
    let _ = window.show();
    exclude(&window);
    pass_clicks(&window);
    restack(app);
    DRAWING.store(true, Ordering::Relaxed);
}

pub fn hide_draw(app: &AppHandle) {
    DRAWING.store(false, Ordering::Relaxed);
    hide(app, DRAW);
}

pub fn follow_draw(app: &AppHandle, target: &Target) {
    if app
        .get_webview_window(DRAW)
        .is_some_and(|window| window.is_visible().unwrap_or(false))
    {
        show_draw(app, target);
    }
}

pub fn camera_layout(app: &AppHandle, target: &Target) -> Option<super::CameraLayout> {
    let window = app.get_webview_window(CAMERA)?;
    if !window.is_visible().unwrap_or(false) {
        return None;
    }
    let rect = targets::screen_rect(target)?;
    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    Some(
        super::CameraLayout {
            x: (position.x - rect.x) as f32 / rect.width as f32,
            y: (position.y - rect.y) as f32 / rect.height as f32,
            size: size.width as f32 / rect.width as f32,
        }
        .clamped(),
    )
}

fn place(app: &AppHandle, target: &Target, rect: Option<&ScreenRect>, camera: bool) {
    let Some(rect) = rect else {
        return;
    };
    if let Some(window) = app.get_webview_window(CONTROLS) {
        let scale = window.scale_factor().unwrap_or(1.0);
        let width = (f64::from(PILL_W) * scale).round() as i32;
        let height = (f64::from(PILL_H) * scale).round() as i32;
        let gap = (12.0 * scale).round() as i32;
        let (x, y) = controls_spot(target, rect, width, height, gap);
        place_window(&window, x, y, width as u32, height as u32);
        let _ = window.show();
        exclude(&window);
        remember(&window, 0);
        restack(app);
    }
    if camera {
        let _ = show_camera(app, target);
    }
    if let Some(window) = app.get_webview_window(DRAW) {
        place_window(
            &window,
            rect.x,
            rect.y,
            rect.width.max(1) as u32,
            rect.height.max(1) as u32,
        );
        exclude(&window);
    }
}

/// Bottom center of the picture. A region keeps its controls just outside when the screen has room.
fn controls_spot(target: &Target, rect: &ScreenRect, width: i32, height: i32, gap: i32) -> (i32, i32) {
    let x = rect.x + (rect.width - width) / 2;
    let inside = (x, rect.y + rect.height - height - gap * 3);
    if target.region.is_none() {
        return inside;
    }
    let Some(work) = targets::monitor_work(target.monitor) else {
        return inside;
    };
    let left = x.clamp(work.x, work.x + work.width - width);
    let below = rect.y + rect.height + gap;
    if below + height <= work.y + work.height {
        return (left, below);
    }
    let above = rect.y - height - gap;
    if above >= work.y {
        return (left, above);
    }
    inside
}

fn open_page(app: &AppHandle, label: &str, flag: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.destroy();
    }
    let window = tauri::WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(format!("Record {label}"))
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
        .initialization_script(format!("window.{flag} = true"))
        .build()
        .map_err(|error| error.to_string())?;
    if label == CAMERA {
        allow_camera(&window);
    }
    exclude(&window);
    Ok(())
}

/// The bubble's `getUserMedia` would otherwise ask every time the window opens.
fn allow_camera(window: &WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND_CAMERA, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    };
    use webview2_com::PermissionRequestedEventHandler;

    let _ = window.with_webview(|webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let handler = PermissionRequestedEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                let mut kind = Default::default();
                args.PermissionKind(&mut kind)?;
                if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                }
            }
            Ok(())
        }));
        let mut token = Default::default();
        let _ = core.add_PermissionRequested(&handler, &mut token);
    });
}

fn place_window(window: &WebviewWindow, x: i32, y: i32, width: u32, height: u32) {
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width.max(1), height.max(1)));
}

/// Controls sit above the draw layer, and the camera sits above the controls.
fn restack(app: &AppHandle) {
    let draw = app.get_webview_window(DRAW);
    let controls = app.get_webview_window(CONTROLS);
    let camera = app.get_webview_window(CAMERA);
    if let (Some(draw), Some(controls)) = (&draw, &controls) {
        above(controls, draw);
    }
    if let (Some(controls), Some(camera)) = (&controls, &camera) {
        above(camera, controls);
    }
}

fn hwnd_of(window: &WebviewWindow) -> Option<HWND> {
    window.hwnd().ok().map(|raw| HWND(raw.0))
}

fn above(front: &WebviewWindow, back: &WebviewWindow) {
    let Some(front_hwnd) = hwnd_of(front) else {
        return;
    };
    let Some(back_hwnd) = hwnd_of(back) else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            front_hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            back_hwnd,
            Some(front_hwnd),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

fn remember(window: &WebviewWindow, slot: usize) {
    if let Some(hwnd) = hwnd_of(window) {
        CHROME[slot].store(hwnd.0 as isize, Ordering::Relaxed);
    }
}

/// Clicks that land on the controls or the camera fall through the draw layer.
fn pass_clicks(window: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(draw_hit_proc), DRAW_PASS, 0);
        let _ = EnumChildWindows(Some(hwnd), Some(pass_child), LPARAM(0));
    }
}

unsafe extern "system" fn pass_child(child: HWND, _: LPARAM) -> BOOL {
    unsafe {
        let _ = SetWindowSubclass(child, Some(draw_hit_proc), DRAW_PASS, 0);
    }
    BOOL(1)
}

unsafe extern "system" fn draw_hit_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCHITTEST && over_chrome(hit_point(lparam)) {
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

fn over_chrome(point: POINT) -> bool {
    CHROME.iter().any(|slot| covers(slot, point))
}

fn covers(slot: &AtomicIsize, point: POINT) -> bool {
    let raw = slot.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
    }
}

fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

fn exclude(window: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    }
}
