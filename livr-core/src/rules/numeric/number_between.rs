use crate::rules::prelude::*;

/// `number_between`: the value must be a number from `min` to `max`. The output is a number.
#[derive(Clone, Copy, Debug)]
pub struct NumberBetween {
    min: f64,
    max: f64,
}

impl NumberBetween {
    /// The rule for the bounds.
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Builds the rule from its arguments: `{"number_between": [1, 10]}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when a bound is missing or not a number.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(
            number_arg(args, 0, "number_between")?,
            number_arg(args, 1, "number_between")?,
        ))
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let (number, is_parsed) = match value {
            Value::Number(number) => (Num::of(number), false),
            Value::Null => return Outcome::Valid,
            Value::String(str) if str.is_empty() => return Outcome::Valid,
            Value::String(str) => match parse_number(str) {
                Some(number) => (number, true),
                None => return Outcome::code("NOT_NUMBER"),
            },
            Value::Bool(_) => return Outcome::code("NOT_NUMBER"),
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        let float = number.as_f64();
        if float < self.min {
            return Outcome::code("TOO_LOW");
        }
        if float > self.max {
            return Outcome::code("TOO_HIGH");
        }
        if is_parsed {
            Outcome::Value(number.into_value())
        } else {
            Outcome::Valid
        }
    }
}
