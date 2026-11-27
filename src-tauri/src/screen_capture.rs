//! Screen pixels for Screen Ruler and Color Picker.
//!
//! A `Surface` is a 32-bit top-down DIB section selected into a memory DC, so
//! GDI can draw into it and Rust can read its pixels directly. Coordinates are
//! physical pixels; the app is per-monitor DPI aware.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// The monitor under a point, with its effective DPI.
#[derive(Clone, Copy, Debug)]
pub struct Monitor {
    pub work: Rect,
    pub dpi: u32,
}

#[cfg(windows)]
pub use native::*;

#[cfg(windows)]
mod native {
    use super::{Monitor, Rect};
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, HBITMAP,
        HDC, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC, SRCCOPY,
        SelectObject,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    pub struct Surface {
        pub dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        bits: *mut u8,
        pub width: i32,
        pub height: i32,
    }

    // The surface is only touched by the thread that owns the overlay.
    unsafe impl Send for Surface {}

    impl Surface {
        pub fn new(width: i32, height: i32) -> Option<Self> {
            let width = width.max(1);
            let height = height.max(1);
            unsafe {
                let screen = GetDC(None);
                let dc = CreateCompatibleDC(Some(screen));
                ReleaseDC(None, screen);
                if dc.is_invalid() {
                    return None;
                }
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits = std::ptr::null_mut();
                let Ok(bitmap) =
                    CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                else {
                    let _ = DeleteDC(dc);
                    return None;
                };
                if bits.is_null() {
                    let _ = DeleteObject(bitmap.into());
                    let _ = DeleteDC(dc);
                    return None;
                }
                let previous = SelectObject(dc, bitmap.into());
                Some(Self {
                    dc,
                    bitmap,
                    previous,
                    bits: bits as *mut u8,
                    width,
                    height,
                })
            }
        }

        /// Copies screen pixels starting at `left`, `top` in virtual-screen coordinates.
        pub fn grab(&mut self, left: i32, top: i32) -> bool {
            unsafe {
                let screen = GetDC(None);
                let copied = BitBlt(
                    self.dc,
                    0,
                    0,
                    self.width,
                    self.height,
                    Some(screen),
                    left,
                    top,
                    SRCCOPY | CAPTUREBLT,
                )
                .is_ok();
                ReleaseDC(None, screen);
                copied
            }
        }

        pub fn bytes(&self) -> &[u8] {
            unsafe {
                std::slice::from_raw_parts(self.bits, (self.width * self.height * 4) as usize)
            }
        }

        pub fn bytes_mut(&mut self) -> &mut [u8] {
            unsafe {
                std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
            }
        }

        /// `[r, g, b]` at a surface coordinate.
        pub fn rgb(&self, x: i32, y: i32) -> Option<[u8; 3]> {
            if x < 0 || y < 0 || x >= self.width || y >= self.height {
                return None;
            }
            let index = ((y * self.width + x) * 4) as usize;
            let bytes = self.bytes();
            Some([bytes[index + 2], bytes[index + 1], bytes[index]])
        }
    }

    impl Drop for Surface {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.previous);
                let _ = DeleteObject(self.bitmap.into());
                let _ = DeleteDC(self.dc);
            }
        }
    }

    pub fn virtual_screen() -> Rect {
        unsafe {
            Rect {
                left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            }
        }
    }

    pub fn cursor() -> (i32, i32) {
        let mut point = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut point);
        }
        (point.x, point.y)
    }

    pub fn monitor_at(x: i32, y: i32) -> Monitor {
        unsafe {
            let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut info);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            Monitor {
                work: rect_of(info.rcWork),
                dpi: dpi_x.max(96),
            }
        }
    }

    pub fn rect_of(rect: RECT) -> Rect {
        Rect {
            left: rect.left,
            top: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        }
    }
}
