// SPDX-License-Identifier: Apache-2.0
//! `MySQL` temporal text (`YYYY-MM-DD`, `YYYY-MM-DD hh:mm:ss[.ffffff]`,
//! `[-]hh:mm:ss[.ffffff]`) to Arrow values and back.
//!
//! Only proleptic Gregorian dates from year 1 and times of day below 24 hours
//! have an exact Arrow form. Zero dates (`0000-00-00`), partial dates
//! (`2024-00-15`), negative times, times of 24 hours or more, and any text in
//! an unexpected shape are [`CellError::Unrepresentable`]: the column falls
//! back to the server's text instead of failing or guessing.
use std::fmt::Write;

use super::CellError;

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;

/// Reads fixed-width digit groups and separators from the front of `rest`.
struct Cursor<'a> {
    rest: &'a [u8],
}

impl Cursor<'_> {
    fn digits(&mut self, width: usize) -> Option<i64> {
        let (digits, rest) = self.rest.split_at_checked(width)?;
        self.rest = rest;
        digits.iter().try_fold(0_i64, |value, byte| {
            byte.is_ascii_digit()
                .then(|| value * 10 + i64::from(byte - b'0'))
        })
    }

    /// One or more digits (an hour count of `TIME` may exceed two).
    fn number(&mut self) -> Option<i64> {
        let width = self.rest.iter().take_while(|b| b.is_ascii_digit()).count();
        if width == 0 || width > 6 {
            return None;
        }
        self.digits(width)
    }

    fn byte(&mut self, expected: u8) -> Option<()> {
        let (&first, rest) = self.rest.split_first()?;
        self.rest = rest;
        (first == expected).then_some(())
    }

    /// `[.d{1,6}]` as microseconds.
    fn fraction(&mut self) -> Option<i64> {
        if self.rest.is_empty() {
            return Some(0);
        }
        self.byte(b'.')?;
        let width = self.rest.len();
        if width == 0 || width > 6 {
            return None;
        }
        let value = self.digits(width)?;
        Some(value * 10_i64.pow(u32::try_from(6 - width).ok()?))
    }

    fn date(&mut self) -> Option<i64> {
        let year = self.digits(4)?;
        self.byte(b'-')?;
        let month = self.digits(2)?;
        self.byte(b'-')?;
        let day = self.digits(2)?;
        valid_date(year, month, day).then(|| days_from_civil(year, month, day))
    }

    /// `hh:mm:ss` of a time of day, as seconds.
    fn clock(&mut self, hours: i64) -> Option<i64> {
        self.byte(b':')?;
        let minutes = self.digits(2)?;
        self.byte(b':')?;
        let seconds = self.digits(2)?;
        (hours < 24 && minutes < 60 && seconds < 60)
            .then_some(hours * 3_600 + minutes * 60 + seconds)
    }
}

const UNREPRESENTABLE: CellError = CellError::Unrepresentable("zero, partial or out-of-range");

/// `DATE` text as days since 1970-01-01.
pub(super) fn date_days(text: &[u8]) -> Result<i32, CellError> {
    let mut cursor = Cursor { rest: text };
    cursor
        .date()
        .filter(|_| cursor.rest.is_empty())
        .and_then(|days| i32::try_from(days).ok())
        .ok_or(UNREPRESENTABLE)
}

/// `DATETIME` or `TIMESTAMP` text as microseconds since 1970-01-01 00:00:00.
pub(super) fn datetime_micros(text: &[u8]) -> Result<i64, CellError> {
    let mut cursor = Cursor { rest: text };
    let parsed = (|| {
        let days = cursor.date()?;
        cursor.byte(b' ')?;
        let hours = cursor.digits(2)?;
        let seconds = cursor.clock(hours)?;
        let micros = cursor.fraction()?;
        Some(days * MICROS_PER_DAY + seconds * MICROS_PER_SECOND + micros)
    })();
    parsed.ok_or(UNREPRESENTABLE)
}

/// `TIME` text as microseconds since midnight; only `00:00:00` to
/// `23:59:59.999999` are a time of day.
pub(super) fn time_micros(text: &[u8]) -> Result<i64, CellError> {
    let mut cursor = Cursor { rest: text };
    let parsed = (|| {
        let hours = cursor.number()?;
        let seconds = cursor.clock(hours)?;
        let micros = cursor.fraction()?;
        Some(seconds * MICROS_PER_SECOND + micros)
    })();
    parsed.ok_or(UNREPRESENTABLE)
}

/// `YYYY-MM-DD`, as the server prints a `DATE`.
pub(super) fn write_date(days: i32, out: &mut impl Write) -> std::fmt::Result {
    let (year, month, day) = civil_from_days(i64::from(days));
    write!(out, "{year:04}-{month:02}-{day:02}")
}

/// `YYYY-MM-DD hh:mm:ss[.f]` with `fsp` fraction digits (trailing zeros
/// trimmed when the precision is unknown).
pub(super) fn write_datetime(
    micros: i64,
    fsp: Option<u8>,
    out: &mut impl Write,
) -> std::fmt::Result {
    let days = micros.div_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    write!(out, "{year:04}-{month:02}-{day:02} ")?;
    write_clock(micros.rem_euclid(MICROS_PER_DAY), fsp, out)
}

/// `hh:mm:ss[.f]` of a time of day.
pub(super) fn write_time(micros: i64, fsp: Option<u8>, out: &mut impl Write) -> std::fmt::Result {
    write_clock(micros, fsp, out)
}

fn write_clock(micros: i64, fsp: Option<u8>, out: &mut impl Write) -> std::fmt::Result {
    let seconds = micros / MICROS_PER_SECOND;
    let (hours, minutes, seconds) = (seconds / 3_600, seconds / 60 % 60, seconds % 60);
    write!(out, "{hours:02}:{minutes:02}:{seconds:02}")?;
    let fraction = micros % MICROS_PER_SECOND;
    match fsp {
        Some(0) => Ok(()),
        Some(digits) => {
            let digits = usize::from(digits.min(6));
            let shown = fraction / 10_i64.pow(u32::try_from(6 - digits).unwrap_or(0));
            write!(out, ".{shown:0digits$}")
        }
        None if fraction == 0 => Ok(()),
        None => {
            let text = format!("{fraction:06}");
            write!(out, ".{}", text.trim_end_matches('0'))
        }
    }
}

fn valid_date(year: i64, month: i64, day: i64) -> bool {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=last).contains(&day)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`] (H. Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(write: impl FnOnce(&mut String) -> std::fmt::Result) -> String {
        let mut out = String::new();
        assert!(write(&mut out).is_ok());
        out
    }

    #[test]
    fn dates_count_days_from_the_unix_epoch() {
        assert_eq!(date_days(b"1970-01-01"), Ok(0));
        assert_eq!(date_days(b"1969-12-31"), Ok(-1));
        assert_eq!(date_days(b"2000-03-01"), Ok(11_017));
        assert_eq!(date_days(b"0001-01-01"), Ok(-719_162));
        assert_eq!(date_days(b"9999-12-31"), Ok(2_932_896));
        assert_eq!(date_days(b"2024-02-29"), Ok(19_782));
    }

    #[test]
    fn zero_partial_and_malformed_dates_are_unrepresentable() {
        for bad in [
            &b"0000-00-00"[..],
            b"2024-00-15",
            b"2024-05-00",
            b"0000-01-01",
            b"2023-02-29",
            b"2024-13-01",
            b"2024-1-01",
            b"2024-01-01x",
            b"",
            b"\xff\xfe",
        ] {
            assert_eq!(date_days(bad), Err(UNREPRESENTABLE), "{bad:?}");
        }
    }

    #[test]
    fn datetimes_carry_microseconds() {
        assert_eq!(datetime_micros(b"1970-01-01 00:00:00"), Ok(0));
        assert_eq!(
            datetime_micros(b"1970-01-02 01:02:03.5"),
            Ok(MICROS_PER_DAY + 3_723 * MICROS_PER_SECOND + 500_000)
        );
        assert_eq!(datetime_micros(b"1969-12-31 23:59:59.999999"), Ok(-1));
        for bad in [
            &b"0000-00-00 00:00:00"[..],
            b"2024-01-01 24:00:00",
            b"2024-01-01 10:60:00",
            b"2024-01-01 10:00:00.",
            b"2024-01-01 10:00:00.1234567",
            b"2024-01-01T10:00:00",
            b"2024-01-01",
        ] {
            assert_eq!(datetime_micros(bad), Err(UNREPRESENTABLE), "{bad:?}");
        }
    }

    #[test]
    fn only_times_of_day_are_typed() {
        assert_eq!(time_micros(b"00:00:00"), Ok(0));
        assert_eq!(
            time_micros(b"23:59:59.000001"),
            Ok(86_399 * MICROS_PER_SECOND + 1)
        );
        for bad in [&b"24:00:00"[..], b"-01:00:00", b"838:59:59", b"12:00", b"x"] {
            assert_eq!(time_micros(bad), Err(UNREPRESENTABLE), "{bad:?}");
        }
    }

    #[test]
    fn typed_values_print_back_as_the_server_text() -> Result<(), CellError> {
        let stamp = "2024-05-06 07:08:09.120";
        let micros = datetime_micros(stamp.as_bytes())?;
        assert_eq!(text(|o| write_datetime(micros, Some(3), o)), stamp);
        assert_eq!(
            text(|o| write_datetime(micros, None, o)),
            "2024-05-06 07:08:09.12"
        );
        assert_eq!(
            text(|o| write_datetime(micros, Some(0), o)),
            "2024-05-06 07:08:09"
        );
        let early = datetime_micros(b"0001-01-01 00:00:00.000001")?;
        assert_eq!(
            text(|o| write_datetime(early, Some(6), o)),
            "0001-01-01 00:00:00.000001"
        );
        let days = date_days(b"0999-12-31")?;
        assert_eq!(text(|o| write_date(days, o)), "0999-12-31");
        let noon = time_micros(b"12:00:00.5")?;
        assert_eq!(text(|o| write_time(noon, Some(2), o)), "12:00:00.50");
        Ok(())
    }
}
