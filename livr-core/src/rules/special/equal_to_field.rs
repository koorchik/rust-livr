use crate::rules::prelude::*;

/// `equal_to_field`: the value must be equal to the raw value of another
/// field of the same object, compared as strings.
#[derive(Clone, Debug)]
pub struct EqualToField {
    field: String,
}

impl EqualToField {
    /// The rule for a field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
        }
    }

    /// Builds the rule from its arguments: `{"equal_to_field": "password"}`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] when the field name is missing.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self::new(string_arg(args, 0, "equal_to_field")?))
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        if is_no_value(value) {
            return Outcome::Valid;
        }
        if !is_primitive(value) {
            return Outcome::code("FORMAT_ERROR");
        }
        let is_equal = match (ctx.field(&self.field), value) {
            (Some(Value::String(other)), Value::String(str)) => other == str,
            (Some(other), _) if is_primitive(other) => to_js_string(other) == to_js_string(value),
            _ => false,
        };
        if is_equal {
            Outcome::Valid
        } else {
            Outcome::code("FIELDS_NOT_EQUAL")
        }
    }
}
