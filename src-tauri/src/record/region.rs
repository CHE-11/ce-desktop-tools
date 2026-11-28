//! Drag a rectangle over a frozen, dimmed copy of the screen to choose a region to record.

use std::sync::{Arc, Mutex};

use windows::Win32::Graphics::Gdi::{BitBlt, HDC, SRCCOPY};

use super::targets::ScreenRect;
use crate::overlay::{Input, Overlay, Reply, frame, label};
use crate::screen_capture::{Rect, Surface, monitor_at, virtual_screen};

const MIN_SIDE: i32 = 64;
const DIM: u32 = 110;

/// Blocks until a region is chosen or the picker is closed. Coordinates are screen pixels.
pub fn pick() -> Option<ScreenRect> {
    if crate::overlay::is_active() {
        crate::overlay::close_active();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    let bounds = virtual_screen();
    let mut capture = Surface::new(bounds.width, bounds.height)?;
    capture.grab(bounds.left, bounds.top);
    let mut dimmed = Surface::new(bounds.width, bounds.height)?;
    let bright = capture.bytes();
    for (out, value) in dimmed.bytes_mut().iter_mut().zip(bright) {
        *out = ((u32::from(*value) * DIM) / 255) as u8;
    }
    let (x, y) = crate::screen_capture::cursor();
    let chosen = Arc::new(Mutex::new(None));
    let picker = Picker {
        bounds,
        capture,
        dimmed,
        cursor: (x - bounds.left, y - bounds.top),
        start: None,
        chosen: chosen.clone(),
    };
    crate::overlay::run(bounds, Box::new(picker));
    chosen.lock().ok().and_then(|slot| *slot)
}

struct Picker {
    bounds: Rect,
    capture: Surface,
    dimmed: Surface,
    cursor: (i32, i32),
    start: Option<(i32, i32)>,
    chosen: Arc<Mutex<Option<ScreenRect>>>,
}

impl Picker {
    fn scale(&self) -> f64 {
        let monitor = monitor_at(
            self.bounds.left + self.cursor.0,
            self.bounds.top + self.cursor.1,
        );
        f64::from(monitor.dpi) / 96.0
    }

    fn selection(&self) -> Option<(i32, i32, i32, i32)> {
        let (sx, sy) = self.start?;
        let (cx, cy) = self.cursor;
        Some((sx.min(cx), sy.min(cy), sx.max(cx), sy.max(cy)))
    }
}

impl Overlay for Picker {
    fn input(&mut self, input: Input) -> Reply {
        match input {
            Input::Move { x, y } => {
                self.cursor = (x, y);
                Reply::Redraw
            }
            Input::LeftDown { x, y } => {
                self.cursor = (x, y);
                self.start = Some((x, y));
                Reply::Redraw
            }
            Input::LeftUp { x, y } => {
                self.cursor = (x, y);
                let Some((left, top, right, bottom)) = self.selection() else {
                    return Reply::Idle;
                };
                self.start = None;
                if right - left < MIN_SIDE || bottom - top < MIN_SIDE {
                    return Reply::Redraw;
                }
                if let Ok(mut slot) = self.chosen.lock() {
                    *slot = Some(ScreenRect {
                        x: self.bounds.left + left,
                        y: self.bounds.top + top,
                        width: right - left,
                        height: bottom - top,
                    });
                }
                Reply::Close
            }
            Input::RightDown | Input::Escape => Reply::Close,
            Input::Wheel(_) | Input::Tick | Input::Refresh => Reply::Idle,
        }
    }

    fn paint(&mut self, dc: HDC, width: i32, height: i32) {
        unsafe {
            let _ = BitBlt(dc, 0, 0, width, height, Some(self.dimmed.dc), 0, 0, SRCCOPY);
        }
        let scale = self.scale();
        let line = (scale.round() as i32).max(1);
        let (x, y) = self.cursor;
        match self.selection() {
            Some((left, top, right, bottom)) => {
                unsafe {
                    let _ = BitBlt(
                        dc,
                        left,
                        top,
                        right - left,
                        bottom - top,
                        Some(self.capture.dc),
                        left,
                        top,
                        SRCCOPY,
                    );
                }
                frame(dc, left, top, right, bottom, [56, 189, 248], line * 2);
                let offset = (14.0 * scale).round() as i32;
                label(
                    dc,
                    x + offset,
                    y + offset,
                    &[format!("{} × {}", right - left, bottom - top)],
                    scale,
                    width,
                    height,
                );
            }
            None => {
                let offset = (18.0 * scale).round() as i32;
                label(
                    dc,
                    x + offset,
                    y + offset,
                    &["Drag to choose what to record".into(), "Esc cancels".into()],
                    scale,
                    width,
                    height,
                );
            }
        }
    }
}
