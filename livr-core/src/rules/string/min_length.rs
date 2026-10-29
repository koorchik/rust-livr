use crate::rules::prelude::*;

/// `min_length`: the value must have at least this many characters.
#[derive(Clone, Copy, Debug)]
pub struct MinLength {
    min: f64,
}

impl MinLength {
    /// The rule for a length.
    #[must_use]
    pub const fn new(min: f64) -> Self {
        Self { min }
    }

    /// Builds the rule from its arguments: `{"min_length": 8}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the length is missing or not a number.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(number_arg(args, 0, "min_length")?))
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
        // A string has at most as many characters as bytes, and at least a
        // quarter of them: count the characters only when it matters
        let bytes = str.len() as f64;
        if bytes < self.min || (bytes < self.min * 4.0 && (char_len(&str) as f64) < self.min) {
            return Outcome::code("TOO_SHORT");
        }
        string_output(value, str)
    }
}
