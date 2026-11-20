//! A tape calculator. Each saved line can be reused as `#1` or `ans`.
//!
//! Plain arithmetic and unit quantities share one expression, so `1 + .5` and
//! `5mm + 10in` both work. The webview owns the tape and sends earlier results
//! back in with the next line.

use serde::{Deserialize, Serialize};

use super::calc::{
    apply_function, factorial, format_named_base, format_number, is_named_base, named_constant,
    named_function,
};
use super::units::{known_unit, show_in_unit, take_number, take_quantity};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalcRef {
    pub id: u32,
    pub base: f64,
    pub dimension: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalcResult {
    pub ok: bool,
    pub title: String,
    pub value: String,
    pub unit: Option<String>,
    pub dimension: Option<String>,
    pub base: Option<f64>,
    pub message: String,
}

#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    Quantity {
        base: f64,
        dimension: String,
        symbol: String,
        temperature: bool,
    },
}

enum Target {
    Unit(String),
    Base(String),
    None,
}

pub fn evaluate(input: &str, previous: &[CalcRef]) -> CalcResult {
    let original = input.trim().trim_start_matches('=').trim();
    if original.is_empty() {
        return fail("Type a calculation.");
    }
    let (expression, target) = split_target(original);
    if expression.is_empty() {
        return fail("Type a calculation.");
    }
    let mut parser = Parser {
        text: expression,
        index: 0,
        previous,
    };
    let value = match parser.expression() {
        Ok(value) => value,
        Err(message) => return fail(message),
    };
    parser.skip();
    if parser.index != parser.text.len() {
        let leftover = parser.rest().trim();
        return fail(format!("Couldn't read \"{leftover}\"."));
    }
    match render(original, value, target) {
        Ok(result) => result,
        Err(message) => fail(message),
    }
}

fn render(original: &str, value: Value, target: Target) -> Result<CalcResult, String> {
    match (value, target) {
        (Value::Number(number), Target::None) => {
            let shown = finite_text(number)?;
            Ok(success(original, &shown, None, None, number))
        }
        (Value::Number(number), Target::Base(name)) => {
            let shown = format_named_base(number, &name)
                .ok_or_else(|| "That isn't a whole number.".to_string())?;
            Ok(success(original, &shown, None, None, number))
        }
        (Value::Number(_), Target::Unit(name)) => Err(format!("Can't convert a number to {name}.")),
        (Value::Quantity { .. }, Target::Base(_)) => Err("Can't show a unit as a base.".into()),
        (
            Value::Quantity {
                base,
                dimension,
                symbol,
                ..
            },
            target,
        ) => {
            let target_name = match target {
                Target::Unit(name) => name,
                Target::None => symbol.clone(),
                Target::Base(_) => unreachable!(),
            };
            let (shown, unit) = show_in_unit(base, &symbol, &dimension, &target_name)?;
            Ok(success(original, &shown, Some(unit), Some(dimension), base))
        }
    }
}

fn success(
    original: &str,
    shown: &str,
    unit: Option<String>,
    dimension: Option<String>,
    base: f64,
) -> CalcResult {
    let display = match &unit {
        Some(unit) => format!("{shown} {unit}"),
        None => shown.to_string(),
    };
    CalcResult {
        ok: true,
        title: format!("{original} = {display}"),
        value: shown.to_string(),
        unit,
        dimension,
        base: Some(base),
        message: String::new(),
    }
}

fn fail(message: impl Into<String>) -> CalcResult {
    let message = message.into();
    CalcResult {
        ok: false,
        title: message.clone(),
        value: String::new(),
        unit: None,
        dimension: None,
        base: None,
        message,
    }
}

fn split_target(text: &str) -> (&str, Target) {
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(usize, Target)> = None;
    for separator in [" to ", " as ", " in "] {
        let Some(position) = lower.rfind(separator) else {
            continue;
        };
        let right = text[position + separator.len()..].trim();
        if right.is_empty() {
            continue;
        }
        let target = if is_named_base(right) {
            Target::Base(right.to_ascii_lowercase())
        } else if known_unit(right) {
            Target::Unit(right.to_string())
        } else {
            continue;
        };
        let replace = best.as_ref().is_none_or(|(at, _)| position > *at);
        if replace {
            best = Some((position, target));
        }
    }
    match best {
        Some((position, target)) => (text[..position].trim(), target),
        None => (text.trim(), Target::None),
    }
}

struct Parser<'a> {
    text: &'a str,
    index: usize,
    previous: &'a [CalcRef],
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.index..]
    }

    fn skip(&mut self) {
        let rest = self.rest();
        self.index += rest.len() - rest.trim_start().len();
    }

    fn consume_char(&mut self, expected: char) -> bool {
        self.skip();
        let mut chars = self.rest().chars();
        if chars.next() == Some(expected) {
            self.index += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, prefix: &str) -> bool {
        self.skip();
        if self.rest().starts_with(prefix) {
            self.index += prefix.len();
            true
        } else {
            false
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.skip();
        self.rest().chars().next()
    }

    fn expression(&mut self) -> Result<Value, String> {
        let mut value = self.term()?;
        loop {
            if self.consume_char('+') {
                value = add(value, self.term()?)?;
            } else if self.consume_char('-') {
                value = subtract(value, self.term()?)?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<Value, String> {
        let mut value = self.unary()?;
        loop {
            if self.consume_char('/') {
                value = divide(value, self.unary()?)?;
            } else if self.consume_char('%') {
                value = modulo(value, self.unary()?)?;
            } else if self.peek_multiply() {
                self.consume_char('*');
                value = multiply(value, self.unary()?)?;
            } else if self.implicit_mul() {
                value = multiply(value, self.unary()?)?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    /// A single `*` multiplies. `**` is power and stays for [`Self::power`].
    fn peek_multiply(&mut self) -> bool {
        self.skip();
        let rest = self.rest();
        rest.starts_with('*') && !rest.starts_with("**")
    }

    fn implicit_mul(&mut self) -> bool {
        match self.peek_char() {
            Some('(' | '#' | 'π') => true,
            Some(ch) => ch.is_ascii_alphabetic(),
            None => false,
        }
    }

    fn unary(&mut self) -> Result<Value, String> {
        if self.consume_char('-') {
            return negate(self.unary()?);
        }
        if self.consume_char('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Value, String> {
        let base = self.postfix()?;
        if self.consume_str("**") || self.consume_char('^') {
            let exponent = self.unary()?;
            return power(base, exponent);
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Value, String> {
        let mut value = self.primary()?;
        while self.consume_char('!') {
            value = bang(value)?;
        }
        Ok(value)
    }

    fn primary(&mut self) -> Result<Value, String> {
        self.skip();
        if self.consume_char('(') {
            let value = self.expression()?;
            if !self.consume_char(')') {
                return Err("Missing ).".into());
            }
            return Ok(value);
        }
        if self.consume_char('#') {
            return self.reference();
        }
        if let Some(name) = self.peek_ident() {
            let lower = name.to_ascii_lowercase();
            if lower == "ans" || named_constant(&lower).is_some() || named_function(&lower) {
                self.index += name.len();
                if lower == "ans" {
                    return self.answer();
                }
                if let Some(value) = named_constant(&lower) {
                    return Ok(Value::Number(value));
                }
                let argument = if self.consume_char('(') {
                    let value = self.expression()?;
                    if !self.consume_char(')') {
                        return Err("Missing ).".into());
                    }
                    value
                } else {
                    self.unary()?
                };
                let number = argument
                    .number()
                    .map_err(|_| format!("{name} needs a number."))?;
                let value = apply_function(&lower, number).ok_or("That is not a number.")?;
                return Ok(Value::Number(value));
            }
        }
        if let Some((quantity, len)) = take_quantity(self.rest()) {
            self.index += len;
            return Ok(Value::Quantity {
                temperature: quantity.dimension == "temperature",
                base: quantity.base,
                dimension: quantity.dimension.to_string(),
                symbol: quantity.symbol.to_string(),
            });
        }
        if let Some((number, len)) = take_number(self.rest()) {
            self.index += len;
            return Ok(Value::Number(number));
        }
        Err("Couldn't read that.".into())
    }

    fn peek_ident(&mut self) -> Option<String> {
        self.skip();
        let rest = self.rest();
        let len = ident_len(rest);
        (len > 0).then(|| rest[..len].to_string())
    }

    fn reference(&mut self) -> Result<Value, String> {
        self.skip();
        let rest = self.rest();
        let len = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if len == 0 {
            return Err("Missing a result number after #.".into());
        }
        let id: u32 = rest[..len]
            .parse()
            .map_err(|_| "That result number is too large.".to_string())?;
        self.index += len;
        self.previous
            .iter()
            .find(|item| item.id == id)
            .map(value_from_ref)
            .ok_or_else(|| format!("No result #{id}."))
    }

    fn answer(&mut self) -> Result<Value, String> {
        self.previous
            .last()
            .map(value_from_ref)
            .ok_or_else(|| "No previous result.".to_string())
    }
}

fn ident_len(text: &str) -> usize {
    let mut len = 0;
    for ch in text.chars() {
        if ch.is_ascii_digit() && len == 0 {
            break;
        }
        if ch.is_alphanumeric() || ch == 'π' {
            len += ch.len_utf8();
        } else {
            break;
        }
    }
    len
}

fn value_from_ref(item: &CalcRef) -> Value {
    match item.dimension.as_deref() {
        Some(dimension) if !dimension.is_empty() => Value::Quantity {
            base: item.base,
            dimension: dimension.to_string(),
            symbol: item
                .unit
                .clone()
                .filter(|unit| !unit.is_empty())
                .unwrap_or_else(|| dimension.to_string()),
            temperature: dimension == "temperature",
        },
        _ => Value::Number(item.base),
    }
}

fn add(left: Value, right: Value) -> Result<Value, String> {
    combine(left, right, "add", |a, b| a + b)
}

fn subtract(left: Value, right: Value) -> Result<Value, String> {
    combine(left, right, "subtract", |a, b| a - b)
}

fn combine(
    left: Value,
    right: Value,
    verb: &str,
    op: fn(f64, f64) -> f64,
) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number(op(left, right)),
        (left, right) => {
            reject_temperature(&left, &right)?;
            match (left, right) {
                (Value::Quantity { .. }, Value::Number(_))
                | (Value::Number(_), Value::Quantity { .. }) => {
                    Err(format!("Can't {verb} a unit and a number."))
                }
                (
                    Value::Quantity {
                        base: left_base,
                        dimension,
                        symbol: left_symbol,
                        temperature,
                    },
                    Value::Quantity {
                        base: right_base,
                        symbol: right_symbol,
                        dimension: right_dimension,
                        ..
                    },
                ) => {
                    if dimension != right_dimension {
                        return Err(format!("Can't {verb} {left_symbol} and {right_symbol}."));
                    }
                    let base = op(left_base, right_base);
                    if !base.is_finite() {
                        return Err("That is not a number.".into());
                    }
                    Ok(Value::Quantity {
                        base,
                        dimension,
                        symbol: left_symbol,
                        temperature,
                    })
                }
                _ => Err(format!("Can't {verb} those.")),
            }
        }
    }
}

fn multiply(left: Value, right: Value) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number(left * right),
        (
            Value::Quantity {
                temperature: true, ..
            },
            _,
        )
        | (
            _,
            Value::Quantity {
                temperature: true, ..
            },
        ) => Err("Temperature can't be used in math.".into()),
        (
            Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            },
            Value::Number(scale),
        )
        | (
            Value::Number(scale),
            Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            },
        ) => {
            let base = base * scale;
            if !base.is_finite() {
                return Err("That is not a number.".into());
            }
            Ok(Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            })
        }
        (Value::Quantity { symbol: left, .. }, Value::Quantity { symbol: right, .. }) => {
            Err(format!("Can't multiply {left} and {right}."))
        }
    }
}

fn divide(left: Value, right: Value) -> Result<Value, String> {
    match (right, left) {
        (Value::Number(right), Value::Number(left)) => {
            if right == 0.0 {
                return Err("Can't divide by zero.".into());
            }
            number(left / right)
        }
        (
            _,
            Value::Quantity {
                temperature: true, ..
            },
        )
        | (
            Value::Quantity {
                temperature: true, ..
            },
            _,
        ) => Err("Temperature can't be used in math.".into()),
        (
            Value::Number(scale),
            Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            },
        ) => {
            if scale == 0.0 {
                return Err("Can't divide by zero.".into());
            }
            let base = base / scale;
            if !base.is_finite() {
                return Err("That is not a number.".into());
            }
            Ok(Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            })
        }
        (
            Value::Quantity {
                base: right_base,
                dimension: right_dimension,
                symbol: right_symbol,
                ..
            },
            Value::Quantity {
                base: left_base,
                dimension,
                symbol: left_symbol,
                ..
            },
        ) => {
            if dimension != right_dimension {
                return Err(format!("Can't divide {left_symbol} by {right_symbol}."));
            }
            if right_base == 0.0 {
                return Err("Can't divide by zero.".into());
            }
            number(left_base / right_base)
        }
        (Value::Quantity { symbol, .. }, Value::Number(_)) => {
            Err(format!("Can't divide a number by {symbol}."))
        }
    }
}

fn modulo(left: Value, right: Value) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            if right == 0.0 {
                return Err("Can't divide by zero.".into());
            }
            number(left % right)
        }
        _ => Err("% needs numbers.".into()),
    }
}

fn power(base: Value, exponent: Value) -> Result<Value, String> {
    match (base, exponent) {
        (Value::Number(base), Value::Number(exponent)) => number(base.powf(exponent)),
        _ => Err("^ needs numbers.".into()),
    }
}

fn bang(value: Value) -> Result<Value, String> {
    match value {
        Value::Number(value) => number(
            factorial(value).ok_or_else(|| "That factorial is not a whole number.".to_string())?,
        ),
        Value::Quantity { .. } => Err("Factorial needs a number.".into()),
    }
}

fn negate(value: Value) -> Result<Value, String> {
    match value {
        Value::Number(value) => number(-value),
        Value::Quantity {
            base,
            dimension,
            symbol,
            temperature,
        } => {
            let base = -base;
            if !base.is_finite() {
                return Err("That is not a number.".into());
            }
            Ok(Value::Quantity {
                base,
                dimension,
                symbol,
                temperature,
            })
        }
    }
}

fn reject_temperature(left: &Value, right: &Value) -> Result<(), String> {
    let temperature = matches!(
        left,
        Value::Quantity {
            temperature: true,
            ..
        }
    ) || matches!(
        right,
        Value::Quantity {
            temperature: true,
            ..
        }
    );
    if temperature {
        Err("Temperature can't be used in math.".into())
    } else {
        Ok(())
    }
}

fn number(value: f64) -> Result<Value, String> {
    if !value.is_finite() {
        return Err("That is not a number.".into());
    }
    Ok(Value::Number(value))
}

fn finite_text(value: f64) -> Result<String, String> {
    if !value.is_finite() {
        return Err("That is not a number.".into());
    }
    Ok(format_number(value))
}

impl Value {
    fn number(self) -> Result<f64, ()> {
        match self {
            Value::Number(value) => Ok(value),
            Value::Quantity { .. } => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CalcRef, evaluate};

    fn line(input: &str) -> super::CalcResult {
        evaluate(input, &[])
    }

    #[test]
    fn plain_math_and_units_share_a_line() {
        let sum = line("1 + .5");
        assert!(sum.ok);
        assert_eq!(sum.value, "1.5");
        assert_eq!(sum.unit, None);

        let length = line("5mm + 10in");
        assert!(length.ok, "{}", length.message);
        assert_eq!(length.value, "259");
        assert_eq!(length.unit.as_deref(), Some("mm"));

        let spaced = line("5 mm + 10 in");
        assert_eq!(spaced.value, "259");

        let shown = line("5mm + 10in to cm");
        assert_eq!(shown.value, "25.9");
        assert_eq!(shown.unit.as_deref(), Some("cm"));

        assert_eq!(line("sqrt(16)").value, "4");
        assert_eq!(line("255 to hex").value, "0xFF");
        assert_eq!(line("(2+3)*4").value, "20");
        assert_eq!(line("1/2 cup + 2 tbsp").value, "0.625");
        assert_eq!(line("1/2 cup + 2 tbsp").unit.as_deref(), Some("cup"));
    }

    #[test]
    fn mismatched_units_and_temperature_are_rejected() {
        let mismatch = line("5mm + 10kg");
        assert!(!mismatch.ok);
        assert_eq!(mismatch.message, "Can't add mm and kg.");
        assert!(line("100 c + 5 c").message.contains("Temperature"));
    }

    #[test]
    fn earlier_results_can_be_reused() {
        let previous = [CalcRef {
            id: 1,
            base: 0.005,
            dimension: Some("length".into()),
            unit: Some("mm".into()),
        }];
        let next = evaluate("#1 + 1in", &previous);
        assert!(next.ok, "{}", next.message);
        assert_eq!(next.value, "30.4");
        assert_eq!(next.unit.as_deref(), Some("mm"));

        let scaled = evaluate("ans * 2", &previous);
        assert_eq!(scaled.value, "10");
        assert_eq!(scaled.unit.as_deref(), Some("mm"));

        assert_eq!(evaluate("#3", &previous).message, "No result #3.");
        assert_eq!(line("ans").message, "No previous result.");
    }
}
