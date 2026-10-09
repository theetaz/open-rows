// SPDX-License-Identifier: Apache-2.0
//! SQL Server temporal values from their wire form: `date` counts days
//! from 0001-01-01; `time(n)` counts 10⁻ⁿ seconds; `datetime2(n)` and
//! `datetimeoffset(n)` (in UTC, plus an offset in minutes) are a time then
//! a date; `datetime` counts days from 1900-01-01 and 1/300 s ticks;
//! `smalldatetime` days and minutes. Text forms match what SQL Server
//! prints (`CONVERT(nvarchar, …)`, styles 120/121 for the legacy types).

use std::fmt::Write as _;

use super::CellError;

/// Days from 0001-01-01 to 1970-01-01.
const EPOCH_FROM_YEAR_ONE: i64 = 719_162;
/// Days from 1900-01-01 to 1970-01-01.
const EPOCH_FROM_1900: i64 = 25_567;
const MICROS_PER_DAY: i64 = 86_400_000_000;
const NANOS_PER_DAY: i64 = 86_400_000_000_000;

fn malformed(what: &str) -> CellError {
    CellError::Malformed(format!("malformed {what} value"))
}

/// Little-endian unsigned integer of up to eight bytes.
fn unsigned(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0, |value, byte| value << 8 | u64::from(*byte))
}

/// Days since 1970-01-01 of a 3-byte `date`.
pub(crate) fn date_days(bytes: &[u8]) -> Result<i64, CellError> {
    if bytes.len() != 3 {
        return Err(malformed("date"));
    }
    // Three bytes always fit an i64.
    Ok(i64::try_from(unsigned(bytes)).unwrap_or_default() - EPOCH_FROM_YEAR_ONE)
}

/// Wire bytes of `time(scale)`.
fn time_len(scale: u8) -> usize {
    match scale {
        0..=2 => 3,
        3 | 4 => 4,
        _ => 5,
    }
}

/// Ticks of 10⁻ˢᶜᵃˡᵉ s since midnight, and the rest of the bytes.
fn split_time(bytes: &[u8], scale: u8) -> Result<(u64, &[u8]), CellError> {
    let len = time_len(scale);
    let (time, rest) = bytes
        .split_at_checked(len)
        .ok_or_else(|| malformed("time"))?;
    Ok((unsigned(time), rest))
}

/// Nanoseconds since midnight of `time(scale)` ticks.
fn tick_nanos(ticks: u64, scale: u8) -> Result<i64, CellError> {
    let factor = 10_u64.pow(9 - u32::from(scale.min(7)));
    ticks
        .checked_mul(factor)
        .and_then(|nanos| i64::try_from(nanos).ok())
        .filter(|nanos| *nanos < NANOS_PER_DAY)
        .ok_or_else(|| malformed("time"))
}

/// `time(scale)` as nanoseconds since midnight.
pub(crate) fn time_nanos(bytes: &[u8], scale: u8) -> Result<i64, CellError> {
    let (ticks, rest) = split_time(bytes, scale)?;
    if !rest.is_empty() {
        return Err(malformed("time"));
    }
    tick_nanos(ticks, scale)
}

/// `datetime2(scale)` as nanoseconds since the epoch; `Unrepresentable`
/// outside 1677–2262 (the range of 64-bit nanoseconds).
pub(crate) fn datetime2_nanos(bytes: &[u8], scale: u8) -> Result<i64, CellError> {
    let (ticks, date) = split_time(bytes, scale)?;
    let (days, time) = (date_days(date)?, tick_nanos(ticks, scale)?);
    days.checked_mul(NANOS_PER_DAY)
        .and_then(|day| day.checked_add(time))
        .ok_or(CellError::Unrepresentable("datetime2"))
}

/// `datetime2(scale)` as microseconds since the epoch (exact for scale ≤ 6).
pub(crate) fn datetime2_micros(bytes: &[u8], scale: u8) -> Result<i64, CellError> {
    let (ticks, date) = split_time(bytes, scale)?;
    let days = date_days(date)?;
    Ok(days * MICROS_PER_DAY + tick_nanos(ticks, scale)? / 1_000)
}

/// `datetime` as microseconds since the epoch, rounded to the millisecond
/// SQL Server prints (1/300 s ticks: `.003`, `.007`).
pub(crate) fn datetime_micros(bytes: &[u8]) -> Result<i64, CellError> {
    let (days, ticks) = bytes
        .split_at_checked(4)
        .filter(|(_, ticks)| ticks.len() == 4)
        .ok_or_else(|| malformed("datetime"))?;
    let days = i64::from(i32::from_le_bytes(days.try_into().unwrap_or_default()));
    let ticks = i64::from(u32::from_le_bytes(ticks.try_into().unwrap_or_default()));
    let millis = (ticks * 10 + 1) / 3;
    Ok((days - EPOCH_FROM_1900) * MICROS_PER_DAY + millis * 1_000)
}

/// `smalldatetime` as microseconds since the epoch.
pub(crate) fn smalldatetime_micros(bytes: &[u8]) -> Result<i64, CellError> {
    let [a, b, c, d] = bytes else {
        return Err(malformed("smalldatetime"));
    };
    let days = i64::from(u16::from_le_bytes([*a, *b]));
    let minutes = i64::from(u16::from_le_bytes([*c, *d]));
    Ok((days - EPOCH_FROM_1900) * MICROS_PER_DAY + minutes * 60_000_000)
}

/// `datetimeoffset(scale)` as SQL Server prints it, e.g.
/// `2024-05-06 13:05:00.2500000 +05:30`: the local time and its offset.
pub(crate) fn datetimeoffset_text(bytes: &[u8], scale: u8) -> Result<String, CellError> {
    let (ticks, rest) = split_time(bytes, scale)?;
    let [d0, d1, d2, o0, o1] = rest else {
        return Err(malformed("datetimeoffset"));
    };
    let utc = date_days(&[*d0, *d1, *d2])? * NANOS_PER_DAY + tick_nanos(ticks, scale)?;
    let offset = i64::from(i16::from_le_bytes([*o0, *o1]));
    let local = utc + offset * 60_000_000_000;
    let mut text = String::new();
    write_timestamp(
        local.div_euclid(1_000),
        (local.rem_euclid(1_000), scale),
        &mut text,
    );
    let sign = if offset < 0 { '-' } else { '+' };
    let minutes = offset.unsigned_abs();
    let _ = write!(text, " {sign}{:02}:{:02}", minutes / 60, minutes % 60);
    Ok(text)
}

/// `(year, month, day)` of days since 1970-01-01 (proleptic Gregorian).
pub(crate) fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    // Month and day are in 1..=12 and 1..=31.
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// `YYYY-MM-DD`.
pub(crate) fn write_date(days: i64, out: &mut String) {
    let (year, month, day) = civil(days);
    let _ = write!(out, "{year:04}-{month:02}-{day:02}");
}

/// `hh:mm:ss` and, for `digits` > 0, `.` and that many fraction digits of
/// a time of day in nanoseconds.
pub(crate) fn write_clock(nanos: i64, digits: u8, out: &mut String) {
    let seconds = nanos / 1_000_000_000;
    let _ = write!(
        out,
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60
    );
    if digits > 0 {
        let fraction = nanos % 1_000_000_000 / 10_i64.pow(9 - u32::from(digits.min(9)));
        let _ = write!(out, ".{fraction:0width$}", width = usize::from(digits));
    }
}

/// `YYYY-MM-DD hh:mm:ss[.fff…]` of microseconds since the epoch plus
/// `extra_nanos` (0–999), with `digits` fraction digits.
pub(crate) fn write_timestamp(micros: i64, (extra_nanos, digits): (i64, u8), out: &mut String) {
    let days = micros.div_euclid(MICROS_PER_DAY);
    let nanos = micros.rem_euclid(MICROS_PER_DAY) * 1_000 + extra_nanos;
    write_date(days, out);
    out.push(' ');
    write_clock(nanos, digits, out);
}

#[cfg(test)]
#[path = "temporal_tests.rs"]
mod tests;
