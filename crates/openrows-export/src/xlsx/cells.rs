// SPDX-License-Identifier: Apache-2.0
//! Typed Excel cells: numbers that Excel's doubles hold exactly, booleans,
//! dates and times as formatted serial numbers, everything else as text.
use crate::cell::Cell;
use crate::text::{Temporal, push_plain};
use arrow_schema::TimeUnit;
use rust_xlsxwriter::{Format as CellFormat, Worksheet, XlsxError};

/// Integers beyond 2^53 lose digits as doubles; they are written as text.
const EXACT: u64 = 1 << 53;
/// Digits a double keeps for any decimal value.
const DOUBLE_DIGITS: usize = 15;
/// Excel's longest cell text, in characters.
const MAX_TEXT: usize = 32_767;
/// Serial number of 1970-01-01 in Excel's 1900 date system.
const UNIX_EPOCH: i64 = 25_569;
/// Serials from 1900-03-01 (Excel's fictitious 1900-02-29 lies before it)
/// up to, not including, 10000-01-01.
const SERIALS: std::ops::Range<i64> = 61..2_958_466;

/// What a temporal serial number shows.
#[derive(Clone, Copy)]
enum Kind {
    Date,
    DateTime,
    Time,
}

/// Writes cells with the workbook's shared number formats.
pub(super) struct CellWriter {
    date: CellFormat,
    date_time: CellFormat,
    time: CellFormat,
    scratch: String,
}

impl CellWriter {
    pub(super) fn new() -> Self {
        Self {
            date: CellFormat::new().set_num_format("yyyy-mm-dd"),
            date_time: CellFormat::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
            time: CellFormat::new().set_num_format("hh:mm:ss"),
            scratch: String::new(),
        }
    }

    /// Write `cell` at `(row, column)`; NULL leaves the cell blank.
    pub(super) fn write(
        &mut self,
        sheet: &mut Worksheet,
        (row, column): (u32, u16),
        cell: &Cell<'_>,
    ) -> Result<(), XlsxError> {
        let format = |kind| match kind {
            Kind::Date => &self.date,
            Kind::DateTime => &self.date_time,
            Kind::Time => &self.time,
        };
        match cell {
            Cell::Null => Ok(()),
            Cell::Bool(value) => sheet.write_boolean(row, column, *value).map(drop),
            Cell::Temporal(value) => match serial(*value) {
                Some((serial, kind)) => sheet
                    .write_number_with_format(row, column, serial, format(kind))
                    .map(drop),
                None => self.text(sheet, (row, column), cell),
            },
            _ => match number(cell) {
                Some(value) => sheet.write_number(row, column, value).map(drop),
                None => self.text(sheet, (row, column), cell),
            },
        }
    }

    /// The canonical text, cut to Excel's cell limit.
    fn text(
        &mut self,
        sheet: &mut Worksheet,
        (row, column): (u32, u16),
        cell: &Cell<'_>,
    ) -> Result<(), XlsxError> {
        self.scratch.clear();
        push_plain(cell, &mut self.scratch);
        if let Some((cut, _)) = self.scratch.char_indices().nth(MAX_TEXT) {
            self.scratch.truncate(cut);
        }
        sheet.write_string(row, column, &self.scratch).map(drop)
    }
}

/// The cell as an exact double, if it is a number Excel can hold.
#[expect(
    clippy::cast_precision_loss,
    reason = "only integers within ±2^53, which doubles hold exactly"
)]
fn number(cell: &Cell<'_>) -> Option<f64> {
    match cell {
        Cell::Int(value) if value.unsigned_abs() <= EXACT => Some(*value as f64),
        Cell::UInt(value) if *value <= EXACT => Some(*value as f64),
        Cell::F64(value) if value.is_finite() => Some(*value),
        // Through the shortest text, so 0.1f32 stays 0.1 instead of 0.10000000149.
        Cell::F32(value) if value.is_finite() => value.to_string().parse().ok(),
        Cell::Number(text) if significant_digits(text) <= DOUBLE_DIGITS => text.parse().ok(),
        _ => None,
    }
}

fn significant_digits(text: &str) -> usize {
    let mantissa = text.split(['e', 'E']).next().unwrap_or_default();
    let digits = mantissa.bytes().filter(u8::is_ascii_digit);
    digits.skip_while(|digit| *digit == b'0').count()
}

/// The Excel serial number of a date, timestamp or time of day; `None`
/// outside Excel's date range and for intervals.
#[expect(
    clippy::cast_precision_loss,
    reason = "units of one day (at most 8.64e13 ns) are exact in a double"
)]
fn serial(value: Temporal) -> Option<(f64, Kind)> {
    match value {
        Temporal::Date(days) => day(i64::from(days), 0.0, Kind::Date),
        Temporal::DateMillis(millis) => day(millis.div_euclid(86_400_000), 0.0, Kind::Date),
        Temporal::Timestamp { value, unit, .. } => {
            let per_day = units_per_day(unit);
            let fraction = value.rem_euclid(per_day) as f64 / per_day as f64;
            day(value.div_euclid(per_day), fraction, Kind::DateTime)
        }
        Temporal::Time { value, unit } => {
            let per_day = units_per_day(unit);
            (0..per_day)
                .contains(&value)
                .then(|| (value as f64 / per_day as f64, Kind::Time))
        }
        Temporal::Interval(_) => None,
    }
}

fn day(days: i64, fraction: f64, kind: Kind) -> Option<(f64, Kind)> {
    let serial = days.checked_add(UNIX_EPOCH)?;
    if !SERIALS.contains(&serial) {
        return None;
    }
    let whole = f64::from(u32::try_from(serial).ok()?);
    Some((whole + fraction, kind))
}

fn units_per_day(unit: TimeUnit) -> i64 {
    match unit {
        TimeUnit::Second => 86_400,
        TimeUnit::Millisecond => 86_400_000,
        TimeUnit::Microsecond => 86_400_000_000,
        TimeUnit::Nanosecond => 86_400_000_000_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(found: Option<(f64, Kind)>, expected: f64) -> bool {
        found.is_some_and(|(serial, _)| (serial - expected).abs() < 1e-9)
    }

    #[test]
    fn dates_map_to_excel_serials_inside_its_range() {
        assert!(close(serial(Temporal::Date(0)), 25_569.0));
        let noon = Temporal::Timestamp {
            value: 43_200,
            unit: TimeUnit::Second,
            utc: true,
        };
        assert!(close(serial(noon), 25_569.5));
        let before = Temporal::Timestamp {
            value: -1,
            unit: TimeUnit::Millisecond,
            utc: false,
        };
        assert!(matches!(serial(before), Some((s, _)) if s < 25_569.0 && s > 25_568.99));
        assert!(serial(Temporal::Date(-25_569)).is_none());
        assert!(serial(Temporal::Date(3_000_000)).is_none());
        let six = Temporal::Time {
            value: 21_600_000_000,
            unit: TimeUnit::Microsecond,
        };
        assert!(close(serial(six), 0.25));
    }

    #[test]
    fn only_exactly_representable_numbers_are_numbers() {
        assert_eq!(number(&Cell::Int(-7)), Some(-7.0));
        assert_eq!(number(&Cell::UInt(u64::MAX)), None);
        assert_eq!(number(&Cell::F32(0.1)), Some(0.1));
        assert_eq!(number(&Cell::F64(f64::NAN)), None);
        assert_eq!(number(&Cell::Number("-0012.50".into())), Some(-12.5));
        assert_eq!(number(&Cell::Number("1234567890.1234567".into())), None);
        assert_eq!(number(&Cell::Text("12")), None);
    }
}
