//! Edge detection and length labels for Screen Ruler. Pure, so it is tested
//! without a screen.

pub const STYLES: &[&str] = &["none", "bounds", "spacing", "horizontal", "vertical"];
pub const UNITS: &[&str] = &["none", "inches", "centimeters", "millimeters"];

/// Inclusive pixel extents of the run of similar pixels around a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Span {
    pub fn width(&self) -> i32 {
        self.right - self.left + 1
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top + 1
    }
}

/// Per channel, every channel must stay within `tolerance`. Otherwise the sum
/// of the channel differences must.
pub fn differs(a: [u8; 3], b: [u8; 3], tolerance: u32, per_channel: bool) -> bool {
    let deltas = [0, 1, 2].map(|index| u32::from(a[index].abs_diff(b[index])));
    if per_channel {
        deltas.iter().any(|delta| *delta > tolerance)
    } else {
        deltas.iter().sum::<u32>() > tolerance
    }
}

/// Walks out from `(x, y)` until a pixel differs from the one under the cursor.
pub fn scan(
    read: impl Fn(i32, i32) -> Option<[u8; 3]>,
    x: i32,
    y: i32,
    horizontal: bool,
    vertical: bool,
    tolerance: u32,
    per_channel: bool,
) -> Span {
    let mut span = Span {
        left: x,
        right: x,
        top: y,
        bottom: y,
    };
    let Some(start) = read(x, y) else {
        return span;
    };
    let same = |px: i32, py: i32| {
        read(px, py)
            .map(|color| !differs(start, color, tolerance, per_channel))
            .unwrap_or(false)
    };
    if horizontal {
        while same(span.left - 1, y) {
            span.left -= 1;
        }
        while same(span.right + 1, y) {
            span.right += 1;
        }
    }
    if vertical {
        while same(x, span.top - 1) {
            span.top -= 1;
        }
        while same(x, span.bottom + 1) {
            span.bottom += 1;
        }
    }
    span
}

fn convert(pixels: i32, dpi: u32, unit: &str) -> Option<(f64, &'static str)> {
    let inches = f64::from(pixels) / f64::from(dpi.max(1));
    match unit {
        "inches" => Some((inches, "in")),
        "centimeters" => Some((inches * 2.54, "cm")),
        "millimeters" => Some((inches * 25.4, "mm")),
        _ => None,
    }
}

/// The label lines. The first line is what a click copies.
pub fn describe(width: Option<i32>, height: Option<i32>, dpi: u32, unit: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let pixels = match (width, height) {
        (Some(w), Some(h)) => format!("{w} × {h} px"),
        (Some(w), None) => format!("{w} px"),
        (None, Some(h)) => format!("{h} px"),
        (None, None) => return lines,
    };
    lines.push(pixels);
    let extra = |value: i32| convert(value, dpi, unit);
    let line = match (width.and_then(extra), height.and_then(extra)) {
        (Some((w, suffix)), Some((h, _))) => Some(format!("{w:.2} × {h:.2} {suffix}")),
        (Some((w, suffix)), None) => Some(format!("{w:.2} {suffix}")),
        (None, Some((h, suffix))) => Some(format!("{h:.2} {suffix}")),
        _ => None,
    };
    lines.extend(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::{Span, describe, differs, scan};

    #[test]
    fn tolerance_is_a_sum_or_per_channel() {
        assert!(!differs([10, 10, 10], [20, 20, 20], 30, false));
        assert!(differs([10, 10, 10], [21, 21, 21], 30, false));
        assert!(!differs([10, 10, 10], [40, 40, 40], 30, true));
        assert!(differs([10, 10, 10], [41, 10, 10], 30, true));
    }

    #[test]
    fn the_scan_stops_at_the_first_edge() {
        // A 10x6 image: a white box from x 2..=6, y 1..=4 on black.
        let read = |x: i32, y: i32| {
            if !(0..10).contains(&x) || !(0..6).contains(&y) {
                return None;
            }
            let inside = (2..=6).contains(&x) && (1..=4).contains(&y);
            Some(if inside { [255; 3] } else { [0; 3] })
        };
        let span = scan(read, 4, 2, true, true, 30, false);
        assert_eq!(
            span,
            Span {
                left: 2,
                right: 6,
                top: 1,
                bottom: 4
            }
        );
        assert_eq!((span.width(), span.height()), (5, 4));

        let horizontal = scan(read, 4, 2, true, false, 30, false);
        assert_eq!((horizontal.top, horizontal.bottom), (2, 2));

        let outside = scan(read, 0, 2, true, true, 30, false);
        assert_eq!((outside.left, outside.right), (0, 1));
        assert_eq!((outside.top, outside.bottom), (0, 5));
    }

    #[test]
    fn labels_add_the_extra_unit() {
        assert_eq!(describe(Some(96), None, 96, "none"), vec!["96 px"]);
        assert_eq!(
            describe(Some(96), Some(48), 96, "inches"),
            vec!["96 × 48 px", "1.00 × 0.50 in"]
        );
        assert_eq!(
            describe(None, Some(96), 96, "millimeters"),
            vec!["96 px", "25.40 mm"]
        );
        assert_eq!(
            describe(Some(192), None, 192, "centimeters"),
            vec!["192 px", "2.54 cm"]
        );
    }
}
