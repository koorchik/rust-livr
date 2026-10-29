use crate::rules::prelude::*;

/// `trim`: removes leading and trailing whitespace.
#[derive(Clone, Copy, Debug, Default)]
pub struct Trim;

impl Trim {
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
        if is_no_value(value) || !is_primitive(value) {
            return Outcome::Valid;
        }
        let str = to_js_string(value);
        let trimmed = js_trim(&str);
        if value.is_string() && trimmed.len() == str.len() {
            return Outcome::Valid;
        }
        Outcome::Value(Value::String(trimmed.to_owned()))
    }
}
