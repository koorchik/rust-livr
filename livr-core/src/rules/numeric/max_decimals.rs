use crate::rules::prelude::*;

/// `max_decimals` (LIVR 2.5): the value must be a number with at most this
/// many digits after the decimal point, trailing zeros not counted:
/// `{"max_decimals": 2}`. The output is a number.
#[derive(Clone, Copy, Debug)]
pub struct MaxDecimals {
    max: u64,
}

impl MaxDecimals {
    /// The rule for a number of decimals.
    #[must_use]
    pub const fn new(max: u64) -> Self {
        Self { max }
    }

    /// Builds the rule from its argument, a non-negative integer (or a
    /// string with one); further arguments are ignored.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without an argument or for another one.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(count_arg(
            first_arg(args, "max_decimals")?,
            "max_decimals",
        )?))
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::Null => Outcome::Valid,
            Value::String(str) if str.is_empty() => Outcome::Valid,
            Value::String(str) => match string_decimals(str.as_bytes()) {
                None => Outcome::code("NOT_NUMBER"),
                Some(decimals) if decimals > self.max => Outcome::code("TOO_MANY_DECIMALS"),
                Some(_) => match parse_number(str) {
                    Some(number) => Outcome::Value(number.into_value()),
                    // Hundreds of digits: not a finite double
                    None => Outcome::code("NOT_NUMBER"),
                },
            },
            Value::Number(number) => {
                let is_integer =
                    !number.is_f64() || number.as_f64().is_some_and(|f| f.fract() == 0.0);
                let decimals = if is_integer {
                    0
                } else {
                    number_decimals(&to_js_string(value))
                };
                if decimals > self.max {
                    Outcome::code("TOO_MANY_DECIMALS")
                } else {
                    Outcome::Valid
                }
            }
            Value::Bool(_) => Outcome::code("NOT_NUMBER"),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }
}

// The digits after the decimal point of `-?[0-9]+(\.[0-9]+)?` without the
// trailing zeros, as written: "1.230" has 2, "0.10000000000000000001" has 20
fn string_decimals(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.strip_prefix(b"-").unwrap_or(bytes);
    let integer_end = digits
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(digits.len());
    if integer_end == 0 {
        return None;
    }
    let Some(fraction) = digits[integer_end..].strip_prefix(b".") else {
        return (integer_end == digits.len()).then_some(0);
    };
    if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let significant = fraction
        .iter()
        .rposition(|&byte| byte != b'0')
        .map_or(0, |i| i + 1);
    Some(significant as u64)
}

// The decimals of the string form of a number that is not an integer:
// "0.125", "-12.25", or the exponent form of small numbers, "1e-7", "1.5e-7"
fn number_decimals(str: &str) -> u64 {
    let (mantissa, exponent) = str.split_once('e').unwrap_or((str, "0"));
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let exponent: i64 = exponent.parse().unwrap_or(0);
    u64::try_from(i64::try_from(fraction).unwrap_or(0) - exponent).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_decimals() {
        assert_eq!(string_decimals(b"1.230"), Some(2));
        assert_eq!(string_decimals(b"5.000"), Some(0));
        assert_eq!(string_decimals(b"-0.10000000000000000001"), Some(20));
        assert_eq!(string_decimals(b"12"), Some(0));
        for str in ["1.", ".5", "-", "1e5", "1.5.5", "+1", " 1", "1,5", "--1"] {
            assert_eq!(string_decimals(str.as_bytes()), None, "{str}");
        }
        assert_eq!(number_decimals("0.125"), 3);
        assert_eq!(number_decimals("-12.25"), 2);
        assert_eq!(number_decimals("1e-7"), 7);
        assert_eq!(number_decimals("1.5e-7"), 8);
    }
}
