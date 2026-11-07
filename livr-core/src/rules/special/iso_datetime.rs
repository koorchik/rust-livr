use crate::rules::prelude::*;

// The instants of 0000-01-01T00:00:00Z and 10000-01-01T00:00:00Z: the output
// form has four digits of the year
const MIN_OUTPUT: i64 = -62_167_219_200_000;
const END_OUTPUT: i64 = 253_402_300_800_000;

/// `iso_datetime` (LIVR 2.5): a date and a time with an offset,
/// `YYYY-MM-DDTHH:MM[:SS[.fraction]](Z|+HH:MM|-HH:MM)`. The output is the
/// instant in UTC, `YYYY-MM-DDTHH:MM:SS.sssZ`. Bounds:
/// `{"iso_datetime": {"min": "2026-01-01T09:00:00Z", "max": "tomorrow"}}`;
/// a date is the start of the UTC day as `min`, its end as `max`.
#[derive(Clone, Copy, Debug, Default)]
pub struct IsoDatetime {
    min: Bound,
    max: Bound,
    clock: Clock,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Bound {
    #[default]
    None,
    // Milliseconds since 1970-01-01T00:00:00Z
    Instant(i64),
    // Days after the current date
    Relative(i64),
}

impl Bound {
    fn parse(options: &Map<String, Value>, key: &str) -> Result<Self, SchemaError> {
        let Some(bound) = options.get(key) else {
            return Ok(Self::None);
        };
        let is_max = key == "max";
        let parsed = match bound.as_str() {
            Some("yesterday") => Some(Self::Relative(-1)),
            Some("today") => Some(Self::Relative(0)),
            Some("tomorrow") => Some(Self::Relative(1)),
            Some(date) if date.len() == 10 => parse_date(date.as_bytes())
                .map(|day| Self::Instant(day * DAY_MS + if is_max { DAY_MS - 1 } else { 0 })),
            // Any year: a bound is only compared
            Some(datetime) => {
                parse_datetime(datetime.as_bytes()).map(|(instant, _)| Self::Instant(instant))
            }
            None => None,
        };
        parsed.ok_or_else(|| {
            let hint = if bound == "current" {
                ", use \"today\" instead of \"current\""
            } else {
                ""
            };
            SchemaError::new(format!(
                "rule [iso_datetime] takes a datetime with an offset, a date YYYY-MM-DD, \"today\", \"yesterday\" or \"tomorrow\" as {key:?}, got {bound}{hint}"
            ))
        })
    }
}

impl IsoDatetime {
    /// The rule without bounds.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min: Bound::None,
            max: Bound::None,
            clock: Clock::System,
        }
    }

    /// Builds the rule: no arguments, or an object with `min` and `max`
    /// (arguments after it are ignored).
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] for another first argument, an unknown option, a
    /// wrong bound, or `min` after `max` when both are fixed or both are
    /// relative.
    pub fn build(args: &[Value], ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        let options = match args {
            [] => return Ok(Self::new()),
            [options @ Value::Object(_), ..] => {
                options_arg(options, &["min", "max"], "iso_datetime")?
            }
            [arg, ..] => {
                return Err(SchemaError::new(format!(
                    "rule [iso_datetime] takes no arguments or an object with \"min\" and \"max\", got {arg}"
                )));
            }
        };
        let (min, max) = (Bound::parse(options, "min")?, Bound::parse(options, "max")?);
        // Bounds are in milliseconds already. A fixed and a relative bound
        // depend on the day: not checked
        let is_empty = match (min, max) {
            (Bound::Instant(min), Bound::Instant(max))
            | (Bound::Relative(min), Bound::Relative(max)) => min > max,
            _ => false,
        };
        if is_empty {
            return Err(SchemaError::new(
                "rule [iso_datetime] got \"min\" later than \"max\": no datetime passes",
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
        let str = match value {
            Value::Null => return Outcome::Valid,
            Value::String(str) if str.is_empty() => return Outcome::Valid,
            Value::String(str) => str,
            Value::Number(_) | Value::Bool(_) => return Outcome::code("WRONG_DATETIME"),
            Value::Array(_) | Value::Object(_) => return Outcome::code("FORMAT_ERROR"),
        };
        let Some((instant, is_output)) = parse_datetime(str.as_bytes()) else {
            return Outcome::code("WRONG_DATETIME");
        };
        if !(MIN_OUTPUT..END_OUTPUT).contains(&instant) {
            return Outcome::code("WRONG_DATETIME");
        }
        if self.min != Bound::None || self.max != Bound::None {
            // One clock read per value
            let today = std::cell::OnceCell::new();
            let today = || *today.get_or_init(|| self.clock.today());
            let min = match self.min {
                Bound::None => None,
                Bound::Instant(min) => Some(min),
                Bound::Relative(days) => Some((today() + days) * DAY_MS),
            };
            if min.is_some_and(|min| instant < min) {
                return Outcome::code("DATETIME_TOO_EARLY");
            }
            let max = match self.max {
                Bound::None => None,
                Bound::Instant(max) => Some(max),
                Bound::Relative(days) => Some((today() + days + 1) * DAY_MS - 1),
            };
            if max.is_some_and(|max| instant > max) {
                return Outcome::code("DATETIME_TOO_LATE");
            }
        }
        if is_output {
            return Outcome::Valid;
        }
        match format_datetime(instant) {
            Some(output) => Outcome::Value(Value::String(output)),
            None => Outcome::code("WRONG_DATETIME"),
        }
    }
}
