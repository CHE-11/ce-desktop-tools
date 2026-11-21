//! Arithmetic typed into the launcher.
//!
//! A recursive-descent parser over `+ - * / % ^ !`, parentheses, a few
//! functions, and the constants `pi` and `e`. `<expr> to hex|bin|oct|dec`
//! prints an integer in another base.

#[derive(Clone, Debug, PartialEq)]
pub struct CalcAnswer {
    pub value: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Base {
    Hex,
    Bin,
    Oct,
    Dec,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Op(char),
    Open,
    Close,
}

pub fn evaluate(input: &str) -> Option<CalcAnswer> {
    let text = input.trim().trim_start_matches('=').trim();
    if text.is_empty() {
        return None;
    }
    let (expression, base) = split_base(text);
    let tokens = tokenize(expression)?;
    if tokens.is_empty() || is_trivial(&tokens, expression) && base.is_none() {
        return None;
    }
    let mut parser = Parser { tokens, index: 0 };
    let value = parser.expression()?;
    if parser.index != parser.tokens.len() || !value.is_finite() {
        return None;
    }
    match base {
        Some(base) => {
            let shown = in_base(value, base)?;
            Some(CalcAnswer {
                detail: format!("{} in {}", format_number(value), base_name(base)),
                value: shown,
            })
        }
        None => Some(CalcAnswer {
            value: format_number(value),
            detail: expression.to_string(),
        }),
    }
}

/// A lone decimal number or word is not a calculation. Typing `e` should find
/// Edge, not Euler's number.
fn is_trivial(tokens: &[Token], text: &str) -> bool {
    if tokens.len() != 1 {
        return false;
    }
    let lower = text.trim().to_ascii_lowercase();
    match tokens[0] {
        Token::Number(_) => !["0x", "0b", "0o"]
            .iter()
            .any(|prefix| lower.starts_with(prefix)),
        _ => true,
    }
}

fn split_base(text: &str) -> (&str, Option<Base>) {
    let lower = text.to_ascii_lowercase();
    for separator in [" to ", " in ", " as "] {
        if let Some(position) = lower.rfind(separator) {
            let target = lower[position + separator.len()..].trim();
            let base = match target {
                "hex" | "hexadecimal" => Some(Base::Hex),
                "bin" | "binary" => Some(Base::Bin),
                "oct" | "octal" => Some(Base::Oct),
                "dec" | "decimal" => Some(Base::Dec),
                _ => None,
            };
            if base.is_some() {
                return (text[..position].trim(), base);
            }
        }
    }
    (text, None)
}

fn base_name(base: Base) -> &'static str {
    match base {
        Base::Hex => "hex",
        Base::Bin => "binary",
        Base::Oct => "octal",
        Base::Dec => "decimal",
    }
}

fn in_base(value: f64, base: Base) -> Option<String> {
    if base == Base::Dec {
        return Some(format_number(value));
    }
    if value.fract() != 0.0 || value.abs() > 9.0e15 {
        return None;
    }
    let magnitude = value.abs() as u64;
    let sign = if value < 0.0 { "-" } else { "" };
    Some(match base {
        Base::Hex => format!("{sign}0x{magnitude:X}"),
        Base::Bin => format!("{sign}0b{magnitude:b}"),
        Base::Oct => format!("{sign}0o{magnitude:o}"),
        Base::Dec => unreachable!(),
    })
}

pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 * value.abs().max(1.0) && rounded.abs() < 1e15 {
        return format!("{}", rounded as i64);
    }
    if value.abs() >= 1e15 || value.abs() < 1e-6 {
        return format!("{value:e}");
    }
    let fixed = format!("{value:.10}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn tokenize(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() || ch == ',' && is_digit_group(&chars, index) {
            index += 1;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let (value, next) = read_number(&chars, index)?;
            tokens.push(Token::Number(value));
            index = next;
            continue;
        }
        if ch.is_alphabetic() || ch == 'π' {
            let start = index;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == 'π') {
                index += 1;
            }
            tokens.push(Token::Ident(
                chars[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            ));
            continue;
        }
        match ch {
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            '*' if chars.get(index + 1) == Some(&'*') => {
                tokens.push(Token::Op('^'));
                index += 1;
            }
            '+' | '-' | '*' | '/' | '%' | '^' | '!' => tokens.push(Token::Op(ch)),
            '×' => tokens.push(Token::Op('*')),
            '÷' => tokens.push(Token::Op('/')),
            '−' => tokens.push(Token::Op('-')),
            _ => return None,
        }
        index += 1;
    }
    Some(tokens)
}

/// `1,000` reads as one thousand. A comma anywhere else is not arithmetic.
fn is_digit_group(chars: &[char], index: usize) -> bool {
    let before = index > 0 && chars[index - 1].is_ascii_digit();
    let after = chars.len() >= index + 4
        && chars[index + 1..index + 4]
            .iter()
            .all(|ch| ch.is_ascii_digit())
        && chars.get(index + 4).is_none_or(|ch| !ch.is_ascii_digit());
    before && after
}

fn read_number(chars: &[char], start: usize) -> Option<(f64, usize)> {
    if chars[start] == '0' {
        let radix = match chars.get(start + 1).map(|ch| ch.to_ascii_lowercase()) {
            Some('x') => Some(16),
            Some('b') => Some(2),
            Some('o') => Some(8),
            _ => None,
        };
        if let Some(radix) = radix {
            let mut end = start + 2;
            while end < chars.len() && chars[end].is_digit(radix) {
                end += 1;
            }
            if end == start + 2 {
                return None;
            }
            let digits: String = chars[start + 2..end].iter().collect();
            let value = u64::from_str_radix(&digits, radix).ok()?;
            return Some((value as f64, end));
        }
    }
    let mut end = start;
    let mut seen_dot = false;
    while end < chars.len() {
        let ch = chars[end];
        if ch.is_ascii_digit() {
            end += 1;
        } else if ch == '.' && !seen_dot {
            seen_dot = true;
            end += 1;
        } else if ch == ',' && is_digit_group(chars, end) {
            end += 1;
        } else {
            break;
        }
    }
    if end < chars.len() && (chars[end] == 'e' || chars[end] == 'E') {
        let mut exponent = end + 1;
        if exponent < chars.len() && (chars[exponent] == '+' || chars[exponent] == '-') {
            exponent += 1;
        }
        let digits_start = exponent;
        while exponent < chars.len() && chars[exponent].is_ascii_digit() {
            exponent += 1;
        }
        let followed_by_letter = chars.get(exponent).is_some_and(|ch| ch.is_alphabetic());
        if exponent > digits_start && !followed_by_letter {
            end = exponent;
        }
    }
    let text: String = chars[start..end].iter().filter(|ch| **ch != ',').collect();
    let value = text.parse::<f64>().ok()?;
    Some((value, end))
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        self.index += 1;
        token
    }

    fn expression(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.index += 1;
            let right = self.term()?;
            value = if op == '+' {
                value + right
            } else {
                value - right
            };
        }
        Some(value)
    }

    fn term(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        loop {
            match self.peek().cloned() {
                Some(Token::Op(op @ ('*' | '/' | '%'))) => {
                    self.index += 1;
                    let right = self.unary()?;
                    value = match op {
                        '*' => value * right,
                        '/' => {
                            if right == 0.0 {
                                return None;
                            }
                            value / right
                        }
                        _ => {
                            if right == 0.0 {
                                return None;
                            }
                            value % right
                        }
                    };
                }
                // `2(3)` and `2pi` multiply.
                Some(Token::Open) | Some(Token::Ident(_)) => {
                    let right = self.unary()?;
                    value *= right;
                }
                _ => return Some(value),
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Token::Op('-')) => {
                self.index += 1;
                Some(-self.unary()?)
            }
            Some(Token::Op('+')) => {
                self.index += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.postfix()?;
        if let Some(Token::Op('^')) = self.peek() {
            self.index += 1;
            let exponent = self.unary()?;
            return Some(base.powf(exponent));
        }
        Some(base)
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut value = self.primary()?;
        while let Some(Token::Op('!')) = self.peek() {
            self.index += 1;
            value = factorial(value)?;
        }
        Some(value)
    }

    fn primary(&mut self) -> Option<f64> {
        match self.next()? {
            Token::Number(value) => Some(value),
            Token::Open => {
                let value = self.expression()?;
                match self.next()? {
                    Token::Close => Some(value),
                    _ => None,
                }
            }
            Token::Ident(name) => {
                if let Some(value) = constant(&name) {
                    return Some(value);
                }
                let function = function(&name)?;
                let argument = match self.peek() {
                    Some(Token::Open) => self.primary()?,
                    _ => self.unary()?,
                };
                let value = function(argument);
                value.is_finite().then_some(value)
            }
            _ => None,
        }
    }
}

fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" | "π" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" => Some(std::f64::consts::TAU),
        _ => None,
    }
}

fn function(name: &str) -> Option<fn(f64) -> f64> {
    Some(match name {
        "sqrt" => f64::sqrt,
        "cbrt" => f64::cbrt,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "asin" => f64::asin,
        "acos" => f64::acos,
        "atan" => f64::atan,
        "log" => f64::log10,
        "ln" => f64::ln,
        "exp" => f64::exp,
        "abs" => f64::abs,
        "round" => f64::round,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        _ => return None,
    })
}

pub(crate) fn is_named_base(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "hex" | "hexadecimal" | "bin" | "binary" | "oct" | "octal" | "dec" | "decimal"
    )
}

pub(crate) fn format_named_base(value: f64, name: &str) -> Option<String> {
    let base = match name.trim().to_ascii_lowercase().as_str() {
        "hex" | "hexadecimal" => Base::Hex,
        "bin" | "binary" => Base::Bin,
        "oct" | "octal" => Base::Oct,
        "dec" | "decimal" => Base::Dec,
        _ => return None,
    };
    in_base(value, base)
}

pub(crate) fn named_constant(name: &str) -> Option<f64> {
    constant(&name.to_ascii_lowercase())
}

pub(crate) fn named_function(name: &str) -> bool {
    function(&name.to_ascii_lowercase()).is_some()
}

pub(crate) fn apply_function(name: &str, value: f64) -> Option<f64> {
    let function = function(&name.to_ascii_lowercase())?;
    let out = function(value);
    out.is_finite().then_some(out)
}

pub(crate) fn factorial(value: f64) -> Option<f64> {
    if value < 0.0 || value.fract() != 0.0 || value > 170.0 {
        return None;
    }
    Some((1..=value as u64).fold(1.0, |product, n| product * n as f64))
}

#[cfg(test)]
mod tests {
    use super::{evaluate, format_number};

    fn value(input: &str) -> String {
        evaluate(input)
            .map(|answer| answer.value)
            .unwrap_or_default()
    }

    #[test]
    fn operators_follow_precedence() {
        assert_eq!(value("1 + 2 * 3"), "7");
        assert_eq!(value("(1 + 2) * 3"), "9");
        assert_eq!(value("10 / 4"), "2.5");
        assert_eq!(value("10 % 4"), "2");
        assert_eq!(value("2 - 3 - 4"), "-5");
        assert_eq!(value("1,000 * 2"), "2000");
    }

    #[test]
    fn power_is_right_associative_and_binds_tighter_than_minus() {
        assert_eq!(value("2^3^2"), "512");
        assert_eq!(value("-2^2"), "-4");
        assert_eq!(value("2**10"), "1024");
        assert_eq!(value("5!"), "120");
    }

    #[test]
    fn functions_and_constants_work() {
        assert_eq!(value("sqrt(16)"), "4");
        assert_eq!(value("sqrt 16 + 1"), "5");
        assert_eq!(value("log(1000)"), "3");
        assert_eq!(value("abs(-3)"), "3");
        assert_eq!(value("2pi"), format_number(2.0 * std::f64::consts::PI));
        assert_eq!(value("round(2.6)"), "3");
    }

    #[test]
    fn bases_convert_both_ways() {
        assert_eq!(value("0xff"), "255");
        assert_eq!(value("255 to hex"), "0xFF");
        assert_eq!(value("0b1010 to dec"), "10");
        assert_eq!(value("8 in binary"), "0b1000");
        assert_eq!(value("42 to hex"), "0x2A");
    }

    #[test]
    fn non_arithmetic_is_ignored() {
        assert!(evaluate("").is_none());
        assert!(evaluate("42").is_none());
        assert!(evaluate("chrome").is_none());
        assert!(evaluate("1 / 0").is_none());
        assert!(evaluate("2 +").is_none());
        assert!(evaluate("(1 + 2").is_none());
        assert!(evaluate("1.5 to hex").is_none());
        assert!(evaluate("notepad++").is_none());
    }
}
