use crate::rules::prelude::*;

/// `any` (LIVR 2.5): any JSON value passes unchanged, objects and lists
/// included, so that the field is kept in the output.
#[derive(Clone, Copy, Debug, Default)]
pub struct Any;

impl Any {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "any")?;
        Ok(Self)
    }

    /// Checks the value: always valid.
    #[inline]
    pub fn check(&self, _value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        Outcome::Valid
    }
}
