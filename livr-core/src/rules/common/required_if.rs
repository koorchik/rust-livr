use crate::rules::prelude::*;

/// `required_if` (LIVR 2.5): the field is required when every referenced
/// field has one of the given values, compared as strings:
/// `{"required_if": {"type": "company", "address/country": ["UA", "PL"]}}`.
/// The referenced values are the input, before their rules.
#[derive(Clone, Debug)]
pub struct RequiredIf {
    conditions: Box<[Condition]>,
}

#[derive(Clone, Debug)]
struct Condition {
    reference: Reference,
    // The string forms of the values; an empty list never matches
    values: Box<[Box<str>]>,
}

impl RequiredIf {
    /// Builds the rule from its argument, an object of references to values.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument, an empty object, a wrong
    /// reference, or a value that is not primitive, is `null` or `""`.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let conditions = match args {
            // Arguments after the object are ignored
            [Value::Object(conditions), ..] if !conditions.is_empty() => conditions,
            _ => {
                return Err(SchemaError::new(
                    "rule [required_if] takes an object of references to values",
                ));
            }
        };
        let value_str = |reference: &str, value: &Value| {
            if is_primitive(value) && !is_no_value(value) {
                Ok(to_js_string(value).into())
            } else {
                Err(SchemaError::new(format!(
                    "rule [required_if] takes a primitive value other than \"\" or a list of them for {reference:?}, got {value}"
                )))
            }
        };
        let conditions = conditions
            .iter()
            .map(|(reference, value)| {
                let values = match value {
                    // Never matches, like ""
                    Value::Array(values) if values.is_empty() => {
                        return Err(SchemaError::new(format!(
                            "rule [required_if] takes a non-empty list of values for {reference:?}"
                        )));
                    }
                    Value::Array(values) => values
                        .iter()
                        .map(|value| value_str(reference, value))
                        .collect::<Result<_, _>>()?,
                    value => Box::new([value_str(reference, value)?]) as Box<[Box<str>]>,
                };
                Ok(Condition {
                    reference: Reference::parse_str(reference, "required_if")?,
                    values,
                })
            })
            .collect::<Result<_, SchemaError>>()?;
        Ok(Self { conditions })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
        if !is_no_value(value) {
            return Outcome::Valid;
        }
        let matches = |condition: &Condition| match condition.reference.resolve(ctx) {
            Some(other) if is_primitive(other) && !is_no_value(other) => {
                let other = to_js_string(other);
                condition.values.iter().any(|value| **value == *other)
            }
            _ => false,
        };
        if self.conditions.iter().all(matches) {
            Outcome::code("REQUIRED")
        } else {
            Outcome::Valid
        }
    }
}
