use crate::rules::prelude::*;

/// `length_between`: the value must have from `min` to `max` characters.
#[derive(Clone, Copy, Debug)]
pub struct LengthBetween {
    min: f64,
    max: f64,
}

impl LengthBetween {
    /// The rule for the lengths.
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Builds the rule from its arguments: `{"length_between": [1, 10]}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when a length is missing or not a number.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(
            number_arg(args, 0, "length_between")?,
            number_arg(args, 1, "length_between")?,
        ))
    }

    /// Checks the value.
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) {
            return Outcome::Valid;
        }
        if !is_primitive(value) {
            return Outcome::code("FORMAT_ERROR");
        }
        let str = to_js_string(value);
        let length = char_len(&str) as f64;
        if length < self.min {
            return Outcome::code("TOO_SHORT");
        }
        if length > self.max {
            return Outcome::code("TOO_LONG");
        }
        string_output(value, str)
    }
}
