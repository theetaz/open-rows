// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL`-compatible text of interval and array values, shared by the
//! drivers' tagged-text fallback and the exporters (docs/04 §5).
use std::fmt::{self, Write};

const MICROS_PER_SECOND: u64 = 1_000_000;
const MICROS_PER_HOUR: u64 = 3_600 * MICROS_PER_SECOND;

/// An interval as `PostgreSQL` stores it: months, days and microseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntervalParts {
    /// Months (years are twelve months).
    pub months: i32,
    /// Days.
    pub days: i32,
    /// Microseconds of the time part.
    pub micros: i64,
}

impl IntervalParts {
    /// `PostgreSQL` 17's `infinity` and `-infinity`.
    #[must_use]
    pub fn infinite(self) -> Option<&'static str> {
        match (self.months, self.days, self.micros) {
            (i32::MAX, i32::MAX, i64::MAX) => Some("infinity"),
            (i32::MIN, i32::MIN, i64::MIN) => Some("-infinity"),
            _ => None,
        }
    }

    /// `IntervalStyle = postgres`, e.g. `1 year 2 mons -3 days +04:05:06.5`
    /// (`EncodeInterval` in `datetime.c`).
    ///
    /// # Errors
    /// Only when `out` fails.
    pub fn write(self, out: &mut impl Write) -> fmt::Result {
        if let Some(text) = self.infinite() {
            return out.write_str(text);
        }
        let mut parts = Parts {
            empty: true,
            negative: false,
        };
        parts.unit(out, i64::from(self.months / 12), "year")?;
        parts.unit(out, i64::from(self.months % 12), "mon")?;
        parts.unit(out, i64::from(self.days), "day")?;
        if parts.empty || self.micros != 0 {
            let sign = match (self.micros < 0, parts.negative) {
                (true, _) => "-",
                (false, true) => "+",
                (false, false) => "",
            };
            let space = if parts.empty { "" } else { " " };
            let magnitude = self.micros.unsigned_abs();
            write!(out, "{space}{sign}{:02}:", magnitude / MICROS_PER_HOUR)?;
            write_minutes_seconds(magnitude % MICROS_PER_HOUR, out)?;
        }
        Ok(())
    }
}

/// Signs of the parts written so far (`AddPostgresIntPart`).
struct Parts {
    empty: bool,
    negative: bool,
}

impl Parts {
    fn unit(&mut self, out: &mut impl Write, value: i64, unit: &str) -> fmt::Result {
        if value == 0 {
            return Ok(());
        }
        let space = if self.empty { "" } else { " " };
        let plus = if self.negative && value > 0 { "+" } else { "" };
        let plural = if value == 1 { "" } else { "s" };
        write!(out, "{space}{plus}{value} {unit}{plural}")?;
        (self.empty, self.negative) = (false, value < 0);
        Ok(())
    }
}

/// `HH:MM:SS[.ffffff]` of a time of day in microseconds (up to `24:00:00`),
/// trailing fractional zeros trimmed.
///
/// # Errors
/// Only when `out` fails.
pub fn write_clock(micros: u64, out: &mut impl Write) -> fmt::Result {
    write!(out, "{:02}:", micros / MICROS_PER_HOUR)?;
    write_minutes_seconds(micros % MICROS_PER_HOUR, out)
}

fn write_minutes_seconds(micros: u64, out: &mut impl Write) -> fmt::Result {
    let seconds = micros / MICROS_PER_SECOND;
    write!(out, "{:02}:{:02}", seconds / 60, seconds % 60)?;
    let fraction = micros % MICROS_PER_SECOND;
    if fraction == 0 {
        return Ok(());
    }
    let (mut digits, mut width) = (fraction, 6);
    while digits % 10 == 0 {
        (digits, width) = (digits / 10, width - 1);
    }
    write!(out, ".{digits:0width$}")
}

/// One array item as `array_out` prints it: empty items, `NULL`, and items
/// with braces, commas, quotes, backslashes or white space are
/// double-quoted, with `"` and `\` escaped.
///
/// # Errors
/// Only when `out` fails.
pub fn write_array_item(item: &str, out: &mut (impl Write + ?Sized)) -> fmt::Result {
    let plain = !item.is_empty()
        && !item.eq_ignore_ascii_case("NULL")
        && !item
            .chars()
            .any(|c| matches!(c, '{' | '}' | ',' | '"' | '\\') || c.is_ascii_whitespace());
    if plain {
        return out.write_str(item);
    }
    out.write_char('"')?;
    for c in item.chars() {
        if matches!(c, '"' | '\\') {
            out.write_char('\\')?;
        }
        out.write_char(c)?;
    }
    out.write_char('"')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(months: i32, days: i32, micros: i64) -> String {
        let mut out = String::new();
        let parts = IntervalParts {
            months,
            days,
            micros,
        };
        assert!(parts.write(&mut out).is_ok());
        out
    }

    #[test]
    fn intervals_print_like_intervalstyle_postgres() {
        let hms = |h: i64, m: i64, s: i64| ((h * 60 + m) * 60 + s) * 1_000_000;
        assert_eq!(interval(0, 0, 0), "00:00:00");
        assert_eq!(
            interval(14, 3, hms(4, 5, 6) + 500_000),
            "1 year 2 mons 3 days 04:05:06.5"
        );
        assert_eq!(interval(0, 1, 0), "1 day");
        assert_eq!(interval(0, -1, hms(1, 0, 0)), "-1 days +01:00:00");
        assert_eq!(
            interval(-14, 3, -hms(0, 0, 1)),
            "-1 years -2 mons +3 days -00:00:01"
        );
        assert_eq!(interval(0, 0, hms(100, 0, 0)), "100:00:00");
        assert_eq!(interval(12, 0, 0), "1 year");
        assert_eq!(interval(i32::MAX, i32::MAX, i64::MAX), "infinity");
        assert_eq!(interval(i32::MIN, i32::MIN, i64::MIN), "-infinity");
    }

    #[test]
    fn clocks_trim_fractions() {
        let mut out = String::new();
        assert!(write_clock(13 * MICROS_PER_HOUR + 120, &mut out).is_ok());
        assert_eq!(out, "13:00:00.00012");
        out.clear();
        assert!(write_clock(24 * MICROS_PER_HOUR, &mut out).is_ok());
        assert_eq!(out, "24:00:00");
    }

    #[test]
    fn array_items_are_quoted_like_array_out() {
        let mut out = String::new();
        for item in ["plain", "", "a b", "NULL", "q\"\\", "{x}"] {
            assert!(write_array_item(item, &mut out).is_ok());
            out.push(',');
        }
        assert_eq!(out, r#"plain,"","a b","NULL","q\"\\","{x}","#);
    }
}
