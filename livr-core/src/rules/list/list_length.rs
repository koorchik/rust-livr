use crate::rules::prelude::*;

/// `list_length` (LIVR 2.5): the value must be a list with this many items:
/// `{"list_length": 2}`, `{"list_length": [1, 10]}`,
/// `{"list_length": {"max": 100}}`. An empty list is a value.
#[derive(Clone, Copy, Debug)]
pub struct ListLength {
    min: u64,
    max: u64,
}

impl ListLength {
    /// The rule for the bounds.
    #[must_use]
    pub const fn new(min: u64, max: u64) -> Self {
        Self { min, max }
    }

    /// Builds the rule from its arguments: a length, `min` and `max`, or an
    /// object with `min`, `max` or both. Numbers may be strings with a
    /// number; further arguments are ignored.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for other arguments, unknown options, or `min`
    /// greater than `max`.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let rule = "list_length";
        let (min, max) = match args {
            [options @ Value::Object(_), ..] => {
                let options = options_arg(options, &["min", "max"], rule)?;
                if options.is_empty() {
                    return Err(SchemaError::new(
                        "rule [list_length] takes \"min\", \"max\" or both",
                    ));
                }
                let bound = |key| {
                    options
                        .get(key)
                        .map(|bound| count_arg(bound, rule))
                        .transpose()
                };
                (
                    bound("min")?.unwrap_or(0),
                    bound("max")?.unwrap_or(u64::MAX),
                )
            }
            [length] => {
                let length = count_arg(length, rule)?;
                (length, length)
            }
            [min, max, ..] => (count_arg(min, rule)?, count_arg(max, rule)?),
            [] => {
                return Err(SchemaError::new(
                    "rule [list_length] takes a length, min and max, or an object with \"min\" and \"max\"",
                ));
            }
        };
        if min > max {
            return Err(SchemaError::new(
                "rule [list_length] got \"min\" greater than \"max\"",
            ));
        }
        Ok(Self::new(min, max))
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::Array(items) => {
                let length = items.len() as u64;
                if length < self.min {
                    Outcome::code("TOO_FEW_ITEMS")
                } else if length > self.max {
                    Outcome::code("TOO_MANY_ITEMS")
                } else {
                    Outcome::Valid
                }
            }
            value if is_no_value(value) => Outcome::Valid,
            _ => Outcome::code("FORMAT_ERROR"),
        }
    }
}
