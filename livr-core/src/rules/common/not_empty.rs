use crate::rules::prelude::*;

/// `not_empty`: the value must not be `""`. Missing values pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotEmpty;

impl NotEmpty {
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
            Value::String(str) if str.is_empty() => Outcome::code("CANNOT_BE_EMPTY"),
            _ => Outcome::Valid,
        }
    }
}
