//! `<number> <unit> to <unit>` typed into the launcher.

use super::calc::format_number;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dimension {
    Length,
    Mass,
    Volume,
    Area,
    Speed,
    Time,
    Data,
    Temperature,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scale {
    /// Multiply by this to reach the dimension's base unit.
    Factor(f64),
    Celsius,
    Fahrenheit,
    Kelvin,
}

struct Unit {
    symbol: &'static str,
    names: &'static [&'static str],
    dimension: Dimension,
    scale: Scale,
}

const fn unit(
    symbol: &'static str,
    names: &'static [&'static str],
    dimension: Dimension,
    factor: f64,
) -> Unit {
    Unit {
        symbol,
        names,
        dimension,
        scale: Scale::Factor(factor),
    }
}

use Dimension::*;

const UNITS: &[Unit] = &[
    // Length, in meters.
    unit("mm", &["mm", "millimeter", "millimetre"], Length, 0.001),
    unit("cm", &["cm", "centimeter", "centimetre"], Length, 0.01),
    unit("m", &["m", "meter", "metre"], Length, 1.0),
    unit("km", &["km", "kilometer", "kilometre"], Length, 1000.0),
    unit("µm", &["um", "µm", "micrometer", "micron"], Length, 1e-6),
    unit("in", &["in", "inch", "inches", "\""], Length, 0.0254),
    unit("ft", &["ft", "foot", "feet", "'"], Length, 0.3048),
    unit("yd", &["yd", "yard"], Length, 0.9144),
    unit("mi", &["mi", "mile"], Length, 1609.344),
    unit("nmi", &["nmi", "nautical mile"], Length, 1852.0),
    // Mass, in kilograms.
    unit("mg", &["mg", "milligram"], Mass, 1e-6),
    unit("g", &["g", "gram", "gr"], Mass, 0.001),
    unit("kg", &["kg", "kilogram", "kilo"], Mass, 1.0),
    unit("t", &["t", "tonne", "metric ton"], Mass, 1000.0),
    unit("oz", &["oz", "ounce"], Mass, 0.028349523125),
    unit("lb", &["lb", "lbs", "pound"], Mass, 0.45359237),
    unit("st", &["st", "stone"], Mass, 6.35029318),
    unit("ton", &["ton", "short ton"], Mass, 907.18474),
    // Volume, in liters.
    unit("ml", &["ml", "milliliter", "millilitre"], Volume, 0.001),
    unit("cl", &["cl", "centiliter", "centilitre"], Volume, 0.01),
    unit("dl", &["dl", "deciliter", "decilitre"], Volume, 0.1),
    unit("L", &["l", "liter", "litre"], Volume, 1.0),
    unit("m³", &["m3", "m³", "cubic meter", "cubic metre"], Volume, 1000.0),
    unit("tsp", &["tsp", "teaspoon"], Volume, 0.00492892159375),
    unit("tbsp", &["tbsp", "tablespoon"], Volume, 0.01478676478125),
    unit("fl oz", &["fl oz", "floz", "fluid ounce"], Volume, 0.0295735295625),
    unit("cup", &["cup"], Volume, 0.2365882365),
    unit("pt", &["pt", "pint"], Volume, 0.473176473),
    unit("qt", &["qt", "quart"], Volume, 0.946352946),
    unit("gal", &["gal", "gallon"], Volume, 3.785411784),
    // Area, in square meters.
    unit("mm²", &["mm2", "mm²", "sqmm"], Area, 1e-6),
    unit("cm²", &["cm2", "cm²", "sqcm"], Area, 1e-4),
    unit("m²", &["m2", "m²", "sqm", "square meter", "square metre"], Area, 1.0),
    unit("km²", &["km2", "km²", "sqkm", "square kilometer"], Area, 1e6),
    unit("ha", &["ha", "hectare"], Area, 1e4),
    unit("acre", &["acre", "ac"], Area, 4046.8564224),
    unit("in²", &["in2", "in²", "sqin", "square inch"], Area, 0.00064516),
    unit("ft²", &["ft2", "ft²", "sqft", "square foot", "square feet"], Area, 0.09290304),
    unit("yd²", &["yd2", "yd²", "sqyd", "square yard"], Area, 0.83612736),
    unit("mi²", &["mi2", "mi²", "sqmi", "square mile"], Area, 2589988.110336),
    // Speed, in meters per second.
    unit("m/s", &["m/s", "mps"], Speed, 1.0),
    unit("km/h", &["km/h", "kmh", "kph", "kmph"], Speed, 1000.0 / 3600.0),
    unit("mph", &["mph", "mi/h"], Speed, 1609.344 / 3600.0),
    unit("ft/s", &["ft/s", "fps"], Speed, 0.3048),
    unit("kn", &["kn", "kt", "knot"], Speed, 1852.0 / 3600.0),
    // Time, in seconds.
    unit("ms", &["ms", "millisecond"], Time, 0.001),
    unit("s", &["s", "sec", "second"], Time, 1.0),
    unit("min", &["min", "minute"], Time, 60.0),
    unit("h", &["h", "hr", "hour"], Time, 3600.0),
    unit("d", &["d", "day"], Time, 86400.0),
    unit("wk", &["wk", "week"], Time, 604800.0),
    unit("mo", &["mo", "month"], Time, 2629746.0),
    unit("yr", &["yr", "y", "year"], Time, 31556952.0),
    // Data, in bytes.
    unit("bit", &["bit"], Data, 0.125),
    unit("B", &["byte"], Data, 1.0),
    unit("KB", &["kb", "kilobyte"], Data, 1e3),
    unit("MB", &["mb", "megabyte"], Data, 1e6),
    unit("GB", &["gb", "gigabyte"], Data, 1e9),
    unit("TB", &["tb", "terabyte"], Data, 1e12),
    unit("PB", &["pb", "petabyte"], Data, 1e15),
    unit("KiB", &["kib", "kibibyte"], Data, 1024.0),
    unit("MiB", &["mib", "mebibyte"], Data, 1048576.0),
    unit("GiB", &["gib", "gibibyte"], Data, 1073741824.0),
    unit("TiB", &["tib", "tebibyte"], Data, 1099511627776.0),
    unit("kbit", &["kbit", "kbps", "kilobit"], Data, 125.0),
    unit("Mbit", &["mbit", "mbps", "megabit"], Data, 125000.0),
    unit("Gbit", &["gbit", "gbps", "gigabit"], Data, 125000000.0),
    Unit {
        symbol: "°C",
        names: &["c", "°c", "celsius", "centigrade", "degc"],
        dimension: Temperature,
        scale: Scale::Celsius,
    },
    Unit {
        symbol: "°F",
        names: &["f", "°f", "fahrenheit", "degf"],
        dimension: Temperature,
        scale: Scale::Fahrenheit,
    },
    Unit {
        symbol: "K",
        names: &["k", "kelvin"],
        dimension: Temperature,
        scale: Scale::Kelvin,
    },
];

#[derive(Clone, Debug, PartialEq)]
pub struct Conversion {
    pub value: String,
    pub title: String,
}

pub fn convert(input: &str) -> Option<Conversion> {
    let text = input.trim();
    let (left, right) = split_target(text)?;
    let (amount, from_name) = split_amount(left)?;
    let from = find_unit(from_name)?;
    let to = find_unit(right)?;
    if from.dimension != to.dimension || std::ptr::eq(from, to) {
        return None;
    }
    let result = to_unit(to_base(amount, from.scale), to.scale);
    if !result.is_finite() {
        return None;
    }
    let value = format_number(round_significant(result));
    Some(Conversion {
        title: format!(
            "{} {} = {} {}",
            format_number(amount),
            from.symbol,
            value,
            to.symbol
        ),
        value,
    })
}

fn split_target(text: &str) -> Option<(&str, &str)> {
    let lower = text.to_lowercase();
    for separator in [" to ", " -> ", " => ", " = ", " as "] {
        if let Some(position) = lower.find(separator) {
            return Some((
                text[..position].trim(),
                text[position + separator.len()..].trim(),
            ));
        }
    }
    // The last " in " splits, so `5 in in cm` reads as inches to centimeters.
    let position = lower.rfind(" in ")?;
    Some((text[..position].trim(), text[position + 4..].trim()))
}

fn split_amount(text: &str) -> Option<(f64, &str)> {
    let end = text
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '+')))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let number: String = text[..end].chars().filter(|ch| *ch != ',').collect();
    let amount = number.parse::<f64>().ok()?;
    let unit = text[end..].trim();
    if unit.is_empty() {
        return None;
    }
    Some((amount, unit))
}

fn find_unit(name: &str) -> Option<&'static Unit> {
    let name = name.trim();
    // Bits and bytes are told apart by case.
    if name == "b" {
        return UNITS.iter().find(|unit| unit.symbol == "bit");
    }
    if name == "B" {
        return UNITS.iter().find(|unit| unit.symbol == "B");
    }
    let lower = name.to_lowercase();
    let lower = lower.trim_start_matches("degrees ").trim_start_matches("degree ");
    if let Some(unit) = lookup(lower) {
        return Some(unit);
    }
    let singular = lower.strip_suffix("es").filter(|stem| stem.len() > 2);
    if let Some(unit) = singular.and_then(lookup) {
        return Some(unit);
    }
    lower
        .strip_suffix('s')
        .filter(|stem| stem.len() > 1)
        .and_then(lookup)
}

fn lookup(name: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|unit| unit.names.contains(&name))
}

fn to_base(value: f64, scale: Scale) -> f64 {
    match scale {
        Scale::Factor(factor) => value * factor,
        Scale::Celsius => value,
        Scale::Fahrenheit => (value - 32.0) * 5.0 / 9.0,
        Scale::Kelvin => value - 273.15,
    }
}

fn to_unit(base: f64, scale: Scale) -> f64 {
    match scale {
        Scale::Factor(factor) => base / factor,
        Scale::Celsius => base,
        Scale::Fahrenheit => base * 9.0 / 5.0 + 32.0,
        Scale::Kelvin => base + 273.15,
    }
}

fn round_significant(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    let digits = 8 - value.abs().log10().ceil() as i32;
    let factor = 10f64.powi(digits.clamp(-15, 15));
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::convert;

    fn value(input: &str) -> String {
        convert(input).map(|answer| answer.value).unwrap_or_default()
    }

    #[test]
    fn lengths_and_masses_convert() {
        assert_eq!(value("5 km to mi"), "3.106856");
        assert_eq!(value("12 in to cm"), "30.48");
        assert_eq!(value("5 in in cm"), "12.7");
        assert_eq!(value("1 mile in feet"), "5280");
        assert_eq!(value("10kg to lbs"), "22.046226");
        assert_eq!(value("2 pounds to kg"), "0.90718474");
    }

    #[test]
    fn temperature_uses_offsets() {
        assert_eq!(value("100 c to f"), "212");
        assert_eq!(value("32 °F to °C"), "0");
        assert_eq!(value("-40 fahrenheit to celsius"), "-40");
        assert_eq!(value("0 c to k"), "273.15");
    }

    #[test]
    fn data_sizes_respect_binary_and_bits() {
        assert_eq!(value("1 GiB to MB"), "1073.7418");
        assert_eq!(value("1 GB to MiB"), "953.67432");
        assert_eq!(value("8 b to B"), "1");
        assert_eq!(value("100 mbps to MB"), "12.5");
    }

    #[test]
    fn mismatched_or_unknown_units_are_ignored() {
        assert!(convert("5 km to kg").is_none());
        assert!(convert("5 km").is_none());
        assert!(convert("chrome to edge").is_none());
        assert!(convert("5 parsecs to km").is_none());
        assert!(convert("5 km to km").is_none());
    }
}
