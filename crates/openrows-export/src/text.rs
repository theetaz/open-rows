// SPDX-License-Identifier: Apache-2.0
//! The canonical text form of a cell, shared by every format.
//!
//! Dates are ISO 8601 (`2024-02-29`), timestamps `YYYY-MM-DD HH:MM:SS[.ffffff]`
//! with `+00:00` for zone-aware values (Arrow stores the UTC instant), times
//! `HH:MM:SS[.ffffff]`, intervals `PostgreSQL`'s `1 year 2 mons 3 days 04:05:06`,
//! binary is `\x` plus lowercase hex (`PostgreSQL`'s `bytea` output), UUIDs
//! are hyphenated lowercase, floats are their shortest round-trip text
//! (exponent form for very small or large magnitudes) and non-finite floats
//! are `NaN`, `Infinity` and `-Infinity`.
use crate::cell::Cell;
use arrow_array::temporal_conversions::{
    date32_to_datetime, date64_to_datetime, timestamp_ms_to_datetime, timestamp_ns_to_datetime,
    timestamp_s_to_datetime, timestamp_us_to_datetime,
};
use arrow_array::types::IntervalMonthDayNano;
use arrow_schema::TimeUnit;
use openrows_driver_api::text::{IntervalParts, write_clock};
use std::fmt::{self, Write as _};

const DATE: &str = "%Y-%m-%d";
const TIMESTAMP: &str = "%Y-%m-%d %H:%M:%S%.f";

/// A date or timestamp, rendered without allocating.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Temporal {
    /// Days since the Unix epoch.
    Date(i32),
    /// Milliseconds since the Unix epoch, shown as a date.
    DateMillis(i64),
    /// An instant or local date-time in `unit`s since the epoch.
    Timestamp {
        value: i64,
        unit: TimeUnit,
        /// Zone-aware: the value is a UTC instant.
        utc: bool,
    },
    /// A time of day in `unit`s since midnight.
    Time { value: i64, unit: TimeUnit },
    /// Months, days and nanoseconds.
    Interval(IntervalMonthDayNano),
}

impl fmt::Display for Temporal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Values beyond chrono's range keep their raw epoch number.
        match *self {
            Self::Date(days) => match date32_to_datetime(days) {
                Some(date) => write!(f, "{}", date.format(DATE)),
                None => write!(f, "{days}"),
            },
            Self::DateMillis(millis) => match date64_to_datetime(millis) {
                Some(date) => write!(f, "{}", date.format(DATE)),
                None => write!(f, "{millis}"),
            },
            Self::Timestamp { value, unit, utc } => {
                let converted = match unit {
                    TimeUnit::Second => timestamp_s_to_datetime(value),
                    TimeUnit::Millisecond => timestamp_ms_to_datetime(value),
                    TimeUnit::Microsecond => timestamp_us_to_datetime(value),
                    TimeUnit::Nanosecond => timestamp_ns_to_datetime(value),
                };
                match converted {
                    Some(time) if utc => write!(f, "{}+00:00", time.format(TIMESTAMP)),
                    Some(time) => write!(f, "{}", time.format(TIMESTAMP)),
                    None => write!(f, "{value}"),
                }
            }
            Self::Time { value, unit } => time(value, unit, f),
            Self::Interval(value) => IntervalParts {
                months: value.months,
                days: value.days,
                micros: value.nanoseconds / 1_000,
            }
            .write(f),
        }
    }
}

/// Sub-microsecond digits of nanosecond times are dropped, as `PostgreSQL`'s
/// `time` keeps microseconds.
fn time(value: i64, unit: TimeUnit, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let micros = match unit {
        TimeUnit::Second => value.saturating_mul(1_000_000),
        TimeUnit::Millisecond => value.saturating_mul(1_000),
        TimeUnit::Microsecond => value,
        TimeUnit::Nanosecond => value / 1_000,
    };
    if micros < 0 {
        f.write_char('-')?;
    }
    write_clock(micros.unsigned_abs(), f)
}

/// True for a JSON number (`-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?`), which
/// is also a valid SQL numeric literal in every supported dialect.
pub(crate) fn is_number(text: &str) -> bool {
    let bytes = text.strip_prefix('-').unwrap_or(text).as_bytes();
    let integer = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if integer == 0 || (integer > 1 && bytes.first() == Some(&b'0')) {
        return false;
    }
    let mut rest = &bytes[integer..];
    if let Some(fraction) = rest.strip_prefix(b".") {
        let digits = fraction.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            return false;
        }
        rest = &fraction[digits..];
    }
    if let Some(exponent) = rest.strip_prefix(b"e").or_else(|| rest.strip_prefix(b"E")) {
        let exponent = exponent
            .strip_prefix(b"+")
            .or_else(|| exponent.strip_prefix(b"-"))
            .unwrap_or(exponent);
        return !exponent.is_empty() && exponent.iter().all(u8::is_ascii_digit);
    }
    rest.is_empty()
}

/// Append the canonical text of `cell`; NULL appends nothing.
pub(crate) fn push_plain(cell: &Cell<'_>, out: &mut String) {
    // Writing to a `String` cannot fail.
    let _ = match cell {
        Cell::Null => Ok(()),
        Cell::Bool(value) => write!(out, "{value}"),
        Cell::Int(value) => write!(out, "{value}"),
        Cell::UInt(value) => write!(out, "{value}"),
        Cell::F32(value) => push_float(f64::from(*value), Float::F32(*value), out),
        Cell::F64(value) => push_float(*value, Float::F64(*value), out),
        Cell::Number(text) => out.write_str(text),
        Cell::Text(text) => out.write_str(text),
        Cell::Json(text) => out.write_str(text),
        Cell::Owned(text) => out.write_str(text),
        Cell::Binary(bytes) => {
            out.push_str("\\x");
            push_hex(bytes, out);
            Ok(())
        }
        Cell::Uuid(bytes) => {
            push_uuid(bytes, out);
            Ok(())
        }
        Cell::Temporal(value) => write!(out, "{value}"),
    };
}

/// A float in its own width, so `0.1f32` prints as `0.1`.
#[derive(Clone, Copy)]
enum Float {
    F32(f32),
    F64(f64),
}

/// Positional notation stays exact and readable in this magnitude range;
/// outside it the digits would run long (`1e-300` has 300) and `MySQL`
/// turns exact literals beyond 65 digits into lossy values.
const PLAIN: std::ops::Range<f64> = 1e-5..1e16;

/// Shortest round-trip text in the value's own width: positional within
/// [`PLAIN`], exponent form (`2.5e-300`) outside; `wide` decides the class.
fn push_float(wide: f64, value: Float, out: &mut String) -> fmt::Result {
    if wide.is_nan() {
        return out.write_str("NaN");
    }
    if wide.is_infinite() {
        return out.write_str(if wide > 0.0 { "Infinity" } else { "-Infinity" });
    }
    let positional = wide == 0.0 || PLAIN.contains(&wide.abs());
    match (value, positional) {
        (Float::F32(value), true) => write!(out, "{value}"),
        (Float::F32(value), false) => write!(out, "{value:e}"),
        (Float::F64(value), true) => write!(out, "{value}"),
        (Float::F64(value), false) => write!(out, "{value:e}"),
    }
}

/// Append lowercase hex digits.
pub(crate) fn push_hex(bytes: &[u8], out: &mut String) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out.reserve(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
}

/// Append a hyphenated UUID (8-4-4-4-12); other widths become plain hex.
fn push_uuid(bytes: &[u8], out: &mut String) {
    if bytes.len() != 16 {
        push_hex(bytes, out);
        return;
    }
    for (index, group) in [0..4, 4..6, 6..8, 8..10, 10..16].into_iter().enumerate() {
        if index > 0 {
            out.push('-');
        }
        push_hex(&bytes[group], out);
    }
}
