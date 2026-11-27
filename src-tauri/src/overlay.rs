//! A full-screen native window for Screen Ruler and Color Picker.
//!
//! Each overlay runs on its own thread with its own message loop. Painting
//! goes to a back buffer first, so redrawing on every mouse move does not
//! flicker. The window is excluded from screen capture, so a continuous
//! capture sees what is underneath it.

#[cfg(windows)]
pub use native::*;

#[cfg(windows)]
mod native {
    use std::cell::RefCell;
    use std::sync::Once;
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, BitBlt, CreateFontW, CreatePen, CreateSolidBrush, DT_CALCRECT, DT_LEFT,
        DT_NOPREFIX, DeleteObject, DrawTextW, EndPaint, FillRect, GetStockObject, HDC,
        InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, RoundRect, SRCCOPY, SelectObject,
        SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
        GetMessageW, HTCLIENT, IDC_CROSS, KillTimer, LoadCursorW, MSG, PostMessageW,
        PostQuitMessage, RegisterClassW, SW_SHOW, SetCursor, SetTimer, SetWindowDisplayAffinity,
        ShowWindow, TranslateMessage, WDA_EXCLUDEFROMCAPTURE, WM_APP, WM_CLOSE, WM_DESTROY,
        WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_PAINT, WM_RBUTTONDOWN, WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };
    use windows::core::{PCWSTR, w};

    use crate::screen_capture::{Rect, Surface};

    const CLASS_NAME: PCWSTR = w!("CeDesktopToolsOverlay");
    const ESCAPE_TIMER: usize = 1;
    const TICK_TIMER: usize = 2;
    const REFRESH_MESSAGE: u32 = WM_APP + 41;

    static ACTIVE: AtomicIsize = AtomicIsize::new(0);
    static REGISTER: Once = Once::new();

    pub enum Input {
        Move { x: i32, y: i32 },
        LeftDown { x: i32, y: i32 },
        LeftUp { x: i32, y: i32 },
        RightDown,
        Wheel(i32),
        Escape,
        Tick,
        Refresh,
    }

    #[derive(PartialEq, Eq)]
    pub enum Reply {
        Idle,
        Redraw,
        Close,
    }

    pub trait Overlay {
        fn input(&mut self, input: Input) -> Reply;
        /// Draws the whole frame into `dc`, which is overlay-sized.
        fn paint(&mut self, dc: HDC, width: i32, height: i32);
        fn tick_ms(&self) -> Option<u32> {
            None
        }
        fn shown(&mut self, _hwnd: HWND) {}
        fn closed(&mut self) {}
    }

    struct Running {
        overlay: Box<dyn Overlay>,
        back: Surface,
    }

    thread_local! {
        static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
    }

    pub fn is_active() -> bool {
        ACTIVE.load(Ordering::SeqCst) != 0
    }

    pub fn close_active() {
        let raw = ACTIVE.load(Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(raw as *mut _)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    /// Asks the open overlay to read shared state again and redraw.
    pub fn refresh_active() {
        let raw = ACTIVE.load(Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(raw as *mut _)),
                    REFRESH_MESSAGE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    /// Opens the overlay over `bounds` and blocks until it closes.
    pub fn run(bounds: Rect, overlay: Box<dyn Overlay>) {
        let Some(back) = Surface::new(bounds.width, bounds.height) else {
            return;
        };
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            REGISTER.call_once(|| {
                let class = WNDCLASSW {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance.into(),
                    lpszClassName: CLASS_NAME,
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let tick = overlay.tick_ms();
            RUNNING.with(|slot| {
                *slot.borrow_mut() = Some(Running { overlay, back });
            });
            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                CLASS_NAME,
                w!("ce-desktop-tools overlay"),
                WS_POPUP,
                bounds.left,
                bounds.top,
                bounds.width,
                bounds.height,
                None,
                None,
                Some(instance.into()),
                None,
            ) else {
                if let Some(mut running) = RUNNING.with(|slot| slot.borrow_mut().take()) {
                    running.overlay.closed();
                }
                return;
            };
            ACTIVE.store(hwnd.0 as isize, Ordering::SeqCst);
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            let _ = ShowWindow(hwnd, SW_SHOW);
            focus(hwnd);
            SetTimer(Some(hwnd), ESCAPE_TIMER, 50, None);
            if let Some(ms) = tick {
                SetTimer(Some(hwnd), TICK_TIMER, ms, None);
            }
            with_overlay(|running| running.overlay.shown(hwnd));

            let mut message = MSG::default();
            loop {
                let status = GetMessageW(&mut message, None, 0, 0);
                if status.0 == 0 || status.0 == -1 {
                    break;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let _ = ACTIVE.compare_exchange(
                hwnd.0 as isize,
                0,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            if let Some(mut running) = RUNNING.with(|slot| slot.borrow_mut().take()) {
                running.overlay.closed();
            }
        }
    }

    fn with_overlay<T>(body: impl FnOnce(&mut Running) -> T) -> Option<T> {
        RUNNING.with(|slot| {
            let mut borrowed = slot.try_borrow_mut().ok()?;
            borrowed.as_mut().map(body)
        })
    }

    fn deliver(hwnd: HWND, input: Input) {
        let reply = with_overlay(|running| running.overlay.input(input)).unwrap_or(Reply::Idle);
        match reply {
            Reply::Idle => {}
            Reply::Redraw => unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            },
            Reply::Close => unsafe {
                let _ = DestroyWindow(hwnd);
            },
        }
    }

    fn point_of(lparam: LPARAM) -> (i32, i32) {
        let x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        (x, y)
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_SETCURSOR => {
                if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                    unsafe {
                        if let Ok(cursor) = LoadCursorW(None, IDC_CROSS) {
                            SetCursor(Some(cursor));
                        }
                    }
                    return LRESULT(1);
                }
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                unsafe {
                    let dc = BeginPaint(hwnd, &mut paint);
                    with_overlay(|running| {
                        let (width, height) = (running.back.width, running.back.height);
                        running.overlay.paint(running.back.dc, width, height);
                        let _ = BitBlt(
                            dc,
                            0,
                            0,
                            width,
                            height,
                            Some(running.back.dc),
                            0,
                            0,
                            SRCCOPY,
                        );
                    });
                    let _ = EndPaint(hwnd, &paint);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = point_of(lparam);
                deliver(hwnd, Input::Move { x, y });
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = point_of(lparam);
                deliver(hwnd, Input::LeftDown { x, y });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let (x, y) = point_of(lparam);
                deliver(hwnd, Input::LeftUp { x, y });
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                deliver(hwnd, Input::RightDown);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
                deliver(hwnd, Input::Wheel(delta));
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 == VK_ESCAPE.0 as usize {
                    deliver(hwnd, Input::Escape);
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == ESCAPE_TIMER {
                    let held = unsafe { GetAsyncKeyState(i32::from(VK_ESCAPE.0)) } as u16 & 0x8000;
                    if held != 0 {
                        deliver(hwnd, Input::Escape);
                    }
                } else if wparam.0 == TICK_TIMER {
                    deliver(hwnd, Input::Tick);
                }
                LRESULT(0)
            }
            REFRESH_MESSAGE => {
                deliver(hwnd, Input::Refresh);
                LRESULT(0)
            }
            WM_CLOSE => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe {
                    let _ = KillTimer(Some(hwnd), ESCAPE_TIMER);
                    let _ = KillTimer(Some(hwnd), TICK_TIMER);
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    /// A key press elsewhere does not give this process focus rights, so attach
    /// to the foreground thread for the switch.
    fn focus(hwnd: HWND) {
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        };
        unsafe {
            let foreground = GetForegroundWindow();
            let current = GetCurrentThreadId();
            let other = if foreground.0.is_null() {
                0
            } else {
                GetWindowThreadProcessId(foreground, None)
            };
            let attached = other != 0
                && other != current
                && AttachThreadInput(current, other, true).as_bool();
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            if attached {
                let _ = AttachThreadInput(current, other, false);
            }
        }
    }

    /// `#rrggbb` to a GDI color.
    pub fn colorref(rgb: [u8; 3]) -> COLORREF {
        COLORREF(u32::from(rgb[0]) | (u32::from(rgb[1]) << 8) | (u32::from(rgb[2]) << 16))
    }

    pub fn fill(dc: HDC, left: i32, top: i32, right: i32, bottom: i32, rgb: [u8; 3]) {
        if right <= left || bottom <= top {
            return;
        }
        unsafe {
            let brush = CreateSolidBrush(colorref(rgb));
            let rect = RECT {
                left,
                top,
                right,
                bottom,
            };
            FillRect(dc, &rect, brush);
            let _ = DeleteObject(brush.into());
        }
    }

    pub fn frame(dc: HDC, left: i32, top: i32, right: i32, bottom: i32, rgb: [u8; 3], width: i32) {
        fill(dc, left, top, right, top + width, rgb);
        fill(dc, left, bottom - width, right, bottom, rgb);
        fill(dc, left, top, left + width, bottom, rgb);
        fill(dc, right - width, top, right, bottom, rgb);
    }

    /// Draws a rounded dark label with one line per entry, placed near
    /// `(x, y)` and kept inside `width` by `height`. Returns its rectangle.
    pub fn label(
        dc: HDC,
        x: i32,
        y: i32,
        lines: &[String],
        scale: f64,
        width: i32,
        height: i32,
    ) -> RECT {
        let text = lines.join("\n");
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut wide_mut = wide.clone();
        let padding = (8.0 * scale).round() as i32;
        unsafe {
            let font = CreateFontW(
                -(13.0 * scale).round() as i32,
                0,
                0,
                0,
                500,
                0,
                0,
                0,
                windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                windows::Win32::Graphics::Gdi::OUT_DEFAULT_PRECIS,
                windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
                windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI"),
            );
            let previous_font = SelectObject(dc, font.into());
            let mut measure = RECT::default();
            DrawTextW(dc, &mut wide_mut, &mut measure, DT_CALCRECT | DT_LEFT | DT_NOPREFIX);
            let box_width = measure.right - measure.left + padding * 2;
            let box_height = measure.bottom - measure.top + padding * 2;
            let mut left = x;
            let mut top = y;
            if left + box_width > width {
                left = (x - box_width - (32.0 * scale) as i32).max(0);
            }
            if top + box_height > height {
                top = (y - box_height - (32.0 * scale) as i32).max(0);
            }
            let outline = CreatePen(PS_SOLID, 1, colorref([82, 82, 91]));
            let background = CreateSolidBrush(colorref([24, 24, 27]));
            let previous_pen = SelectObject(dc, outline.into());
            let previous_brush = SelectObject(dc, background.into());
            let radius = (8.0 * scale).round() as i32;
            let _ = RoundRect(
                dc,
                left,
                top,
                left + box_width,
                top + box_height,
                radius,
                radius,
            );
            SelectObject(dc, previous_brush);
            SelectObject(dc, previous_pen);
            let _ = DeleteObject(outline.into());
            let _ = DeleteObject(background.into());
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, colorref([244, 244, 245]));
            let mut area = RECT {
                left: left + padding,
                top: top + padding,
                right: left + box_width - padding,
                bottom: top + box_height - padding,
            };
            let mut draw = wide;
            DrawTextW(dc, &mut draw, &mut area, DT_LEFT | DT_NOPREFIX);
            SelectObject(dc, previous_font);
            let _ = DeleteObject(font.into());
            RECT {
                left,
                top,
                right: left + box_width,
                bottom: top + box_height,
            }
        }
    }

    /// An outline with no fill, for the magnifier.
    pub fn outline(dc: HDC, left: i32, top: i32, right: i32, bottom: i32, rgb: [u8; 3], width: i32) {
        unsafe {
            let pen = CreatePen(PS_SOLID, width, colorref(rgb));
            let previous_pen = SelectObject(dc, pen.into());
            let previous_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
            let _ = windows::Win32::Graphics::Gdi::Rectangle(dc, left, top, right, bottom);
            SelectObject(dc, previous_brush);
            SelectObject(dc, previous_pen);
            let _ = DeleteObject(pen.into());
        }
    }
}

/// `#rrggbb` (or `rrggbb`) to bytes.
pub fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::parse_hex;

    #[test]
    fn hex_colors_parse_with_or_without_a_hash() {
        assert_eq!(parse_hex("#ff4500"), Some([255, 69, 0]));
        assert_eq!(parse_hex("A855F7"), Some([0xA8, 0x55, 0xF7]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
    }
}
