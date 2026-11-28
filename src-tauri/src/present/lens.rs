//! The zoom lens is one native magnifier window that moves to whichever monitor it is needed on.
//! The Magnification API also hides the system cursor while a Present cursor stands in for it.
//! Everything here runs on the main thread, which owns the lens window.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Magnification::{
    MAGTRANSFORM, MW_FILTERMODE_EXCLUDE, MagInitialize, MagSetWindowFilterList, MagSetWindowSource,
    MagSetWindowTransform, MagShowSystemCursor, MagUninitialize, WC_MAGNIFIER,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, HWND_TOPMOST,
    LWA_ALPHA, RegisterClassExW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
};
use windows::core::{BOOL, w};

use super::PresentRect;

static LENS_HWND: AtomicIsize = AtomicIsize::new(0);
static MAG_HWND: AtomicIsize = AtomicIsize::new(0);
static MAG_READY: AtomicBool = AtomicBool::new(false);
static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Glass, zoom, and source for a locked selection. Rects are fractions of the monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockLens {
    pub glass: PresentRect,
    pub zoom: f32,
    pub source: PresentRect,
}

/// A rectangle in physical screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn ensure() -> Result<(), String> {
    if MAG_READY.load(Ordering::Acquire) && LENS_HWND.load(Ordering::Relaxed) != 0 {
        return Ok(());
    }
    if !MAG_READY.load(Ordering::Acquire) {
        if !unsafe { MagInitialize() }.as_bool() {
            return Err(windows::core::Error::from_win32().to_string());
        }
        MAG_READY.store(true, Ordering::Release);
    }
    register_class()?;
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?;
    let host = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            w!("CePresentLens"),
            w!("Present lens"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            40,
            40,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    unsafe {
        let _ = SetLayeredWindowAttributes(host, COLORREF(0), 255, LWA_ALPHA);
    }
    let glass = unsafe {
        CreateWindowExW(
            WS_EX_TRANSPARENT,
            WC_MAGNIFIER,
            w!("Present magnifier"),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            40,
            40,
            Some(host),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    LENS_HWND.store(host.0 as isize, Ordering::Relaxed);
    MAG_HWND.store(glass.0 as isize, Ordering::Relaxed);
    hide();
    Ok(())
}

pub fn host() -> Option<HWND> {
    let raw = LENS_HWND.load(Ordering::Relaxed);
    (raw != 0).then(|| HWND(raw as *mut core::ffi::c_void))
}

/// Shows `source` magnified by `zoom` inside `glass`, less a frame of `inset` pixels.
/// `exclude` are the windows the magnifier must not sample: the stages and the palette.
pub fn show(glass: ScreenBox, zoom: f32, source: ScreenBox, inset: i32, exclude: &[HWND]) -> bool {
    let Some(host) = host() else {
        return false;
    };
    let mag = MAG_HWND.load(Ordering::Relaxed);
    if mag == 0 || !MAG_READY.load(Ordering::Relaxed) {
        return false;
    }
    if glass.width <= inset * 2 || glass.height <= inset * 2 {
        hide();
        return false;
    }
    let magnifier = HWND(mag as *mut core::ffi::c_void);
    unsafe {
        let _ = SetWindowPos(
            host,
            Some(HWND_TOPMOST),
            glass.x + inset,
            glass.y + inset,
            glass.width - inset * 2,
            glass.height - inset * 2,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(host, SW_SHOWNA);
        let _ = SetWindowPos(
            magnifier,
            None,
            0,
            0,
            glass.width - inset * 2,
            glass.height - inset * 2,
            SWP_NOACTIVATE | SWP_NOMOVE,
        );
    }
    let mut transform = MAGTRANSFORM { v: [0.0; 9] };
    transform.v[0] = zoom;
    transform.v[4] = zoom;
    transform.v[8] = 1.0;
    unsafe {
        let _ = MagSetWindowTransform(magnifier, &mut transform);
        let _ = MagSetWindowSource(
            magnifier,
            RECT {
                left: source.x,
                top: source.y,
                right: source.x + source.width,
                bottom: source.y + source.height,
            },
        );
    }
    filter(magnifier, exclude);
    true
}

fn filter(magnifier: HWND, roots: &[HWND]) {
    let mut list = Vec::new();
    for root in roots {
        list.push(*root);
        unsafe {
            let _ = EnumChildWindows(
                Some(*root),
                Some(collect_child),
                LPARAM(&mut list as *mut Vec<HWND> as isize),
            );
        }
    }
    if list.is_empty() {
        return;
    }
    unsafe {
        let _ = MagSetWindowFilterList(
            magnifier,
            MW_FILTERMODE_EXCLUDE,
            list.len() as i32,
            list.as_mut_ptr(),
        );
    }
}

unsafe extern "system" fn collect_child(child: HWND, param: LPARAM) -> BOOL {
    let list = unsafe { &mut *(param.0 as *mut Vec<HWND>) };
    list.push(child);
    BOOL(1)
}

pub fn hide() {
    if let Some(host) = host() {
        unsafe {
            let _ = ShowWindow(host, SW_HIDE);
        }
    }
}

/// Keeps the lens just under `stage`, so the stage can draw the frame on top of it.
pub fn tuck_under(stage: HWND) {
    let Some(host) = host() else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            host,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            host,
            Some(stage),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Hides or shows the system cursor everywhere. Only changes anything after `ensure`.
pub fn hide_cursor(hidden: bool) {
    if !MAG_READY.load(Ordering::Acquire) {
        return;
    }
    if CURSOR_HIDDEN.swap(hidden, Ordering::AcqRel) == hidden {
        return;
    }
    unsafe {
        let _ = MagShowSystemCursor(!hidden);
    }
}

pub fn destroy() {
    hide_cursor(false);
    hide();
    let host = LENS_HWND.swap(0, Ordering::Relaxed);
    MAG_HWND.store(0, Ordering::Relaxed);
    if host != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(host as *mut core::ffi::c_void));
        }
    }
    if MAG_READY.swap(false, Ordering::AcqRel) {
        unsafe {
            let _ = MagUninitialize();
        }
    }
}

unsafe extern "system" fn lens_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn register_class() -> Result<(), String> {
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?;
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(lens_proc),
        hInstance: instance.into(),
        lpszClassName: w!("CePresentLens"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassExW(&class) };
    if atom == 0 {
        let error = windows::core::Error::from_win32();
        // ERROR_CLASS_ALREADY_EXISTS: a previous session registered it.
        if error.code().0 != 1410 && error.code().0 as u32 != 0x8007_0582 {
            return Err(error.to_string());
        }
    }
    Ok(())
}

/// Screen rectangle magnified into the lens, centered on `aim` and kept inside `bounds`.
#[allow(clippy::too_many_arguments)]
pub fn zoom_source(
    lens_x: i32,
    lens_y: i32,
    lens_w: i32,
    lens_h: i32,
    aim_x: i32,
    aim_y: i32,
    zoom: f32,
    bounds: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let _ = (lens_x, lens_y);
    let zoom = if zoom < 1.05 { 1.05 } else { zoom };
    let sw = ((lens_w as f32) / zoom).round().max(1.0) as i32;
    let sh = ((lens_h as f32) / zoom).round().max(1.0) as i32;
    let (bx, by, bw, bh) = bounds;
    let x = if sw >= bw {
        bx
    } else {
        (aim_x - sw / 2).clamp(bx, bx + bw - sw)
    };
    let y = if sh >= bh {
        by
    } else {
        (aim_y - sh / 2).clamp(by, by + bh - sh)
    };
    (x, y, sw, sh)
}

/// Glass that shows `selection` at up to 2x. `inset` is the frame around the magnifier, in pixels.
/// A selection larger than the monitor shrinks the zoom until the whole selection still fits.
pub fn lock_lens(
    selection: PresentRect,
    window_w: i32,
    window_h: i32,
    inset: i32,
) -> Option<LockLens> {
    if window_w < 1 || window_h < 1 || !selection.width.is_finite() || !selection.height.is_finite()
    {
        return None;
    }
    let max_w = window_w as f32;
    let max_h = window_h as f32;
    let sw = (selection.width * max_w).round().max(1.0);
    let sh = (selection.height * max_h).round().max(1.0);
    let inset = inset.max(0) as f32;
    let mut inner_w = (sw * 2.0).round().max(1.0);
    let mut inner_h = (sh * 2.0).round().max(1.0);
    let room_w = (max_w - inset * 2.0).max(1.0);
    let room_h = (max_h - inset * 2.0).max(1.0);
    if inner_w > room_w || inner_h > room_h {
        let fit = (room_w / inner_w).min(room_h / inner_h);
        inner_w = (inner_w * fit).round().max(1.0);
        inner_h = (inner_h * fit).round().max(1.0);
    }
    inner_w = inner_w.min(room_w);
    inner_h = inner_h.min(room_h);
    let zoom = (inner_w / sw).min(inner_h / sh).max(1.0);
    let source_w = (inner_w / zoom).clamp(1.0, max_w);
    let source_h = (inner_h / zoom).clamp(1.0, max_h);
    let sel_x = (selection.x * max_w).clamp(0.0, max_w);
    let sel_y = (selection.y * max_h).clamp(0.0, max_h);
    let cx = sel_x + sw / 2.0;
    let cy = sel_y + sh / 2.0;
    let source_x = contain(cx - source_w / 2.0, source_w, sel_x, sw, max_w);
    let source_y = contain(cy - source_h / 2.0, source_h, sel_y, sh, max_h);
    let outer_w = (inner_w + inset * 2.0).min(max_w).max(1.0);
    let outer_h = (inner_h + inset * 2.0).min(max_h).max(1.0);
    let glass_x = (cx - outer_w / 2.0).clamp(0.0, (max_w - outer_w).max(0.0));
    let glass_y = (cy - outer_h / 2.0).clamp(0.0, (max_h - outer_h).max(0.0));
    Some(LockLens {
        glass: PresentRect {
            x: glass_x / max_w,
            y: glass_y / max_h,
            width: outer_w / max_w,
            height: outer_h / max_h,
        },
        zoom,
        source: PresentRect {
            x: source_x / max_w,
            y: source_y / max_h,
            width: source_w / max_w,
            height: source_h / max_h,
        },
    })
}

fn contain(origin: f32, size: f32, cover: f32, cover_size: f32, limit: f32) -> f32 {
    let mut origin = origin;
    if origin > cover {
        origin = cover;
    }
    if origin + size < cover + cover_size {
        origin = cover + cover_size - size;
    }
    origin.clamp(0.0, (limit - size).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::{PresentRect, lock_lens, zoom_source};

    fn covers(outer: PresentRect, inner: PresentRect) -> bool {
        outer.x <= inner.x + 0.01
            && outer.y <= inner.y + 0.01
            && outer.x + outer.width + 0.01 >= inner.x + inner.width
            && outer.y + outer.height + 0.01 >= inner.y + inner.height
    }

    #[test]
    fn zoom_source_is_the_lens_divided_by_the_factor_and_kept_on_the_monitor() {
        let source = zoom_source(100, 80, 200, 120, 200, 140, 2.0, (0, 0, 1000, 800));
        assert_eq!(source, (150, 110, 100, 60));
        let clamped = zoom_source(0, 0, 200, 120, 10, 10, 2.0, (0, 0, 1000, 800));
        assert_eq!(clamped, (0, 0, 100, 60));
        let second = zoom_source(0, 0, 200, 120, 1930, 10, 2.0, (1920, 0, 1920, 1080));
        assert_eq!(second, (1920, 0, 100, 60));
    }

    #[test]
    fn lock_lens_doubles_a_small_selection_and_keeps_it_in_view() {
        let selection = PresentRect {
            x: 0.2,
            y: 0.2,
            width: 0.1,
            height: 0.1,
        };
        let lens = lock_lens(selection, 1000, 800, 0).expect("lens");
        assert!((lens.zoom - 2.0).abs() < 0.05, "zoom {}", lens.zoom);
        assert!((lens.glass.width - 0.2).abs() < 0.02);
        assert!((lens.glass.height - 0.2).abs() < 0.02);
        assert!(lens.glass.x >= 0.0 && lens.glass.y >= 0.0);
        assert!(lens.glass.x + lens.glass.width <= 1.01);
        assert!(lens.glass.y + lens.glass.height <= 1.01);
        assert!(covers(lens.source, selection));
    }

    #[test]
    fn lock_lens_shrinks_zoom_so_a_large_selection_still_fits() {
        let selection = PresentRect {
            x: 0.0,
            y: 0.0,
            width: 0.8,
            height: 0.8,
        };
        let lens = lock_lens(selection, 1000, 800, 0).expect("lens");
        assert!(lens.zoom < 2.0, "zoom {}", lens.zoom);
        assert!(lens.glass.x >= -0.001 && lens.glass.y >= -0.001);
        assert!(lens.glass.x + lens.glass.width <= 1.01);
        assert!(lens.glass.y + lens.glass.height <= 1.01);
        assert!(covers(lens.source, selection));
    }
}
