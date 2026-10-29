use crate::rules::prelude::*;

/// `decimal`: the value must be a number. The output is a number.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decimal;

impl Decimal {
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
            Value::Number(_) | Value::Null => Outcome::Valid,
            Value::String(str) if str.is_empty() => Outcome::Valid,
            Value::String(str) => match parse_number(str) {
                Some(number) => Outcome::Value(Num::Float(number.as_f64()).into_value()),
                None => Outcome::code("NOT_DECIMAL"),
            },
            Value::Bool(_) => Outcome::code("NOT_DECIMAL"),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }
}
