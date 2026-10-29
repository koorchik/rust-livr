use crate::rules::prelude::*;

/// `string`: the value must be primitive; numbers and booleans become
/// strings.
#[derive(Clone, Copy, Debug, Default)]
pub struct StringRule;

impl StringRule {
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
            Value::String(_) | Value::Null => Outcome::Valid,
            Value::Number(_) | Value::Bool(_) => {
                Outcome::Value(Value::String(to_js_string(value).into_owned()))
            }
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }
}
