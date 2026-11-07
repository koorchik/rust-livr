//! Dates, datetimes and times of day (`iso_date`, `iso_datetime`,
//! `iso_time`), and the clock of the bounds `today`, `yesterday` and
//! `tomorrow`. Parsed by hand over bytes, with the civil calendar arithmetic
//! of Howard Hinnant: no regular expressions and no date library.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds in a day.
pub(crate) const DAY_MS: i64 = 86_400_000;

/// Where `today` comes from: the system time in UTC, a fixed time, or a
/// function (for tests).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Clock {
    #[default]
    System,
    /// Milliseconds since 1970-01-01T00:00:00Z.
    Fixed(i64),
    Function(fn() -> SystemTime),
}

impl Clock {
    /// The current time in milliseconds since 1970-01-01T00:00:00Z.
    #[inline]
    pub(crate) fn now_ms(self) -> i64 {
        match self {
            Self::System => system_time_ms(SystemTime::now()),
            Self::Fixed(ms) => ms,
            Self::Function(now) => system_time_ms(now()),
        }
    }

    /// The current date in UTC, as days since 1970-01-01.
    #[inline]
    pub(crate) fn today(self) -> i64 {
        self.now_ms().div_euclid(DAY_MS)
    }
}

/// Milliseconds since 1970-01-01T00:00:00Z, negative before it.
pub(crate) fn system_time_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).unwrap_or(i64::MAX),
        // Before 1970: rounded down, like the milliseconds of later times
        Err(before) => {
            let before = before.duration();
            let ms = i64::try_from(before.as_millis()).unwrap_or(i64::MAX);
            if before.subsec_nanos() % 1_000_000 == 0 {
                -ms
            } else {
                -ms - 1
            }
        }
    }
}

/// Days since 1970-01-01 of a date of the proleptic Gregorian calendar
/// ("days_from_civil").
pub(crate) fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date of a number of days since 1970-01-01 ("civil_from_days").
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153; // March is 0
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    // In range: a month is 1-12, a day 1-31
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (year, month as u32, day as u32)
}

// Two ASCII digits as a number
#[inline]
fn two_digits(high: u8, low: u8) -> Option<u32> {
    (high.is_ascii_digit() && low.is_ascii_digit())
        .then(|| u32::from(high - b'0') * 10 + u32::from(low - b'0'))
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A date `YYYY-MM-DD` of the calendar (`0000` is a leap year) as days since
/// 1970-01-01.
pub(crate) fn parse_date(bytes: &[u8]) -> Option<i64> {
    let &[y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = bytes else {
        return None;
    };
    let year = i64::from(two_digits(y1, y2)? * 100 + two_digits(y3, y4)?);
    let month = two_digits(m1, m2)?;
    let day = two_digits(d1, d2)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// A datetime of `iso_datetime`,
/// `YYYY-MM-DD "T" HH ":" MM [ ":" SS [ "." 1*9DIGIT ] ] ( "Z" / ( "+" / "-" ) HH ":" MM )`:
/// the instant in milliseconds since 1970-01-01T00:00:00Z (further digits
/// are cut off), and whether the string is already the output form,
/// `YYYY-MM-DDTHH:MM:SS.sssZ`.
pub(crate) fn parse_datetime(bytes: &[u8]) -> Option<(i64, bool)> {
    if bytes.len() < 17 || bytes[10] != b'T' || bytes[13] != b':' {
        return None;
    }
    let days = parse_date(&bytes[..10])?;
    let hours = two_digits(bytes[11], bytes[12]).filter(|&hours| hours <= 23)?;
    let minutes = two_digits(bytes[14], bytes[15]).filter(|&minutes| minutes <= 59)?;
    let mut seconds = 0;
    let mut ms = 0;
    let mut i = 16;
    if bytes[i] == b':' {
        let (&high, &low) = (bytes.get(17)?, bytes.get(18)?);
        seconds = two_digits(high, low).filter(|&seconds| seconds <= 59)?;
        i = 19;
        if bytes.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() && i - start < 9 {
                if i - start < 3 {
                    ms = ms * 10 + u32::from(bytes[i] - b'0');
                }
                i += 1;
            }
            match i - start {
                0 => return None,
                1 => ms *= 100,
                2 => ms *= 10,
                _ => {}
            }
        }
    }
    let offset: i64 = match bytes.get(i)? {
        b'Z' if i + 1 == bytes.len() => 0,
        sign @ (b'+' | b'-') if i + 6 == bytes.len() && bytes[i + 3] == b':' => {
            let offset_hours = two_digits(bytes[i + 1], bytes[i + 2]).filter(|&h| h <= 23)?;
            let offset_minutes = two_digits(bytes[i + 4], bytes[i + 5]).filter(|&m| m <= 59)?;
            let offset = i64::from(offset_hours * 60 + offset_minutes);
            if *sign == b'-' { -offset } else { offset }
        }
        _ => return None,
    };
    let minutes_of_day = i64::from(hours * 60 + minutes) - offset;
    let instant = days * DAY_MS + (minutes_of_day * 60 + i64::from(seconds)) * 1000 + i64::from(ms);
    // "Z" at 23: seconds and three fraction digits
    Some((instant, i == 23 && bytes[i] == b'Z'))
}

/// An instant as `YYYY-MM-DDTHH:MM:SS.sssZ`, `None` outside the years
/// 0000-9999.
pub(crate) fn format_datetime(instant: i64) -> Option<String> {
    let (year, month, day) = civil_from_days(instant.div_euclid(DAY_MS));
    if !(0..=9999).contains(&year) {
        return None;
    }
    let ms = instant.rem_euclid(DAY_MS);
    let mut bytes = *b"0000-00-00T00:00:00.000Z";
    // The digits of a number ending at `end`
    let mut put = |end: usize, mut number: i64, width: usize| {
        for byte in bytes[end - width..end].iter_mut().rev() {
            // A digit
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let digit = (number % 10) as u8;
            *byte = b'0' + digit;
            number /= 10;
        }
    };
    put(4, year, 4);
    put(7, i64::from(month), 2);
    put(10, i64::from(day), 2);
    put(13, ms / 3_600_000, 2);
    put(16, ms / 60_000 % 60, 2);
    put(19, ms / 1000 % 60, 2);
    put(23, ms % 1000, 3);
    Some(String::from_utf8(bytes.to_vec()).expect("ASCII"))
}

/// A time of day of `iso_time`, `HH:MM`, `HH:MM:SS` or `HH:MM:SS.` and 1-9
/// digits, as nanoseconds since midnight.
pub(crate) fn parse_time(bytes: &[u8]) -> Option<u64> {
    let length = bytes.len();
    if length != 5 && length != 8 && !(10..=18).contains(&length) || bytes[2] != b':' {
        return None;
    }
    let hours = two_digits(bytes[0], bytes[1]).filter(|&hours| hours <= 23)?;
    let minutes = two_digits(bytes[3], bytes[4]).filter(|&minutes| minutes <= 59)?;
    let mut seconds = u64::from(hours * 3600 + minutes * 60);
    if length == 5 {
        return Some(seconds * 1_000_000_000);
    }
    if bytes[5] != b':' {
        return None;
    }
    seconds += u64::from(two_digits(bytes[6], bytes[7]).filter(|&seconds| seconds <= 59)?);
    let mut nanos = 0;
    if length > 8 {
        if bytes[8] != b'.' {
            return None;
        }
        let mut unit = 100_000_000;
        for &byte in &bytes[9..] {
            if !byte.is_ascii_digit() {
                return None;
            }
            nanos += u64::from(byte - b'0') * unit;
            unit /= 10;
        }
    }
    Some(seconds * 1_000_000_000 + nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_days_and_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(0, 1, 1), -719_528);
        for days in [-800_000, -719_528, -1, 0, 1, 11_016, 20_000, 2_932_896] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(2_932_896), (9999, 12, 31));
    }

    #[test]
    fn parses_dates() {
        for date in [
            "2024-02-29",
            "2000-02-29",
            "1999-12-31",
            "0000-01-01",
            "0000-02-29",
        ] {
            assert!(parse_date(date.as_bytes()).is_some(), "{date}");
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
            "２０２４-01-01",
        ] {
            assert!(parse_date(date.as_bytes()).is_none(), "{date:?}");
        }
    }

    fn datetime(str: &str) -> Option<String> {
        let (instant, is_output) = parse_datetime(str.as_bytes())?;
        let output = format_datetime(instant)?;
        assert_eq!(is_output, output == str, "{str}");
        Some(output)
    }

    #[test]
    fn parses_and_formats_datetimes() {
        for (value, output) in [
            (
                "2026-10-29T10:00:00.123456+02:00",
                "2026-10-29T08:00:00.123Z",
            ),
            ("2026-10-29T10:00Z", "2026-10-29T10:00:00.000Z"),
            ("2026-10-29T10:00:05.1Z", "2026-10-29T10:00:05.100Z"),
            ("2026-10-29T10:00:05.12-00:00", "2026-10-29T10:00:05.120Z"),
            ("2026-10-29T10:00:05.123Z", "2026-10-29T10:00:05.123Z"),
            ("2026-10-29T10:00:05.123456789Z", "2026-10-29T10:00:05.123Z"),
            ("2024-03-01T00:30+01:00", "2024-02-29T23:30:00.000Z"),
            ("2026-12-31T23:30-01:00", "2027-01-01T00:30:00.000Z"),
            ("9999-12-31T23:59:59.999Z", "9999-12-31T23:59:59.999Z"),
        ] {
            assert_eq!(datetime(value).as_deref(), Some(output), "{value}");
        }
        for value in [
            "2026-10-29",
            "2026-10-29T10Z",
            "2026-10-29T10:00",
            "2026-10-29t10:00Z",
            "2026-10-29T10:00z",
            "2026-10-29T24:00Z",
            "2026-10-29T10:60Z",
            "2026-10-29T10:00:60Z",
            "2026-10-29T10:00:00.Z",
            "2026-10-29T10:00:00.1234567890Z",
            "2026-10-29T10:00+24:00",
            "2026-10-29T10:00+02:60",
            "2026-10-29T10:00+0200",
            "2026-10-29T10:00Z\n",
            "2026-02-30T10:00Z",
            "2026-10-29T10:00:5Z",
        ] {
            assert_eq!(datetime(value), None, "{value:?}");
        }
        // Valid, but outside the years of the output
        let (instant, _) = parse_datetime(b"0000-01-01T00:00+01:00").unwrap();
        assert_eq!(format_datetime(instant), None);
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_time(b"09:00"), Some(32_400_000_000_000));
        assert_eq!(parse_time(b"09:00:00.000"), parse_time(b"09:00"));
        assert_eq!(parse_time(b"00:00:00.000000001"), Some(1));
        assert_eq!(parse_time(b"23:59:59.9"), Some(86_399_900_000_000));
        for time in [
            "9:00",
            "24:00",
            "09:60",
            "09:00:60",
            "09:00:00.",
            "09:00:00.1234567890",
            "09:00Z",
            "09:00:00+02:00",
            "09-00",
            "09:00\n",
        ] {
            assert_eq!(parse_time(time.as_bytes()), None, "{time:?}");
        }
    }

    #[test]
    fn reads_the_clock() {
        assert_eq!(Clock::Fixed(DAY_MS - 1).today(), 0);
        assert_eq!(Clock::Fixed(-1).today(), -1);
        let before = UNIX_EPOCH - std::time::Duration::from_micros(1500);
        assert_eq!(system_time_ms(before), -2);
        assert_eq!(Clock::Function(|| UNIX_EPOCH).now_ms(), 0);
    }
}
