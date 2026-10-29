use crate::rules::prelude::*;

/// `length_equal`: the value must have exactly this many characters.
#[derive(Clone, Copy, Debug)]
pub struct LengthEqual {
    length: f64,
}

impl LengthEqual {
    /// The rule for a length.
    #[must_use]
    pub const fn new(length: f64) -> Self {
        Self { length }
    }

    /// Builds the rule from its arguments: `{"length_equal": 5}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the length is missing or not a number.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(number_arg(args, 0, "length_equal")?))
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
        if length < self.length {
            return Outcome::code("TOO_SHORT");
        }
        if length > self.length {
            return Outcome::code("TOO_LONG");
        }
        string_output(value, str)
    }
}
