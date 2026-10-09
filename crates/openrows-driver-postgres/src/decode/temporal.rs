// SPDX-License-Identifier: Apache-2.0
//! `time`, `timetz` and `interval` binary values, and their `PostgreSQL`
//! text forms (`DateStyle` ISO, `IntervalStyle` postgres).
use super::scalar::{CellError, array};
use arrow_array::types::IntervalMonthDayNano;
use openrows_driver_api::text::{self, IntervalParts};
use std::fmt::{self, Write};

pub(crate) const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_HOUR: i64 = 3_600 * MICROS_PER_SECOND;
const MICROS_PER_DAY: i64 = 24 * MICROS_PER_HOUR;

/// `time` as microseconds since midnight; `24:00:00` has no `Time64` value.
pub(crate) fn time_micros(bytes: &[u8]) -> Result<i64, CellError> {
    let micros = i64::from_be_bytes(array(bytes)?);
    if (0..MICROS_PER_DAY).contains(&micros) {
        Ok(micros)
    } else {
        Err(CellError::Unrepresentable("time 24:00:00"))
    }
}

/// `interval` wire value: microseconds, days, months.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Interval {
    pub(crate) micros: i64,
    pub(crate) days: i32,
    pub(crate) months: i32,
}

impl Interval {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, CellError> {
        let bytes: [u8; 16] = array(bytes)?;
        let (micros, rest) = bytes.split_at(8);
        let (days, months) = rest.split_at(4);
        Ok(Self {
            micros: i64::from_be_bytes(array(micros)?),
            days: i32::from_be_bytes(array(days)?),
            months: i32::from_be_bytes(array(months)?),
        })
    }

    /// The Arrow value; infinities and time parts beyond ±292 years of
    /// nanoseconds fall back to text.
    pub(crate) fn month_day_nano(self) -> Result<IntervalMonthDayNano, CellError> {
        let infinite = matches!(
            (self.months, self.days, self.micros),
            (i32::MAX, i32::MAX, i64::MAX) | (i32::MIN, i32::MIN, i64::MIN)
        );
        if infinite {
            return Err(CellError::Unrepresentable("infinite interval"));
        }
        let nanoseconds = self
            .micros
            .checked_mul(1_000)
            .ok_or(CellError::Unrepresentable("interval"))?;
        Ok(IntervalMonthDayNano::new(
            self.months,
            self.days,
            nanoseconds,
        ))
    }

    pub(crate) fn from_month_day_nano(value: IntervalMonthDayNano) -> Self {
        Self {
            micros: value.nanoseconds / 1_000,
            days: value.days,
            months: value.months,
        }
    }

    /// `IntervalStyle = postgres`, e.g. `1 year 2 mons -3 days +04:05:06.5`.
    pub(crate) fn write(self, out: &mut impl Write) -> fmt::Result {
        IntervalParts {
            months: self.months,
            days: self.days,
            micros: self.micros,
        }
        .write(out)
    }
}

/// `HH:MM:SS[.ffffff]` of microseconds since midnight (up to `24:00:00`).
pub(crate) fn write_clock(micros: i64, out: &mut impl Write) -> fmt::Result {
    text::write_clock(micros.unsigned_abs(), out)
}

/// `timetz`: microseconds since midnight and the zone in seconds *west* of UTC.
pub(crate) fn write_time_tz(bytes: &[u8], out: &mut impl Write) -> Result<(), CellError> {
    let bytes: [u8; 12] = array(bytes)?;
    let (micros, zone) = bytes.split_at(8);
    let micros = i64::from_be_bytes(array(micros)?);
    let east = -i64::from(i32::from_be_bytes(array(zone)?));
    write_clock(micros, out).map_err(|_| CellError::Format)?;
    write_offset(east, out).map_err(|_| CellError::Format)
}

/// `+HH`, `+HH:MM` or `+HH:MM:SS` as `PostgreSQL` prints zone offsets.
fn write_offset(east_seconds: i64, out: &mut impl Write) -> fmt::Result {
    let sign = if east_seconds < 0 { '-' } else { '+' };
    let seconds = east_seconds.unsigned_abs();
    write!(out, "{sign}{:02}", seconds / 3_600)?;
    let (minutes, rest) = (seconds / 60 % 60, seconds % 60);
    if minutes != 0 || rest != 0 {
        write!(out, ":{minutes:02}")?;
    }
    if rest != 0 {
        write!(out, ":{rest:02}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_become_month_day_nano_unless_out_of_range() {
        let value = Interval {
            micros: -1,
            days: 2,
            months: 3,
        };
        assert_eq!(
            value.month_day_nano(),
            Ok(IntervalMonthDayNano::new(3, 2, -1_000))
        );
        let huge = Interval {
            micros: i64::MAX / 10,
            ..value
        };
        assert!(huge.month_day_nano().is_err());
        assert_eq!(
            Interval::from_month_day_nano(IntervalMonthDayNano::new(3, 2, -1_000)),
            value
        );
    }

    #[test]
    fn times_and_zones_print_like_postgres() -> Result<(), CellError> {
        let mut out = String::new();
        let micros = 13 * MICROS_PER_HOUR + 5 * 60 * MICROS_PER_SECOND + 120;
        let mut wire = micros.to_be_bytes().to_vec();
        wire.extend_from_slice(&(-19_800_i32).to_be_bytes());
        write_time_tz(&wire, &mut out)?;
        assert_eq!(out, "13:05:00.00012+05:30");
        out.clear();
        let mut wire = 0_i64.to_be_bytes().to_vec();
        wire.extend_from_slice(&3_600_i32.to_be_bytes());
        write_time_tz(&wire, &mut out)?;
        assert_eq!(out, "00:00:00-01");
        assert_eq!(
            time_micros(&MICROS_PER_HOUR.to_be_bytes()),
            Ok(MICROS_PER_HOUR)
        );
        assert!(time_micros(&MICROS_PER_DAY.to_be_bytes()).is_err());
        out.clear();
        assert!(write_clock(MICROS_PER_DAY, &mut out).is_ok());
        assert_eq!(out, "24:00:00");
        Ok(())
    }
}
