use crate::rules::prelude::*;

/// `iso_date`: the value must be a date, `YYYY-MM-DD`. LIVR 2.5 adds bounds:
/// `{"iso_date": {"min": "2026-06-01", "max": "today"}}`, inclusive, with
/// `today`, `yesterday` and `tomorrow` in UTC. A first argument that is not
/// an object is a schema error (LIVR 2.0 ignored it); arguments after the
/// object are ignored.
#[derive(Clone, Copy, Debug, Default)]
pub struct IsoDate {
    min: Bound,
    max: Bound,
    clock: Clock,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Bound {
    #[default]
    None,
    // Days since 1970-01-01
    Day(i64),
    // Days after the current date
    Relative(i64),
}

impl Bound {
    fn parse(options: &Map<String, Value>, key: &str) -> Result<Self, SchemaError> {
        let Some(bound) = options.get(key) else {
            return Ok(Self::None);
        };
        let parsed = match bound.as_str() {
            Some("yesterday") => Some(Self::Relative(-1)),
            Some("today") => Some(Self::Relative(0)),
            Some("tomorrow") => Some(Self::Relative(1)),
            Some(date) => parse_date(date.as_bytes()).map(Self::Day),
            None => None,
        };
        parsed.ok_or_else(|| {
            let hint = if bound == "current" {
                ", use \"today\" instead of \"current\""
            } else {
                ""
            };
            SchemaError::new(format!(
                "rule [iso_date] takes a date YYYY-MM-DD, \"today\", \"yesterday\" or \"tomorrow\" as {key:?}, got {bound}{hint}"
            ))
        })
    }

    // The day of the bound, if any
    #[inline]
    fn day(self, today: impl Fn() -> i64) -> Option<i64> {
        match self {
            Self::None => None,
            Self::Day(day) => Some(day),
            Self::Relative(days) => Some(today() + days),
        }
    }
}

impl IsoDate {
    /// The rule without bounds, as in LIVR 2.0.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min: Bound::None,
            max: Bound::None,
            clock: Clock::System,
        }
    }

    /// Builds the rule: no arguments, `null` (as in LIVR 2.0), or an object
    /// with `min` and `max` (arguments after it are ignored).
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another first argument, an unknown option, a
    /// wrong bound, or `min` after `max` when both are dates or both are
    /// relative.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let options = match args {
            // LIVR 2.0 ignored the arguments: `{"iso_date": null}` is the
            // plain rule
            [] | [Value::Null] => return Ok(Self::new()),
            [options @ Value::Object(_), ..] => options,
            [arg, ..] => {
                return Err(SchemaError::new(format!(
                    "rule [iso_date] takes no arguments or an object with \"min\" and \"max\", got {arg}"
                )));
            }
        };
        if let Value::Object(map) = options
            && map.contains_key("format")
        {
            return Err(SchemaError::new(
                "rule [iso_date] got an unknown option \"format\", use \"iso_datetime\" for datetimes",
            ));
        }
        let options = options_arg(options, &["min", "max"], "iso_date")?;
        let (min, max) = (Bound::parse(options, "min")?, Bound::parse(options, "max")?);
        // A date and a relative bound depend on the day: not checked
        let is_empty = match (min, max) {
            (Bound::Day(min), Bound::Day(max)) | (Bound::Relative(min), Bound::Relative(max)) => {
                min > max
            }
            _ => false,
        };
        if is_empty {
            return Err(SchemaError::new(
                "rule [iso_date] got \"min\" later than \"max\": no date passes",
            ));
        }
        Ok(Self {
            min,
            max,
            clock: ctx.clock(),
        })
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::Null => Outcome::Valid,
            Value::String(str) if str.is_empty() => Outcome::Valid,
            Value::String(str) => match parse_date(str.as_bytes()) {
                None => Outcome::code("WRONG_DATE"),
                Some(_) if self.min == Bound::None && self.max == Bound::None => Outcome::Valid,
                Some(day) => self.check_bounds(day),
            },
            Value::Number(_) | Value::Bool(_) => Outcome::code("WRONG_DATE"),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }

    fn check_bounds(&self, day: i64) -> Outcome {
        // One clock read per value
        let today = std::cell::OnceCell::new();
        let today = || *today.get_or_init(|| self.clock.today());
        if self.min.day(today).is_some_and(|min| day < min) {
            Outcome::code("DATE_TOO_EARLY")
        } else if self.max.day(today).is_some_and(|max| day > max) {
            Outcome::code("DATE_TOO_LATE")
        } else {
            Outcome::Valid
        }
    }
}
