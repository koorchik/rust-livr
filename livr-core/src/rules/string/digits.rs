use crate::rules::prelude::*;

/// `digits` (LIVR 2.5): the string form must be one or more ASCII digits.
/// Leading zeros are kept, the output is a string: `7` becomes `"7"`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Digits;

impl Digits {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "digits")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) {
            return Outcome::Valid;
        }
        if !is_primitive(value) {
            return Outcome::code("FORMAT_ERROR");
        }
        let str = to_js_string(value);
        if str.bytes().all(|byte| byte.is_ascii_digit()) {
            string_output(value, str)
        } else {
            Outcome::code("NOT_DIGITS")
        }
    }
}
