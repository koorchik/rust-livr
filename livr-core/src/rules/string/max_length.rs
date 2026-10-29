use crate::rules::prelude::*;

/// `max_length`: the value must have at most this many characters.
#[derive(Clone, Copy, Debug)]
pub struct MaxLength {
    max: f64,
}

impl MaxLength {
    /// The rule for a length.
    #[must_use]
    pub const fn new(max: f64) -> Self {
        Self { max }
    }

    /// Builds the rule from its arguments: `{"max_length": 10}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the length is missing or not a number.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(number_arg(args, 0, "max_length")?))
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
        // A string has at most as many characters as bytes: count the
        // characters only when the bytes are over the limit
        if str.len() as f64 > self.max && char_len(&str) as f64 > self.max {
            return Outcome::code("TOO_LONG");
        }
        string_output(value, str)
    }
}
