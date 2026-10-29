use crate::rules::prelude::*;

/// `iso_date`: the value must be a date, `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, Default)]
pub struct IsoDate;

// Days in each month, index 0 is January
const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

impl IsoDate {
    /// Builds the rule; it takes no arguments.
    ///
    /// # Errors
    ///
    /// None, the signature is the one of all rule builders.
    pub fn build(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<Self, SchemaError> {
        Ok(Self)
    }

    /// Checks the value.
    #[inline]
    pub fn check(&self, value: &Value, _ctx: &Ctx<'_>) -> Outcome {
        match value {
            Value::Null => Outcome::Valid,
            Value::String(str) if str.is_empty() => Outcome::Valid,
            Value::String(str) if is_iso_date(str.as_bytes()) => Outcome::Valid,
            Value::String(_) | Value::Number(_) | Value::Bool(_) => Outcome::code("WRONG_DATE"),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }
}

fn is_iso_date(bytes: &[u8]) -> bool {
    let &[y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = bytes else {
        return false;
    };
    let (Some(year), Some(month), Some(day)) = (
        digits(&[y1, y2, y3, y4]),
        digits(&[m1, m2]),
        digits(&[d1, d2]),
    ) else {
        return false;
    };
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = if month == 2 && is_leap_year {
        29
    } else {
        DAYS_IN_MONTH[month as usize - 1]
    };
    day <= max_day
}

fn digits(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0, |number, &byte| {
        byte.is_ascii_digit()
            .then(|| number * 10 + u32::from(byte - b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::is_iso_date;

    #[test]
    fn checks_calendar_dates() {
        for date in ["2024-02-29", "2000-02-29", "1999-12-31", "0000-01-01"] {
            assert!(is_iso_date(date.as_bytes()), "{date}");
        }
        for date in [
            "1900-02-29",
            "2023-02-29",
            "2024-04-31",
            "2024-13-01",
            "2024-00-10",
            "2024-01-00",
            "2024-1-01",
            "2024-01-01\n",
            "2014-10-10T22:22",
        ] {
            assert!(!is_iso_date(date.as_bytes()), "{date:?}");
        }
    }
}
