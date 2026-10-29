use crate::rules::prelude::*;

/// `default`: the value of an empty field (missing, `null` or `""`).
#[derive(Clone, Debug)]
pub struct DefaultRule {
    value: Value,
}

impl DefaultRule {
    /// The rule for a value.
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self { value }
    }

    /// Builds the rule from its arguments: `{"default": 10}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the value is missing.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(arg(args, 0, "default")?.clone()))
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) {
            Outcome::Value(self.value.clone())
        } else {
            Outcome::Valid
        }
    }
}
