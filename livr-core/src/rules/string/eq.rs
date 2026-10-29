use crate::rules::prelude::*;

/// `eq`: the value must be equal to the argument, compared as strings. The
/// output is the argument: `{"eq": 2}` turns `"2"` into `2`.
#[derive(Clone, Debug)]
pub struct EqRule {
    allowed: Value,
    allowed_str: String,
}

impl EqRule {
    /// The rule for a primitive value.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the value is not primitive.
    pub fn new(allowed: Value) -> Result<Self, SchemaError> {
        if !is_primitive(&allowed) {
            return Err(SchemaError::new(format!(
                "rule [eq] expects a primitive value, got {allowed}"
            )));
        }
        let allowed_str = to_js_string(&allowed).into_owned();
        Ok(Self {
            allowed,
            allowed_str,
        })
    }

    /// Builds the rule from its arguments: `{"eq": value}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the value is missing or not primitive.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Self::new(arg(args, 0, "eq")?.clone())
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
        if to_js_string(value) != self.allowed_str {
            return Outcome::code("NOT_ALLOWED_VALUE");
        }
        if *value == self.allowed {
            Outcome::Valid
        } else {
            Outcome::Value(self.allowed.clone())
        }
    }
}
