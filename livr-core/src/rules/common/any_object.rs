use crate::rules::prelude::*;

/// `any_object`: the value must be an object, with any fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnyObject;

impl AnyObject {
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
        if value.is_object() || is_no_value(value) {
            Outcome::Valid
        } else {
            Outcome::code("FORMAT_ERROR")
        }
    }
}
