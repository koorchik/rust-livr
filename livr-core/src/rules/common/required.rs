use crate::rules::prelude::*;

/// `required`: the value must not be empty (missing, `null` or `""`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Required;

impl Required {
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
        if is_no_value(value) {
            Outcome::code("REQUIRED")
        } else {
            Outcome::Valid
        }
    }
}
