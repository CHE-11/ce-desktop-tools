//! Windows, monitors, and screen regions Record can capture, plus the HWND or monitor a take uses.

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    MONITORINFOF_PRIMARY, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

use super::StartRequest;

const MIN_REGION: i32 = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingTarget {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub hwnd: isize,
    pub monitor: isize,
    pub label: String,
    /// Screen pixels of a region take. The capture is the monitor that holds it, cropped.
    pub region: Option<ScreenRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn list() -> Vec<RecordingTarget> {
    let mut found: Vec<RecordingTarget> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor),
            LPARAM(&mut found as *mut Vec<RecordingTarget> as isize),
        );
        let _ = EnumWindows(
            Some(enum_window),
            LPARAM(&mut found as *mut Vec<RecordingTarget> as isize),
        );
    }
    found
}

pub fn resolve(request: &StartRequest) -> Result<Target, String> {
    match request.source.as_str() {
        "window" => {
            let hwnd = request
                .target
                .parse::<isize>()
                .map_err(|_| "Choose a window to record.".to_string())?;
            if hwnd == 0
                || !unsafe { IsWindow(Some(HWND(hwnd as *mut core::ffi::c_void))).as_bool() }
            {
                return Err("That window is no longer open.".into());
            }
            Ok(Target {
                hwnd,
                monitor: 0,
                label: window_title(hwnd).unwrap_or_else(|| "Window".into()),
                region: None,
            })
        }
        "display" => {
            let monitor = request
                .target
                .parse::<isize>()
                .ok()
                .filter(|monitor| monitor_rect(*monitor).is_some())
                .unwrap_or_else(cursor_monitor);
            if monitor == 0 {
                return Err("Choose a screen to record.".into());
            }
            Ok(Target {
                hwnd: 0,
                monitor,
                label: monitor_label(monitor).unwrap_or_else(|| "Screen".into()),
                region: None,
            })
        }
        "region" => {
            let rect = request.region.ok_or("Select a region to record.")?;
            region_target(rect)
        }
        _ => Err("Choose what to record.".into()),
    }
}

/// The region clipped to the monitor under its center, at least 64 px on each side.
pub fn region_target(rect: ScreenRect) -> Result<Target, String> {
    let center = POINT {
        x: rect.x + rect.width / 2,
        y: rect.y + rect.height / 2,
    };
    let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) }.0 as isize;
    let bounds = monitor_rect(monitor).ok_or("That region is not on a screen.")?;
    let left = rect.x.max(bounds.x);
    let top = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bottom = (rect.y + rect.height).min(bounds.y + bounds.height);
    let width = (right - left) / 2 * 2;
    let height = (bottom - top) / 2 * 2;
    if width < MIN_REGION || height < MIN_REGION {
        return Err("The region is too small to record.".into());
    }
    Ok(Target {
        hwnd: 0,
        monitor,
        label: format!("Region {width}×{height}"),
        region: Some(ScreenRect {
            x: left,
            y: top,
            width,
            height,
        }),
    })
}

/// The window in front, unless it belongs to this app.
pub fn foreground_window() -> Option<isize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() || is_ours(hwnd) {
        return None;
    }
    Some(hwnd.0 as isize)
}

pub fn cursor_monitor() -> isize {
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
        MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST).0 as isize
    }
}

pub fn screen_rect(target: &Target) -> Option<ScreenRect> {
    if let Some(region) = target.region {
        return Some(region);
    }
    if target.monitor != 0 {
        return monitor_rect(target.monitor);
    }
    if target.hwnd == 0 {
        return None;
    }
    let hwnd = HWND(target.hwnd as *mut core::ffi::c_void);
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return None;
    }
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok()? };
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    if width < 40 || height < 40 {
        return None;
    }
    Some(ScreenRect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

/// The region inside the monitor's own frame, which is where WGC puts its pixels.
pub fn crop_in_monitor(target: &Target) -> Option<ScreenRect> {
    let region = target.region?;
    let monitor = monitor_rect(target.monitor)?;
    Some(ScreenRect {
        x: region.x - monitor.x,
        y: region.y - monitor.y,
        width: region.width,
        height: region.height,
    })
}

/// Pointer position in the frame, in pixels. It can sit outside the picture.
pub fn cursor_in_frame(target: &Target, width: u32, height: u32) -> Option<(f32, f32)> {
    let rect = screen_rect(target)?;
    if rect.width <= 0 || rect.height <= 0 || width == 0 || height == 0 {
        return None;
    }
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    let x = (point.x - rect.x) as f32 / rect.width as f32 * width as f32;
    let y = (point.y - rect.y) as f32 / rect.height as f32 * height as f32;
    Some((x, y))
}

pub fn alive(target: &Target) -> bool {
    if target.monitor != 0 {
        return monitor_rect(target.monitor).is_some();
    }
    target.hwnd != 0
        && unsafe { IsWindow(Some(HWND(target.hwnd as *mut core::ffi::c_void))).as_bool() }
}

pub fn monitor_rect(monitor: isize) -> Option<ScreenRect> {
    let info = monitor_info(monitor)?;
    let rect = info.rcMonitor;
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    (width > 0 && height > 0).then_some(ScreenRect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

pub fn monitor_work(monitor: isize) -> Option<ScreenRect> {
    let info = monitor_info(monitor)?;
    let rect = info.rcWork;
    Some(ScreenRect {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}

fn monitor_info(monitor: isize) -> Option<MONITORINFO> {
    if monitor == 0 {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let ok = unsafe {
        GetMonitorInfoW(HMONITOR(monitor as *mut core::ffi::c_void), &mut info).as_bool()
    };
    ok.then_some(info)
}

fn is_ours(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid == std::process::id()
}

unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let found = unsafe { &mut *(data.0 as *mut Vec<RecordingTarget>) };
    if !unsafe { IsWindowVisible(hwnd).as_bool() } || unsafe { IsIconic(hwnd).as_bool() } {
        return BOOL(1);
    }
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if style & (WS_EX_TOOLWINDOW.0 as isize) != 0 || is_ours(hwnd) {
        return BOOL(1);
    }
    let Some(title) = window_title(hwnd.0 as isize) else {
        return BOOL(1);
    };
    if title == "Program Manager" {
        return BOOL(1);
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect).is_err() } {
        return BOOL(1);
    }
    if rect.right - rect.left < 80 || rect.bottom - rect.top < 80 {
        return BOOL(1);
    }
    found.push(RecordingTarget {
        id: (hwnd.0 as isize).to_string(),
        kind: "window".into(),
        label: title,
    });
    BOOL(1)
}

unsafe extern "system" fn enum_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let found = unsafe { &mut *(data.0 as *mut Vec<RecordingTarget>) };
    let number = found.iter().filter(|item| item.kind == "display").count() + 1;
    let size = monitor_label(monitor.0 as isize).unwrap_or_else(|| "Screen".into());
    let primary = monitor_info(monitor.0 as isize)
        .is_some_and(|info| info.dwFlags & MONITORINFOF_PRIMARY != 0);
    found.push(RecordingTarget {
        id: (monitor.0 as isize).to_string(),
        kind: "display".into(),
        label: if primary {
            format!("Screen {number}: {size} (main)")
        } else {
            format!("Screen {number}: {size}")
        },
    });
    BOOL(1)
}

fn window_title(hwnd: isize) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(HWND(hwnd as *mut core::ffi::c_void), &mut buffer) };
    if length <= 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&buffer[..length as usize])
        .trim()
        .to_string();
    (!title.is_empty()).then_some(title)
}

fn monitor_label(monitor: isize) -> Option<String> {
    let rect = monitor_rect(monitor)?;
    Some(format!("{}×{}", rect.width, rect.height))
}
