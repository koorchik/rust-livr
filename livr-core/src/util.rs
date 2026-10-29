//! Helpers for rule builders: the arguments of a rule ([`arg`],
//! [`number_arg`], [`string_arg`]) and empty values ([`is_no_value`]).
//!
//! The other items are used by the built-in rules and the code of `schema!`;
//! they are hidden from the documentation and not part of the public API.

use std::borrow::Cow;

use serde_json::{Number, Value};

use crate::SchemaError;

/// Whether the value is empty for LIVR: missing (`null`) or an empty string.
/// All rules except `required`, `not_empty` and `not_empty_list` skip empty
/// values.
#[inline]
#[must_use]
pub fn is_no_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(str) => str.is_empty(),
        _ => false,
    }
}

/// Whether the value is primitive: a string, a number or a boolean.
#[inline]
#[must_use]
#[doc(hidden)]
pub fn is_primitive(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

/// A primitive value as a string, like JavaScript's `String()`, so values
/// are compared the same way in all implementations: `10.0` becomes `"10"`.
/// Lists and objects become their JSON.
#[must_use]
#[doc(hidden)]
pub fn to_js_string(value: &Value) -> Cow<'_, str> {
    match value {
        Value::String(str) => Cow::Borrowed(str),
        Value::Number(number) => Cow::Owned(number_to_js_string(number)),
        Value::Bool(true) => Cow::Borrowed("true"),
        Value::Bool(false) => Cow::Borrowed("false"),
        Value::Null => Cow::Borrowed("null"),
        Value::Array(_) | Value::Object(_) => Cow::Owned(value.to_string()),
    }
}

/// A number as a string, like JavaScript's `String()`: `10.0` becomes `"10"`,
/// `1e21` stays `"1e+21"`.
#[must_use]
#[doc(hidden)]
pub fn number_to_js_string(number: &Number) -> String {
    match number.as_f64() {
        Some(float) if number.is_f64() => ryu_js::Buffer::new().format(float).to_owned(),
        _ => number.to_string(),
    }
}

/// A number parsed from a string: an integer when it fits `i64`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[doc(hidden)]
pub enum Num {
    /// An integer that fits `i64`.
    Int(i64),
    /// Any other number.
    Float(f64),
}

impl Num {
    /// The number of a JSON number.
    #[inline]
    #[must_use]
    pub fn of(number: &Number) -> Self {
        match number.as_i64() {
            Some(int) => Self::Int(int),
            // u64 beyond i64 and floats
            None => Self::Float(number.as_f64().unwrap_or(f64::NAN)),
        }
    }

    /// The number as `f64`.
    #[inline]
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(int) => int as f64,
            Self::Float(float) => float,
        }
    }

    /// The number as a JSON value.
    #[must_use]
    pub fn into_value(self) -> Value {
        match self {
            Self::Int(int) => Value::from(int),
            // Parsed numbers are finite
            Self::Float(float) => Number::from_f64(float).map_or(Value::Null, Value::Number),
        }
    }
}

/// Parses a decimal number: `-?[0-9]+(\.[0-9]+)?`. No whitespace, `+`,
/// exponent, hexadecimal, `Infinity` or `NaN`. Returns `None` for other
/// strings and for numbers too large for `f64`.
#[must_use]
#[doc(hidden)]
pub fn parse_number(str: &str) -> Option<Num> {
    let bytes = str.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    let is_negative = i == 1;

    // Digits are accumulated while they fit: the fast path for the common
    // case of an integer
    let integer_start = i;
    let mut integer: u64 = 0;
    let mut is_overflow = false;
    while let Some(&byte) = bytes.get(i) {
        if !byte.is_ascii_digit() {
            break;
        }
        match integer
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(byte - b'0')))
        {
            Some(n) => integer = n,
            None => is_overflow = true,
        }
        i += 1;
    }
    if i == integer_start {
        return None;
    }

    if i == bytes.len() {
        if !is_overflow {
            if !is_negative && i64::try_from(integer).is_ok() {
                return i64::try_from(integer).ok().map(Num::Int);
            }
            if is_negative && integer <= i64::MIN.unsigned_abs() {
                return Some(Num::Int(0i64.wrapping_sub_unsigned(integer)));
            }
        }
        return parse_float(str);
    }

    if bytes[i] != b'.' {
        return None;
    }
    i += 1;
    let fraction_start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == fraction_start || i != bytes.len() {
        return None;
    }
    parse_float(str)
}

fn parse_float(str: &str) -> Option<Num> {
    str.parse::<f64>()
        .ok()
        .filter(|float| float.is_finite())
        .map(Num::Float)
}

/// The number of characters (Unicode code points) of a string.
#[inline]
#[must_use]
#[doc(hidden)]
pub fn char_len(str: &str) -> usize {
    if str.is_ascii() {
        str.len()
    } else {
        str.chars().count()
    }
}

/// Whether the character is whitespace for JavaScript's `trim()`.
#[inline]
#[must_use]
#[doc(hidden)]
pub fn is_js_whitespace(char: char) -> bool {
    // White_Space without U+0085 (NEXT LINE), plus U+FEFF (BOM)
    match char {
        '\t'..='\r' | ' ' => true,
        _ if char.is_ascii() => false,
        '\u{85}' => false,
        '\u{feff}' => true,
        _ => char.is_whitespace(),
    }
}

/// The string without leading and trailing whitespace, like JavaScript's
/// `trim()`.
#[inline]
#[must_use]
#[doc(hidden)]
pub fn js_trim(str: &str) -> &str {
    str.trim_matches(is_js_whitespace)
}

/// The camelCase form of a snake_case rule name: `min_length` becomes
/// `minLength`.
#[must_use]
#[doc(hidden)]
pub fn camelize(name: &str) -> String {
    let mut camelized = String::with_capacity(name.len());
    let mut is_after_underscore = false;
    for char in name.chars() {
        if char == '_' {
            is_after_underscore = true;
        } else if is_after_underscore {
            camelized.extend(char.to_uppercase());
            is_after_underscore = false;
        } else {
            camelized.push(char);
        }
    }
    camelized
}

/// The argument of a rule at `index`, for rule builders.
///
/// # Errors
///
/// A [`SchemaError`] when the rule has fewer arguments.
pub fn arg<'a>(args: &'a [Value], index: usize, rule: &str) -> Result<&'a Value, SchemaError> {
    args.get(index).ok_or_else(|| {
        SchemaError::new(format!(
            "rule [{rule}] expects at least {} argument(s), got {}",
            index + 1,
            args.len()
        ))
    })
}

/// A number argument of a rule: a number or a string with a number.
///
/// # Errors
///
/// A [`SchemaError`] when the argument is missing or not a number.
pub fn number_arg(args: &[Value], index: usize, rule: &str) -> Result<f64, SchemaError> {
    let value = arg(args, index, rule)?;
    let number = match value {
        Value::Number(number) => Some(Num::of(number)),
        Value::String(str) => parse_number(str),
        _ => None,
    };
    number
        .map(Num::as_f64)
        .ok_or_else(|| SchemaError::new(format!("rule [{rule}] expects a number, got {value}")))
}

/// A string argument of a rule. Numbers and booleans are converted with
/// [`to_js_string`].
///
/// # Errors
///
/// A [`SchemaError`] when the argument is missing, a list or an object.
pub fn string_arg(args: &[Value], index: usize, rule: &str) -> Result<String, SchemaError> {
    let value = arg(args, index, rule)?;
    if is_primitive(value) {
        Ok(to_js_string(value).into_owned())
    } else {
        Err(SchemaError::new(format!(
            "rule [{rule}] expects a string, got {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_decimal_numbers_only() {
        assert_eq!(parse_number("10"), Some(Num::Int(10)));
        assert_eq!(parse_number("-10"), Some(Num::Int(-10)));
        assert_eq!(parse_number("010"), Some(Num::Int(10)));
        assert_eq!(parse_number("1.5"), Some(Num::Float(1.5)));
        assert_eq!(parse_number("-0.25"), Some(Num::Float(-0.25)));
        assert_eq!(
            parse_number("9223372036854775807"),
            Some(Num::Int(i64::MAX))
        );
        assert_eq!(
            parse_number("-9223372036854775808"),
            Some(Num::Int(i64::MIN))
        );
        assert_eq!(
            parse_number("9223372036854775808"),
            Some(Num::Float(9.223_372_036_854_776e18))
        );
        for str in [
            "", "-", "+1", " 1", "1 ", "1.", ".5", "1e3", "0x10", "Infinity", "NaN", "1,5", "1.2.3",
        ] {
            assert_eq!(parse_number(str), None, "{str:?}");
        }
    }

    #[test]
    fn formats_numbers_like_javascript() {
        assert_eq!(to_js_string(&json!(10)), "10");
        assert_eq!(to_js_string(&json!(10.0)), "10");
        assert_eq!(to_js_string(&json!(-0.0)), "0");
        assert_eq!(to_js_string(&json!(1.5)), "1.5");
        assert_eq!(to_js_string(&json!(1e21)), "1e+21");
        assert_eq!(to_js_string(&json!(true)), "true");
    }

    #[test]
    fn trims_like_javascript() {
        assert_eq!(js_trim("\u{feff}\u{a0} a b\u{3000}\n"), "a b");
        assert_eq!(js_trim("\u{85}a"), "\u{85}a");
    }

    #[test]
    fn camelizes_rule_names() {
        assert_eq!(
            camelize("list_of_different_objects"),
            "listOfDifferentObjects"
        );
        assert_eq!(camelize("required"), "required");
    }
}
