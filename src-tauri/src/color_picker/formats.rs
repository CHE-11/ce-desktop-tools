//! Color format templates.
//!
//! A template is text with `%` tokens, such as `rgb(%Re, %Gr, %Bl)`. Built-in
//! formats are templates too, so a custom format can use every token.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn from_hex(value: &str) -> Option<Self> {
        let [r, g, b] = crate::overlay::parse_hex(value)?;
        Some(Self { r, g, b })
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    fn unit(self) -> (f64, f64, f64) {
        (
            f64::from(self.r) / 255.0,
            f64::from(self.g) / 255.0,
            f64::from(self.b) / 255.0,
        )
    }

    fn hue(self) -> f64 {
        let (r, g, b) = self.unit();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        if delta == 0.0 {
            return 0.0;
        }
        let hue = if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        if hue < 0.0 { hue + 360.0 } else { hue }
    }

    /// `(saturation, lightness)` as 0..1.
    fn hsl(self) -> (f64, f64) {
        let (r, g, b) = self.unit();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        let delta = max - min;
        let saturation = if delta == 0.0 {
            0.0
        } else {
            delta / (1.0 - (2.0 * lightness - 1.0).abs())
        };
        (saturation, lightness)
    }

    /// `(saturation, value)` as 0..1.
    fn hsv(self) -> (f64, f64) {
        let (r, g, b) = self.unit();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let saturation = if max == 0.0 { 0.0 } else { (max - min) / max };
        (saturation, max)
    }

    /// `(saturation, intensity)` as 0..1.
    fn hsi(self) -> (f64, f64) {
        let (r, g, b) = self.unit();
        let intensity = (r + g + b) / 3.0;
        let min = r.min(g).min(b);
        let saturation = if intensity == 0.0 {
            0.0
        } else {
            1.0 - min / intensity
        };
        (saturation, intensity)
    }

    /// `(whiteness, blackness)` as 0..1.
    fn hwb(self) -> (f64, f64) {
        let (r, g, b) = self.unit();
        (r.min(g).min(b), 1.0 - r.max(g).max(b))
    }

    fn cmyk(self) -> (f64, f64, f64, f64) {
        let (r, g, b) = self.unit();
        let k = 1.0 - r.max(g).max(b);
        if k >= 1.0 {
            return (0.0, 0.0, 0.0, 1.0);
        }
        (
            (1.0 - r - k) / (1.0 - k),
            (1.0 - g - k) / (1.0 - k),
            (1.0 - b - k) / (1.0 - k),
            k,
        )
    }

    /// CIE XYZ (D65, 0..100).
    pub fn xyz(self) -> (f64, f64, f64) {
        fn linear(channel: f64) -> f64 {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }
        let (r, g, b) = self.unit();
        let (r, g, b) = (linear(r), linear(g), linear(b));
        (
            (r * 0.4124564 + g * 0.3575761 + b * 0.1804375) * 100.0,
            (r * 0.2126729 + g * 0.7151522 + b * 0.0721750) * 100.0,
            (r * 0.0193339 + g * 0.1191920 + b * 0.9503041) * 100.0,
        )
    }

    /// CIE L*a*b* (D65).
    pub fn lab(self) -> (f64, f64, f64) {
        fn f(t: f64) -> f64 {
            let delta: f64 = 6.0 / 29.0;
            if t > delta.powi(3) {
                t.cbrt()
            } else {
                t / (3.0 * delta * delta) + 4.0 / 29.0
            }
        }
        let (x, y, z) = self.xyz();
        let (fx, fy, fz) = (f(x / 95.047), f(y / 100.0), f(z / 108.883));
        (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
    }

    fn ncol_hue(self) -> String {
        const LETTERS: [char; 6] = ['R', 'Y', 'G', 'C', 'B', 'M'];
        let hue = self.hue();
        let mut index = (hue / 60.0).floor() as usize % 6;
        let mut percent = ((hue % 60.0) / 60.0 * 100.0).round() as u32;
        if percent == 100 {
            percent = 0;
            index = (index + 1) % 6;
        }
        format!("{}{}", LETTERS[index], percent)
    }
}

pub struct BuiltIn {
    pub name: &'static str,
    pub template: &'static str,
    pub visible: bool,
}

pub const BUILT_INS: &[BuiltIn] = &[
    BuiltIn { name: "HEX", template: "%Rex%Grx%Blx", visible: true },
    BuiltIn { name: "RGB", template: "rgb(%Re, %Gr, %Bl)", visible: true },
    BuiltIn { name: "HSL", template: "hsl(%Hu, %Sl%, %Ll%)", visible: true },
    BuiltIn { name: "HSV", template: "hsv(%Hu, %Sb%, %Va%)", visible: false },
    BuiltIn { name: "CMYK", template: "cmyk(%Cy%, %Ma%, %Ye%, %Bk%)", visible: false },
    BuiltIn { name: "HSB", template: "hsb(%Hu, %Sb%, %Br%)", visible: false },
    BuiltIn { name: "HSI", template: "hsi(%Hu, %Si%, %In%)", visible: false },
    BuiltIn { name: "HWB", template: "hwb(%Hu, %Wh%, %Bn%)", visible: false },
    BuiltIn { name: "NCol", template: "%Hn, %Wh%, %Bn%", visible: false },
    BuiltIn { name: "CIELAB", template: "CIELab(%Lc, %Ca, %Cb)", visible: false },
    BuiltIn { name: "CIEXYZ", template: "XYZ(%Xv, %Yv, %Zv)", visible: false },
    BuiltIn { name: "VEC4", template: "(%Reff, %Grff, %Blff, 1f)", visible: false },
    BuiltIn { name: "Decimal", template: "%Dv", visible: false },
    BuiltIn { name: "HEX Int", template: "0xFF%ReX%GrX%BlX", visible: false },
];

pub fn built_in(name: &str) -> Option<&'static BuiltIn> {
    BUILT_INS.iter().find(|format| format.name == name)
}

/// Longer tokens come first so `%Reff` is not read as `%Re` + `ff`.
const TOKENS: &[&str] = &[
    "%Reff", "%Grff", "%Blff", "%Rex", "%ReX", "%Grx", "%GrX", "%Blx", "%BlX", "%Re", "%Gr",
    "%Bl", "%Al", "%Cy", "%Ma", "%Ye", "%Bk", "%Hu", "%Sl", "%Ll", "%Sb", "%Va", "%Br", "%Si",
    "%In", "%Wh", "%Bn", "%Hn", "%Lc", "%Ca", "%Cb", "%Xv", "%Yv", "%Zv", "%Dv", "%Na",
];

/// Every token with a short description, for the format editor.
pub const TOKEN_HELP: &[(&str, &str)] = &[
    ("%Re", "Red 0-255"),
    ("%Gr", "Green 0-255"),
    ("%Bl", "Blue 0-255"),
    ("%Al", "Alpha 0-255"),
    ("%Rex", "Red hex, lowercase"),
    ("%ReX", "Red hex, uppercase"),
    ("%Grx", "Green hex, lowercase"),
    ("%GrX", "Green hex, uppercase"),
    ("%Blx", "Blue hex, lowercase"),
    ("%BlX", "Blue hex, uppercase"),
    ("%Reff", "Red 0-1"),
    ("%Grff", "Green 0-1"),
    ("%Blff", "Blue 0-1"),
    ("%Hu", "Hue 0-360"),
    ("%Sl", "HSL saturation %"),
    ("%Ll", "HSL lightness %"),
    ("%Sb", "HSV saturation %"),
    ("%Va", "HSV value %"),
    ("%Br", "HSB brightness %"),
    ("%Si", "HSI saturation %"),
    ("%In", "HSI intensity %"),
    ("%Wh", "HWB whiteness %"),
    ("%Bn", "HWB blackness %"),
    ("%Hn", "NCol hue"),
    ("%Cy", "CMYK cyan %"),
    ("%Ma", "CMYK magenta %"),
    ("%Ye", "CMYK yellow %"),
    ("%Bk", "CMYK black %"),
    ("%Lc", "CIELAB lightness"),
    ("%Ca", "CIELAB a"),
    ("%Cb", "CIELAB b"),
    ("%Xv", "CIEXYZ X"),
    ("%Yv", "CIEXYZ Y"),
    ("%Zv", "CIEXYZ Z"),
    ("%Dv", "Decimal value"),
    ("%Na", "Color name"),
];

fn percent(value: f64) -> String {
    format!("{}", (value * 100.0).round() as i64)
}

fn trimmed(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

fn token_value(token: &str, color: Rgb) -> String {
    match token {
        "%Re" => color.r.to_string(),
        "%Gr" => color.g.to_string(),
        "%Bl" => color.b.to_string(),
        "%Al" => "255".into(),
        "%Rex" => format!("{:02x}", color.r),
        "%ReX" => format!("{:02X}", color.r),
        "%Grx" => format!("{:02x}", color.g),
        "%GrX" => format!("{:02X}", color.g),
        "%Blx" => format!("{:02x}", color.b),
        "%BlX" => format!("{:02X}", color.b),
        "%Reff" => format!("{}f", trimmed(f64::from(color.r) / 255.0, 2)),
        "%Grff" => format!("{}f", trimmed(f64::from(color.g) / 255.0, 2)),
        "%Blff" => format!("{}f", trimmed(f64::from(color.b) / 255.0, 2)),
        "%Hu" => format!("{}", color.hue().round() as i64 % 360),
        "%Sl" => percent(color.hsl().0),
        "%Ll" => percent(color.hsl().1),
        "%Sb" => percent(color.hsv().0),
        "%Va" | "%Br" => percent(color.hsv().1),
        "%Si" => percent(color.hsi().0),
        "%In" => percent(color.hsi().1),
        "%Wh" => percent(color.hwb().0),
        "%Bn" => percent(color.hwb().1),
        "%Hn" => color.ncol_hue(),
        "%Cy" => percent(color.cmyk().0),
        "%Ma" => percent(color.cmyk().1),
        "%Ye" => percent(color.cmyk().2),
        "%Bk" => percent(color.cmyk().3),
        "%Lc" => format!("{:.2}", color.lab().0),
        "%Ca" => format!("{:.2}", color.lab().1),
        "%Cb" => format!("{:.2}", color.lab().2),
        "%Xv" => format!("{:.4}", color.xyz().0),
        "%Yv" => format!("{:.4}", color.xyz().1),
        "%Zv" => format!("{:.4}", color.xyz().2),
        "%Dv" => (u32::from(color.r) + u32::from(color.g) * 256 + u32::from(color.b) * 65536)
            .to_string(),
        "%Na" => super::names::nearest(color).to_string(),
        _ => token.to_string(),
    }
}

pub fn render(template: &str, color: Rgb) -> String {
    let mut output = String::new();
    let mut rest = template;
    while !rest.is_empty() {
        if rest.starts_with('%') {
            if let Some(token) = TOKENS.iter().find(|token| rest.starts_with(**token)) {
                output.push_str(&token_value(token, color));
                rest = &rest[token.len()..];
                continue;
            }
        }
        let mut chars = rest.chars();
        if let Some(next) = chars.next() {
            output.push(next);
        }
        rest = chars.as_str();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{BUILT_INS, Rgb, render};

    fn sample() -> Rgb {
        Rgb::from_hex("#ffe4b5").unwrap()
    }

    fn built(name: &str) -> String {
        let format = BUILT_INS.iter().find(|format| format.name == name).unwrap();
        render(format.template, sample())
    }

    #[test]
    fn built_ins_match_the_powertoys_samples() {
        assert_eq!(built("HEX"), "ffe4b5");
        assert_eq!(built("RGB"), "rgb(255, 228, 181)");
        assert_eq!(built("HSL"), "hsl(38, 100%, 85%)");
        assert_eq!(built("HSV"), "hsv(38, 29%, 100%)");
        assert_eq!(built("CMYK"), "cmyk(0%, 11%, 29%, 0%)");
        assert_eq!(built("HSB"), "hsb(38, 29%, 100%)");
        assert_eq!(built("HSI"), "hsi(38, 18%, 87%)");
        assert_eq!(built("HWB"), "hwb(38, 71%, 0%)");
        assert_eq!(built("NCol"), "R64, 71%, 0%");
        assert_eq!(built("VEC4"), "(1f, 0.89f, 0.71f, 1f)");
        assert_eq!(built("HEX Int"), "0xFFFFE4B5");
        assert_eq!(built("Decimal"), "11920639");
    }

    #[test]
    fn cie_values_are_close_to_the_powertoys_samples() {
        let (x, y, z) = sample().xyz();
        assert!((x - 77.3209).abs() < 0.1, "{x}");
        assert!((y - 80.0841).abs() < 0.1, "{y}");
        assert!((z - 55.1024).abs() < 0.1, "{z}");
        let (l, a, b) = sample().lab();
        assert!((l - 91.72).abs() < 0.05, "{l}");
        assert!((a - 2.44).abs() < 0.1, "{a}");
        assert!((b - 26.36).abs() < 0.1, "{b}");
    }

    #[test]
    fn custom_templates_keep_plain_text_and_percent_signs() {
        assert_eq!(render("#%ReX%GrX%BlX 100%", sample()), "#FFE4B5 100%");
        assert_eq!(render("%Na", Rgb::from_hex("#ff0000").unwrap()), "Red");
        assert_eq!(render("%Unknown", sample()), "%Unknown");
    }

    #[test]
    fn black_white_and_gray_have_no_hue() {
        let black = Rgb::from_hex("#000000").unwrap();
        assert_eq!(render("hsl(%Hu, %Sl%, %Ll%)", black), "hsl(0, 0%, 0%)");
        assert_eq!(render("cmyk(%Cy%, %Ma%, %Ye%, %Bk%)", black), "cmyk(0%, 0%, 0%, 100%)");
        let white = Rgb::from_hex("#ffffff").unwrap();
        assert_eq!(render("%Hn, %Wh%, %Bn%", white), "R0, 100%, 0%");
    }
}
