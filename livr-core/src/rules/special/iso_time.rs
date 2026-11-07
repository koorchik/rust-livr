use crate::rules::prelude::*;

/// `iso_time` (LIVR 2.5): a time of day, `HH:MM`, `HH:MM:SS` or
/// `HH:MM:SS.fraction` (1-9 digits), without an offset. Bounds:
/// `{"iso_time": {"min": "09:00", "max": "18:00"}}`; with `min` after `max`
/// the range goes over midnight.
#[derive(Clone, Copy, Debug, Default)]
pub struct IsoTime {
    // Nanoseconds since midnight
    min: Option<u64>,
    max: Option<u64>,
}

impl IsoTime {
    /// Builds the rule: no arguments, or an object with `min` and `max`.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another argument, an unknown option or a wrong
    /// bound.
    pub fn build(args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        // Arguments after the object are ignored
        let options = match args {
            [] => return Ok(Self::default()),
            [options @ Value::Object(_), ..] => options_arg(options, &["min", "max"], "iso_time")?,
            [arg, ..] => {
                return Err(SchemaError::new(format!(
                    "rule [iso_time] takes no arguments or an object with \"min\" and \"max\", got {arg}"
                )));
            }
        };
        let bound = |key: &str| {
            options
                .get(key)
                .map(|bound| {
                    bound
                        .as_str()
                        .and_then(|time| parse_time(time.as_bytes()))
                        .ok_or_else(|| {
                            SchemaError::new(format!(
                                "rule [iso_time] got a wrong time as {key:?}: {bound}"
                            ))
                        })
                })
                .transpose()
        };
        Ok(Self {
            min: bound("min")?,
            max: bound("max")?,
        })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        let str = match value {
            Value::Null => return Outcome::Valid,
            Value::String(str) if str.is_empty() => return Outcome::Valid,
            Value::String(str) => str,
            Value::Number(_) | Value::Bool(_) => return Outcome::code("WRONG_TIME"),
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        let Some(time) = parse_time(str.as_bytes()) else {
            return Outcome::code("WRONG_TIME");
        };
        match (self.min, self.max) {
            // Over midnight: a time in the gap gets the error of the nearer
            // bound, TIME_TOO_EARLY in the middle
            (Some(min), Some(max)) if min > max => {
                if time >= min || time <= max {
                    Outcome::Valid
                } else if time - max < min - time {
                    Outcome::code("TIME_TOO_LATE")
                } else {
                    Outcome::code("TIME_TOO_EARLY")
                }
            }
            (min, _) if min.is_some_and(|min| time < min) => Outcome::code("TIME_TOO_EARLY"),
            (_, max) if max.is_some_and(|max| time > max) => Outcome::code("TIME_TOO_LATE"),
            _ => Outcome::Valid,
        }
    }
}
