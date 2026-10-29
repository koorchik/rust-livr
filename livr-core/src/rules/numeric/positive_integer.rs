use crate::rules::prelude::*;

// 2^53: larger floats cannot be checked for being integers exactly
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;

/// `positive_integer`: the value must be an integer, 1 or more. The output
/// is an integer number.
#[derive(Clone, Copy, Debug, Default)]
pub struct PositiveInteger;

impl PositiveInteger {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// None, the signature is the one of all rule builders.
    pub fn build(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let number = match value {
            Value::Number(number) => match number.as_i64() {
                Some(int) if int >= 1 => return Outcome::Valid,
                Some(_) => return Outcome::code("NOT_POSITIVE_INTEGER"),
                // Integers are i64: larger ones (u64) are not integers
                None if number.is_u64() => return Outcome::code("NOT_POSITIVE_INTEGER"),
                None => Num::of(number),
            },
            Value::String(str) if str.is_empty() => return Outcome::Valid,
            Value::String(str) => match parse_number(str) {
                Some(number) => number,
                None => return Outcome::code("NOT_POSITIVE_INTEGER"),
            },
            Value::Null => return Outcome::Valid,
            Value::Bool(_) => return Outcome::code("NOT_POSITIVE_INTEGER"),
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        match number {
            Num::Int(int) if int >= 1 => Outcome::Value(Value::from(int)),
            Num::Float(float)
                if float >= 1.0 && float.fract() == 0.0 && float <= MAX_SAFE_INTEGER =>
            {
                Outcome::Value(Value::from(float as i64))
            }
            _ => Outcome::code("NOT_POSITIVE_INTEGER"),
        }
    }
}
