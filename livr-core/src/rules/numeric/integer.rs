use crate::rules::prelude::*;

// 2^53: larger floats cannot be checked for being integers exactly
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;

/// `integer`: the value must be an integer. The output is an integer number.
#[derive(Clone, Copy, Debug, Default)]
pub struct Integer;

impl Integer {
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
            // Integers are i64: larger ones (u64) are not integers
            Value::Number(number) if number.is_i64() => return Outcome::Valid,
            Value::Number(number) if number.is_u64() => return Outcome::code("NOT_INTEGER"),
            Value::Number(number) => Num::of(number),
            Value::String(str) if str.is_empty() => return Outcome::Valid,
            Value::String(str) => match parse_number(str) {
                Some(number) => number,
                None => return Outcome::code("NOT_INTEGER"),
            },
            Value::Null => return Outcome::Valid,
            Value::Bool(_) => return Outcome::code("NOT_INTEGER"),
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        match number {
            Num::Int(int) => Outcome::Value(Value::from(int)),
            Num::Float(float) if float.fract() == 0.0 && float.abs() <= MAX_SAFE_INTEGER => {
                Outcome::Value(Value::from(float as i64))
            }
            Num::Float(_) => Outcome::code("NOT_INTEGER"),
        }
    }
}
