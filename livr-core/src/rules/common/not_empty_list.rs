use crate::rules::prelude::*;

/// `not_empty_list`: the value must be a list with items.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotEmptyList;

impl NotEmptyList {
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
            Value::Array(items) if items.is_empty() => Outcome::code("CANNOT_BE_EMPTY"),
            Value::Array(_) => Outcome::Valid,
            _ if is_no_value(value) => Outcome::code("CANNOT_BE_EMPTY"),
            _ => Outcome::code("FORMAT_ERROR"),
        }
    }
}
