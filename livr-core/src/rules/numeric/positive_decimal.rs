use crate::rules::prelude::*;

/// `positive_decimal`: the value must be a number above 0. The output is a
/// number.
#[derive(Clone, Copy, Debug, Default)]
pub struct PositiveDecimal;

impl PositiveDecimal {
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
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::Number(number) => {
                if Num::of(number).as_f64() > 0.0 {
                    Outcome::Valid
                } else {
                    Outcome::code("NOT_POSITIVE_DECIMAL")
                }
            }
            Value::Null => Outcome::Valid,
            Value::String(str) if str.is_empty() => Outcome::Valid,
            Value::String(str) => match parse_number(str).map(Num::as_f64) {
                Some(float) if float > 0.0 => Outcome::Value(Num::Float(float).into_value()),
                _ => Outcome::code("NOT_POSITIVE_DECIMAL"),
            },
            Value::Bool(_) => Outcome::code("NOT_POSITIVE_DECIMAL"),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }
}
