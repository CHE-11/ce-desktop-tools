//! `<number> <unit> to <unit>` typed into the launcher.

use serde::Serialize;

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
    unit(
        "m³",
        &["m3", "m³", "cubic meter", "cubic metre"],
        Volume,
        1000.0,
    ),
    unit("tsp", &["tsp", "teaspoon"], Volume, 0.00492892159375),
    unit("tbsp", &["tbsp", "tablespoon"], Volume, 0.01478676478125),
    unit(
        "fl oz",
        &["fl oz", "floz", "fluid ounce"],
        Volume,
        0.0295735295625,
    ),
    unit("cup", &["cup"], Volume, 0.2365882365),
    unit("pt", &["pt", "pint"], Volume, 0.473176473),
    unit("qt", &["qt", "quart"], Volume, 0.946352946),
    unit("gal", &["gal", "gallon"], Volume, 3.785411784),
    // Area, in square meters.
    unit("mm²", &["mm2", "mm²", "sqmm"], Area, 1e-6),
    unit("cm²", &["cm2", "cm²", "sqcm"], Area, 1e-4),
    unit(
        "m²",
        &["m2", "m²", "sqm", "square meter", "square metre"],
        Area,
        1.0,
    ),
    unit(
        "km²",
        &["km2", "km²", "sqkm", "square kilometer"],
        Area,
        1e6,
    ),
    unit("ha", &["ha", "hectare"], Area, 1e4),
    unit("acre", &["acre", "ac"], Area, 4046.8564224),
    unit(
        "in²",
        &["in2", "in²", "sqin", "square inch"],
        Area,
        0.00064516,
    ),
    unit(
        "ft²",
        &["ft2", "ft²", "sqft", "square foot", "square feet"],
        Area,
        0.09290304,
    ),
    unit(
        "yd²",
        &["yd2", "yd²", "sqyd", "square yard"],
        Area,
        0.83612736,
    ),
    unit(
        "mi²",
        &["mi2", "mi²", "sqmi", "square mile"],
        Area,
        2589988.110336,
    ),
    // Speed, in meters per second.
    unit("m/s", &["m/s", "mps"], Speed, 1.0),
    unit(
        "km/h",
        &["km/h", "kmh", "kph", "kmph"],
        Speed,
        1000.0 / 3600.0,
    ),
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
    let lower = lower
        .trim_start_matches("degrees ")
        .trim_start_matches("degree ");
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitRow {
    pub symbol: String,
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitsAnswer {
    /// `conversion`, `math`, or `message`.
    pub kind: String,
    pub title: String,
    pub value: String,
    pub dimension: Option<String>,
    pub rows: Vec<UnitRow>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDimension {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogUnit {
    pub symbol: String,
    pub label: String,
    pub dimension: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitsCatalog {
    pub dimensions: Vec<CatalogDimension>,
    pub units: Vec<CatalogUnit>,
}

struct Measure {
    amount: f64,
    unit: &'static Unit,
}

/// A conversion, every equivalent unit, or a calculation.
pub fn interpret(input: &str) -> UnitsAnswer {
    let text = input.trim();
    if text.is_empty() {
        return message("");
    }
    if let Some(answer) = try_convert(text) {
        return answer;
    }
    if let Some(measures) = parse_measures(text) {
        return conversion_answer(&measures, None);
    }
    if let Some(answer) = super::calc::evaluate(text) {
        return math_answer(answer.detail, answer.value);
    }
    if let Some(value) = lone_constant(text) {
        return math_answer(text.to_string(), format_number(value));
    }
    if let Some(error) = convert_failure(text) {
        return message(&error);
    }
    if let Some(name) = leftover_unit(text) {
        return message(&format!("Unknown unit \"{name}\"."));
    }
    message("Type a conversion or a calculation.")
}

pub fn catalog() -> UnitsCatalog {
    let mut dimensions = Vec::new();
    let mut units = Vec::new();
    for unit in UNITS {
        let id = dimension_id(unit.dimension);
        if !dimensions
            .iter()
            .any(|item: &CatalogDimension| item.id == id)
        {
            dimensions.push(CatalogDimension {
                id: id.to_string(),
                label: dimension_label(unit.dimension).to_string(),
            });
        }
        units.push(CatalogUnit {
            symbol: unit.symbol.to_string(),
            label: unit_label(unit),
            dimension: id.to_string(),
        });
    }
    UnitsCatalog { dimensions, units }
}

fn try_convert(text: &str) -> Option<UnitsAnswer> {
    let (left, right) = split_target(text)?;
    let measures = parse_measures(left)?;
    let to = find_unit(right)?;
    if measures[0].unit.dimension != to.dimension {
        return None;
    }
    Some(conversion_answer(&measures, Some(to)))
}

fn convert_failure(text: &str) -> Option<String> {
    let (left, right) = split_target(text)?;
    let measures = parse_measures(left)?;
    let right = right.trim();
    if right.is_empty() {
        return None;
    }
    match find_unit(right) {
        Some(to) if to.dimension != measures[0].unit.dimension => Some(format!(
            "Can't convert {} to {}.",
            measures[0].unit.symbol, to.symbol
        )),
        Some(_) => None,
        None => Some(format!("Unknown unit \"{right}\".")),
    }
}

fn conversion_answer(measures: &[Measure], to: Option<&Unit>) -> UnitsAnswer {
    let base: f64 = measures
        .iter()
        .map(|measure| to_base(measure.amount, measure.unit.scale))
        .sum();
    if !base.is_finite() {
        return message("That conversion is not a number.");
    }
    let dimension = measures[0].unit.dimension;
    let rows = rows_for(base, dimension);
    let label = measures
        .iter()
        .map(|measure| format!("{} {}", format_number(measure.amount), measure.unit.symbol))
        .collect::<Vec<_>>()
        .join(" ");
    let (value, title, to_symbol) = if let Some(to) = to {
        let Some(value) = format_scaled(base, to.scale) else {
            return message("That conversion is not a number.");
        };
        (
            value.clone(),
            format!("{label} = {value} {}", to.symbol),
            Some(to.symbol.to_string()),
        )
    } else {
        let value = rows
            .iter()
            .find(|row| row.symbol == measures[0].unit.symbol)
            .map(|row| row.value.clone())
            .unwrap_or_else(|| format_number(measures[0].amount));
        (value, label, None)
    };
    UnitsAnswer {
        kind: "conversion".into(),
        title,
        value,
        dimension: Some(dimension_id(dimension).into()),
        rows,
        from: Some(measures[0].unit.symbol.into()),
        to: to_symbol,
    }
}

fn rows_for(base: f64, dimension: Dimension) -> Vec<UnitRow> {
    UNITS
        .iter()
        .filter(|unit| unit.dimension == dimension)
        .filter_map(|unit| {
            Some(UnitRow {
                symbol: unit.symbol.to_string(),
                label: unit_label(unit),
                value: format_scaled(base, unit.scale)?,
            })
        })
        .collect()
}

fn format_scaled(base: f64, scale: Scale) -> Option<String> {
    let result = to_unit(base, scale);
    result
        .is_finite()
        .then(|| format_number(round_significant(result)))
}

fn math_answer(detail: String, value: String) -> UnitsAnswer {
    UnitsAnswer {
        kind: "math".into(),
        title: detail,
        value,
        dimension: None,
        rows: Vec::new(),
        from: None,
        to: None,
    }
}

fn message(title: &str) -> UnitsAnswer {
    UnitsAnswer {
        kind: "message".into(),
        title: title.to_string(),
        value: String::new(),
        dimension: None,
        rows: Vec::new(),
        from: None,
        to: None,
    }
}

fn lone_constant(text: &str) -> Option<f64> {
    match text
        .trim()
        .trim_start_matches('=')
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "pi" | "π" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" => Some(std::f64::consts::TAU),
        _ => None,
    }
}

fn leftover_unit(text: &str) -> Option<String> {
    let text = text.trim();
    let (_, len) = read_amount(text)?;
    let rest = text[len..].trim();
    let first = rest.chars().next()?;
    if !first.is_alphabetic() && first != '°' && first != 'µ' {
        return None;
    }
    Some(rest.to_string())
}

fn parse_measures(text: &str) -> Option<Vec<Measure>> {
    let mut rest = text.trim();
    if rest.is_empty() {
        return None;
    }
    let mut measures = Vec::new();
    while !rest.is_empty() {
        let (amount, amount_len) = read_amount(rest)?;
        let after_amount = rest[amount_len..].trim_start();
        let (unit, unit_len) = take_unit(after_amount)?;
        if unit_len == 0 {
            return None;
        }
        measures.push(Measure { amount, unit });
        rest = after_amount[unit_len..].trim_start();
        rest = rest
            .trim_start_matches(|ch: char| matches!(ch, '.' | ','))
            .trim_start();
        if let Some(next) = strip_and(rest) {
            rest = next;
        }
    }
    let dimension = measures.first()?.unit.dimension;
    if measures
        .iter()
        .any(|measure| measure.unit.dimension != dimension)
    {
        return None;
    }
    Some(measures)
}

fn strip_and(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    lower.strip_prefix("and")?;
    let after = &text["and".len()..];
    if after.is_empty() {
        return Some(after);
    }
    after
        .starts_with(|ch: char| ch.is_whitespace())
        .then(|| after.trim_start())
}

fn read_amount(text: &str) -> Option<(f64, usize)> {
    let (whole, whole_len) = read_decimal(text)?;
    if let Some(divided) = read_division(text, whole, whole_len) {
        return Some(divided);
    }
    let rest = text[whole_len..].trim_start();
    let gap = text[whole_len..].len() - rest.len();
    if gap > 0
        && let Some((numer, numer_len)) = read_decimal(rest)
        && numer >= 0.0
        && let Some((fraction, fraction_end)) = read_division(rest, numer, numer_len)
    {
        let sign = if whole.is_sign_negative() { -1.0 } else { 1.0 };
        return Some((
            whole + sign * fraction.abs(),
            whole_len + gap + fraction_end,
        ));
    }
    Some((whole, whole_len))
}

fn read_division(text: &str, numer: f64, numer_len: usize) -> Option<(f64, usize)> {
    let rest = text[numer_len..].trim_start();
    if !rest.starts_with('/') {
        return None;
    }
    let slash_gap = text[numer_len..].len() - rest.len();
    let after = rest[1..].trim_start();
    let den_gap = rest[1..].len() - after.len();
    let (den, den_len) = read_decimal(after)?;
    if den == 0.0 || den.is_sign_negative() {
        return None;
    }
    Some((numer / den, numer_len + slash_gap + 1 + den_gap + den_len))
}

fn read_decimal(text: &str) -> Option<(f64, usize)> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut index = 0;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        index = 1;
    }
    let mut seen_digit = false;
    let mut seen_dot = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if ch.is_ascii_digit() {
            seen_digit = true;
            index += 1;
        } else if ch == b',' && comma_group(bytes, index) {
            index += 1;
        } else if ch == b'.' && !seen_dot {
            seen_dot = true;
            index += 1;
        } else {
            break;
        }
    }
    if !seen_digit {
        return None;
    }
    let raw: String = text[..index].chars().filter(|ch| *ch != ',').collect();
    let value = raw.parse::<f64>().ok()?;
    value.is_finite().then_some((value, index))
}

fn comma_group(bytes: &[u8], index: usize) -> bool {
    let before = index > 0 && bytes[index - 1].is_ascii_digit();
    let after = bytes.len() >= index + 4
        && bytes[index + 1..index + 4]
            .iter()
            .all(|ch| ch.is_ascii_digit())
        && bytes.get(index + 4).is_none_or(|ch| !ch.is_ascii_digit());
    before && after
}

fn take_unit(text: &str) -> Option<(&'static Unit, usize)> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let lead = text.len() - trimmed.len();
    let body = skip_degrees(trimmed);
    let skipped = trimmed.len() - body.len();
    let (unit, len) = quote_unit(body)
        .or_else(|| longest_name(body))
        .or_else(|| phrase_unit(body))?;
    (len > 0).then_some((unit, lead + skipped + len))
}

fn skip_degrees(text: &str) -> &str {
    let lower = text.to_ascii_lowercase();
    for prefix in ["degrees ", "degree "] {
        if lower.starts_with(prefix) {
            return text[prefix.len()..].trim_start();
        }
    }
    text
}

fn quote_unit(text: &str) -> Option<(&'static Unit, usize)> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    let unit = if matches!(ch, '\'' | '′' | '’') {
        find_unit("ft")?
    } else if matches!(ch, '"' | '″' | '”') {
        find_unit("in")?
    } else {
        return None;
    };
    Some((unit, text.len() - chars.as_str().len()))
}

fn longest_name(text: &str) -> Option<(&'static Unit, usize)> {
    let mut best: Option<(&Unit, usize)> = None;
    for unit in UNITS {
        for name in unit.names {
            if let Some(len) = name_prefix(text, name) {
                let replace = best.map(|(_, current)| len > current).unwrap_or(true);
                if replace {
                    best = Some((unit, len));
                }
            }
        }
    }
    best
}

fn name_prefix(text: &str, name: &str) -> Option<usize> {
    let mut rest = text;
    for name_ch in name.chars() {
        let mut chars = rest.chars();
        let text_ch = chars.next()?;
        if !text_ch.eq_ignore_ascii_case(&name_ch) {
            return None;
        }
        rest = chars.as_str();
    }
    let consumed = text.len() - rest.len();
    let boundary = rest.is_empty()
        || rest.starts_with(|ch: char| {
            ch.is_whitespace()
                || ch.is_ascii_digit()
                || matches!(
                    ch,
                    '\'' | '"'
                        | '′'
                        | '″'
                        | '’'
                        | '”'
                        | ','
                        | '.'
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '%'
                        | '^'
                        | '!'
                        | '('
                        | ')'
                        | '#'
                        | '='
                )
        });
    boundary.then_some(consumed)
}

fn phrase_unit(text: &str) -> Option<(&'static Unit, usize)> {
    let mut best: Option<(&Unit, usize)> = None;
    let mut cursor = 0;
    for _ in 0..3 {
        if cursor >= text.len() {
            break;
        }
        let rest = &text[cursor..];
        if rest.starts_with(|ch: char| {
            ch.is_whitespace() || matches!(ch, '\'' | '"' | '′' | '″' | '’' | '”')
        }) {
            break;
        }
        let word_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
        cursor += word_len;
        if let Some(unit) = find_unit(&text[..cursor]) {
            best = Some((unit, cursor));
        }
        let after = text[cursor..].trim_start();
        if after.len() == text[cursor..].len()
            || after.is_empty()
            || after.starts_with(|ch: char| {
                ch.is_ascii_digit() || matches!(ch, '\'' | '"' | '′' | '″' | '’' | '”')
            })
        {
            break;
        }
        cursor = text.len() - after.len();
    }
    best
}

fn unit_label(unit: &Unit) -> String {
    let name = unit
        .names
        .iter()
        .copied()
        .find(|name| name.contains(' '))
        .or_else(|| {
            unit.names
                .iter()
                .copied()
                .find(|name| name.len() >= 4 && name.chars().all(|ch| ch.is_ascii_alphabetic()))
        });
    match name {
        Some(name) => title_case(name),
        None => unit.symbol.to_string(),
    }
}

fn title_case(name: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for ch in name.chars() {
        if ch.is_whitespace() {
            cap = true;
            out.push(ch);
        } else if cap {
            out.extend(ch.to_uppercase());
            cap = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn dimension_id(dimension: Dimension) -> &'static str {
    match dimension {
        Length => "length",
        Mass => "mass",
        Volume => "volume",
        Area => "area",
        Speed => "speed",
        Time => "time",
        Data => "data",
        Temperature => "temperature",
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TakenQuantity {
    pub base: f64,
    pub dimension: &'static str,
    pub symbol: &'static str,
}

/// A leading number, including `1/2` and `.5`. The length counts a leading space.
pub(crate) fn take_number(text: &str) -> Option<(f64, usize)> {
    let lead = text.len() - text.trim_start().len();
    let (value, len) = read_amount(text.trim_start())?;
    Some((value, lead + len))
}

/// One quantity, including a mixed length such as `5 ft 6 in`.
/// Stops before `+`, `-`, or another operator so `5mm+10in` is two quantities.
pub(crate) fn take_quantity(text: &str) -> Option<(TakenQuantity, usize)> {
    let mut index = text.len() - text.trim_start().len();
    let mut measures = Vec::new();
    loop {
        let slice = text.get(index..)?;
        let lead = slice.len() - slice.trim_start().len();
        let body = slice.trim_start();
        let (amount, amount_len) = read_amount(body)?;
        let after_amount_full = &body[amount_len..];
        let after_amount = after_amount_full.trim_start();
        let (unit, unit_len) = take_unit(after_amount)?;
        if unit_len == 0 {
            return None;
        }
        measures.push(Measure { amount, unit });
        let body_start = index + lead;
        let after_amount_start =
            body_start + amount_len + (after_amount_full.len() - after_amount.len());
        index = after_amount_start + unit_len;

        let tail = text.get(index..)?;
        let trimmed = tail.trim_start();
        let mut next_index = index + (tail.len() - trimmed.len());
        let punct = trimmed.trim_start_matches(|ch: char| matches!(ch, '.' | ','));
        next_index += trimmed.len() - punct.len();
        if let Some(after_and) = strip_and(text.get(next_index..)?) {
            next_index = text.len() - after_and.len();
        }
        let next = text.get(next_index..)?.trim_start();
        if next.is_empty() || is_joiner(next) || read_amount(next).is_none() {
            break;
        }
        index = text.len() - next.len();
    }
    let first = measures.first()?;
    let dimension = first.unit.dimension;
    if measures
        .iter()
        .any(|measure| measure.unit.dimension != dimension)
    {
        return None;
    }
    let base: f64 = measures
        .iter()
        .map(|measure| to_base(measure.amount, measure.unit.scale))
        .sum();
    if !base.is_finite() {
        return None;
    }
    Some((
        TakenQuantity {
            base,
            dimension: dimension_id(dimension),
            symbol: first.unit.symbol,
        },
        index,
    ))
}

pub(crate) fn known_unit(name: &str) -> bool {
    find_unit(name).is_some()
}

pub(crate) fn show_in_unit(
    base: f64,
    from_symbol: &str,
    dimension: &str,
    target_name: &str,
) -> Result<(String, String), String> {
    let unit = find_unit(target_name).ok_or_else(|| format!("Unknown unit \"{target_name}\"."))?;
    if dimension_id(unit.dimension) != dimension {
        return Err(format!("Can't convert {from_symbol} to {}.", unit.symbol));
    }
    let shown = format_scaled(base, unit.scale)
        .ok_or_else(|| "That conversion is not a number.".to_string())?;
    Ok((shown, unit.symbol.to_string()))
}

fn is_joiner(text: &str) -> bool {
    matches!(
        text.chars().next(),
        Some('+' | '-' | '*' | '/' | '%' | '^' | '!' | ')' | '(' | '#' | '=')
    )
}

fn dimension_label(dimension: Dimension) -> &'static str {
    match dimension {
        Length => "Length",
        Mass => "Mass",
        Volume => "Volume",
        Area => "Area",
        Speed => "Speed",
        Time => "Time",
        Data => "Data",
        Temperature => "Temperature",
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
    use super::{convert, interpret};

    fn value(input: &str) -> String {
        convert(input)
            .map(|answer| answer.value)
            .unwrap_or_default()
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

    #[test]
    fn interpret_matches_the_launcher_converter() {
        for input in [
            "5 km to mi",
            "12 in to cm",
            "100 c to f",
            "1 GiB to MB",
            "8 b to B",
        ] {
            assert_eq!(interpret(input).value, value(input), "{input}");
            assert_eq!(interpret(input).kind, "conversion");
        }
    }

    #[test]
    fn fractions_and_mixed_lengths_convert() {
        assert_eq!(interpret("1/2 cup to ml").value, "118.29412");
        assert_eq!(interpret("1 1/2 cup to ml").value, "354.88235");
        assert_eq!(interpret("5 ft 6 in to cm").value, "167.64");
        assert_eq!(interpret("5'6\" to cm").value, "167.64");
        assert_eq!(interpret("5 ft and 6 in to cm").value, "167.64");
        let bare = interpret("5 km");
        assert_eq!(bare.kind, "conversion");
        assert_eq!(bare.value, "5");
        assert_eq!(bare.to, None);
        assert!(
            bare.rows
                .iter()
                .any(|row| row.symbol == "mi" && row.value == "3.106856")
        );
    }

    #[test]
    fn bad_units_explain_themselves_and_math_still_runs() {
        let mismatch = interpret("5 km to kg");
        assert_eq!(mismatch.kind, "message");
        assert_eq!(mismatch.value, "");
        assert_eq!(mismatch.title, "Can't convert km to kg.");
        assert_eq!(interpret("5 parsecs").title, "Unknown unit \"parsecs\".");
        assert_eq!(interpret("(2+3)*4").kind, "math");
        assert_eq!(interpret("(2+3)*4").value, "20");
        assert_eq!(interpret("255 to hex").value, "0xFF");
        assert_eq!(interpret("pi").kind, "math");
        assert!(!interpret("pi").value.is_empty());
    }
}
