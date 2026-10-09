// SPDX-License-Identifier: Apache-2.0
//! Tagged-text fallback (docs/04 §5): `PostgreSQL`'s text for temporal,
//! decimal and array values, written from binary wire values or from
//! already-decoded Arrow values.
use super::scalar::{CellError, PG_EPOCH_DAYS};
use super::temporal::{self, MICROS_PER_SECOND};
use super::{array, text};
use crate::types::ColumnKind;
use arrow_array::Array;
use arrow_array::builder::StringBuilder;
use std::fmt::{self, Write};

const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;

/// True for kinds whose Arrow type cannot hold every server value.
pub(crate) fn supports(kind: ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::Timestamp
            | ColumnKind::TimestampTz
            | ColumnKind::Date
            | ColumnKind::Time
            | ColumnKind::Interval
            | ColumnKind::Decimal { .. }
            | ColumnKind::Array { .. }
    )
}

/// Append one binary wire value as text.
pub(crate) fn append_wire(
    kind: ColumnKind,
    bytes: &[u8],
    out: &mut StringBuilder,
) -> Result<(), CellError> {
    match kind.element() {
        Some(element) => array::write_text(element, bytes, out)?,
        None => text::write_wire(kind, bytes, out)?,
    }
    out.append_value("");
    Ok(())
}

/// Append every value of a finished typed column (same kind) as text.
pub(crate) fn append_array(
    kind: ColumnKind,
    values: &dyn Array,
    out: &mut StringBuilder,
) -> Result<(), CellError> {
    for index in 0..values.len() {
        if values.is_null(index) {
            out.append_null();
            continue;
        }
        match kind.element() {
            Some(element) => array::write_arrow_text(element, values, index, out)?,
            None => text::write_arrow(kind, values, index, out)?,
        }
        out.append_value("");
    }
    Ok(())
}

/// `pg_micros` counts from 2000-01-01; `i64::MIN`/`MAX` are ∓infinity.
pub(crate) fn write_timestamp(pg_micros: i64, tz: bool, out: &mut impl Write) -> fmt::Result {
    match pg_micros {
        i64::MAX => return out.write_str("infinity"),
        i64::MIN => return out.write_str("-infinity"),
        _ => {}
    }
    let days = pg_micros.div_euclid(MICROS_PER_DAY);
    let time = pg_micros.rem_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil(days + i64::from(PG_EPOCH_DAYS));
    write_year(year, out)?;
    write!(out, "-{month:02}-{day:02} ")?;
    temporal::write_clock(time, out)?;
    if tz {
        out.write_str("+00")?;
    }
    write_era(year, out)
}

/// `pg_days` counts from 2000-01-01; `i32::MIN`/`MAX` are ∓infinity.
pub(crate) fn write_date(pg_days: i32, out: &mut impl Write) -> fmt::Result {
    match pg_days {
        i32::MAX => return out.write_str("infinity"),
        i32::MIN => return out.write_str("-infinity"),
        _ => {}
    }
    let (year, month, day) = civil(i64::from(pg_days) + i64::from(PG_EPOCH_DAYS));
    write_year(year, out)?;
    write!(out, "-{month:02}-{day:02}")?;
    write_era(year, out)
}

pub(crate) fn write_decimal(unscaled: i128, scale: i8, out: &mut impl Write) -> fmt::Result {
    let scale = u32::try_from(scale).unwrap_or(0);
    let divisor = 10_u128.pow(scale.min(38));
    let magnitude = unscaled.unsigned_abs();
    let sign = if unscaled < 0 { "-" } else { "" };
    write!(out, "{sign}{}", magnitude / divisor)?;
    if scale > 0 {
        let width = usize::try_from(scale).unwrap_or(0);
        write!(out, ".{:0width$}", magnitude % divisor)?;
    }
    Ok(())
}

/// ISO years have at least four digits; year 0 and below are 1 BC and earlier.
fn write_year(year: i64, out: &mut impl Write) -> fmt::Result {
    let shown = if year > 0 { year } else { 1 - year };
    write!(out, "{shown:04}")
}

fn write_era(year: i64, out: &mut impl Write) -> fmt::Result {
    if year > 0 {
        Ok(())
    } else {
        out.write_str(" BC")
    }
}

/// Proleptic Gregorian (year, month, day) for days since 1970-01-01
/// (H. Hinnant's `civil_from_days`, astronomical years).
fn civil(days: i64) -> (i64, i64, i64) {
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
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::super::scalar::PG_EPOCH_MICROS;
    use super::*;

    fn text(write: impl FnOnce(&mut String) -> fmt::Result) -> String {
        let mut out = String::new();
        assert!(write(&mut out).is_ok());
        out
    }

    #[test]
    fn timestamps_match_postgres_iso_output() {
        let micros = |days: i64, extra: i64| days * MICROS_PER_DAY + extra;
        assert_eq!(
            text(|o| write_timestamp(0, false, o)),
            "2000-01-01 00:00:00"
        );
        let value = micros(31, 3_723_000_500);
        assert_eq!(
            text(|o| write_timestamp(value, true, o)),
            "2000-02-01 01:02:03.0005+00"
        );
        assert_eq!(text(|o| write_timestamp(i64::MAX, false, o)), "infinity");
        assert_eq!(text(|o| write_timestamp(i64::MIN, true, o)), "-infinity");
        // PostgreSQL's maximum, beyond Arrow's µs range.
        let max = 9_223_371_331_199_999_999;
        assert_eq!(
            text(|o| write_timestamp(max, false, o)),
            "294276-12-31 23:59:59.999999"
        );
        // Julian day 0 in the proleptic Gregorian calendar.
        let min = -211_813_488_000_000_000;
        assert_eq!(
            text(|o| write_timestamp(min, true, o)),
            "4714-11-24 00:00:00+00 BC"
        );
    }

    #[test]
    fn dates_and_decimals_match_postgres_output() {
        assert_eq!(text(|o| write_date(-1, o)), "1999-12-31");
        assert_eq!(text(|o| write_date(-730_120, o)), "0001-12-31 BC");
        assert_eq!(text(|o| write_date(i32::MAX, o)), "infinity");
        assert_eq!(text(|o| write_date(i32::MIN, o)), "-infinity");
        assert_eq!(text(|o| write_decimal(-1_234, 2, o)), "-12.34");
        assert_eq!(text(|o| write_decimal(5, 3, o)), "0.005");
        assert_eq!(text(|o| write_decimal(42, 0, o)), "42");
    }

    #[test]
    fn typed_arrays_convert_back_to_the_same_text() -> Result<(), CellError> {
        use arrow_array::{Date32Array, TimestampMicrosecondArray};
        let mut out = StringBuilder::new();
        let stamps = TimestampMicrosecondArray::from(vec![Some(PG_EPOCH_MICROS), None]);
        append_array(ColumnKind::Timestamp, &stamps, &mut out)?;
        append_array(ColumnKind::Date, &Date32Array::from(vec![0]), &mut out)?;
        append_wire(ColumnKind::Date, &i32::MAX.to_be_bytes(), &mut out)?;
        let texts = out.finish();
        let texts: Vec<_> = texts.iter().collect();
        assert_eq!(
            texts,
            [
                Some("2000-01-01 00:00:00"),
                None,
                Some("1970-01-01"),
                Some("infinity")
            ]
        );
        let wrong = append_array(ColumnKind::Date, &stamps, &mut StringBuilder::new());
        assert_eq!(wrong, Err(CellError::Format));
        assert!(supports(ColumnKind::TimestampTz) && !supports(ColumnKind::Int4));
        Ok(())
    }
}
