use crate::rules::prelude::*;

/// `boolean` (LIVR 2.5): `true`, `false`, `1`, `0`, `"1"`, `"0"`, `"true"`
/// and `"false"`; the output is a real boolean. Strings are case-sensitive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Boolean;

impl Boolean {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for arguments.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        no_args(args, "boolean")?;
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let boolean = match value {
            Value::Bool(_) | Value::Null => return Outcome::Valid,
            Value::String(str) => match str.as_str() {
                "" => return Outcome::Valid,
                "1" | "true" => true,
                "0" | "false" => false,
                _ => return Outcome::code("NOT_BOOLEAN"),
            },
            // By value: 1.0 is 1
            Value::Number(number) => match number.as_f64() {
                Some(1.0) => true,
                Some(0.0) => false,
                _ => return Outcome::code("NOT_BOOLEAN"),
            },
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        Outcome::Value(Value::Bool(boolean))
    }
}
