use crate::rules::prelude::*;

/// `starts_with` (LIVR 2.5): the string form must start with one of the
/// arguments, case-sensitive: `{"starts_with": ["https://", "http://"]}`. The output is the
/// string form.
#[derive(Clone, Debug)]
pub struct StartsWith {
    strings: Box<[Box<str>]>,
}

impl StartsWith {
    /// Builds the rule from its arguments, one or more non-empty strings.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] without strings, or for an empty string or another
    /// value.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let strings: Box<[Box<str>]> = args
            .iter()
            .map(|arg| match text_arg(arg) {
                Some(str) if !str.is_empty() => Ok(str.into()),
                _ => Err(SchemaError::new(format!(
                    "rule [starts_with] takes one or more non-empty strings, got {arg}"
                ))),
            })
            .collect::<Result<_, _>>()?;
        if strings.is_empty() {
            return Err(SchemaError::new(
                "rule [starts_with] takes one or more non-empty strings",
            ));
        }
        Ok(Self { strings })
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
        if self.strings.iter().any(|string| str.starts_with(&**string)) {
            string_output(value, str)
        } else {
            Outcome::code("WRONG_FORMAT")
        }
    }
}
