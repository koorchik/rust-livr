use crate::rules::prelude::*;

/// `split` (LIVR 2.5): a string becomes the list of its parts between the
/// separator, empty parts kept: `{"split": ","}`. Numbers and booleans are
/// split in their string form; lists, objects and empty values stay as they
/// are.
#[derive(Clone, Debug)]
pub struct Split {
    separator: String,
}

impl Split {
    /// Builds the rule from its argument, a non-empty string (not a regular
    /// expression) or a number in its string form; further arguments are
    /// ignored.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        match args.first().and_then(text_arg) {
            Some(separator) if !separator.is_empty() => Ok(Self {
                separator: separator.into_owned(),
            }),
            _ => Err(SchemaError::new(
                "rule [split] takes a non-empty separator string",
            )),
        }
    }

    /// Checks the value: never fails.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::String(str) if !str.is_empty() => Outcome::Value(Value::Array(
                str.split(&*self.separator)
                    .map(|part| Value::String(part.to_owned()))
                    .collect(),
            )),
            Value::Number(_) | Value::Bool(_) => Outcome::Value(Value::Array(
                to_js_string(value)
                    .split(&*self.separator)
                    .map(|part| Value::String(part.to_owned()))
                    .collect(),
            )),
            _ => Outcome::Valid,
        }
    }
}
